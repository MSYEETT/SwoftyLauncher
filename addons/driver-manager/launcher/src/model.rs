use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::game::{GameData, List};

pub const FIRST_DRIVER_ID: u32 = 208;
pub const LAST_DRIVER_ID: u32 = 253;
pub const FIRST_PRESET_ID: u32 = 366;
pub const MAX_DRIVERS: usize = (LAST_DRIVER_ID - FIRST_DRIVER_ID + 1) as usize;
pub const STATS: [(&str, &str); 5] = [
    ("experience", "Experience"),
    ("racecraft", "Racecraft"),
    ("awareness", "Awareness"),
    ("pace", "Pace"),
    ("focus", "Focus"),
];
pub const TRAITS: [(&str, &str); 6] = [
    ("aggression", "Aggression"),
    ("control", "Car control"),
    ("mistakes", "Mistake avoidance"),
    ("reaction_speed", "Reaction speed"),
    ("speed_in", "Corner entry speed"),
    ("speed_out", "Corner exit speed"),
];
pub const DEFAULT_TRAITS: [f64; 6] = [0.53, 0.53, 0.51, 0.53, 0.54, 1.0];
pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Birth {
    pub year: u32,
    pub month: u32,
    pub day: u32,
}

impl Default for Birth {
    fn default() -> Self {
        Birth {
            year: 2005,
            month: 3,
            day: 14,
        }
    }
}

impl Birth {
    pub fn days_in_month(&self) -> u32 {
        match self.month {
            2 if self.year.is_multiple_of(4)
                && (!self.year.is_multiple_of(100) || self.year.is_multiple_of(400)) =>
            {
                29
            }
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        }
    }

    pub fn month_name(&self) -> &'static str {
        MONTHS
            .get((self.month as usize).wrapping_sub(1))
            .copied()
            .unwrap_or("January")
    }
}

fn default_traits() -> [f64; 6] {
    DEFAULT_TRAITS
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rating {
    pub start: u32,
    pub peak: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Driver {
    pub stat_id: u32,
    pub preset_id: u32,
    pub first: String,
    pub last: String,
    pub reversed: bool,
    pub code: String,
    pub number: u32,
    pub nationality: String,
    pub head: String,
    pub helmet: String,
    pub suit: String,
    pub gloves: String,
    pub pose: String,
    pub emote: String,
    pub voice: String,
    pub ratings: [Rating; 5],
    pub seasons: u32,
    pub garage: u32,
    #[serde(default)]
    pub birth: Birth,
    #[serde(default = "default_traits")]
    pub traits: [f64; 6],
    #[serde(default)]
    pub retired: bool,
}

impl Driver {
    pub fn is_new(&self) -> bool {
        self.stat_id == 0
    }

    pub fn label(&self) -> String {
        format!("dm_{}", self.stat_id)
    }

    pub fn look_label(&self) -> String {
        format!("dm_{}_look", self.stat_id)
    }

    pub fn lang_key(&self, part: &str) -> String {
        format!("lng_dm_{}_{part}", self.stat_id)
    }

    pub fn full_name(&self) -> String {
        let (first, last) = (self.first.trim(), self.last.trim());
        if self.reversed {
            format!("{last} {first}")
        } else {
            format!("{first} {last}")
        }
        .trim()
        .to_owned()
    }

    pub fn display_name(&self) -> String {
        let name = self.full_name();
        if name.is_empty() {
            "Unnamed driver".into()
        } else {
            name
        }
    }

    pub fn blank(game: Option<&GameData>, number: u32) -> Driver {
        let pick = |list: List, want: &[&str]| -> String {
            let Some(g) = game else {
                return want.first().copied().unwrap_or_default().to_owned();
            };
            want.iter()
                .find(|w| g.available(list, w))
                .map(|w| (*w).to_owned())
                .or_else(|| g.choices(list).next().map(|c| c.value.clone()))
                .unwrap_or_default()
        };
        Driver {
            stat_id: 0,
            preset_id: 0,
            first: String::new(),
            last: String::new(),
            reversed: false,
            code: String::new(),
            number,
            nationality: pick(List::Nationality, &["britain"]),
            head: pick(List::Head, &["male_ls144"]),
            helmet: pick(List::Helmet, &["tier01_livery52"]),
            suit: pick(List::Suit, &["titlesponsor_livery15"]),
            gloves: pick(List::Gloves, &["titlesponsor_livery15"]),
            pose: pick(List::Pose, &["Pose03"]),
            emote: pick(List::Emote, &["Emote01"]),
            voice: pick(List::Voice, &["DefaultName"]),
            ratings: [Rating { start: 70, peak: 80 }; 5],
            seasons: 0,
            garage: 0,
            birth: Birth::default(),
            traits: DEFAULT_TRAITS,
            retired: false,
        }
    }

    pub fn duplicate(&self) -> Driver {
        Driver {
            stat_id: 0,
            preset_id: 0,
            code: String::new(),
            retired: false,
            ..self.clone()
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Roster {
    pub schema: u32,
    pub next_driver: u32,
    pub next_preset: u32,
    pub drivers: Vec<Driver>,
}

impl Roster {
    pub fn path(data_dir: &Path) -> PathBuf {
        data_dir.join("drivers.json")
    }

    pub fn load(data_dir: &Path) -> Result<Roster, String> {
        let path = Self::path(data_dir);
        match std::fs::read(&path) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|e| format!("{} is unreadable: {e}", path.display()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Roster {
                schema: SCHEMA,
                next_driver: FIRST_DRIVER_ID,
                next_preset: FIRST_PRESET_ID,
                drivers: Vec::new(),
            }),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn save(&self, data_dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(data_dir).map_err(|e| format!("{}: {e}", data_dir.display()))?;
        let path = Self::path(data_dir);
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn get(&self, stat_id: u32) -> Option<&Driver> {
        self.drivers.iter().find(|d| d.stat_id == stat_id)
    }

    pub fn get_mut(&mut self, stat_id: u32) -> Option<&mut Driver> {
        self.drivers.iter_mut().find(|d| d.stat_id == stat_id)
    }

    pub fn free_ids(&self, game: &GameData) -> usize {
        (self.next_driver.max(FIRST_DRIVER_ID)..=LAST_DRIVER_ID)
            .filter(|id| !game.driver_ids.contains(id) && self.get(*id).is_none())
            .count()
    }

    pub fn allocate(&mut self, game: &GameData) -> Result<(u32, u32), String> {
        let driver = (self.next_driver.max(FIRST_DRIVER_ID)..=LAST_DRIVER_ID)
            .find(|id| !game.driver_ids.contains(id) && self.get(*id).is_none())
            .ok_or_else(|| {
                format!("Every driver id from {FIRST_DRIVER_ID} to {LAST_DRIVER_ID} is used; no more drivers can be added.")
            })?;
        let preset = (self.next_preset.max(FIRST_PRESET_ID)..)
            .find(|id| !game.preset_ids.contains(id) && !self.drivers.iter().any(|d| d.preset_id == *id))
            .expect("unbounded range");
        self.next_driver = driver + 1;
        self.next_preset = preset + 1;
        Ok((driver, preset))
    }

    pub fn free_number(&self, game: Option<&GameData>) -> u32 {
        let taken = |n: u32| {
            self.drivers.iter().any(|d| d.number == n)
                || game.is_some_and(|g| g.stock.iter().any(|s| s.number == n))
        };
        (2..=99).find(|n| !taken(*n)).unwrap_or(99)
    }
}
