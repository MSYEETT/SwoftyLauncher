use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::builtin::CORE_ID;

pub const VANILLA: &str = "vanilla";

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 40
        && name != VANILLA
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema_version: u32,
    pub name: String,
    #[serde(default)]
    pub mods: Vec<ProfileMod>,
    #[serde(default)]
    pub maps: Vec<String>,
    #[serde(default)]
    pub options: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProfileMod {
    pub id: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
}

fn enabled() -> bool {
    true
}

impl Profile {
    pub fn new(name: &str) -> Self {
        let mut p = Self {
            schema_version: 1,
            name: name.into(),
            mods: Vec::new(),
            maps: Vec::new(),
            options: BTreeMap::new(),
        };
        p.normalise();
        p
    }

    pub fn vanilla() -> Self {
        Self {
            schema_version: 1,
            name: VANILLA.into(),
            mods: Vec::new(),
            maps: Vec::new(),
            options: BTreeMap::new(),
        }
    }

    pub fn is_vanilla(&self) -> bool {
        self.name == VANILLA
    }

    pub fn save_namespace(&self) -> String {
        format!("swofty-{}", self.name)
    }

    pub fn normalise(&mut self) {
        if self.is_vanilla() {
            *self = Self::vanilla();
            return;
        }
        self.mods.retain(|m| m.id != CORE_ID);
        self.mods.insert(
            0,
            ProfileMod {
                id: CORE_ID.into(),
                enabled: true,
            },
        );
    }

    pub fn enabled(&self) -> impl Iterator<Item = &str> {
        self.mods
            .iter()
            .filter(|m| m.enabled || m.id == CORE_ID)
            .map(|m| m.id.as_str())
    }

    pub fn set_enabled(&mut self, id: &str, on: bool) -> crate::Result<()> {
        if self.is_vanilla() {
            return Err(crate::Error::Other(format!(
                "{VANILLA} is built in and cannot be changed"
            )));
        }
        if id == CORE_ID && !on {
            return Err(crate::Error::Other(format!(
                "{CORE_ID} is built in and cannot be disabled"
            )));
        }
        match self.mods.iter_mut().find(|m| m.id == id) {
            Some(m) => m.enabled = on,
            None => self.mods.push(ProfileMod {
                id: id.into(),
                enabled: on,
            }),
        }
        Ok(())
    }

    pub fn set_map(&mut self, code: &str, on: bool) -> crate::Result<()> {
        if self.is_vanilla() {
            return Err(crate::Error::Other(format!(
                "{VANILLA} is built in and cannot be changed"
            )));
        }
        self.maps.retain(|m| m != code);
        if on {
            self.maps.push(code.into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_is_always_first_and_enabled() {
        let mut p: Profile = serde_json::from_str(
            r#"{"schema_version":1,"name":"x","mods":[{"id":"a.b"},{"id":"swofty.core","enabled":false}]}"#,
        )
        .unwrap();
        p.normalise();
        assert_eq!(p.enabled().collect::<Vec<_>>(), ["swofty.core", "a.b"]);
        assert!(p.set_enabled(CORE_ID, false).is_err());
        p.set_enabled("a.b", false).unwrap();
        assert_eq!(p.enabled().collect::<Vec<_>>(), ["swofty.core"]);
        p.set_map("okayama", true).unwrap();
        p.set_map("okayama", true).unwrap();
        assert_eq!(p.maps, ["okayama"]);
        p.set_map("okayama", false).unwrap();
        assert!(p.maps.is_empty());
    }

    #[test]
    fn profile_names() {
        assert!(valid_name("grid-24_b"));
        assert!(!valid_name(VANILLA));
        assert!(!valid_name("a b"));
        assert!(!valid_name("../x"));
    }
}
