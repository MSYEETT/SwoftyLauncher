//! Driver Manager's drivers as the DB Editor sees them. They aren't in the stock
//! database: Driver Manager adds their rows when a profile with it on builds,
//! after every other addon, so the DB Editor reads them from its data file and
//! checks them the way its build does, all but whether a head or kit is installed.

use std::collections::HashSet;
use std::path::Path;

use serde::Deserialize;

use super::{Catalog, LinkSpec, modules, rows};
use crate::store::Store;

pub const ID: &str = "swofty.driver-manager";
/// The tables Driver Manager adds a row to for every driver, labelled with the driver's label.
pub const TABLES: [&str; 2] = [
    "entrants.drivers.driver",
    "entrants.entrant_lists.entrants.entrant",
];
const FILE: &str = "drivers.json";
const IDS: std::ops::RangeInclusive<u32> = 208..=253;
const FIRST_LOOK: u32 = 366;
const NAME_MAX: usize = 24;
const NATIONALITIES: &str = "entrants.nationality.nationality";
const KIT: [&str; 6] = [
    "entrants.animation.driver_heads.driver_head",
    "customisation.custom_avatar.helmets.Helmet",
    "customisation.custom_avatar.suits.Suit",
    "customisation.custom_avatar.gloves.Gloves",
    "customisation.custom_avatar.poses.Pose",
    "customisation.custom_avatar.podium_emotes.PodiumEmote",
];
const VOICES: &str = "entrants.audio_names.audio_name";
const LOOKS: &str = "entrants.driver_customisations.folder";

/// A driver Driver Manager adds when a profile with it on builds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Driver {
    /// `dm_<driver id>`, the label of the driver's row and entrant row.
    pub label: String,
    pub name: String,
    pub code: String,
    pub number: u32,
    /// Out of the driver pools of new careers; the driver and entrant rows stay.
    pub retired: bool,
}

impl Driver {
    /// The name choices and search show: `Jacob Swofty (Driver Manager)`.
    pub fn title(&self) -> String {
        match self.retired {
            true => format!("{} (Driver Manager, retired)", self.name),
            false => format!("{} (Driver Manager)", self.name),
        }
    }
}

#[derive(Deserialize)]
struct Roster {
    drivers: Vec<Saved>,
}

#[derive(Deserialize)]
struct Rating {
    start: u32,
    peak: u32,
}

#[derive(Deserialize)]
struct Birth {
    year: u32,
    month: u32,
    day: u32,
}

/// A driver in Driver Manager's `drivers.json`, with the fields its build reads.
#[derive(Deserialize)]
struct Saved {
    stat_id: u32,
    preset_id: u32,
    first: String,
    last: String,
    reversed: bool,
    code: String,
    number: u32,
    nationality: String,
    head: String,
    helmet: String,
    suit: String,
    gloves: String,
    pose: String,
    emote: String,
    voice: String,
    ratings: [Rating; 5],
    seasons: u32,
    garage: u32,
    #[serde(default)]
    birth: Option<Birth>,
    #[serde(default)]
    traits: [f64; 6],
    #[serde(default)]
    retired: bool,
}

impl Saved {
    fn label(&self) -> String {
        format!("dm_{}", self.stat_id)
    }

    fn name(&self) -> String {
        let (first, last) = (self.first.trim(), self.last.trim());
        match self.reversed {
            true => format!("{last} {first}"),
            false => format!("{first} {last}"),
        }
        .trim()
        .to_owned()
    }

    /// Nationality, then [`KIT`] in order, then commentary name.
    fn picks(&self) -> [&str; 8] {
        [
            &self.nationality,
            &self.head,
            &self.helmet,
            &self.suit,
            &self.gloves,
            &self.pose,
            &self.emote,
            &self.voice,
        ]
    }
}

impl Birth {
    fn days(&self) -> u32 {
        let leap =
            self.year.is_multiple_of(4) && (!self.year.is_multiple_of(100) || self.year.is_multiple_of(400));
        match self.month {
            2 if leap => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        }
    }
}

/// What Driver Manager's checks read from the stock database.
struct Stock {
    /// One per [`Saved::picks`].
    picks: Vec<HashSet<String>>,
    drivers: HashSet<u32>,
    looks: HashSet<u32>,
}

fn labelled<'a>(cat: &'a Catalog, table: &str) -> impl Iterator<Item = (u64, &'a str)> {
    rows::rows(cat, table)
        .into_iter()
        .filter_map(move |r| Some((r, rows::value_in(cat, r, "_metadata_").filter(|l| !l.is_empty())?)))
}

