use serde_json::{Map, Value, json};

use crate::check;
use crate::fields::DRIVER_FIELDS;
use crate::game::GameData;
use crate::model::{Driver, Rating, Roster, STATS, TRAITS};

pub const DRIVERS: &str = "entrants.drivers";
pub const DRIVER_FROM: &str = "entrants.drivers.driver[_metadata_=jarno_opmeer]";
pub const LOOKS: &str = "entrants.driver_customisations.folder[_metadata_=ai_driver_customisations]";
pub const LOOK_FROM: &str = "entrants.driver_customisations.folder[_metadata_=ai_driver_customisations].preloaded_driver_customisation_properties[_metadata_=official_driver_customisation_for_driver_pool]";
pub const STATS_TABLE: &str = "driver_statistics.driver_statistics";
pub const STATS_FROM: &str =
    "driver_statistics.driver_statistics.DriverStatisticsHolder[_metadata_=jarno_opmeer]";
pub const ENTRANTS: &str = "entrants.entrant_lists.entrants";
pub const ENTRANT_FROM: &str = "entrants.entrant_lists.entrants.entrant[_metadata_=jarno_opmeer]";
pub const POOLS: [&str; 2] = [
    "entrants.entrant_lists.entrant_list[_metadata_=f1_2026_driver_pool]",
    "entrants.entrant_lists.entrant_list[_metadata_=f1_2025_driver_pool]",
];
const POOL_FROM_ROW: &str = "entrant[_metadata_=opmeer]";

const LOOK_FIELDS: [&str; 12] = [
    "_metadata_",
    "available",
    "badge",
    "car_livery",
    "gloves",
    "helmet",
    "linked_driver_stat_id",
    "podium_emote",
    "pose",
    "stat_id",
    "suit",
    "victory_vo",
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

fn stats_fields() -> Vec<String> {
    let mut f: Vec<String> = [
        "_metadata_",
        "driver",
        "_StatisticsDataSet_instancecount_",
        "StatisticsDataSet_0._metadata_",
        "StatisticsDataSet_0.game_setup_modes",
        "StatisticsDataSet_0.is_invalid",
        "StatisticsDataSet_0.seasons",
        "StatisticsDataSet_0.weekend_formats",
        "StatisticsDataSet_0._Statistic_instancecount_",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    for i in 0..STATS.len() {
        for part in ["_metadata_", "info", "evolution"] {
            f.push(format!("StatisticsDataSet_0.Statistic_{i}.{part}"));
        }
    }
    f
}

pub fn fields() -> Value {
    let mut m = Map::new();
    m.insert(format!("{DRIVERS}.driver"), json!(DRIVER_FIELDS.as_slice()));
    m.insert(
        format!("{LOOKS}.preloaded_driver_customisation_properties"),
        json!(LOOK_FIELDS),
    );
    m.insert(
        format!("{STATS_TABLE}.DriverStatisticsHolder"),
        json!(stats_fields()),
    );
    m.insert(format!("{ENTRANTS}.entrant"), json!(ENTRANT_FIELDS));
    for pool in POOLS {
        m.insert(format!("{pool}.entrant"), json!(ENTRANT_FIELDS));
    }
    Value::Object(m)
}

fn b(v: bool) -> String {
    if v { "True" } else { "False" }.to_owned()
}

fn frac(v: f64) -> String {
    let t = format!("{:.3}", v.clamp(0., 1.));
    let t = t.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() { "0".into() } else { t.to_owned() }
}

pub fn curve(r: Rating) -> String {
    let s = f64::from(r.start) / 100.;
    let p = f64::from(r.peak) / 100.;
    let m = (s + p) / 2.;
    format!(
        "@v2@minx:0,maxx:10,miny:0,maxy:100,0:{},0.25:{},0.5:{},1:{}",
        frac(s),
        frac(m),
        frac(p),
        frac(p)
    )
}

pub fn surname(d: &Driver) -> String {
    d.last.trim().to_uppercase()
}

pub fn full_name(d: &Driver) -> String {
    let (first, last) = (d.first.trim(), surname(d));
    if d.reversed {
        format!("{last} {first}")
    } else {
        format!("{first} {last}")
    }
}

fn set(pairs: &[(&str, String)]) -> Value {
    Value::Object(
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), Value::String(v.clone())))
            .collect(),
    )
}

fn add_row(table: &str, from: &str, pairs: &[(&str, String)]) -> Value {
    json!({ "op": "db.add-row", "table": table, "from": from, "set": set(pairs) })
}

pub fn trait_value(v: f64) -> String {
    let t = format!("{:.2}", v.clamp(0., 1.));
    let t = t.trim_end_matches('0');
    if t.ends_with('.') {
        format!("{t}0")
    } else {
        t.to_owned()
    }
}

fn driver_set(d: &Driver) -> Vec<(&'static str, String)> {
    let mut set = vec![
        ("_metadata_", d.label()),
        ("audio_name", d.voice.clone()),
        ("available", b(true)),
        ("car_number", d.number.to_string()),
        ("date_of_birth.year", d.birth.year.to_string()),
        ("date_of_birth.month", d.birth.month_name().to_owned()),
        ("date_of_birth.day", d.birth.day.to_string()),
        ("f1_seasons_completed", d.seasons.to_string()),
        ("first_name", d.lang_key("first")),
        ("full_name", d.lang_key("full")),
        ("garage_slot_preference", d.garage.to_string()),
        ("head", d.head.clone()),
        ("initials", d.code.clone()),
        ("is_name_reversed", b(d.reversed)),
        ("last_name", d.lang_key("last")),
        ("nationality", d.nationality.clone()),
        ("stat_id", d.stat_id.to_string()),
    ];
    const KEYS: [&str; 6] = [
        "driver_characteristics.aggression",
        "driver_characteristics.control",
        "driver_characteristics.mistakes",
        "driver_characteristics.reaction_speed",
        "driver_characteristics.speed_in",
        "driver_characteristics.speed_out",
    ];
    for ((key, (name, _)), v) in KEYS.iter().zip(TRAITS.iter()).zip(d.traits.iter()) {
        debug_assert!(key.ends_with(name));
        set.push((key, trait_value(*v)));
    }
    set
}

