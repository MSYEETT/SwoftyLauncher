use serde_json::Value;

use crate::ui::{int, title_case, track_name};

// F1 25's stock tracks: gamedb code, statId, the name shown here and its country.
const STOCK: &[(&str, u8, &str, &str)] = &[
    ("melbourne", 0, "Melbourne", "Australia"),
    ("paul_ricard", 1, "Paul Ricard", "France"),
    ("shanghai", 2, "Shanghai", "China"),
    ("bahrain", 3, "Sakhir", "Bahrain"),
    ("catalunya", 4, "Barcelona", "Spain"),
    ("monaco", 5, "Monaco", "Monaco"),
    ("montreal", 6, "Montreal", "Canada"),
    ("silverstone", 7, "Silverstone", "Great Britain"),
    ("hanoi", 8, "Hanoi", "Vietnam"),
    ("hungaroring", 9, "Hungaroring", "Hungary"),
    ("spa_francorchamps", 10, "Spa-Francorchamps", "Belgium"),
    ("monza", 11, "Monza", "Italy"),
    ("singapore", 12, "Singapore", "Singapore"),
    ("suzuka", 13, "Suzuka", "Japan"),
    ("abu_dhabi", 14, "Yas Marina", "Abu Dhabi"),
    ("texas", 15, "Austin", "USA"),
    ("brazil", 16, "Interlagos", "Brazil"),
    ("austria", 17, "Red Bull Ring", "Austria"),
    ("sochi", 18, "Sochi", "Russia"),
    ("mexico", 19, "Mexico City", "Mexico"),
    ("baku", 20, "Baku", "Azerbaijan"),
    ("bahrain_pad", 21, "Sakhir Short", "Bahrain"),
    ("silverstone_int", 22, "Silverstone Short", "Great Britain"),
    ("texas_nat", 23, "Austin Short", "USA"),
    ("suzuka_east", 24, "Suzuka Short", "Japan"),
    ("styria", 25, "Styria", "Austria"),
    ("zandvoort", 26, "Zandvoort", "Netherlands"),
    ("imola", 27, "Imola", "Italy"),
    ("portimao", 28, "Portimão", "Portugal"),
    ("jeddah", 29, "Jeddah", "Saudi Arabia"),
    ("miami", 30, "Miami", "USA"),
    ("las_vegas", 31, "Las Vegas", "USA"),
    ("losail", 32, "Lusail", "Qatar"),
    ("imola_prototype", 33, "Imola Prototype", "Italy"),
    ("melbourne24", 34, "Melbourne 2024", "Australia"),
    ("bahrain24", 35, "Sakhir 2024", "Bahrain"),
    ("suzuka24", 36, "Suzuka 2024", "Japan"),
    ("imola24", 37, "Imola 2024", "Italy"),
    ("miami24", 38, "Miami 2024", "USA"),
    ("silverstone_reverse", 39, "Silverstone Reverse", "Great Britain"),
    ("austria_reverse", 40, "Red Bull Ring Reverse", "Austria"),
    ("zandvoort_reverse", 41, "Zandvoort Reverse", "Netherlands"),
    ("madrid", 42, "Madrid", "Spain"),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub name: String,
    pub country: Option<String>,
    pub modded: bool,
}

impl Track {
    pub fn label(&self) -> String {
        match &self.country {
            _ if self.modded => format!("{} (modded)", self.name),
            Some(c) if *c != self.name => format!("{} ({c})", self.name),
            _ => self.name.clone(),
        }
    }
}

pub fn index_name(weekend: &str) -> bool {
    !weekend.is_empty() && weekend.len() <= 3 && weekend.bytes().all(|b| b.is_ascii_digit())
}

pub fn index_named(rounds: &[Value]) -> bool {
    rounds
        .iter()
        .any(|r| r["weekend"].as_str().is_some_and(index_name))
}

fn stock(code: &str) -> Option<Track> {
    STOCK.iter().find(|s| s.0 == code).map(|s| Track {
        name: s.2.to_owned(),
        country: Some(s.3.to_owned()),
        modded: false,
    })
}

fn code_of(weekend: &str) -> Option<String> {
    if index_name(weekend) {
        return None;
    }
    let t = weekend.strip_prefix("rw_").unwrap_or(weekend);
    let t = t
        .strip_suffix("_26")
        .or_else(|| t.strip_suffix("_25"))
        .unwrap_or(t);
    Some(t.to_owned())
}

pub fn from_profile(t: &Value) -> Track {
    let code = t["code"].as_str().unwrap_or_default();
    let named = t["name"].as_str().map(str::trim).filter(|n| !n.is_empty());
    if t["modded"] == true {
        return Track {
            name: named.map_or_else(|| title_case(&code.replace('-', "_")), str::to_owned),
            country: None,
            modded: true,
        };
    }
    stock(code).unwrap_or_else(|| Track {
        name: named.map_or_else(|| track_name(code), str::to_owned),
        country: None,
        modded: false,
    })
}

