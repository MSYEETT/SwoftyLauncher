use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use driver_manager::game::{self, GameData};
use driver_manager::model::{Driver, Rating, Roster};
use serde_json::{Value, json};
use swofty_addon::asset::{AssetOp, Features, Kind, Op, Origin};
use swofty_addon::helper::Built;
use swofty_core::planner::Planner;
use swofty_formats::gvdb::Gvdb;

const ADDON: &str = "swofty.driver-manager";

fn pristine() -> Option<PathBuf> {
    let path = std::env::var_os("F1_25_DIR")
        .map(PathBuf::from)?
        .join(game::DB_REL);
    path.is_file().then_some(path)
}

fn driver(game: &GameData, roster: &mut Roster, first: &str, last: &str, code: &str, number: u32) -> Driver {
    let mut d = Driver::blank(Some(game), number);
    d.first = first.into();
    d.last = last.into();
    d.code = code.into();
    d.ratings[3] = Rating { start: 81, peak: 92 };
    let (id, preset) = roster.allocate(game).unwrap();
    d.stat_id = id;
    d.preset_id = preset;
    d
}

fn helper_build(data_dir: &Path, cache_dir: &Path, db_path: &Path) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_driver-manager-helper"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut send = |v: Value| writeln!(stdin, "{v}").unwrap();
    send(json!({ "id": 0, "method": "hello", "params": {
        "api": 1, "addon": ADDON, "data_dir": data_dir, "cache_dir": cache_dir,
        "profile": { "name": "default", "vanilla": false, "enabled": true }, "launcher": "1.0.11" } }));
    let hello: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert_eq!(hello["result"]["api"], 1);
    send(json!({ "id": 1, "method": "build", "params": {
        "options": {}, "features": { "stable": ["group", "erp", "db", "lang", "file"], "experimental": [] } } }));
    let mut calls = Vec::new();
    let result = loop {
        let msg: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
        if let (Some(id), Some(method)) = (msg["id"].as_u64(), msg["method"].as_str()) {
            calls.push(method.to_owned());
            let reply = match method {
                "game_build_key" => json!({ "id": id, "result": "25251167-test" }),
                "game_original" => {
                    assert_eq!(msg["params"]["rel"], game::DB_REL);
                    json!({ "id": id, "result": db_path })
                }
                "game_dir" => json!({ "id": id, "result": null }),
                other => json!({ "id": id, "error": format!("unexpected host call {other}") }),
            };
            send(reply);
            continue;
        }
        if msg.get("method").is_some() {
            continue;
        }
        assert_eq!(msg["id"], 1);
        break msg["result"].clone();
    };
    drop(stdin);
    child.wait().unwrap();
    eprintln!("host calls during build: {calls:?}");
    result
}
struct Rows<'a>(&'a Gvdb);

impl Rows<'_> {
    fn text(&self, path: &str) -> String {
        self.0.get_text(path).unwrap_or_default()
    }

    fn count(&self, container: &str, row: &str) -> usize {
        self.0.instance_count(container, row).unwrap_or(0)
    }

    fn find(&self, container: &str, row: &str, label: &str) -> Vec<usize> {
        (0..self.count(container, row))
            .filter(|i| self.text(&format!("{container}.{row}_{i}._metadata_")) == label)
            .collect()
    }

    fn where_field(&self, container: &str, row: &str, field: &str, value: &str) -> Vec<usize> {
        (0..self.count(container, row))
            .filter(|i| self.text(&format!("{container}.{row}_{i}.{field}")) == value)
            .collect()
    }
}

const POOL_2026: &str = "entrants.entrant_lists.entrant_list_67";
const POOL_2025: &str = "entrants.entrant_lists.entrant_list_60";
const LOOKS: &str = "entrants.driver_customisations.folder_0";
const HOLDERS: &str = "driver_statistics.driver_statistics";

