use serde_json::{Value, json};

use crate::host::Host;
use crate::ui::track_name;

pub const MIN_LAUNCHER: (u64, u64, u64) = (1, 0, 12);
const MAX_TRACK_ID: u64 = 254;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Championship,
    Other,
    Modded,
}

impl Group {
    pub fn heading(self) -> &'static str {
        match self {
            Group::Championship => "In this championship",
            Group::Other => "Other tracks",
            Group::Modded => "Modded tracks",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pick {
    pub weekend: String,
    pub track_stat_id: u8,
    pub label: String,
    pub group: Group,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Catalogue {
    pub picks: Vec<Pick>,
    pub refused: Vec<(String, String)>,
    pub resolved: Vec<(String, u8)>,
}

pub fn supported(launcher: &str) -> bool {
    let mut parts = launcher
        .trim()
        .trim_start_matches('v')
        .split(['.', '-', '+'])
        .map(|p| p.parse::<u64>().ok());
    match (
        parts.next().flatten(),
        parts.next().flatten(),
        parts.next().flatten(),
    ) {
        (Some(a), Some(b), Some(c)) => (a, b, c) >= MIN_LAUNCHER,
        _ => false,
    }
}

pub fn fetch(host: &mut Host, launcher: &str) -> Result<Value, String> {
    if !supported(launcher) {
        return Err(format!(
            "Swofty Launcher {}.{}.{} or later lists every track in this profile; this one offers only tracks the championship already has.",
            MIN_LAUNCHER.0, MIN_LAUNCHER.1, MIN_LAUNCHER.2
        ));
    }
    match host.call("profile_tracks", Value::Null) {
        Ok(Value::Null) => {
            Err("The launcher hasn't found F1 25, so it can't list the tracks in this profile.".into())
        }
        Ok(v) => Ok(v),
        Err(e) => Err(format!("The launcher couldn't list this profile's tracks: {e}")),
    }
}

pub fn valid_weekend(name: &str) -> bool {
    (3..=31).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

pub fn modded_weekend(code: &str, style_26: bool) -> String {
    let code: String = code
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("rw_{code}{}", if style_26 { "_26" } else { "" })
}

fn preferred(events: &[String], style_26: bool) -> Option<String> {
    let usable: Vec<&String> = events.iter().filter(|e| !e.ends_with("_beta")).collect();
    usable
        .iter()
        .find(|e| e.ends_with("_26") == style_26)
        .or_else(|| usable.first())
        .map(|e| (*e).clone())
}

fn label_of(t: &Value) -> String {
    let track = crate::calendar::from_profile(t);
    if track.modded { track.name } else { track.label() }
}

pub fn catalogue(profile: &Value, in_season: &[String], season_ids: &[u8], known: &[String]) -> Catalogue {
    let style_26 = in_season.iter().any(|w| w.ends_with("_26"));
    let tracks = profile["tracks"].as_array().cloned().unwrap_or_default();
    let mut out = Catalogue::default();
    let mut names: Vec<(String, u8, usize)> = Vec::new();
    for (i, t) in tracks.iter().enumerate() {
        let Some(id) = t["stat_id"].as_u64().filter(|&id| id <= MAX_TRACK_ID) else {
            continue;
        };
        let id = id as u8;
        let events: Vec<String> = t["events"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| e.as_str().map(str::to_owned))
            .collect();
        for e in &events {
            names.push((e.clone(), id, i));
        }
        if t["modded"] == true && events.is_empty() {
            let code = t["code"].as_str().unwrap_or_default();
            for s in [false, true] {
                names.push((modded_weekend(code, s), id, i));
            }
        }
    }
    let lookup = |w: &str| names.iter().find(|(n, _, _)| n == w).map(|&(_, id, i)| (id, i));
    out.resolved = names.iter().map(|(n, id, _)| (n.clone(), *id)).collect();
    let mut taken: Vec<usize> = in_season
        .iter()
        .filter_map(|w| lookup(w).map(|(_, i)| i))
        .collect();
    taken.extend(tracks.iter().enumerate().filter_map(|(i, t)| {
        t["stat_id"]
            .as_u64()
            .is_some_and(|id| season_ids.iter().any(|&s| u64::from(s) == id))
            .then_some(i)
    }));
    let mut used: Vec<usize> = taken.clone();
    for w in known
        .iter()
        .filter(|w| !in_season.contains(w) && !crate::calendar::index_name(w))
    {
        match lookup(w) {
            Some((id, i)) if tracks[i]["playable"] == true => {
                if taken.contains(&i) {
                    continue;
                }
                used.push(i);
                out.picks.push(Pick {
                    weekend: w.clone(),
                    track_stat_id: id,
                    label: label_of(&tracks[i]),
                    group: if tracks[i]["modded"] == true {
                        Group::Modded
                    } else {
                        Group::Championship
                    },
                });
            }
            Some((_, i)) => out.refused.push((
                label_of(&tracks[i]),
                "it isn't in this profile's Grand Prix track list".into(),
            )),
            None => out.refused.push((
                track_name(w),
                format!("{w} isn't a track in this profile's last build; turn its mod back on and build"),
            )),
        }
    }
    for (i, t) in tracks.iter().enumerate() {
        if used.contains(&i) {
            continue;
        }
        let modded = t["modded"] == true;
        let code = t["code"].as_str().unwrap_or_default();
        let events: Vec<String> = t["events"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| e.as_str().map(str::to_owned))
            .collect();
        if !modded && (events.is_empty() || t["playable"] != true) {
            continue;
        }
        let label = label_of(t);
        let id = t["stat_id"].as_u64().unwrap_or(u64::MAX);
        if id > MAX_TRACK_ID {
            out.refused
                .push((label, format!("its track id {id} is above {MAX_TRACK_ID}")));
            continue;
        }
        if t["playable"] != true {
            out.refused
                .push((label, "it isn't in this profile's Grand Prix track list".into()));
            continue;
        }
        let weekend = preferred(&events, style_26).unwrap_or_else(|| modded_weekend(code, style_26));
        if !valid_weekend(&weekend) {
            out.refused.push((
                label,
                format!("its code {code} doesn't make a weekend name (3 to 31 of a-z, 0-9 and _)"),
            ));
            continue;
        }
        if in_season.contains(&weekend) {
            continue;
        }
        out.picks.push(Pick {
            weekend,
            track_stat_id: id as u8,
            label,
            group: if modded { Group::Modded } else { Group::Other },
        });
    }
    out.picks
        .sort_by(|a, b| a.group.cmp(&b.group).then_with(|| a.label.cmp(&b.label)));
    out
}

impl Catalogue {
    pub fn track_of(&self, weekend: &str) -> Option<u8> {
        self.resolved
            .iter()
            .find(|(n, _)| n == weekend)
            .map(|&(_, id)| id)
    }

    pub fn options(&self, placeholder: &str, headings: bool) -> Vec<Value> {
        let mut opts = vec![json!({ "value": "", "label": placeholder })];
        let mut group = None;
        for p in &self.picks {
            if headings && group != Some(p.group) {
                opts.push(json!({ "heading": p.group.heading() }));
                group = Some(p.group);
            }
            let label = if headings {
                p.label.clone()
            } else {
                format!("{} · {}", p.label, p.group.heading())
            };
            opts.push(json!({ "value": p.weekend, "label": label }));
        }
        opts
    }
}

pub fn verify(args: &mut Value, cat: Option<&Catalogue>) -> Result<(), String> {
    let Some(cat) = cat else {
        return Ok(());
    };
    let w = args["weekend"].as_str().unwrap_or_default().to_owned();
    match cat.track_of(&w) {
        Some(id) => {
            args["verified"] = json!(true);
            args["track_stat_id"] = json!(id);
            Ok(())
        }
        None => Err(format!(
            "{} isn't a track in this profile's last build, so the editor won't add it.",
            track_name(&w)
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> Value {
        json!({ "built": true, "buildhash": "abc", "tracks": [
            { "code": "suzuka", "stat_id": 13, "events": ["rw_suzuka", "rw_suzuka_26"], "playable": true, "modded": false },
            { "code": "monaco", "stat_id": 5, "events": ["rw_monaco", "rw_monaco_26"], "playable": true, "modded": false },
            { "code": "bahrain", "stat_id": 3, "events": ["rw_bahrain", "rw_bahrain_beta", "rw_bahrain_26"], "playable": true, "modded": false },
            { "code": "portimao", "stat_id": 28, "events": ["rw_portimao"], "playable": false, "modded": false },
            { "code": "hanoi", "stat_id": 8, "events": [], "playable": false, "modded": false },
            { "code": "bahrain24", "stat_id": 35, "events": [], "playable": true, "modded": true, "name": "Sepang", "addon": "teamsimplified.mods" },
            { "code": "okayama", "stat_id": 43, "events": [], "playable": true, "modded": true, "name": "Okayama" },
            { "code": "big-ring", "stat_id": 255, "events": [], "playable": true, "modded": true }
        ] })
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn groups_stock_and_modded_tracks_and_skips_the_calendar() {
        let season = s(&["rw_bahrain_26", "rw_suzuka_26"]);
        let known = s(&["rw_bahrain_26", "rw_suzuka_26", "rw_monaco_26", "rw_gone_26"]);
        let cat = catalogue(&profile(), &season, &[], &known);
        let picks: Vec<(&str, u8, Group)> = cat
            .picks
            .iter()
            .map(|p| (p.weekend.as_str(), p.track_stat_id, p.group))
            .collect();
        assert_eq!(
            picks,
            [
                ("rw_monaco_26", 5, Group::Championship),
                ("rw_okayama_26", 43, Group::Modded),
                ("rw_bahrain24_26", 35, Group::Modded),
            ]
        );
        assert_eq!(cat.picks[2].label, "Sepang");
        let why: Vec<&str> = cat.refused.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(why, ["Gone", "Big Ring"]);
        assert_eq!(cat.track_of("rw_suzuka"), Some(13));
        assert_eq!(cat.track_of("rw_okayama"), Some(43));
        assert_eq!(cat.track_of("rw_hanoi"), None);
    }

    #[test]
    fn a_2025_calendar_takes_the_plain_event_names() {
        let cat = catalogue(&profile(), &s(&["rw_bahrain"]), &[], &[]);
        let names: Vec<&str> = cat.picks.iter().map(|p| p.weekend.as_str()).collect();
        assert_eq!(names, ["rw_monaco", "rw_suzuka", "rw_okayama", "rw_bahrain24"]);
        assert!(cat.picks.iter().all(|p| p.group != Group::Championship));
    }

    #[test]
    fn verify_adds_the_track_id_or_refuses() {
        let cat = catalogue(&profile(), &s(&["rw_bahrain_26"]), &[], &[]);
        let mut args = json!({ "weekend": "rw_okayama_26", "at": 3 });
        verify(&mut args, Some(&cat)).unwrap();
        assert_eq!(
            args,
            json!({ "weekend": "rw_okayama_26", "at": 3, "verified": true, "track_stat_id": 43 })
        );
        let mut bad = json!({ "weekend": "rw_hanoi_26" });
        assert!(verify(&mut bad, Some(&cat)).is_err());
        let mut old = json!({ "weekend": "rw_hanoi_26" });
        verify(&mut old, None).unwrap();
        assert_eq!(old, json!({ "weekend": "rw_hanoi_26" }));
    }

    #[test]
    fn options_group_under_headings() {
        let cat = catalogue(&profile(), &s(&["rw_bahrain_26"]), &[], &s(&["rw_monaco_26"]));
        let opts = cat.options("Choose a track…", true);
        let heads: Vec<&str> = opts.iter().filter_map(|o| o["heading"].as_str()).collect();
        assert_eq!(heads, ["In this championship", "Other tracks", "Modded tracks"]);
        let flat = cat.options("Choose a track…", false);
        assert!(flat.iter().all(|o| o.get("heading").is_none()));
        assert_eq!(flat[1]["label"], "Monaco · In this championship");
    }

    #[test]
    fn an_index_named_season_is_matched_by_track_id() {
        let season = s(&["0", "1", "2"]);
        let known = s(&["0", "1", "2", "3"]);
        let cat = catalogue(&profile(), &season, &[3, 13, 43], &known);
        let names: Vec<&str> = cat.picks.iter().map(|p| p.weekend.as_str()).collect();
        assert_eq!(names, ["rw_monaco", "rw_bahrain24"]);
        assert!(cat.refused.iter().all(|(l, _)| l != "3"));
        assert_eq!(cat.picks[0].label, "Monaco");
    }

    #[test]
    fn launcher_versions() {
        assert!(supported("1.0.12") && supported("1.1.0") && supported("v2.0.0-rc1"));
        assert!(!supported("1.0.11") && !supported("") && !supported("dev"));
    }
}
