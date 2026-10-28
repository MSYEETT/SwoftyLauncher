use driver_manager::check;
use driver_manager::game;
use driver_manager::model::{Birth, Driver, Rating, Roster};
use swofty_core::dbedit::{Catalog, driver_manager as dm};
use swofty_formats::gvdb::{Gvdb, Record};

type Variant = (&'static str, fn(&mut Driver));

fn database() -> Gvdb {
    let mut records = Vec::new();
    let mut table = |rows: &str, labels: &[&str], set: &[(&str, &str)]| {
        let (container, word) = rows.rsplit_once('.').unwrap();
        let count = format!("{container}._{word}_instancecount_");
        records.push(Record::new(&count, 0, labels.len().to_string()));
        for (i, l) in labels.iter().enumerate() {
            records.push(Record::new(&format!("{rows}_{i}._metadata_"), 2, *l));
            for (field, value) in set {
                records.push(Record::new(&format!("{rows}_{i}.{field}"), 2, *value));
            }
        }
    };
    table("entrants.nationality.nationality", &["default", "britain"], &[]);
    table(
        "entrants.animation.driver_heads.driver_head",
        &["male_ls144"],
        &[],
    );
    table(
        "customisation.custom_avatar.helmets.Helmet",
        &["tier01_livery52"],
        &[],
    );
    table(
        "customisation.custom_avatar.suits.Suit",
        &["titlesponsor_livery15"],
        &[],
    );
    table(
        "customisation.custom_avatar.gloves.Gloves",
        &["titlesponsor_livery15"],
        &[],
    );
    table("customisation.custom_avatar.poses.Pose", &["Pose03"], &[]);
    table(
        "customisation.custom_avatar.podium_emotes.PodiumEmote",
        &["Emote01"],
        &[],
    );
    table(
        "entrants.audio_names.audio_name",
        &["DefaultName", "Smith", "Smith_disabled", "Jones"],
        &[],
    );
    table("entrants.drivers.driver", &["max_verstappen", "new_rookie"], &[]);
    table(
        "entrants.driver_customisations.folder",
        &["official_driver_customisations"],
        &[],
    );
    table(
        "entrants.driver_customisations.folder_0.preloaded_driver_customisation_properties",
        &["max_look"],
        &[],
    );
    records.extend([
        Record::new(
            "entrants.audio_names.audio_name_1.selection_type",
            2,
            "custom_standard",
        ),
        Record::new(
            "entrants.audio_names.audio_name_2.selection_type",
            2,
            "custom_standard",
        ),
        Record::new("entrants.audio_names.audio_name_3.selection_type", 2, "official"),
        Record::new("entrants.drivers.driver_0.stat_id", 0, "9"),
        Record::new("entrants.drivers.driver_1.stat_id", 0, "250"),
        Record::new(
            "entrants.driver_customisations.folder_0.preloaded_driver_customisation_properties_0.stat_id",
            0,
            "450",
        ),
    ]);
    Gvdb { records }
}

#[test]
fn the_db_editor_offers_the_drivers_the_build_adds() {
    let db = database();
    let game = game::extract(&db, None, "test");
    let cat = Catalog::from_gvdb(db, String::new());
    let base = |id: u32, code: &str| {
        let mut d = Driver::blank(Some(&game), 2 + id - 208);
        d.first = "Test".into();
        d.last = format!("Driver {id}");
        d.code = code.into();
        d.stat_id = id;
        d.preset_id = id + 158;
        d
    };
    let variants: Vec<Variant> = vec![
        ("valid", |_| {}),
        ("retired", |d| d.retired = true),
        ("surname first", |d| d.reversed = true),
        ("empty first name", |d| d.first = "  ".into()),
        ("long last name", |d| d.last = "x".repeat(25)),
        ("control character", |d| d.first = "Te\tst".into()),
        ("short code", |d| d.code = "AB".into()),
        ("lowercase code", |d| d.code = "abc".into()),
        ("code of an earlier driver", |d| d.code = "AAA".into()),
        ("code of the game's driver", |d| d.code = "VER".into()),
        ("number 0", |d| d.number = 0),
        ("number 100", |d| d.number = 100),
        ("default nationality", |d| d.nationality = "default".into()),
        ("missing head", |d| d.head = "male_ls999".into()),
        ("missing helmet", |d| d.helmet = "x".into()),
        ("missing suit", |d| d.suit = "x".into()),
        ("missing gloves", |d| d.gloves = "x".into()),
        ("missing pose", |d| d.pose = "x".into()),
        ("missing emote", |d| d.emote = "x".into()),
        ("generic voice", |d| d.voice = "Smith".into()),
        ("disabled voice", |d| d.voice = "Smith_disabled".into()),
        ("voice that isn't generic", |d| d.voice = "Jones".into()),
        ("peak below start", |d| {
            d.ratings[1] = Rating { start: 80, peak: 70 }
        }),
        ("rating above 100", |d| {
            d.ratings[2] = Rating { start: 90, peak: 101 }
        }),
        ("born 1949", |d| d.birth.year = 1949),
        ("born 2012", |d| d.birth.year = 2012),
        ("month 13", |d| d.birth.month = 13),
        ("29 February 2003", |d| {
            d.birth = Birth {
                year: 2003,
                month: 2,
                day: 29,
            }
        }),
        ("29 February 2004", |d| {
            d.birth = Birth {
                year: 2004,
                month: 2,
                day: 29,
            }
        }),
        ("31 April", |d| {
            d.birth = Birth {
                year: 2004,
                month: 4,
                day: 31,
            }
        }),
        ("trait above 1", |d| d.traits[0] = 1.5),
        ("negative trait", |d| d.traits[5] = -0.1),
        ("31 seasons", |d| d.seasons = 31),
        ("garage 2", |d| d.garage = 2),
        ("id the game uses", |d| d.stat_id = 250),
        ("look id the game uses", |d| d.preset_id = 450),
        ("look id below 366", |d| d.preset_id = 365),
        ("look id of an earlier driver", |d| d.preset_id = 366),
    ];
    let mut roster = Roster {
        schema: 1,
        next_driver: 208,
        next_preset: 366,
        drivers: vec![base(208, "AAA")],
    };
    for (i, (_, change)) in variants.iter().enumerate() {
        let id = 209 + i as u32;
        let code: String = [b'B' + (i / 26) as u8, b'A' + (i % 26) as u8, b'Z']
            .iter()
            .map(|&b| b as char)
            .collect();
        let mut d = base(id, &code);
        change(&mut d);
        roster.drivers.push(d);
    }
    let dir = std::env::temp_dir().join(format!("dm-db-editor-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    roster.save(&dir).unwrap();
    let offered = dm::drivers_in(&dir, &cat);
    let mut added = 0;
    for (d, (what, _)) in roster.drivers[1..].iter().zip(&variants) {
        let valid = check::is_valid(&check::check(d, &roster, Some(&game)));
        let shown = offered.iter().find(|o| o.label == d.label());
        assert_eq!(
            shown.is_some(),
            valid,
            "{what}: {:?}",
            check::check(d, &roster, Some(&game))
        );
        if let Some(o) = shown {
            assert_eq!(
                (o.name.as_str(), o.retired),
                (d.display_name().as_str(), d.retired),
                "{what}"
            );
            added += 1;
        }
    }
    assert!(
        added >= 5 && added < variants.len() / 2,
        "{added} of {} added",
        variants.len()
    );
    std::fs::remove_dir_all(&dir).ok();
}
