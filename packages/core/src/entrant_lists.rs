use std::collections::{BTreeSet, HashMap, HashSet};

use swofty_formats::gvdb::{Gvdb, Record, instance_count_path};
use swofty_formats::hash::fnv1a64;

use crate::ops::{DbOp, DbOps, record_key};

const TABLE: &str = "entrants.entrant_lists";
const ROW: &str = "entrant_list";
const SCOPES: [&str; 3] = [TABLE, "entrants.entrant_setup_modes", "content_sets"];
const MAX_ROWS: usize = 256;

const LIST_FIELDS: [&str; 13] = [
    "_metadata_",
    "_entrant_instancecount_",
    "_vehicle_package_override_instancecount_",
    "available",
    "display_name",
    "entrant_list_references",
    "fixed_garage_assignments",
    "leaderboard",
    "previous_season_driver_standings",
    "previous_season_team_standings",
    "restrict_custom_driver_numbers",
    "season",
    "stat_id",
];
const ENTRANT_FIELDS: [&str; 10] = [
    "_metadata_",
    "badge_customisation",
    "car_number",
    "driver",
    "driver_customisation",
    "entrant_reference",
    "team",
    "team_position",
    "vehicle",
    "vehicle_customisation",
];
const OVERRIDE_FIELDS: [&str; 3] = ["_metadata_", "override_vehicle_package", "target_vehicle_package"];
const MAX_ENTRANTS: usize = 64;
const MAX_OVERRIDES: usize = 8;

