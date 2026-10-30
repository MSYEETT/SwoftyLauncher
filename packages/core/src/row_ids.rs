//! The install's ids for the rows `add-row` gives an id of its own, in `$DATA/registry/row-ids.json`
//! next to the core's `ids.json` (docs/std/registry.md "Stat ids"). A key names an addon's row,
//! `<addon>/<table>.<row>/<label>`, and keeps its id at every build of every profile, whatever else
//! the build adds, since careers in the shared Steam save hold the ids.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use swofty_formats::gvdb::{Gvdb, instance_count_path};
use swofty_formats::hash::fnv1a64;

use crate::ops::{Claimed, RowIds, stat_id_field};
use crate::stdlib::reports;
use crate::{Loaded, Result, io_err};

pub const FILE: &str = "row-ids.json";
const LOCK: &str = "row-ids.lock";
const VERSION: u32 = 1;

/// The core's registries whose `ids.json` ids are ids of these tables' rows.
const REGISTRY_TABLES: &[(&str, &str)] = &[
    ("vehicle", "entrants.vehicles.vehicle"),
    (
        "vehicle.package",
        "entrants.vehicles.vehicle_packages.vehicle_package",
    ),
    ("content.package", "entrants.entrant_lists.entrant_list"),
    ("content.package.scenario", "game_setup.scenarios.scenario"),
    ("team.identity", "entrants.teams.team"),
    ("driver.identity", "entrants.drivers.driver"),
    ("driver.head", "entrants.animation.driver_heads.driver_head"),
];

#[derive(Debug, Default, Serialize, Deserialize)]
struct File {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    rows: BTreeMap<String, i64>,
    /// The profiles whose last build before the store seeded it.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    seeded: BTreeSet<String>,
}

pub fn path(data: &Path) -> PathBuf {
    reports::path(data, FILE)
}

/// Held while a build reads and writes `row-ids.json`; another build waits until it is dropped.
pub fn lock(data: &Path) -> Result<std::fs::File> {
    let dir = reports::dir(data);
    std::fs::create_dir_all(&dir).map_err(io_err(&dir))?;
    let path = dir.join(LOCK);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(io_err(&path))?;
    file.lock().map_err(io_err(&path))?;
    Ok(file)
}

/// The database file `rel` of the profile's last build in `overlays`, other than the build `current`.
pub fn last_build(overlays: &Path, current: &str, rel: &str) -> Option<PathBuf> {
    std::fs::read_dir(overlays)
        .ok()?
        .flatten()
        .filter(|e| e.file_name() != current)
        .map(|e| e.path().join(rel))
        .filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p)))
        .max()
        .map(|(_, p)| p)
}

#[derive(Default)]
struct Seed {
    path: Option<PathBuf>,
    db: Option<Option<(Gvdb, HashMap<u64, usize>)>>,
    tables: HashMap<String, HashMap<String, Vec<i64>>>,
}

impl Seed {
    // The id of the `nth` row labelled `label` in the last build's `table` rows named `row`.
    fn id(&mut self, table: &str, row: &str, label: &str, nth: usize) -> Option<i64> {
        let path = self.path.as_ref()?;
        let (db, index) = self
            .db
            .get_or_insert_with(|| {
                let db = Gvdb::parse(&std::fs::read(path).ok()?).ok()?;
                let index = db.records.iter().enumerate().map(|(i, r)| (r.key, i)).collect();
                Some((db, index))
            })
            .as_ref()?;
        let rows = self.tables.entry(format!("{table}.{row}")).or_insert_with(|| {
            let text = |p: &str| index.get(&fnv1a64(p)).map(|&i| db.records[i].text().into_owned());
            let field = stat_id_field(table, row);
            let n = text(&instance_count_path(table, row))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            let mut out: HashMap<String, Vec<i64>> = HashMap::new();
            for i in 0..n {
                let label = text(&format!("{table}.{row}_{i}._metadata_"));
                let id = text(&format!("{table}.{row}_{i}.{field}")).and_then(|v| v.trim().parse().ok());
                if let (Some(label), Some(id)) = (label, id) {
                    out.entry(label).or_default().push(id);
                }
            }
            out
        });
        rows.get(label)?.get(nth).copied()
    }
}

/// The kept ids, the ids the core's registry holds, and what this build claimed and changed.
#[derive(Default)]
pub struct Store {
    rows: BTreeMap<String, i64>,
    seeded: BTreeSet<String>,
    by_id: HashMap<String, BTreeMap<i64, String>>,
    pinned: HashMap<String, HashSet<i64>>,
    seed: Seed,
    claimed: HashSet<String>,
    building: HashSet<String>,
    added: usize,
    moved: usize,
    reused: usize,
    changed: bool,
    read_only: bool,
}