pub fn track(stat_id: Option<i64>, weekend: &str, profile: Option<&Value>) -> Option<Track> {
    let tracks = profile.and_then(|p| p["tracks"].as_array());
    if let Some(id) = stat_id {
        if let Some(t) = tracks
            .into_iter()
            .flatten()
            .find(|t| t["stat_id"].as_i64() == Some(id))
        {
            return Some(from_profile(t));
        }
        if let Some(s) = STOCK.iter().find(|s| i64::from(s.1) == id) {
            return stock(s.0);
        }
    }
    let code = code_of(weekend)?;
    if let Some(t) = tracks.into_iter().flatten().find(|t| {
        t["code"].as_str() == Some(code.as_str())
            || t["events"]
                .as_array()
                .is_some_and(|e| e.iter().any(|e| e.as_str() == Some(weekend)))
    }) {
        return Some(from_profile(t));
    }
    Some(stock(&code).unwrap_or_else(|| Track {
        name: track_name(weekend),
        country: None,
        modded: false,
    }))
}

pub fn round_track(round: &Value, profile: Option<&Value>) -> Option<Track> {
    track(
        int(&round["track_stat_id"]),
        round["weekend"].as_str().unwrap_or_default(),
        profile,
    )
}

pub fn round_label(round: &Value, profile: Option<&Value>) -> String {
    round_track(round, profile).map_or_else(|| "Unknown track".into(), |t| t.label())
}

pub fn short_name(round: &Value, profile: Option<&Value>) -> String {
    round_track(round, profile).map_or_else(
        || format!("Round {}", int(&round["round"]).unwrap_or(0)),
        |t| t.name,
    )
}

pub fn move_order<T: Clone>(future: &[T], from: usize, to: usize) -> Vec<T> {
    let mut v = future.to_vec();
    if from < v.len() && to < v.len() {
        let x = v.remove(from);
        v.insert(to, x);
    }
    v
}

fn list(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

// `names` are the upcoming rounds in calendar order, the first being round `first`.
pub fn move_preview(names: &[String], first: i64, from: usize, to: usize) -> String {
    let (Some(moved), true) = (names.get(from), to < names.len()) else {
        return String::new();
    };
    if from == to {
        return format!("{moved} is already round {}.", first + to as i64);
    }
    let at = first + to as i64;
    if to < from {
        let shifted = &names[to..from];
        if let [one] = shifted {
            format!("{moved} becomes round {at}, {one} moves to round {}.", at + 1)
        } else {
            format!(
                "{moved} becomes round {at}; {} each move one round later (to rounds {} to {}).",
                list(shifted),
                at + 1,
                first + from as i64
            )
        }
    } else {
        let shifted = &names[from + 1..=to];
        let back = first + from as i64;
        if let [one] = shifted {
            format!("{moved} becomes round {at}, {one} moves to round {back}.")
        } else {
            format!(
                "{moved} becomes round {at}; {} each move one round earlier (to rounds {back} to {}).",
                list(shifted),
                at - 1
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn profile() -> Value {
        json!({ "tracks": [
            { "code": "suzuka", "stat_id": 13, "events": ["rw_suzuka", "rw_suzuka_26"], "playable": true, "modded": false },
            { "code": "bahrain24", "stat_id": 35, "events": [], "playable": true, "modded": true, "name": "Sepang" },
            { "code": "okayama", "stat_id": 43, "events": [], "playable": true, "modded": true, "name": "Okayama" }
        ] })
    }

    #[test]
    fn rounds_are_named_by_track_id_before_weekend_name() {
        let p = profile();
        let r = |w: &str, id: Value| json!({ "round": 2, "weekend": w, "track_stat_id": id });
        assert_eq!(round_label(&r("1", json!(13)), Some(&p)), "Suzuka (Japan)");
        assert_eq!(round_label(&r("1", json!(13)), None), "Suzuka (Japan)");
        assert_eq!(round_label(&r("4", json!(43)), Some(&p)), "Okayama (modded)");
        assert_eq!(
            round_label(&r("rw_bahrain24_26", json!(35)), Some(&p)),
            "Sepang (modded)"
        );
        assert_eq!(round_label(&r("rw_monaco_26", Value::Null), None), "Monaco");
        assert_eq!(
            round_label(&r("rw_abu_dhabi_26", Value::Null), None),
            "Yas Marina (Abu Dhabi)"
        );
        assert_eq!(round_label(&r("1", Value::Null), None), "Unknown track");
        assert_eq!(short_name(&r("1", Value::Null), None), "Round 2");
        assert!(index_named(&[
            r("rw_monaco_26", Value::Null),
            r("12", Value::Null)
        ]));
        assert!(!index_named(&[r("rw_monaco_26", Value::Null)]));
    }

    #[test]
    fn a_move_shifts_the_rounds_between() {
        let f = ["a", "b", "c", "d"];
        assert_eq!(move_order(&f, 1, 0), ["b", "a", "c", "d"]);
        assert_eq!(move_order(&f, 0, 3), ["b", "c", "d", "a"]);
        assert_eq!(move_order(&f, 2, 2), f);
        let n: Vec<String> = ["Melbourne", "Suzuka", "Monaco", "Monza"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(
            move_preview(&n, 1, 1, 0),
            "Suzuka becomes round 1, Melbourne moves to round 2."
        );
        assert_eq!(
            move_preview(&n, 9, 3, 0),
            "Monza becomes round 9; Melbourne, Suzuka and Monaco each move one round later (to rounds 10 to 12)."
        );
        assert_eq!(
            move_preview(&n, 9, 0, 1),
            "Melbourne becomes round 10, Suzuka moves to round 9."
        );
        assert_eq!(
            move_preview(&n, 9, 0, 2),
            "Melbourne becomes round 11; Suzuka and Monaco each move one round earlier (to rounds 9 to 10)."
        );
        assert_eq!(move_preview(&n, 9, 2, 2), "Monaco is already round 11.");
    }
}
