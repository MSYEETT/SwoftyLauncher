use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use swofty_formats::gvdb::{Gvdb, Record};
use swofty_formats::hash::fnv1a64;

use crate::launcher::{ProfileTrack, ProfileTracks};
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
}
