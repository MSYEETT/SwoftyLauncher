pub struct ModRow {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub kinds: &'static str,
    pub builtin: bool,
}

pub const MODS: &[ModRow] = &[
    ModRow {
        id: "swofty.core",
        name: "Swofty Mod",
        version: "0.1.0",
        kinds: "ui · native",
        builtin: true,
    },
    ModRow {
        id: "swofty.f1-2026-grid24",
        name: "F1 2026 · 24-Car Grid",
        version: "1.0.0",
        kinds: "database",
        builtin: false,
    },
];

pub struct ProfileRow {
    pub name: &'static str,
    pub mods: usize,
    pub maps: usize,
}

pub const PROFILES: &[ProfileRow] = &[
    ProfileRow {
        name: "default",
        mods: 1,
        maps: 0,
    },
    ProfileRow {
        name: "grid24",
        mods: 2,
        maps: 0,
    },
];

#[cfg(windows)]
const DATA_DIR: &str = "%LOCALAPPDATA%\\Swofty";
#[cfg(not(windows))]
const DATA_DIR: &str = "~/.local/share/swofty";

pub const SETTINGS: &[(&str, &str)] = &[
    ("Game directory", "Not detected"),
    ("Steam library", "Not detected"),
    ("Proton prefix", "Not detected"),
    ("Data directory", DATA_DIR),
];