fn addon_of(key: &str) -> &str {
    key.split('/').next().unwrap_or(key)
}

impl Store {
    /// The store in `data`, the ids the core's `ids.json` holds (in `data`, else in the game folder
    /// `game`), and, at `profile`'s first build with the store, its last build's database `last` to
    /// seed rows the store has no id for. The strings are warnings.
    pub fn load(
        data: &Path,
        game: Option<&Path>,
        profile: &str,
        last: Option<PathBuf>,
    ) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        let mut store = Self::default();
        let file = path(data);
        match crate::load_json::<File>(&file) {
            Ok(Loaded::Found(f)) => {
                store.read_only = f.version > VERSION;
                store.seeded = f.seeded;
                for (key, id) in f.rows {
                    store.insert(key, id);
                }
            }
            Ok(Loaded::Missing) => {}
            Ok(Loaded::Damaged(to)) => warnings.push(format!(
                "database: {FILE} was damaged and was moved to {}; added rows take the ids of the last build",
                to.display()
            )),
            Err(e) => {
                store.read_only = true;
                warnings.push(format!(
                    "database: {FILE} can't be read ({e}); it is left as it is"
                ));
            }
        }
        if let Some(ids) = reports::found(data, game, reports::IDS)
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        {
            store.pinned = registry_ids(&ids);
        }
        if store.seeded.insert(profile.to_owned()) {
            store.seed.path = last;
            store.changed = true;
        }
        (store, warnings)
    }

    fn insert(&mut self, key: String, id: i64) {
        let table = key.split('/').nth(1).unwrap_or_default().to_owned();
        if let Some(old) = self.rows.insert(key.clone(), id)
            && let Some(ids) = self.by_id.get_mut(&table)
            && ids.get(&old) == Some(&key)
        {
            ids.remove(&old);
        }
        if let Some(prev) = self.by_id.entry(table).or_default().insert(id, key.clone())
            && prev != key
        {
            self.rows.remove(&prev);
        }
    }

    fn put(&mut self, key: &str, id: i64) {
        match self.rows.get(key) {
            Some(&kept) if kept == id => return,
            Some(_) => self.moved += 1,
            None => self.added += 1,
        }
        self.insert(key.to_owned(), id);
        self.changed = true;
    }

    /// Starts a build of `addons`; ids kept for other addons' rows are reused when a table is full.
    pub fn building(&mut self, addons: impl IntoIterator<Item = String>) {
        self.building = addons.into_iter().collect();
        self.claimed.clear();
        (self.added, self.moved, self.reused) = (0, 0, 0);
    }

    /// The id kept for `key`.
    pub fn get(&self, key: &str) -> Option<i64> {
        self.rows.get(key).copied()
    }

    /// The build line saying what the last build changed, if anything.
    pub fn summary(&self) -> Option<String> {
        if self.added + self.moved + self.reused == 0 {
            return None;
        }
        let mut parts = vec![format!("{} new", self.added)];
        if self.moved > 0 {
            parts.push(format!("{} moved", self.moved));
        }
        if self.reused > 0 {
            parts.push(format!("{} reused", self.reused));
        }
        Some(format!(
            "database: {FILE} keeps {} added-row id(s) ({})",
            self.rows.len(),
            parts.join(", ")
        ))
    }

    /// Writes the store to `data` when this build changed it.
    pub fn save(&self, data: &Path) -> Result<()> {
        if !self.changed || self.read_only {
            return Ok(());
        }
        let file = File {
            version: VERSION,
            rows: self.rows.clone(),
            seeded: self.seeded.clone(),
        };
        crate::write_json(&path(data), &file)
    }
}

// The ids `ids.json` holds and its careers use, by the table whose rows they are ids of.
fn registry_ids(doc: &Value) -> HashMap<String, HashSet<i64>> {
    let mut out: HashMap<String, HashSet<i64>> = HashMap::new();
    for (registry, table) in REGISTRY_TABLES {
        let ids = out.entry((*table).to_owned()).or_default();
        if let Some(records) = doc["ids"][*registry].as_object() {
            ids.extend(
                records
                    .values()
                    .filter_map(|e| e.as_i64().or_else(|| e["id"].as_i64())),
            );
        }
        if let Some(careers) = doc["careers"].as_object() {
            for career in careers.values() {
                if let Some(records) = career[*registry].as_object() {
                    ids.extend(records.values().filter_map(Value::as_i64));
                }
            }
        }
    }
    out.retain(|_, ids| !ids.is_empty());
    out
}

