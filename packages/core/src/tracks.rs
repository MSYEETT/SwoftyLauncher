use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use swofty_addon::asset::{AssetOp, FileKind, LangOp, OpFile, TrackOp};
use swofty_addon::manifest::AddonManifest;
use swofty_addon::ops::DbOp;
use swofty_formats::gvdb::{Gvdb, Record};
use swofty_formats::hash::fnv1a64;

use crate::launcher::{ProfileTrack, ProfileTracks};
use crate::map::Unusable;
use crate::store::Store;
use crate::{Result, io_err};

const TRACKS: &str = "tracks.Track";
const EVENTS: &str = "schedule.all_track_events.fixed_event";
const ACTIVITIES: &str = "schedule.activities.activity_race_weekend";
const CONTENT_SETS: &str = "content_sets.content_set";
const SEASON_SETS: [&str; 2] = ["2025", "2026"];
const RESOURCE_PREFIX: &str = "swofty_";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PlanTrack {
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addon: Option<String>,
}

struct Rows<'a> {
    by_key: HashMap<u64, &'a Record>,
}

impl<'a> Rows<'a> {
    fn new(db: &'a Gvdb) -> Self {
        Self {
            by_key: db.records.iter().map(|r| (r.key, r)).collect(),
        }
    }

    fn text(&self, path: &str) -> Option<String> {
        self.by_key
            .get(&fnv1a64(path))
            .map(|r| r.text().trim().to_owned())
    }

    fn table(&self, path: &str, fields: &[&str]) -> Vec<Vec<Option<String>>> {
        let (container, row) = path.rsplit_once('.').unwrap_or(("", path));
        let count = self
            .text(&format!("{container}._{row}_instancecount_"))
            .and_then(|n| n.parse::<usize>().ok())
            .unwrap_or(0);
        (0..count)
            .map(|i| {
                fields
                    .iter()
                    .map(|f| self.text(&format!("{path}_{i}.{f}")))
                    .collect()
            })
            .collect()
    }
}

pub fn list(db: &Gvdb, added: &[PlanTrack]) -> Vec<ProfileTrack> {
    let rows = Rows::new(db);
    let activities: HashMap<String, String> = rows
        .table(ACTIVITIES, &["_metadata_", "track"])
        .into_iter()
        .filter_map(|r| Some((r[0].clone()?, r[1].clone()?)))
        .collect();
    let mut events: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in rows.table(EVENTS, &["_metadata_", "activity"]) {
        let (Some(name), Some(activity)) = (r[0].clone(), r[1].clone()) else {
            continue;
        };
        let track = activities.get(&activity).cloned().unwrap_or(activity);
        events.entry(track).or_default().push(name);
    }
    let mut playable: Vec<String> = Vec::new();
    for (i, set) in rows.table(CONTENT_SETS, &["_metadata_"]).into_iter().enumerate() {
        if !set[0].as_deref().is_some_and(|l| SEASON_SETS.contains(&l)) {
            continue;
        }
        let lists = rows.table(&format!("{CONTENT_SETS}_{i}.available_tracks"), &["tracks"]);
        for list in lists.into_iter().filter_map(|r| r[0].clone()) {
            playable.extend(
                list.split(',')
                    .map(|c| c.trim().to_owned())
                    .filter(|c| !c.is_empty()),
            );
        }
    }
    let mut out: Vec<ProfileTrack> = Vec::new();
    for r in rows.table(TRACKS, &["_metadata_", "statId", "track_resource"]) {
        let (Some(code), Some(stat_id)) = (r[0].clone(), r[1].as_deref().and_then(|s| s.parse::<u32>().ok()))
        else {
            continue;
        };
        if out.iter().any(|t| t.code == code) {
            continue;
        }
        let package = added.iter().find(|p| p.code == code);
        let modded = package.is_some() || r[2].as_deref() == Some(&format!("{RESOURCE_PREFIX}{code}"));
        out.push(ProfileTrack {
            events: events.remove(&code).unwrap_or_default(),
            stat_id,
            playable: playable.contains(&code),
            modded,
            name: package.and_then(|p| p.name.clone()),
            addon: package.and_then(|p| p.addon.clone()),
            code,
        });
    }
    out
}