struct List {
    name: String,
    keys: Vec<(String, u8, Vec<u8>)>,
    needs: Vec<(Table, String)>,
    lists: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Table {
    Driver,
    Team,
    Vehicle,
    Livery,
}

impl Table {
    fn path(self) -> (&'static str, &'static str) {
        match self {
            Table::Driver => ("entrants.drivers", "driver"),
            Table::Team => ("entrants.teams", "team"),
            Table::Vehicle => ("entrants.vehicles", "vehicle"),
            Table::Livery => (
                "entrants.vehicles.vehicle_customisations",
                "vehicle_customisation",
            ),
        }
    }
}

fn labels(db: &Gvdb, at: &HashMap<u64, usize>, table: Table) -> HashSet<String> {
    let (container, row) = table.path();
    (0..count(db, at, container, row))
        .filter_map(|i| text(db, at, &format!("{container}.{row}_{i}._metadata_")))
        .collect()
}

fn row_paths(row: &str, entrants: usize, overrides: usize) -> Vec<String> {
    let mut paths: Vec<String> = LIST_FIELDS.iter().map(|f| format!("{row}.{f}")).collect();
    for k in 0..entrants {
        paths.extend(ENTRANT_FIELDS.iter().map(|f| format!("{row}.entrant_{k}.{f}")));
    }
    for k in 0..overrides {
        paths.extend(
            OVERRIDE_FIELDS
                .iter()
                .map(|f| format!("{row}.vehicle_package_override_{k}.{f}")),
        );
    }
    paths
}

/// The game's entrant lists and the fields that name them, from the stock database.
pub struct StockLists {
    lists: Vec<List>,
    refs: Vec<String>,
    watched: HashSet<u64>,
}

/// The entrant lists before one addon's database ops.
pub struct Before {
    names: HashSet<String>,
    refs: Vec<Option<Vec<String>>>,
}

fn index(db: &Gvdb) -> HashMap<u64, usize> {
    let mut at = HashMap::with_capacity(db.records.len());
    for (i, r) in db.records.iter().enumerate() {
        at.entry(r.key).or_insert(i);
    }
    at
}

fn text(db: &Gvdb, at: &HashMap<u64, usize>, path: &str) -> Option<String> {
    at.get(&fnv1a64(path)).map(|&i| db.records[i].text().into_owned())
}

fn count(db: &Gvdb, at: &HashMap<u64, usize>, container: &str, row: &str) -> usize {
    text(db, at, &instance_count_path(container, row))
        .and_then(|t| t.trim().parse().ok())
        .unwrap_or(0)
}

fn name(db: &Gvdb, at: &HashMap<u64, usize>, i: usize) -> Option<String> {
    text(db, at, &format!("{TABLE}.{ROW}_{i}._metadata_"))
}

fn items(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(String::from)
        .collect()
}

fn put(db: &mut Gvdb, at: &mut HashMap<u64, usize>, key: u64, tag: u8, value: Vec<u8>) {
    match at.get(&key) {
        Some(&i) => {
            db.records[i].tag = tag;
            db.records[i].value = value;
        }
        None => {
            at.insert(key, db.records.len());
            db.records.push(Record { key, tag, value });
        }
    }
}

fn in_scope(path: &str) -> bool {
    SCOPES.iter().any(|s| path.starts_with(s))
}

impl StockLists {
    pub fn new(stock: &Gvdb) -> Self {
        let at = index(stock);
        let rows: Vec<String> = (0..count(stock, &at, TABLE, ROW))
            .map(|i| format!("{TABLE}.{ROW}_{i}"))
            .collect();
        let known: HashMap<Table, HashSet<String>> =
            [Table::Driver, Table::Team, Table::Vehicle, Table::Livery]
                .into_iter()
                .map(|t| (t, labels(stock, &at, t)))
                .collect();
        let lists = rows
            .iter()
            .filter_map(|row| {
                let name = text(stock, &at, &format!("{row}._metadata_"))?;
                let entrants = (0..MAX_ENTRANTS)
                    .take_while(|k| at.contains_key(&fnv1a64(format!("{row}.entrant_{k}._metadata_"))))
                    .count();
                let overrides = (0..MAX_OVERRIDES)
                    .take_while(|k| {
                        at.contains_key(&fnv1a64(format!("{row}.vehicle_package_override_{k}._metadata_")))
                    })
                    .count();
                let prefix = format!("{row}.");
                let keys: Vec<(String, u8, Vec<u8>)> = row_paths(row, entrants, overrides)
                    .into_iter()
                    .filter_map(|path| {
                        let r = &stock.records[*at.get(&fnv1a64(&path))?];
                        Some((path[prefix.len()..].to_owned(), r.tag, r.value.clone()))
                    })
                    .collect();
                let value = |rel: &str| {
                    keys.iter()
                        .find(|(k, ..)| k == rel)
                        .map(|(_, _, v)| String::from_utf8_lossy(v).into_owned())
                        .unwrap_or_default()
                };
                let mut needs = Vec::new();
                for k in 0..entrants {
                    for (field, table) in [
                        ("driver", Table::Driver),
                        ("badge_customisation", Table::Driver),
                        ("team", Table::Team),
                        ("vehicle", Table::Vehicle),
                        ("vehicle_customisation", Table::Livery),
                    ] {
                        let label = value(&format!("entrant_{k}.{field}"));
                        if !label.is_empty() && known[&table].contains(&label) {
                            needs.push((table, label));
                        }
                    }
                }
                needs.sort_by(|a, b| (a.0 as u8, &a.1).cmp(&(b.0 as u8, &b.1)));
                needs.dedup();
                let lists = items(&value("entrant_list_references"));
                Some(List {
                    name,
                    keys,
                    needs,
                    lists,
                })
            })
            .collect();
        let mut refs = Vec::new();
        for i in 0..count(stock, &at, "content_sets", "content_set") {
            let set = format!("content_sets.content_set_{i}");
            for j in 0..count(stock, &at, &set, "available_entrant_lists") {
                refs.push(format!("{set}.available_entrant_lists_{j}.entrant_lists"));
            }
        }
        for i in 0..count(stock, &at, "entrants.entrant_setup_modes", "entrant_setup_mode") {
            for field in ["entrant_lists", "selectable_entrant_lists"] {
                refs.push(format!(
                    "entrants.entrant_setup_modes.entrant_setup_mode_{i}.{field}"
                ));
            }
        }
        refs.retain(|p| at.contains_key(&fnv1a64(p)));
        let mut watched: HashSet<u64> = refs.iter().map(fnv1a64).collect();
        watched.insert(fnv1a64(instance_count_path(TABLE, ROW)));
        for i in 0..MAX_ROWS {
            for field in ["_metadata_", "stat_id"] {
                watched.insert(fnv1a64(format!("{TABLE}.{ROW}_{i}.{field}")));
            }
        }
        Self { lists, refs, watched }
    }