impl Stock {
    fn of(cat: &Catalog) -> Stock {
        let set = |table: &str, keep: &dyn Fn(u64, &str) -> bool| -> HashSet<String> {
            labelled(cat, table)
                .filter(|&(r, l)| keep(r, l))
                .map(|(_, l)| l.to_owned())
                .collect()
        };
        let mut picks = vec![set(NATIONALITIES, &|_, l| l != "default")];
        picks.extend(KIT.iter().map(|t| set(t, &|_, _| true)));
        picks.push(set(VOICES, &|r, l| {
            l == "DefaultName"
                || (rows::value_in(cat, r, "selection_type") == Some("custom_standard")
                    && !l.to_lowercase().contains("disabled"))
        }));
        let id = |r: u64| {
            let v = rows::value_in(cat, r, "stat_id")?.trim().parse::<f64>().ok()?;
            Some(v as u32)
        };
        Stock {
            picks,
            drivers: rows::rows(cat, TABLES[0]).into_iter().filter_map(id).collect(),
            looks: rows::rows(cat, LOOKS)
                .into_iter()
                .flat_map(|f| cat.rows_of(f, "preloaded_driver_customisation_properties"))
                .filter_map(id)
                .collect(),
        }
    }
}

/// Whether Driver Manager's checks pass `d`: the errors that leave a driver out of its build.
fn added(d: &Saved, all: &[Saved], stock: &Stock) -> bool {
    let named = |v: &str| {
        let v = v.trim();
        !v.is_empty() && v.chars().count() <= NAME_MAX && !v.chars().any(char::is_control)
    };
    let new = d.stat_id == 0;
    let others: Vec<&Saved> = all.iter().filter(|o| o.stat_id != d.stat_id || new).collect();
    let code = d.code.len() == 3
        && d.code.bytes().all(|b| b.is_ascii_uppercase())
        && !others
            .iter()
            .any(|o| o.code == d.code && (new || o.stat_id < d.stat_id));
    let ids = match new {
        true => all.len() < IDS.clone().count(),
        false => {
            IDS.contains(&d.stat_id)
                && d.preset_id >= FIRST_LOOK
                && !stock.drivers.contains(&d.stat_id)
                && !stock.looks.contains(&d.preset_id)
                && !others
                    .iter()
                    .any(|o| o.stat_id == d.stat_id || o.preset_id == d.preset_id)
        }
    };
    let birth = d.birth.as_ref().is_none_or(|b| {
        (1950..=2012).contains(&b.year) && (1..=12).contains(&b.month) && (1..=b.days()).contains(&b.day)
    });
    named(&d.first)
        && named(&d.last)
        && code
        && (1..=99).contains(&d.number)
        && d.picks()
            .iter()
            .zip(&stock.picks)
            .all(|(v, have)| have.contains(*v))
        && d.ratings.iter().all(|r| r.start <= r.peak && r.peak <= 100)
        && birth
        && d.traits.iter().all(|v| (0.0..=1.0).contains(v))
        && d.seasons <= 30
        && d.garage <= 1
        && ids
}