#[test]
fn build_reply_applies_to_the_real_database() {
    let Some(db_path) = pristine() else {
        eprintln!("F1_25_DIR is not set or has no game database; skipping");
        return;
    };
    let bytes = std::fs::read(&db_path).unwrap();
    let stock = Gvdb::parse(&bytes).unwrap();
    let game = game::extract(&stock, None, "test");
    let scratch = std::env::var_os("DM_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(format!("dm-real-db-{}", std::process::id())));
    let (data_dir, cache_dir) = (scratch.join("addon-data"), scratch.join("cache"));
    std::fs::remove_dir_all(&data_dir).ok();
    std::fs::remove_dir_all(&cache_dir).ok();
    let mut roster = Roster::load(&data_dir).unwrap();
    let a = driver(&game, &mut roster, "Test", "Driver", "TST", 42);
    let b = driver(&game, &mut roster, "Second", "Person", "SEC", 43);
    let mut c = driver(&game, &mut roster, "Old", "Timer", "OLD", 44);
    c.retired = true;
    roster.drivers = vec![a.clone(), b.clone(), c.clone()];
    roster.save(&data_dir).unwrap();
    assert_eq!((a.stat_id, a.preset_id), (208, 366));
    assert_eq!((c.stat_id, c.preset_id), (210, 368));

    let reply = helper_build(&data_dir, &cache_dir, &db_path);
    assert_eq!(reply["warnings"], json!([]), "{reply:#}");
    let built: Built = serde_json::from_value(reply.clone()).expect("the launcher parses the build reply");
    let lang = built
        .ops
        .iter()
        .filter(|op| matches!(op, AssetOp::Lang(_)))
        .count();
    let adds = built.ops.iter().filter(|op| matches!(op, AssetOp::Db(_))).count();
    assert_eq!(adds, 6 + 6 + 4);
    assert_eq!(lang, 9);

    let features = Features {
        stable: BTreeSet::from([Kind::Group, Kind::Erp, Kind::Db, Kind::Lang, Kind::File]),
        experimental: BTreeSet::new(),
    };
    let opt_in = BTreeSet::new();
    let mut planner = Planner::new(&features, &opt_in);
    planner.add_fields(ADDON, built.fields.clone());
    for op in built.ops {
        planner.push(Op {
            op,
            origin: Origin {
                addon: ADDON.into(),
                item: None,
            },
        });
    }
    let mut db = stock.clone();
    planner.apply_db(&mut db).unwrap();
    let out = db.to_bytes();
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::write(scratch.join("gamedatabases.2025.bin"), &out).unwrap();
    std::fs::write(
        scratch.join("build-reply.json"),
        serde_json::to_vec_pretty(&reply).unwrap(),
    )
    .unwrap();
    let db = Gvdb::parse(&out).unwrap();
    let (before, after) = (Rows(&stock), Rows(&db));

    let mut report = Vec::new();
    for (container, row) in [
        ("entrants.drivers", "driver"),
        (LOOKS, "preloaded_driver_customisation_properties"),
        (HOLDERS, "DriverStatisticsHolder"),
        ("entrants.entrant_lists.entrants", "entrant"),
        (POOL_2026, "entrant"),
        (POOL_2025, "entrant"),
    ] {
        let (n0, n1) = (before.count(container, row), after.count(container, row));
        report.push(format!("{container}.{row}: {n0} -> {n1}"));
        let want = if container.starts_with("entrants.entrant_lists.entrant_list_") {
            2
        } else {
            3
        };
        assert_eq!(n1, n0 + want, "{container}");
    }

    let rows = after.find("entrants.drivers", "driver", "dm_208");
    assert_eq!(rows.len(), 1);
    let row = format!("entrants.drivers.driver_{}", rows[0]);
    let mut read = BTreeMap::new();
    for f in [
        "_metadata_",
        "stat_id",
        "initials",
        "car_number",
        "first_name",
        "last_name",
        "full_name",
        "nationality",
        "head",
        "audio_name",
        "available",
        "is_name_reversed",
        "f1_seasons_completed",
        "garage_slot_preference",
    ] {
        read.insert(f, after.text(&format!("{row}.{f}")));
    }
    assert_eq!(read["stat_id"], "208");
    assert_eq!(read["initials"], "TST");
    assert_eq!(read["available"], "True");
    assert_eq!(read["first_name"], "lng_dm_208_first");
    assert_eq!(
        after.0.get(&format!("{row}.available")).unwrap().tag,
        stock.get("entrants.drivers.driver_147.available").unwrap().tag
    );
    report.push(format!("{row}: {read:?}"));

    let stock_holders = before.count(HOLDERS, "DriverStatisticsHolder");
    for d in [&a, &b, &c] {
        report.extend(verify_driver(&after, stock_holders, d));
    }
    eprintln!("{}", report.join("\n"));
    let added = db.records.len() - stock.records.len();
    eprintln!("records added: {added}");
    assert_eq!(added, 3 * (29 + 12 + 24 + 10) + 4 * 10);
}

fn verify_driver(after: &Rows, stock_holders: usize, d: &Driver) -> Vec<String> {
    let mut report = Vec::new();
    let driver_labels: Vec<String> = (0..after.count("entrants.drivers", "driver"))
        .map(|i| after.text(&format!("entrants.drivers.driver_{i}._metadata_")))
        .collect();
    let rows = after.find("entrants.drivers", "driver", &d.label());
    assert_eq!(rows.len(), 1, "{} has one driver row", d.label());
    let row = format!("entrants.drivers.driver_{}", rows[0]);
    assert_eq!(after.text(&format!("{row}.stat_id")), d.stat_id.to_string());
    assert_eq!(after.text(&format!("{row}.initials")), d.code);
    assert_eq!(after.text(&format!("{row}.available")), "True");
    let h = after.find(HOLDERS, "DriverStatisticsHolder", &d.label());
    assert_eq!(h.len(), 1);
    assert!(h[0] >= stock_holders, "holder appended after every stock holder");
    let holder = format!("{HOLDERS}.DriverStatisticsHolder_{}", h[0]);
    let target = after.text(&format!("{holder}.driver"));
    assert!(
        driver_labels.contains(&target),
        "holder driver label {target} resolves"
    );
    let pace = after.text(&format!("{holder}.StatisticsDataSet_0.Statistic_3.evolution"));
    assert_eq!(
        after.text(&format!("{holder}.StatisticsDataSet_0.Statistic_3.info")),
        "pace"
    );
    assert!(pace.split(',').skip(4).count() >= 4);
    report.push(format!(
        "{holder}.driver = {target} (resolves to {row}); pace = {pace}"
    ));

    let e = after.find("entrants.entrant_lists.entrants", "entrant", &d.label());
    assert_eq!(e.len(), 1);
    let entrant = format!("entrants.entrant_lists.entrants.entrant_{}", e[0]);
    assert_eq!(after.text(&format!("{entrant}.driver")), d.label());
    let look = after.text(&format!("{entrant}.driver_customisation"));
    let l = after.find(LOOKS, "preloaded_driver_customisation_properties", &look);
    assert_eq!(l.len(), 1, "entrant look {look} resolves");
    let preset = format!("{LOOKS}.preloaded_driver_customisation_properties_{}", l[0]);
    assert_eq!(after.text(&format!("{preset}.stat_id")), d.preset_id.to_string());
    assert_eq!(after.text(&format!("{preset}.helmet")), d.helmet);

    for pool in [POOL_2026, POOL_2025] {
        let p = after.where_field(pool, "entrant", "entrant_reference", &d.label());
        if d.retired {
            assert!(p.is_empty(), "retired {} has no pool row in {pool}", d.label());
            report.push(format!("{pool}: no row for retired {}", d.label()));
        } else {
            assert_eq!(p.len(), 1);
            let r = format!("{pool}.entrant_{}", p[0]);
            assert_eq!(after.text(&format!("{r}.team_position")), "-1");
            assert_eq!(after.text(&format!("{r}.car_number")), "255");
            report.push(format!("{r}.entrant_reference = {} -> {entrant}", d.label()));
        }
    }
    report
}

#[test]
fn launcher_built_database_has_the_drivers() {
    let (Some(db_path), Some(built), Some(data_dir)) = (
        pristine(),
        std::env::var_os("DM_BUILT_DB").map(PathBuf::from),
        std::env::var_os("DM_DATA_DIR").map(PathBuf::from),
    ) else {
        eprintln!("F1_25_DIR, DM_BUILT_DB or DM_DATA_DIR is not set; skipping");
        return;
    };
    let stock = Gvdb::parse(&std::fs::read(db_path).unwrap()).unwrap();
    let db = Gvdb::parse(&std::fs::read(built).unwrap()).unwrap();
    let game = game::extract(&stock, None, "test");
    let roster = Roster::load(&data_dir).unwrap();
    let (before, after) = (Rows(&stock), Rows(&db));
    let stock_holders = before.count(HOLDERS, "DriverStatisticsHolder");
    let mut report = Vec::new();
    let (mut valid, mut active) = (0, 0);
    for d in &roster.drivers {
        if driver_manager::check::is_valid(&driver_manager::check::check(d, &roster, Some(&game))) {
            valid += 1;
            active += usize::from(!d.retired);
            report.extend(verify_driver(&after, stock_holders, d));
        } else {
            assert!(
                after.find("entrants.drivers", "driver", &d.label()).is_empty(),
                "invalid {} left out",
                d.label()
            );
            assert!(
                after
                    .find(HOLDERS, "DriverStatisticsHolder", &d.label())
                    .is_empty()
            );
            report.push(format!("invalid {} ({}) left out", d.label(), d.display_name()));
        }
    }
    for (container, row, want) in [
        ("entrants.drivers", "driver", valid),
        (HOLDERS, "DriverStatisticsHolder", valid),
        (POOL_2026, "entrant", active),
        (POOL_2025, "entrant", active),
    ] {
        let (n0, n1) = (before.count(container, row), after.count(container, row));
        report.push(format!("{container}.{row}: {n0} -> {n1}"));
        assert_eq!(n1, n0 + want, "{container}");
    }
    eprintln!("{}", report.join("\n"));
}