#[derive(Deserialize)]
struct PlanListing {
    #[serde(default)]
    buildhash: Option<String>,
    #[serde(default)]
    tracks: Vec<PlanTrack>,
}

type Cached = (PathBuf, Option<SystemTime>, u64, ProfileTracks);

static CACHE: Mutex<Option<Cached>> = Mutex::new(None);

fn stamp(path: &Path) -> (Option<SystemTime>, u64) {
    std::fs::metadata(path).map_or((None, 0), |m| (m.modified().ok(), m.len()))
}

pub fn for_profile(data: &Path, game: &crate::install::Game, profile: &str) -> Result<ProfileTracks> {
    let plan = crate::cache::latest_plan(&data.join("cache/overlay").join(profile));
    let listing: Option<PlanListing> = plan.as_deref().and_then(|p| crate::read_json(p).ok());
    let built_db = plan
        .as_deref()
        .and_then(Path::parent)
        .map(|d| d.join(crate::build::GVDB))
        .filter(|p| p.is_file());
    let db_path = built_db
        .clone()
        .unwrap_or_else(|| crate::build::original(data, game, crate::build::GVDB));
    let (modified, len) = stamp(&db_path);
    let buildhash = listing.as_ref().and_then(|l| l.buildhash.clone());
    let added = listing.map(|l| l.tracks).unwrap_or_default();
    {
        let cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((p, m, l, tracks)) = cache.as_ref()
            && *p == db_path
            && *m == modified
            && *l == len
            && tracks.buildhash == buildhash
        {
            return Ok(tracks.clone());
        }
    }
    let bytes = std::fs::read(&db_path).map_err(io_err(&db_path))?;
    let db = Gvdb::parse(&bytes)?;
    let tracks = ProfileTracks {
        built: plan.is_some(),
        buildhash,
        tracks: list(&db, &added),
    };
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((db_path, modified, len, tracks.clone()));
    Ok(tracks)
}

/// A data addon that is only tracks: its ops add one or more tracks and give
/// them names (`lang.set`, `lang.default`, `db.set` on a `TrackNameData` field), and it has no
/// code, screens, files, settings, launcher pages or other edits. The launcher
/// lists these on the Tracks page instead of the Addons page. An op file that
/// doesn't read makes the addon an ordinary one.
pub fn is_track_addon(dir: &Path, m: &AddonManifest) -> bool {
    let o = &m.operations;
    let other = !o.database.is_empty()
        || !o.erp.is_empty()
        || !o.ui.is_empty()
        || !o.files.is_empty()
        || !o.native.is_empty()
        || !o.screens.is_empty();
    if other || o.assets.is_empty() || m.launcher.is_some() || !m.options.is_empty() || m.saves.affects {
        return false;
    }
    let mut tracks = 0;
    for rel in &o.assets {
        let Some(doc) = std::fs::read(dir.join(rel))
            .ok()
            .and_then(|d| serde_json::from_slice(&d).ok())
        else {
            return false;
        };
        let Ok(file) = OpFile::from_value(doc, FileKind::Assets) else {
            return false;
        };
        for op in file.ops {
            match op {
                AssetOp::Track(TrackOp::Add { .. }) => tracks += 1,
                AssetOp::Lang(LangOp::Set { .. } | LangOp::Default { .. }) | AssetOp::Warn { .. } => {}
                AssetOp::Db(DbOp::Set { field, .. }) if field.starts_with("TrackNameData") => {}
                _ => return false,
            }
        }
    }
    tracks > 0
}