fn roster(dir: &Path) -> Vec<Saved> {
    std::fs::read(dir.join(FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<Roster>(&b).ok())
        .map_or_else(Vec::new, |r| r.drivers)
}

/// The drivers Driver Manager adds when a profile with it on builds, retired ones
/// last: none when it isn't installed or its data file doesn't read, and none of the
/// ones its checks leave out. Its check that a head or kit is installed in the game
/// folder isn't made; a build that leaves such a driver out says so.
pub fn drivers(store: &Store, cat: &Catalog) -> Vec<Driver> {
    match store
        .addons_dir()
        .join(ID)
        .join(crate::manifest::MANIFEST_FILE)
        .is_file()
    {
        true => drivers_in(&store.addon_data_dir(ID), cat),
        false => Vec::new(),
    }
}

/// [`drivers`] from Driver Manager's data folder `dir`.
pub fn drivers_in(dir: &Path, cat: &Catalog) -> Vec<Driver> {
    let all = roster(dir);
    if all.is_empty() {
        return Vec::new();
    }
    let stock = Stock::of(cat);
    let mut out: Vec<Driver> = all
        .iter()
        .filter(|d| added(d, &all, &stock))
        .map(|d| Driver {
            label: d.label(),
            name: d.name(),
            code: d.code.clone(),
            number: d.number,
            retired: d.retired,
        })
        .collect();
    out.sort_by_key(|d| d.retired);
    out
}

/// The DB Editor's link to Driver Manager's page.
pub fn link() -> LinkSpec {
    modules()
        .iter()
        .flat_map(|m| &m.links)
        .find(|l| l.addon.as_deref() == Some(ID))
        .cloned()
        .expect("modules.json links Driver Manager")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use swofty_formats::gvdb::{Gvdb, Record};

    /// A stock database with one driver, entrant and grid seat, and the choices the fixture drivers pick.
    pub(crate) fn database() -> Gvdb {
        let mut records = Vec::new();
        let mut table = |rows: &str, labels: &[&str]| {
            let (container, word) = rows.rsplit_once('.').unwrap();
            records.push(Record::new(
                &format!("{container}._{word}_instancecount_"),
                0,
                labels.len().to_string(),
            ));
            for (i, l) in labels.iter().enumerate() {
                records.push(Record::new(&format!("{rows}_{i}._metadata_"), 2, *l));
            }
        };
        table(NATIONALITIES, &["default", "australia"]);
        table(KIT[0], &["male_ls144"]);
        table(KIT[1], &["tier02_livery01"]);
        table(KIT[2], &["titlesponsor_livery10"]);
        table(KIT[3], &["titlesponsor_livery10"]);
        table(KIT[4], &["Pose03"]);
        table(KIT[5], &["Emote01"]);
        table(VOICES, &["DefaultName"]);
        table(TABLES[0], &["max_verstappen"]);
        table(TABLES[1], &["max_verstappen"]);
        table("entrants.entrant_lists.entrant_list", &["f1_2026"]);
        table(
            "entrants.entrant_lists.entrant_list_0.entrant",
            &["f1_2026_red_bull_1"],
        );
        table(LOOKS, &["official_driver_customisations"]);
        table(
            "entrants.driver_customisations.folder_0.preloaded_driver_customisation_properties",
            &["max_look"],
        );
        records.extend([
            Record::new("entrants.drivers.driver_0.stat_id", 0, "1"),
            Record::new("entrants.drivers.driver_0.full_name", 2, "lng_max"),
            Record::new(
                "entrants.entrant_lists.entrants.entrant_0.driver",
                2,
                "max_verstappen",
            ),
            Record::new(
                "entrants.entrant_lists.entrant_list_0.entrant_0.entrant_reference",
                2,
                "max_verstappen",
            ),
            Record::new(
                "entrants.entrant_lists.entrant_list_0.entrant_0.team",
                2,
                "red_bull_26",
            ),
            Record::new(
                "entrants.driver_customisations.folder_0.preloaded_driver_customisation_properties_0.stat_id",
                0,
                "380",
            ),
        ]);
        Gvdb { records }
    }

    fn driver(id: u32, first: &str, last: &str, code: &str) -> serde_json::Value {
        serde_json::json!({
            "stat_id": id, "preset_id": id + 158, "first": first, "last": last, "reversed": false,
            "code": code, "number": id - 187, "nationality": "australia", "head": "male_ls144",
            "helmet": "tier02_livery01", "suit": "titlesponsor_livery10", "gloves": "titlesponsor_livery10",
            "pose": "Pose03", "emote": "Emote01", "voice": "DefaultName",
            "ratings": [{"start": 72, "peak": 90}, {"start": 84, "peak": 95}, {"start": 82, "peak": 94},
                        {"start": 87, "peak": 97}, {"start": 83, "peak": 95}],
            "seasons": 0, "garage": 0, "retired": false
        })
    }

    /// Writes a `drivers.json` to `dir`: Jacob Swofty (dm_208), the retired Ret Ired
    /// (dm_209), and three Driver Manager leaves out: a bad code (dm_210), a head the
    /// game lacks (dm_211) and a look id the game uses (dm_212).
    pub(crate) fn fixture(dir: &Path) {
        let mut retired = driver(209, "Ret", "Ired", "RET");
        retired["retired"] = true.into();
        let mut head = driver(211, "No", "Head", "NOH");
        head["head"] = "male_ls999".into();
        let mut look = driver(212, "Used", "Look", "USL");
        look["preset_id"] = 380.into();
        let drivers = [
            driver(208, "Jacob", "Swofty", "SWO"),
            retired,
            driver(210, "Bad", "Code", "B4D"),
            head,
            look,
        ];
        std::fs::create_dir_all(dir).unwrap();
        let roster =
            serde_json::json!({ "schema": 1, "next_driver": 213, "next_preset": 371, "drivers": drivers });
        std::fs::write(dir.join(FILE), roster.to_string()).unwrap();
    }

    pub(crate) fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-dm-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        dir
    }

    #[test]
    fn reads_the_drivers_driver_manager_adds_and_leaves_out_the_rest() {
        let cat = Catalog::from_gvdb(database(), String::new());
        let dir = temp("read");
        fixture(&dir);
        let got = drivers_in(&dir, &cat);
        let labels: Vec<&str> = got.iter().map(|d| d.label.as_str()).collect();
        assert_eq!(labels, ["dm_208", "dm_209"]);
        assert_eq!(got[0].title(), "Jacob Swofty (Driver Manager)");
        assert_eq!(got[1].title(), "Ret Ired (Driver Manager, retired)");
        assert_eq!((got[0].code.as_str(), got[0].number), ("SWO", 21));

        let store = Store::new(dir.join("data"));
        fixture(&store.addon_data_dir(ID));
        assert!(drivers(&store, &cat).is_empty(), "Driver Manager isn't installed");
        std::fs::create_dir_all(store.addons_dir().join(ID)).unwrap();
        std::fs::write(store.addons_dir().join(ID).join("swofty.json"), "{}").unwrap();
        assert_eq!(drivers(&store, &cat), got);

        std::fs::write(dir.join(FILE), "{\"drivers\": [{\"stat_id\": 208}]}").unwrap();
        assert!(
            drivers_in(&dir, &cat).is_empty(),
            "a file Driver Manager can't read adds nothing"
        );
        assert_eq!(link().addon.as_deref(), Some(ID));
        std::fs::remove_dir_all(&dir).ok();
    }
}
