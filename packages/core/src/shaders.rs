//! Shader configs and effects in the store (DESIGN.md §1, §5, §6): personal configs in
//! `<data>/shaders/`, addon configs and effects from installed addons, the active config per profile,
//! and the files the game reads.

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub use swofty_shaders::{Active, ActiveInstance, Catalog};

use crate::profile::Profile;
use crate::store::Store;
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConfigRef {
    /// `personal:<slug>`: `<data>/shaders/<slug>.swfx`.
    Personal(String),
    /// `addon:<id>/<file stem>`.
    Addon { addon: String, file: String },
}

impl fmt::Display for ConfigRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Personal(slug) => write!(f, "personal:{slug}"),
            Self::Addon { addon, file } => write!(f, "addon:{addon}/{file}"),
        }
    }
}

impl FromStr for ConfigRef {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        if let Some(slug) = s.strip_prefix("personal:") {
            return Ok(Self::Personal(slug.into()));
        }
        if let Some((addon, file)) = s.strip_prefix("addon:").and_then(|r| r.split_once('/')) {
            return Ok(Self::Addon {
                addon: addon.into(),
                file: file.into(),
            });
        }
        Err(Error::Other(format!("{s:?} is not a shader config")))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigEntry {
    pub r#ref: ConfigRef,
    pub name: String,
    /// "Personal", or the addon's name.
    pub source: String,
    pub editable: bool,
    pub path: PathBuf,
}

/// The library's effects and every installed addon's, whether the addon is on or off.
pub fn catalog(store: &Store) -> Catalog {
    let _ = store;
    Catalog::default()
}

/// Personal configs first (by name), then addon configs in load order.
pub fn list(store: &Store) -> Vec<ConfigEntry> {
    let _ = store;
    Vec::new()
}

pub fn read(store: &Store, r: &ConfigRef) -> Result<String> {
    let _ = (store, r);
    Err(Error::Other("not written yet".into()))
}

/// Personal configs only.
pub fn save(store: &Store, r: &ConfigRef, text: &str) -> Result<()> {
    let _ = (store, r, text);
    Err(Error::Other("not written yet".into()))
}

/// `None` starts from the starter template.
pub fn create(store: &Store, name: &str, text: Option<&str>) -> Result<ConfigRef> {
    let _ = (store, name, text);
    Err(Error::Other("not written yet".into()))
}

pub fn copy_to_personal(store: &Store, r: &ConfigRef) -> Result<ConfigRef> {
    let _ = (store, r);
    Err(Error::Other("not written yet".into()))
}

pub fn rename(store: &Store, r: &ConfigRef, name: &str) -> Result<ConfigRef> {
    let _ = (store, r, name);
    Err(Error::Other("not written yet".into()))
}

pub fn delete(store: &Store, r: &ConfigRef) -> Result<()> {
    let _ = (store, r);
    Err(Error::Other("not written yet".into()))
}

pub fn import(store: &Store, path: &Path) -> Result<ConfigRef> {
    let _ = (store, path);
    Err(Error::Other("not written yet".into()))
}

pub fn export(store: &Store, r: &ConfigRef, path: &Path) -> Result<()> {
    let _ = (store, r, path);
    Err(Error::Other("not written yet".into()))
}

pub fn active(store: &Store, profile: &Profile) -> Option<ConfigRef> {
    let _ = store;
    profile.shader.as_deref().and_then(|s| s.parse().ok())
}

pub fn set_active(store: &Store, profile: &mut Profile, r: Option<ConfigRef>) -> Result<()> {
    profile.shader = r.map(|r| r.to_string());
    store.save_profile(profile)
}

/// The config an enabled addon forces, with the addon's name.
pub fn forced(store: &Store, profile: &Profile) -> Option<(ConfigRef, String)> {
    let _ = (store, profile);
    None
}

/// The in-game on/off key, `"B"` by default; `"None"` is no key.
pub fn toggle_key(store: &Store) -> String {
    let _ = store;
    "B".into()
}

pub fn set_toggle_key(store: &Store, key: &str) -> Result<()> {
    let _ = (store, key);
    Err(Error::Other("not written yet".into()))
}

/// What the build and `push_live` write as `active.json`.
pub fn active_file(store: &Store, profile: &Profile) -> Active {
    Active {
        version: 1,
        serial: 0,
        name: String::new(),
        source: String::new(),
        forced_by: String::new(),
        toggle_key: toggle_key(store),
        instances: active(store, profile).map(|_| Vec::new()).unwrap_or_default(),
    }
}

/// Writes `<game>/swofty/shaders/active.json` atomically, for the running game to pick up.
pub fn push_live(game_dir: &Path, active: &Active) -> Result<()> {
    let _ = (game_dir, active);
    Err(Error::Other("not written yet".into()))
}
