use std::collections::{BTreeMap, BTreeSet};

use swofty_core::asset::{FileKind, OpFile, Origin};
use swofty_core::ops::DbReport;
use swofty_core::planner::{Features, Planner};
use swofty_formats::gvdb::{Gvdb, Record};

const VEHICLES: &str = "entrants.vehicles";
const VEHICLE: &str = "entrants.vehicles.vehicle";

fn stock() -> Gvdb {
    let mut records = vec![Record::new(
        &format!("{VEHICLES}._vehicle_instancecount_"),
        0,
        "3",
    )];
    for (i, (label, id)) in [
        ("mercedes_2025", "169"),
        ("redbull_2025", "166"),
        ("f2_car_26", "508"),
    ]
    .iter()
    .enumerate()
    {
        let row = format!("{VEHICLE}_{i}");
        records.push(Record::new(&format!("{row}._metadata_"), 4, *label));
        records.push(Record::new(&format!("{row}.stat_id"), 0, *id));
        records.push(Record::new(
            &format!("{row}.display_name"),
            4,
            format!("lng_{label}"),
        ));
    }
    Gvdb { records }
}

fn file(json: &str) -> OpFile {
    OpFile::from_value(serde_json::from_str(json).unwrap(), FileKind::Database).unwrap()
}

fn build(addons: &[(&str, String)]) -> (Gvdb, DbReport) {
    let features = Features::default();
    let opt_in = BTreeSet::new();
    let mut planner = Planner::new(&features, &opt_in);
    for (id, json) in addons {
        let origin = Origin {
            addon: (*id).into(),
            item: None,
        };
        planner.push_file(&origin, FileKind::Database, file(json));
    }
    let mut db = stock();
    let keys = db.records.iter().map(|r| r.key).collect();
    let mut report = DbReport::default();
    planner
        .apply_db_report(&mut db, &keys, &Default::default(), &mut report)
        .unwrap();
    (db, report)
}

fn old_pack() -> String {
    format!(
        r#"{{"schema_version":1,"ops":[{{"op":"set","select":"{VEHICLES}","field":"_vehicle_instancecount_","value":"2"}}]}}"#
    )
}

fn classic() -> String {
    format!(
        r#"{{"schema_version":1,"ops":[{{"op":"add-row","table":"{VEHICLES}","from":"{VEHICLE}[_metadata_=mercedes_2025]","set":{{"_metadata_":"cs_2016"}}}}]}}"#
    )
}

fn labels(db: &Gvdb) -> Vec<String> {
    let n = db.instance_count(VEHICLES, "vehicle").unwrap();
    (0..n)
        .filter_map(|i| db.get_text(&format!("{VEHICLE}_{i}._metadata_")))
        .collect()
}

#[test]
fn a_pack_before_an_addon_that_adds_vehicles_does_not_lose_the_stock_vehicle_its_count_hid() {
    let features = Features::default();
    let opt_in = BTreeSet::new();
    let mut planner = Planner::new(&features, &opt_in);
    for (id, json) in [("pack", old_pack()), ("classic", classic())] {
        let origin = Origin {
            addon: id.into(),
            item: None,
        };
        planner.push_file(&origin, FileKind::Database, file(&json));
    }
    let mut db = stock();
    let keys = db.records.iter().map(|r| r.key).collect();
    planner
        .apply_db_report(&mut db, &keys, &Default::default(), &mut DbReport::default())
        .unwrap();
    assert_eq!(
        labels(&db),
        ["mercedes_2025", "redbull_2025", "f2_car_26", "cs_2016"]
    );
    assert_eq!(
        planner.take_kept_rows(),
        BTreeMap::from([("pack".to_owned(), vec!["1 vehicle(s)".to_owned()])])
    );
}

#[test]
fn a_pack_after_an_addon_that_adds_vehicles_does_not_hide_them() {
    let (db, _) = build(&[("classic", classic()), ("pack", old_pack())]);
    assert_eq!(
        labels(&db),
        ["mercedes_2025", "redbull_2025", "f2_car_26", "cs_2016"]
    );
}