/// The store as one addon's build sees it.
pub struct Scoped<'a> {
    pub addon: &'a str,
    pub store: &'a RefCell<Store>,
}

impl RowIds for Scoped<'_> {
    fn claim(&self, table: &str, row: &str, label: &str) -> Claimed {
        let mut s = self.store.borrow_mut();
        let base = format!("{}/{table}.{row}/{label}", self.addon);
        let mut nth = 0;
        let key = loop {
            let key = match nth {
                0 => base.clone(),
                n => format!("{base}#{}", n + 1),
            };
            if !s.claimed.contains(&key) {
                break key;
            }
            nth += 1;
        };
        s.claimed.insert(key.clone());
        let kept = s.get(&key);
        let seed = match kept {
            Some(_) => None,
            None => s.seed.id(table, row, label, nth),
        };
        Claimed { key, kept, seed }
    }

    fn taken(&self, table: &str, row: &str, key: &str, id: i64) -> bool {
        let s = self.store.borrow();
        let table = format!("{table}.{row}");
        s.by_id
            .get(&table)
            .and_then(|ids| ids.get(&id))
            .is_some_and(|k| k != key)
            || s.pinned.get(&table).is_some_and(|ids| ids.contains(&id))
    }

    fn keep(&self, _: &str, _: &str, key: &str, id: i64) {
        self.store.borrow_mut().put(key, id);
    }

    fn reclaim(
        &self,
        table: &str,
        row: &str,
        key: &str,
        free: &dyn Fn(i64) -> bool,
    ) -> Option<(i64, String)> {
        let table = format!("{table}.{row}");
        let spare: Vec<(i64, String)> = {
            let s = self.store.borrow();
            let pinned = s.pinned.get(&table);
            s.by_id
                .get(&table)?
                .iter()
                .filter(|(id, k)| !s.building.contains(addon_of(k)) && pinned.is_none_or(|p| !p.contains(id)))
                .map(|(id, k)| (*id, k.clone()))
                .collect()
        };
        let (id, from) = spare.into_iter().find(|(id, _)| free(*id))?;
        let mut s = self.store.borrow_mut();
        s.put(key, id);
        s.reused += 1;
        Some((id, from))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::{Features, Op, Origin};
    use crate::ops::DbReport;
    use crate::planner::Planner;
    use std::collections::BTreeSet;
    use swofty_formats::gvdb::Record;

    const VEHICLES: (&str, &str) = ("entrants.vehicles", "vehicle");
    const PACKAGES: (&str, &str) = ("entrants.vehicles.vehicle_packages", "vehicle_package");

    fn db((table, row): (&str, &str), stock: i64) -> Gvdb {
        Gvdb {
            records: vec![
                Record::new(&format!("{table}._{row}_instancecount_"), 0, "1"),
                Record::new(&format!("{table}.{row}_0._metadata_"), 4, "base"),
                Record::new(&format!("{table}.{row}_0.stat_id"), 0, stock.to_string()),
            ],
        }
    }

    struct Built {
        ids: BTreeMap<String, i64>,
        store: Store,
        lines: Vec<String>,
    }

    fn try_build(
        (table, row): (&str, &str),
        stock: i64,
        addons: &[(&str, &[&str])],
        store: Store,
    ) -> crate::Result<Built> {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let mut p = Planner::new(&features, &opt_in);
        for (addon, labels) in addons {
            p.add_fields(
                addon,
                BTreeMap::from([(
                    format!("{table}.{row}"),
                    vec!["_metadata_".to_owned(), "stat_id".to_owned()],
                )]),
            );
            for label in *labels {
                let json = format!(
                    r#"{{"op":"db.add-row","table":"{table}","from":"{table}.{row}_0","set":{{"_metadata_":"{label}"}}}}"#
                );
                p.push(Op {
                    op: serde_json::from_str(&json).unwrap(),
                    origin: Origin {
                        addon: (*addon).to_owned(),
                        item: None,
                    },
                });
            }
        }
        p.keep_row_ids(store);
        let mut gvdb = db((table, row), stock);
        let stock_keys = gvdb.records.iter().map(|r| r.key).collect();
        let mut report = DbReport::default();
        p.apply_db_report(&mut gvdb, &stock_keys, &HashSet::new(), &mut report)?;
        let n = gvdb.instance_count(table, row).unwrap();
        let ids = (1..n)
            .map(|i| {
                let label = gvdb.get_text(&format!("{table}.{row}_{i}._metadata_")).unwrap();
                let id = gvdb.get_text(&format!("{table}.{row}_{i}.stat_id")).unwrap();
                (label, id.parse().unwrap())
            })
            .collect();
        Ok(Built {
            ids,
            store: p.take_row_ids().unwrap(),
            lines: report.lines,
        })
    }

    fn build(table: (&str, &str), stock: i64, addons: &[(&str, &[&str])], store: Store) -> Built {
        try_build(table, stock, addons, store).unwrap()
    }

    fn data_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-row-ids-{}-{name}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn put(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn an_order_swap_keeps_each_rows_id() {
        let first = build(
            VEHICLES,
            10,
            &[("a", &["a1", "a2"]), ("b", &["b1"])],
            Store::default(),
        );
        assert_eq!(
            first.ids,
            BTreeMap::from([("a1".into(), 11), ("a2".into(), 12), ("b1".into(), 13)])
        );
        let swapped = build(VEHICLES, 10, &[("b", &["b1"]), ("a", &["a1", "a2"])], first.store);
        assert_eq!(swapped.ids, first.ids);
        assert!(
            swapped
                .lines
                .iter()
                .any(|l| l.ends_with(": stat_id 13 (the source's is 10; kept from row-ids.json)")),
            "{:?}",
            swapped.lines
        );
        assert_eq!(swapped.store.summary(), None);
    }

    #[test]
    fn an_update_that_adds_a_row_keeps_the_old_ids_and_gives_the_new_row_a_fresh_one() {
        let first = build(
            VEHICLES,
            10,
            &[("a", &["a1", "a2"]), ("b", &["b1"])],
            Store::default(),
        );
        let updated = build(
            VEHICLES,
            10,
            &[("a", &["a0", "a1", "a2"]), ("b", &["b1"])],
            first.store,
        );
        assert_eq!(updated.ids["a1"], 11);
        assert_eq!(updated.ids["a2"], 12);
        assert_eq!(updated.ids["b1"], 13);
        assert_eq!(updated.ids["a0"], 14);
        assert_eq!(
            updated.store.summary().as_deref(),
            Some("database: row-ids.json keeps 4 added-row id(s) (1 new)")
        );
    }

    #[test]
    fn a_removed_addon_keeps_its_ids_and_gets_them_back() {
        let first = build(VEHICLES, 10, &[("a", &["a1"]), ("b", &["b1"])], Store::default());
        assert_eq!((first.ids["a1"], first.ids["b1"]), (11, 12));
        let without = build(VEHICLES, 10, &[("b", &["b1"]), ("c", &["c1"])], first.store);
        assert_eq!((without.ids["b1"], without.ids["c1"]), (12, 13));
        let back = build(
            VEHICLES,
            10,
            &[("a", &["a1"]), ("b", &["b1"]), ("c", &["c1"])],
            without.store,
        );
        assert_eq!(
            back.ids,
            BTreeMap::from([("a1".into(), 11), ("b1".into(), 12), ("c1".into(), 13)])
        );
    }

    #[test]
    fn two_profiles_share_a_rows_id_through_the_install_store() {
        let data = data_dir("profiles");
        let (store, warnings) = Store::load(&data, None, "one", None);
        assert!(warnings.is_empty(), "{warnings:?}");
        let one = build(VEHICLES, 10, &[("a", &["a1"]), ("b", &["b1"])], store);
        one.store.save(&data).unwrap();
        let saved: Value = serde_json::from_slice(&std::fs::read(path(&data)).unwrap()).unwrap();
        assert_eq!(saved["rows"]["b/entrants.vehicles.vehicle/b1"], 12);
        let (store, _) = Store::load(&data, None, "two", None);
        let two = build(VEHICLES, 10, &[("b", &["b1"])], store);
        assert_eq!(two.ids["b1"], 12);
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn ids_the_core_registry_holds_are_passed_over() {
        let data = data_dir("registry");
        put(
            &reports::path(&data, reports::IDS),
            r#"{"version":1,"ids":{"vehicle":{"mod.x/car":{"id":11,"since":"2026-10-29"},"mod.x/old":12}},
                "careers":{"dc-1":{"vehicle":{"mod.y/car":13}}}}"#,
        );
        let (store, _) = Store::load(&data, None, "p", None);
        let first = build(VEHICLES, 10, &[("a", &["a1"])], store);
        assert_eq!(first.ids["a1"], 14);
        let mut kept = Store::default();
        kept.put("a/entrants.vehicles.vehicle/a1", 11);
        kept.pinned = Store::load(&data, None, "p", None).0.pinned;
        let moved = build(VEHICLES, 10, &[("a", &["a1"])], kept);
        assert_eq!(moved.ids["a1"], 14);
        assert!(
            moved.lines.iter().any(|l| l.ends_with(
                "stat_id 14 (the source's is 10; 11, kept for it, is kept for another row or held by a record of the core's registry)"
            )),
            "{:?}",
            moved.lines
        );
        assert_eq!(moved.store.get("a/entrants.vehicles.vehicle/a1"), Some(14));
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_full_table_reuses_an_id_kept_for_an_addon_the_build_leaves_out_and_logs_it() {
        let first = build(
            PACKAGES,
            250,
            &[("a", &["a1", "a2", "a3", "a4"])],
            Store::default(),
        );
        assert_eq!(
            first.ids.values().copied().collect::<Vec<_>>(),
            [251, 252, 253, 254]
        );
        let e = try_build(
            PACKAGES,
            250,
            &[("a", &["a1", "a2", "a3", "a4"]), ("b", &["b1"])],
            Store::default(),
        )
        .err()
        .unwrap();
        assert!(e.to_string().contains("has no free stat_id up to 254"), "{e}");
        let b = build(PACKAGES, 250, &[("b", &["b1", "b2"])], first.store);
        assert_eq!((b.ids["b1"], b.ids["b2"]), (251, 252));
        let line = "b: add-row entrants.vehicles.vehicle_packages.vehicle_package_1 (from entrants.vehicles.vehicle_packages.vehicle_package_0): stat_id 251 (the source's is 250; no other id up to 254 is free; reused from a/entrants.vehicles.vehicle_packages.vehicle_package/a1, which this build leaves out)";
        assert!(b.lines.iter().any(|l| l == line), "{:?}", b.lines);
        assert_eq!(
            b.store
                .get("a/entrants.vehicles.vehicle_packages.vehicle_package/a1"),
            None
        );
        assert_eq!(
            b.store
                .get("a/entrants.vehicles.vehicle_packages.vehicle_package/a3"),
            Some(253)
        );
        assert_eq!(
            b.store.summary().as_deref(),
            Some("database: row-ids.json keeps 4 added-row id(s) (2 new, 2 reused)")
        );
    }

    #[test]
    fn rows_the_store_has_no_id_for_take_the_profiles_last_build_ids() {
        let data = data_dir("seed");
        let overlays = data.join("cache/overlay/p");
        let (table, row) = VEHICLES;
        let mut last = db(VEHICLES, 10);
        last.set(&format!("{table}._{row}_instancecount_"), "3", None)
            .unwrap();
        for (i, (label, id)) in [("a1", "40"), ("b1", "41")].iter().enumerate() {
            last.set(&format!("{table}.{row}_{}._metadata_", i + 1), *label, Some(4))
                .unwrap();
            last.set(&format!("{table}.{row}_{}.stat_id", i + 1), *id, Some(0))
                .unwrap();
        }
        let rel = "game_package/db.bin";
        put(&overlays.join("old").join(rel), "");
        std::fs::write(overlays.join("old").join(rel), last.to_bytes()).unwrap();
        assert_eq!(
            last_build(&overlays, "new", rel),
            Some(overlays.join("old").join(rel))
        );
        assert_eq!(last_build(&overlays, "old", rel), None);
        let (store, _) = Store::load(&data, None, "p", last_build(&overlays, "new", rel));
        let first = build(
            VEHICLES,
            10,
            &[("b", &["b1"]), ("a", &["a1"]), ("c", &["c1"])],
            store,
        );
        assert_eq!(
            first.ids,
            BTreeMap::from([("a1".into(), 40), ("b1".into(), 41), ("c1".into(), 42)])
        );
        assert!(
            first
                .lines
                .iter()
                .any(|l| l.ends_with("stat_id 41 (the source's is 10; as the profile's last build gave it)")),
            "{:?}",
            first.lines
        );
        assert_eq!(first.store.get("a/entrants.vehicles.vehicle/a1"), Some(40));
        first.store.save(&data).unwrap();
        let seed = |profile: &str| Store::load(&data, None, profile, last_build(&overlays, "new", rel)).0;
        assert_eq!(seed("p").seed.path, None);
        assert!(seed("q").seed.path.is_some());
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn two_rows_of_one_addon_with_one_label_keep_their_own_ids() {
        let first = build(VEHICLES, 10, &[("a", &["x", "x"])], Store::default());
        assert_eq!(first.store.get("a/entrants.vehicles.vehicle/x"), Some(11));
        assert_eq!(first.store.get("a/entrants.vehicles.vehicle/x#2"), Some(12));
        let again = build(VEHICLES, 10, &[("b", &["b1"]), ("a", &["x", "x"])], first.store);
        assert_eq!(again.ids["b1"], 13);
    }
}