// The package folders an addon's `track.add` ops in its op files name.
fn added_dirs(store: &Store, id: &str, m: &AddonManifest) -> Vec<PathBuf> {
    let dir = store.addons_dir().join(id);
    let mut out = Vec::new();
    for rel in &m.operations.assets {
        let Some(file) = std::fs::read(dir.join(rel))
            .ok()
            .and_then(|d| serde_json::from_slice(&d).ok())
            .and_then(|doc| OpFile::from_value(doc, FileKind::Assets).ok())
        else {
            continue;
        };
        for op in file.ops {
            if let AssetOp::Track(TrackOp::Add { source }) = op
                && let Ok(d) = source.resolve(&crate::build::source_dir(store, id, source.root))
            {
                out.push(d);
            }
        }
    }
    out
}

/// The tracks installed addons add with `track.add`, named from their
/// packages, by addon id; the first addon to add a track keeps it. Ops or
/// packages that don't read are skipped.
pub fn installed(store: &Store) -> Vec<PlanTrack> {
    let mut out: Vec<PlanTrack> = Vec::new();
    for (id, m) in store.addons().unwrap_or_default() {
        for dir in added_dirs(store, &id, &m) {
            let Ok(pkg) = crate::map::read_package(&dir) else {
                continue;
            };
            if out.iter().any(|t| t.code == pkg.track_code) {
                continue;
            }
            out.push(PlanTrack {
                name: pkg.track_name.filter(|n| !n.trim().is_empty()),
                addon: Some(id.clone()),
                code: pkg.track_code,
            });
        }
    }
    out
}

/// The track packages an addon's op files add that builds leave out.
pub fn unusable(store: &Store, id: &str, m: &AddonManifest) -> Vec<Unusable> {
    added_dirs(store, id, m)
        .iter()
        .filter_map(|d| crate::map::read_package(d).err())
        .collect()
}

pub fn left_out_line(addon: &str, id: &str, why: &Unusable) -> String {
    match addon == id {
        true => format!("{id}: {}", why.text()),
        false => format!("{addon} ({id}): {}", why.text()),
    }
}

/// A [`left_out_line`] for each track package `m`'s op files add that builds leave out.
pub fn left_out_lines(store: &Store, m: &AddonManifest) -> Vec<String> {
    unusable(store, &m.id, m)
        .iter()
        .map(|u| left_out_line(&m.name, &m.id, u))
        .collect()
}

