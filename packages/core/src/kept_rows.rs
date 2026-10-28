use std::collections::HashMap;

use swofty_formats::gvdb::{Gvdb, instance_count_path};
use swofty_formats::hash::fnv1a64;

const TABLES: [(&str, &str, &str); 5] = [
    ("entrants.vehicles", "vehicle", "vehicle"),
    (
        "entrants.vehicles.vehicle_packages",
        "vehicle_package",
        "vehicle package",
    ),
    (
        "car_setup.presets",
        "vehicle_package_setups",
        "vehicle package setup",
    ),
    ("entrants.teams", "team", "team"),
    ("entrants.drivers", "driver", "driver"),
];

pub type Rows = [Option<usize>; TABLES.len()];

fn index(db: &Gvdb) -> HashMap<u64, usize> {
    let mut at = HashMap::with_capacity(db.records.len());
    for (i, r) in db.records.iter().enumerate() {
        at.entry(r.key).or_insert(i);
    }
    at
}

pub fn counts(db: &Gvdb) -> Rows {
    let at = index(db);
    let mut out = [None; TABLES.len()];
    for (slot, (container, row, _)) in out.iter_mut().zip(TABLES) {
        *slot = at
            .get(&fnv1a64(instance_count_path(container, row)))
            .and_then(|&i| db.records[i].text().trim().parse().ok());
    }
    out
}

pub fn keep(db: &mut Gvdb, before: &Rows) -> Vec<String> {
    let at = index(db);
    let mut kept = Vec::new();
    for ((container, row, label), was) in TABLES.into_iter().zip(before) {
        let Some(was) = *was else { continue };
        let Some(&i) = at.get(&fnv1a64(instance_count_path(container, row))) else {
            continue;
        };
        let now: usize = db.records[i].text().trim().parse().unwrap_or(was);
        if now >= was {
            continue;
        }
        db.records[i].value = was.to_string().into_bytes();
        kept.push(format!("{} {label}(s)", was - now));
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use swofty_formats::gvdb::Record;

    fn db(vehicles: usize, teams: usize) -> Gvdb {
        Gvdb {
            records: vec![
                Record::new(
                    "entrants.vehicles._vehicle_instancecount_",
                    0,
                    vehicles.to_string(),
                ),
                Record::new("entrants.teams._team_instancecount_", 0, teams.to_string()),
            ],
        }
    }

    fn get(db: &Gvdb, path: &str) -> Option<String> {
        db.get_text(path)
    }

    #[test]
    fn raises_a_count_an_addon_lowered_back() {
        let mut db = db(413, 358);
        let before = counts(&db);
        db.set("entrants.vehicles._vehicle_instancecount_", "412", None)
            .unwrap();
        db.set("entrants.teams._team_instancecount_", "347", None)
            .unwrap();
        assert_eq!(keep(&mut db, &before), ["1 vehicle(s)", "11 team(s)"]);
        assert_eq!(
            get(&db, "entrants.vehicles._vehicle_instancecount_").as_deref(),
            Some("413")
        );
        assert_eq!(
            get(&db, "entrants.teams._team_instancecount_").as_deref(),
            Some("358")
        );
    }

    #[test]
    fn leaves_a_count_an_addon_raised_or_kept() {
        let mut db = db(413, 358);
        let before = counts(&db);
        db.set("entrants.vehicles._vehicle_instancecount_", "420", None)
            .unwrap();
        assert!(keep(&mut db, &before).is_empty());
        assert_eq!(
            get(&db, "entrants.vehicles._vehicle_instancecount_").as_deref(),
            Some("420")
        );
        assert_eq!(
            get(&db, "entrants.teams._team_instancecount_").as_deref(),
            Some("358")
        );
    }
}
