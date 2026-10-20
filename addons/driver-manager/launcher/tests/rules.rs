use driver_manager::check::{self, Level};
use driver_manager::game::{Choice, GameData, StockDriver};
use driver_manager::model::{Driver, Rating, Roster};
use driver_manager::ops;
use serde_json::json;

fn choice(v: &str) -> Vec<Choice> {
    vec![Choice {
        value: v.into(),
        label: v.into(),
    }]
}

fn game() -> GameData {
    GameData {
        nationalities: choice("britain"),
        heads: choice("male_ls144"),
        helmets: choice("tier01_livery52"),
        suits: choice("titlesponsor_livery15"),
        gloves: choice("titlesponsor_livery15"),
        poses: choice("Pose03"),
        emotes: choice("Emote01"),
        voices: choice("DefaultName"),
        stock: vec![StockDriver {
            label: "max_verstappen".into(),
            name: "Max VERSTAPPEN".into(),
            code: "VER".into(),
            number: 1,
            stat_id: 9,
        }],
        driver_ids: [9, 207].into(),
        preset_ids: [365].into(),
        ..GameData::default()
    }
}

fn add(roster: &mut Roster, g: &GameData, first: &str, code: &str, number: u32) -> Driver {
    let mut d = Driver::blank(Some(g), number);
    d.first = first.into();
    d.last = "Driver".into();
    d.code = code.into();
    let (id, preset) = roster.allocate(g).unwrap();
    d.stat_id = id;
    d.preset_id = preset;
    roster.drivers.push(d.clone());
    d
}

#[test]
fn rules_flag_bad_drivers_and_the_build_leaves_them_out() {
    let g = game();
    let mut roster = Roster {
        schema: 1,
        next_driver: 208,
        next_preset: 366,
        drivers: Vec::new(),
    };
    let ok = add(&mut roster, &g, "Good", "GOO", 1);
    let dup = add(&mut roster, &g, "Copy", "GOO", 7);
    let mut bad = Driver::blank(Some(&g), 0);
    bad.code = "ab".into();
    bad.ratings[0] = Rating { start: 90, peak: 80 };
    bad.helmet = "gone".into();

    let issues = check::check(&ok, &roster, Some(&g));
    assert!(check::is_valid(&issues));
    assert!(
        issues
            .iter()
            .any(|i| i.level == Level::Warn && i.text.contains("Max VERSTAPPEN"))
    );
    let issues = check::check(&dup, &roster, Some(&g));
    assert!(
        issues
            .iter()
            .any(|i| i.level == Level::Error && i.text.contains("Code GOO"))
    );
    let texts: Vec<String> = check::check(&bad, &roster, Some(&g))
        .into_iter()
        .map(|i| i.text)
        .collect();
    for want in [
        "First name is empty.",
        "Code must be 3 letters",
        "Number must be from 1 to 99.",
        "Helmet \"gone\"",
        "Experience peak is below",
    ] {
        assert!(texts.iter().any(|t| t.contains(want)), "{want} in {texts:?}");
    }
    assert!(
        check::check(&ok, &roster, None)
            .iter()
            .any(|i| i.level == Level::Error)
    );

    let reply = ops::build(&roster, Some(&g), &json!({ "stable": ["db", "lang"] }));
    let labels: Vec<&str> = reply["ops"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| o["set"]["_metadata_"].as_str())
        .collect();
    assert!(labels.contains(&"dm_208"));
    assert!(
        !labels.iter().any(|l| l.starts_with("dm_209")),
        "invalid driver left out: {labels:?}"
    );
    assert!(reply["warnings"][0].as_str().unwrap().contains("left out"));
    assert_eq!(ops::build(&roster, None, &json!({}))["ops"], json!([]));
}

#[test]
fn ids_are_never_reused_and_skip_the_game() {
    let mut g = game();
    g.driver_ids.insert(209);
    let mut roster = Roster {
        schema: 1,
        next_driver: 208,
        next_preset: 366,
        drivers: Vec::new(),
    };
    let a = add(&mut roster, &g, "A", "AAA", 2);
    let b = add(&mut roster, &g, "B", "BBB", 3);
    assert_eq!((a.stat_id, b.stat_id), (208, 210));
    roster.drivers.clear();
    let c = add(&mut roster, &g, "C", "CCC", 4);
    assert_eq!((c.stat_id, c.preset_id), (211, 368));
    while roster.free_ids(&g) > 0 {
        add(&mut roster, &g, "X", "XXX", 5);
    }
    assert!(roster.allocate(&g).is_err());
}

#[test]
fn curves_have_four_points_from_start_to_peak() {
    assert_eq!(
        ops::curve(Rating { start: 70, peak: 85 }),
        "@v2@minx:0,maxx:10,miny:0,maxy:100,0:0.7,0.25:0.775,0.5:0.85,1:0.85"
    );
    assert_eq!(
        ops::curve(Rating { start: 0, peak: 100 }),
        "@v2@minx:0,maxx:10,miny:0,maxy:100,0:0,0.25:0.5,0.5:1,1:1"
    );
}