/// Puts each race weekend at a track a build left out back at its stock
/// track, and says so; a weekend the game doesn't ship is only named.
pub(crate) fn restore_weekends(db: &mut Gvdb, shipped: &Gvdb, left_out: &[&str]) -> Vec<String> {
    let text = |db: &Gvdb, path: &str| db.get_text(path).map(|t| t.trim().to_owned());
    let count = |db: &Gvdb, table: &str| {
        let (container, row) = table.rsplit_once('.').expect("a table path");
        db.instance_count(container, row).unwrap_or(0)
    };
    let mut gone: Option<Vec<&str>> = None;
    let mut out = Vec::new();
    for i in 0..count(db, ACTIVITIES) {
        let path = format!("{ACTIVITIES}_{i}.track");
        let Some(code) = text(db, &path).filter(|c| left_out.contains(&c.as_str())) else {
            continue;
        };
        let gone = gone.get_or_insert_with(|| {
            let have: Vec<String> = (0..count(db, TRACKS))
                .filter_map(|t| text(db, &format!("{TRACKS}_{t}._metadata_")))
                .collect();
            left_out
                .iter()
                .copied()
                .filter(|c| !have.iter().any(|h| h == c))
                .collect()
        });
        if !gone.contains(&code.as_str()) {
            continue;
        }
        let weekend = text(db, &format!("{ACTIVITIES}_{i}._metadata_")).unwrap_or_else(|| i.to_string());
        let stock = shipped
            .get(&path)
            .filter(|r| !left_out.contains(&r.text().trim()))
            .cloned();
        match stock {
            Some(r) => {
                let to = r.text().trim().to_owned();
                if db.set(&path, r.value, Some(r.tag)).is_ok() {
                    out.push(format!(
                        "race weekend {weekend}: races at {to} again; track {code} is left out of this build"
                    ));
                }
            }
            None => out.push(format!(
                "race weekend {weekend}: races at {code}, which is left out of this build; set its track in Race weekends"
            )),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(path: &str, value: &str) -> Record {
        Record::new(path, 2, value.as_bytes().to_vec())
    }

    fn db() -> Gvdb {
        let mut records = vec![
            rec("tracks._Track_instancecount_", "3"),
            rec("schedule.all_track_events._fixed_event_instancecount_", "3"),
            rec("schedule.activities._activity_race_weekend_instancecount_", "1"),
            rec("schedule.activities.activity_race_weekend_0._metadata_", "suzuka"),
            rec("schedule.activities.activity_race_weekend_0.track", "suzuka"),
            rec("content_sets._content_set_instancecount_", "2"),
            rec("content_sets.content_set_0._metadata_", "2024"),
            rec("content_sets.content_set_0._available_tracks_instancecount_", "1"),
            rec(
                "content_sets.content_set_0.available_tracks_0.tracks",
                "bahrain24",
            ),
            rec("content_sets.content_set_1._metadata_", "2026"),
            rec("content_sets.content_set_1._available_tracks_instancecount_", "1"),
            rec(
                "content_sets.content_set_1.available_tracks_0.tracks",
                "suzuka,okayama",
            ),
        ];
        for (i, (code, id, res)) in [
            ("suzuka", "13", "2019"),
            ("bahrain24", "35", "swofty_bahrain24"),
            ("okayama", "43", "swofty_okayama"),
        ]
        .into_iter()
        .enumerate()
        {
            records.push(rec(&format!("tracks.Track_{i}._metadata_"), code));
            records.push(rec(&format!("tracks.Track_{i}.statId"), id));
            records.push(rec(&format!("tracks.Track_{i}.track_resource"), res));
        }
        for (i, (name, activity)) in [
            ("rw_suzuka", "suzuka"),
            ("rw_suzuka_26", "suzuka"),
            ("rw_gone", "gone"),
        ]
        .into_iter()
        .enumerate()
        {
            records.push(rec(
                &format!("schedule.all_track_events.fixed_event_{i}._metadata_"),
                name,
            ));
            records.push(rec(
                &format!("schedule.all_track_events.fixed_event_{i}.activity"),
                activity,
            ));
        }
        Gvdb { records }
    }

    #[test]
    fn events_resolve_through_activities_and_packages_name_modded_tracks() {
        let added = [PlanTrack {
            code: "okayama".into(),
            name: Some("Okayama".into()),
            addon: Some("teamsimplified.mods".into()),
        }];
        let tracks = list(&db(), &added);
        let by = |c: &str| tracks.iter().find(|t| t.code == c).unwrap();
        assert_eq!(tracks.len(), 3);
        assert_eq!(by("suzuka").stat_id, 13);
        assert_eq!(by("suzuka").events, ["rw_suzuka", "rw_suzuka_26"]);
        assert!(!by("suzuka").modded);
        assert!(by("suzuka").playable && by("okayama").playable && !by("bahrain24").playable);
        assert!(
            by("bahrain24").modded && by("bahrain24").name.is_none() && by("bahrain24").events.is_empty()
        );
        let o = by("okayama");
        assert_eq!(
            (o.stat_id, o.modded, o.name.as_deref()),
            (43, true, Some("Okayama"))
        );
        assert_eq!(o.addon.as_deref(), Some("teamsimplified.mods"));
    }

    #[test]
    fn lists_the_tracks_installed_addons_add() {
        let root = std::env::temp_dir().join(format!("swofty-installed-tracks-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let store = Store::new(&root);
        let addon = |id: &str, ops: &str, package: Option<String>| {
            let dir = store.addons_dir().join(id);
            std::fs::create_dir_all(dir.join("ops")).unwrap();
            std::fs::write(
                dir.join("swofty.json"),
                format!(
                    r#"{{"schema_version":1,"id":"{id}","name":"{id}","version":"1.0.0","author":"t",
                    "compat":{{"game":"f1_25"}},"operations":{{"assets":["ops/track.json"]}}}}"#
                ),
            )
            .unwrap();
            std::fs::write(dir.join("ops/track.json"), ops).unwrap();
            if let Some(package) = package {
                std::fs::create_dir_all(dir.join("track")).unwrap();
                std::fs::write(dir.join("track/manifest.json"), package).unwrap();
            }
        };
        let add = r#"{"schema_version":1,"ops":[{"op":"track.add","source":"track"}]}"#;
        let like = |code: &str, name: &str| {
            crate::map::tests::OKAYAMA_LIKE
                .replace(
                    r#""track_code": "testring""#,
                    &format!(r#""track_code": "{code}""#),
                )
                .replace(
                    r#""track_name": "Test Ring""#,
                    &format!(r#""track_name": "{name}""#),
                )
        };
        addon("mod.bahrain24", add, Some(like("bahrain24", "Sepang")));
        addon("mod.okayama", add, Some(like("okayama", "")));
        addon("mod.twice", add, Some(like("okayama", "Again")));
        addon("mod.unread", add, None);
        addon("mod.other", r#"{"schema_version":1,"ops":[]}"#, None);
        addon(
            "mod.portimao",
            add,
            Some(r#"{ "track_code": "portimao", "track_name": "Portimao", "erp": "portimao.erp" }"#.into()),
        );
        let found = installed(&store);
        assert_eq!(
            found,
            [
                PlanTrack {
                    code: "bahrain24".into(),
                    name: Some("Sepang".into()),
                    addon: Some("mod.bahrain24".into()),
                },
                PlanTrack {
                    code: "okayama".into(),
                    name: None,
                    addon: Some("mod.okayama".into()),
                },
            ]
        );
        let addons = store.addons().unwrap();
        let bad = |id: &str| unusable(&store, id, &addons[id]);
        assert!(bad("mod.okayama").is_empty() && bad("mod.other").is_empty());
        assert_eq!(bad("mod.unread")[0].why, "it has no manifest.json");
        let portimao = bad("mod.portimao");
        assert_eq!(portimao.len(), 1);
        assert_eq!(portimao[0].why, crate::map::NO_BASE_TRACK);
        assert_eq!(
            left_out_line("Portimao", "mod.portimao", &portimao[0]),
            "Portimao (mod.portimao): track Portimao is left out; its manifest.json doesn't say which track \
             it replaces or copies. Import a fixed version, or ask its author for one"
        );
        assert!(
            left_out_line("mod.portimao", "mod.portimao", &portimao[0]).starts_with("mod.portimao: track ")
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn weekends_at_a_left_out_track_go_back_to_their_stock_track() {
        let weekend = |i: usize, label: &str, track: &str| {
            [
                rec(&format!("{ACTIVITIES}_{i}._metadata_"), label),
                rec(&format!("{ACTIVITIES}_{i}.track"), track),
            ]
        };
        let mut shipped = db();
        shipped
            .set(
                "schedule.activities._activity_race_weekend_instancecount_",
                "2",
                None,
            )
            .unwrap();
        shipped.records.extend(weekend(1, "rw_okayama", "okayama"));
        let mut built = shipped.clone();
        built
            .set(
                "schedule.activities._activity_race_weekend_instancecount_",
                "4",
                None,
            )
            .unwrap();
        built
            .set(&format!("{ACTIVITIES}_0.track"), "portimao", None)
            .unwrap();
        built.records.extend(weekend(2, "rw_added", "portimao"));
        built.records.extend(weekend(3, "rw_other", "monza"));
        let warnings = restore_weekends(&mut built, &shipped, &["portimao", "okayama"]);
        assert_eq!(
            warnings,
            [
                "race weekend suzuka: races at suzuka again; track portimao is left out of this build",
                "race weekend rw_added: races at portimao, which is left out of this build; set its track in Race weekends",
            ]
        );
        let track = |i: usize| built.get_text(&format!("{ACTIVITIES}_{i}.track")).unwrap();
        assert_eq!(
            [track(0), track(1), track(2), track(3)],
            ["suzuka", "okayama", "portimao", "monza"]
        );
        let mut untouched = shipped.clone();
        assert!(restore_weekends(&mut untouched, &shipped, &["monza"]).is_empty());
    }
}