    /// Whether `ops` can rename, remove or stop naming an entrant list.
    pub fn touched(&self, ops: &DbOps) -> bool {
        let key = |k: &str| match k.contains('[') {
            true => in_scope(k),
            false => record_key(k).is_ok_and(|k| self.watched.contains(&k)),
        };
        ops.ops.iter().any(|op| match op {
            DbOp::PutRecord { key: k, .. } | DbOp::RemoveRecord { key: k, .. } => key(k),
            DbOp::Set { select, .. } | DbOp::ListAppend { select, .. } | DbOp::ListRemove { select, .. } => {
                in_scope(select)
            }
            DbOp::AddRow { .. } | DbOp::RemoveRow { .. } => false,
        })
    }

    pub fn before(&self, db: &Gvdb) -> Before {
        let at = index(db);
        let count = count(db, &at, TABLE, ROW);
        Before {
            names: (0..count).filter_map(|i| name(db, &at, i)).collect(),
            refs: self
                .refs
                .iter()
                .map(|p| text(db, &at, p).map(|v| items(&v)))
                .collect(),
        }
    }

    /// Puts back the game's entrant lists one addon's ops renamed or removed,
    /// the rows a count it lowered hides and the list names it took out of
    /// content sets and entrant setup modes, and returns those lists and the
    /// lists it left out because the built database cannot resolve their entrants.
    pub fn keep(&self, db: &mut Gvdb, before: &Before) -> (Vec<String>, Vec<String>) {
        let mut at = index(db);
        let had = count(db, &at, TABLE, ROW);
        let shown: HashSet<String> = (0..had).filter_map(|i| name(db, &at, i)).collect();
        let mut n = had;
        let mut names = shown.clone();
        let have: HashMap<Table, HashSet<String>> =
            [Table::Driver, Table::Team, Table::Vehicle, Table::Livery]
                .into_iter()
                .map(|t| (t, labels(db, &at, t)))
                .collect();
        let mut left_out: BTreeSet<String> = BTreeSet::new();
        loop {
            let mut grew = false;
            for list in &self.lists {
                if !before.names.contains(&list.name) || names.contains(&list.name) {
                    continue;
                }
                let whole = list.needs.iter().all(|(t, l)| have[t].contains(l))
                    && list.lists.iter().all(|l| names.contains(l));
                if !whole {
                    left_out.insert(list.name.clone());
                    continue;
                }
                left_out.remove(&list.name);
                let row = format!("{TABLE}.{ROW}_{n}");
                let stale: HashSet<u64> = row_paths(&row, MAX_ENTRANTS, MAX_OVERRIDES)
                    .iter()
                    .map(fnv1a64)
                    .collect();
                db.records.retain(|r| !stale.contains(&r.key));
                at = index(db);
                for (rel, tag, value) in &list.keys {
                    put(db, &mut at, fnv1a64(format!("{row}.{rel}")), *tag, value.clone());
                }
                n += 1;
                names.insert(list.name.clone());
                grew = true;
            }
            if !grew {
                break;
            }
        }
        if n != had
            && let Some(&i) = at.get(&fnv1a64(instance_count_path(TABLE, ROW)))
        {
            db.records[i].value = n.to_string().into_bytes();
        }
        let mut kept: BTreeSet<String> = before
            .names
            .iter()
            .filter(|l| !shown.contains(*l) && names.contains(*l))
            .cloned()
            .collect();
        for (path, was) in self.refs.iter().zip(&before.refs) {
            let (Some(was), Some(&i)) = (was, at.get(&fnv1a64(path))) else {
                continue;
            };
            let mut now = items(&db.records[i].text());
            let back: Vec<String> = was
                .iter()
                .filter(|l| !now.contains(l) && names.contains(*l))
                .cloned()
                .collect();
            if back.is_empty() {
                continue;
            }
            now.extend(back.iter().cloned());
            db.records[i].value = now.join(",").into_bytes();
            kept.extend(back);
        }
        (kept.into_iter().collect(), left_out.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(path: &str, tag: u8, value: &str) -> Record {
        Record::new(path, tag, value)
    }

    fn stock() -> Gvdb {
        let mut records = vec![
            rec("entrants.entrant_lists._entrant_list_instancecount_", 0, "3"),
            rec("content_sets._content_set_instancecount_", 0, "1"),
            rec(
                "content_sets.content_set_0._available_entrant_lists_instancecount_",
                0,
                "1",
            ),
            rec(
                "content_sets.content_set_0.available_entrant_lists_0.entrant_lists",
                13,
                "f1_2026,f2_2025,f2_2026",
            ),
            rec(
                "entrants.entrant_setup_modes._entrant_setup_mode_instancecount_",
                0,
                "1",
            ),
            rec(
                "entrants.entrant_setup_modes.entrant_setup_mode_0.entrant_lists",
                13,
                "f2_2025,f2_2026",
            ),
        ];
        for (i, (name, id)) in [("f1_2026", "67"), ("f2_2026", "104"), ("f2_2025", "95")]
            .into_iter()
            .enumerate()
        {
            let row = format!("entrants.entrant_lists.entrant_list_{i}");
            records.push(rec(&format!("{row}._metadata_"), 4, name));
            records.push(rec(&format!("{row}.stat_id"), 0, id));
            records.push(rec(&format!("{row}.available"), 4, "False"));
            records.push(rec(&format!("{row}._entrant_instancecount_"), 0, "1"));
            records.push(rec(&format!("{row}.entrant_list_references"), 13, ""));
            records.push(rec(&format!("{row}.entrant_0._metadata_"), 4, name));
            records.push(rec(&format!("{row}.entrant_0.driver"), 4, "ann"));
            records.push(rec(&format!("{row}.entrant_0.team"), 4, "alpha"));
            records.push(rec(&format!("{row}.entrant_0.vehicle"), 4, "car"));
            records.push(rec(&format!("{row}.entrant_0.car_number"), 0, id));
        }
        for (container, row, label) in [
            ("entrants.drivers", "driver", "ann"),
            ("entrants.teams", "team", "alpha"),
            ("entrants.vehicles", "vehicle", "car"),
        ] {
            records.push(rec(&instance_count_path(container, row), 0, "1"));
            records.push(rec(&format!("{container}.{row}_0._metadata_"), 4, label));
        }
        Gvdb { records }
    }

    fn get(db: &Gvdb, path: &str) -> Option<String> {
        db.get_text(path)
    }

    #[test]
    fn puts_back_a_list_an_older_database_renamed_and_drops_from_names() {
        let stock = stock();
        let lists = StockLists::new(&stock);
        let mut db = stock.clone();
        let before = lists.before(&db);
        db.set(
            "entrants.entrant_lists.entrant_list_1._metadata_",
            "f2_2025",
            None,
        )
        .unwrap();
        db.set("entrants.entrant_lists.entrant_list_1.stat_id", "95", None)
            .unwrap();
        db.set("entrants.entrant_lists._entrant_list_instancecount_", "2", None)
            .unwrap();
        db.set(
            "content_sets.content_set_0.available_entrant_lists_0.entrant_lists",
            "f1_2026,f2_2025",
            None,
        )
        .unwrap();
        db.set(
            "entrants.entrant_setup_modes.entrant_setup_mode_0.entrant_lists",
            "f2_2025",
            None,
        )
        .unwrap();
        assert_eq!(lists.keep(&mut db, &before), (vec!["f2_2026".to_owned()], vec![]));
        assert_eq!(
            get(&db, "entrants.entrant_lists._entrant_list_instancecount_").as_deref(),
            Some("3")
        );
        assert_eq!(
            get(&db, "entrants.entrant_lists.entrant_list_2._metadata_").as_deref(),
            Some("f2_2026")
        );
        assert_eq!(
            get(&db, "entrants.entrant_lists.entrant_list_2.stat_id").as_deref(),
            Some("104")
        );
        assert_eq!(
            get(
                &db,
                "entrants.entrant_lists.entrant_list_2._entrant_instancecount_"
            )
            .as_deref(),
            Some("1")
        );
        assert_eq!(
            get(&db, "entrants.entrant_lists.entrant_list_2.entrant_0.driver").as_deref(),
            Some("ann")
        );
        assert_eq!(
            get(
                &db,
                "content_sets.content_set_0.available_entrant_lists_0.entrant_lists"
            )
            .as_deref(),
            Some("f1_2026,f2_2025,f2_2026")
        );
        assert_eq!(
            get(
                &db,
                "entrants.entrant_setup_modes.entrant_setup_mode_0.entrant_lists"
            )
            .as_deref(),
            Some("f2_2025,f2_2026")
        );
    }

    #[test]
    fn leaves_changes_that_keep_every_list() {
        let stock = stock();
        let lists = StockLists::new(&stock);
        let mut db = stock.clone();
        let before = lists.before(&db);
        db.set("entrants.entrant_lists.entrant_list_1.available", "True", None)
            .unwrap();
        db.set(
            "entrants.entrant_setup_modes.entrant_setup_mode_0.entrant_lists",
            "f2_2026,f2_2025,f1_2026",
            None,
        )
        .unwrap();
        let want = db.clone();
        assert_eq!(lists.keep(&mut db, &before), (vec![], vec![]));
        assert_eq!(db.to_bytes(), want.to_bytes());
    }

    fn pack(stock: &Gvdb) -> Gvdb {
        let mut db = stock.clone();
        db.set(
            "entrants.entrant_lists.entrant_list_1._metadata_",
            "f2_2025",
            None,
        )
        .unwrap();
        db.set("entrants.entrant_lists._entrant_list_instancecount_", "2", None)
            .unwrap();
        db.set(
            "content_sets.content_set_0.available_entrant_lists_0.entrant_lists",
            "f1_2026,f2_2025",
            None,
        )
        .unwrap();
        db
    }

    #[test]
    fn restored_list_replaces_what_the_pack_left_in_its_slot() {
        let stock = stock();
        let lists = StockLists::new(&stock);
        let mut db = pack(&stock);
        let before = lists.before(&stock);
        db.records.push(rec(
            "entrants.entrant_lists.entrant_list_2.entrant_0.driver",
            4,
            "gone",
        ));
        db.records.push(rec(
            "entrants.entrant_lists.entrant_list_2.entrant_5.driver",
            4,
            "gone",
        ));
        lists.keep(&mut db, &before);
        assert_eq!(
            get(&db, "entrants.entrant_lists.entrant_list_2.entrant_0.driver").as_deref(),
            Some("ann")
        );
        assert_eq!(
            get(&db, "entrants.entrant_lists.entrant_list_2.entrant_5.driver"),
            None
        );
    }

    #[test]
    fn leaves_out_a_list_whose_entrants_do_not_resolve() {
        let stock = stock();
        let lists = StockLists::new(&stock);
        let mut db = pack(&stock);
        let before = lists.before(&stock);
        db.set("entrants.drivers._driver_instancecount_", "0", None)
            .unwrap();
        let (kept, left_out) = lists.keep(&mut db, &before);
        assert!(kept.is_empty());
        assert_eq!(left_out, ["f2_2026"]);
        assert_eq!(
            get(&db, "entrants.entrant_lists._entrant_list_instancecount_").as_deref(),
            Some("2")
        );
        assert_eq!(
            get(
                &db,
                "content_sets.content_set_0.available_entrant_lists_0.entrant_lists"
            )
            .as_deref(),
            Some("f1_2026,f2_2025")
        );
    }
}