pub fn driver_ops(d: &Driver) -> Vec<Value> {
    let label = d.label();
    let look = d.look_label();
    let mut ops = vec![
        add_row(DRIVERS, DRIVER_FROM, &driver_set(d)),
        add_row(
            LOOKS,
            LOOK_FROM,
            &[
                ("_metadata_", look.clone()),
                ("available", b(false)),
                ("badge", "Basic Badge".into()),
                ("car_livery", "livery_titlesponsor_livery23".into()),
                ("gloves", d.gloves.clone()),
                ("helmet", d.helmet.clone()),
                ("linked_driver_stat_id", label.clone()),
                ("podium_emote", d.emote.clone()),
                ("pose", d.pose.clone()),
                ("stat_id", d.preset_id.to_string()),
                ("suit", d.suit.clone()),
                ("victory_vo", String::new()),
            ],
        ),
    ];
    let mut stats = vec![
        ("_metadata_", label.clone()),
        ("driver", label.clone()),
        ("_StatisticsDataSet_instancecount_", "1".into()),
        ("StatisticsDataSet_0._metadata_", "default".into()),
        ("StatisticsDataSet_0.game_setup_modes", String::new()),
        ("StatisticsDataSet_0.is_invalid", b(false)),
        ("StatisticsDataSet_0.seasons", String::new()),
        ("StatisticsDataSet_0.weekend_formats", String::new()),
        (
            "StatisticsDataSet_0._Statistic_instancecount_",
            STATS.len().to_string(),
        ),
    ];
    let keys: Vec<[String; 3]> = (0..STATS.len())
        .map(|i| {
            ["_metadata_", "info", "evolution"].map(|p| format!("StatisticsDataSet_0.Statistic_{i}.{p}"))
        })
        .collect();
    let mut stat_pairs: Vec<(String, String)> = Vec::new();
    for (i, ((name, _), r)) in STATS.iter().zip(d.ratings.iter()).enumerate() {
        stat_pairs.push((keys[i][0].clone(), (*name).to_owned()));
        stat_pairs.push((keys[i][1].clone(), (*name).to_owned()));
        stat_pairs.push((keys[i][2].clone(), curve(*r)));
    }
    stats.extend(stat_pairs.iter().map(|(k, v)| (k.as_str(), v.clone())));
    ops.push(add_row(STATS_TABLE, STATS_FROM, &stats));
    ops.push(add_row(
        ENTRANTS,
        ENTRANT_FROM,
        &[
            ("_metadata_", label.clone()),
            ("badge_customisation", String::new()),
            ("car_number", d.number.to_string()),
            ("driver", label.clone()),
            ("driver_customisation", look.clone()),
            ("entrant_reference", String::new()),
            ("team", String::new()),
            ("team_position", "-1".into()),
            ("vehicle", String::new()),
            ("vehicle_customisation", String::new()),
        ],
    ));
    if !d.retired {
        for pool in POOLS {
            ops.push(add_row(
                pool,
                &format!("{pool}.{POOL_FROM_ROW}"),
                &[
                    ("_metadata_", label.clone()),
                    ("badge_customisation", String::new()),
                    ("car_number", "255".into()),
                    ("driver", String::new()),
                    ("driver_customisation", String::new()),
                    ("entrant_reference", label.clone()),
                    ("team", String::new()),
                    ("team_position", "-1".into()),
                    ("vehicle", String::new()),
                    ("vehicle_customisation", String::new()),
                ],
            ));
        }
    }
    for (part, value) in [
        ("first", d.first.trim().to_owned()),
        ("last", d.last.trim().to_owned()),
        ("full", full_name(d)),
    ] {
        ops.push(json!({ "op": "lang.set", "language": "*", "key": d.lang_key(part), "value": value }));
    }
    ops
}

fn has_kind(features: &Value, kind: &str) -> bool {
    ["stable", "experimental"].iter().any(|k| {
        features[*k]
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(kind)))
    })
}

pub fn build(roster: &Roster, game: Option<&GameData>, features: &Value) -> Value {
    let mut ops = Vec::new();
    let mut lines = Vec::new();
    let mut warnings = Vec::new();
    let Some(game) = game else {
        warnings.push("The game database couldn't be read, so no drivers were added.".to_owned());
        return json!({ "ops": [], "lines": lines, "warnings": warnings });
    };
    let (mut active, mut retired, mut skipped) = (0, 0, 0);
    for d in &roster.drivers {
        let issues = check::check(d, roster, Some(game));
        if !check::is_valid(&issues) {
            skipped += 1;
            let first = issues
                .iter()
                .find(|i| i.level == check::Level::Error)
                .map(|i| i.text.clone())
                .unwrap_or_default();
            warnings.push(format!("{} left out: {first}", d.display_name()));
            continue;
        }
        if d.retired {
            retired += 1;
        } else {
            active += 1;
        }
        ops.extend(driver_ops(d));
    }
    if !ops.is_empty() && !has_kind(features, "lang") {
        warnings.push("The in-game loader can't add text, so the new drivers' names won't show.".to_owned());
    }
    lines.push(format!(
        "{active} free agent{}, {retired} retired, {skipped} left out",
        if active == 1 { "" } else { "s" }
    ));
    json!({ "ops": ops, "fields": fields(), "lines": lines, "warnings": warnings })
}
