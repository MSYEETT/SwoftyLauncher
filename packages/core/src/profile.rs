use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::builtin::CORE_ID;
use crate::manifest::{AddonManifest, AddonOption, Binds};

pub const VANILLA: &str = "vanilla";
pub const SCHEMA_VERSION: u32 = 2;

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
    #[serde(default, alias = "mods")]
    pub addons: Vec<ProfileAddon>,
    #[serde(default)]
    pub maps: Vec<String>,
    #[serde(default)]
    pub options: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub applied: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub state: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProfileAddon {
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
            schema_version: SCHEMA_VERSION,
            name: name.into(),
            addons: Vec::new(),
            maps: Vec::new(),
            options: BTreeMap::new(),
            applied: BTreeMap::new(),
            state: BTreeMap::new(),
        };
        p.normalise();
        p
    }

    pub fn vanilla() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            name: VANILLA.into(),
            addons: Vec::new(),
            maps: Vec::new(),
            options: BTreeMap::new(),
            applied: BTreeMap::new(),
            state: BTreeMap::new(),
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
        self.schema_version = SCHEMA_VERSION;
        self.addons.retain(|m| m.id != CORE_ID);
        self.addons.insert(
            0,
            ProfileAddon {
                id: CORE_ID.into(),
                enabled: true,
            },
        );
    }

    pub fn enabled(&self) -> impl Iterator<Item = &str> {
        self.addons
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
        match self.addons.iter_mut().find(|m| m.id == id) {
            Some(m) => m.enabled = on,
            None => self.addons.push(ProfileAddon {
                id: id.into(),
                enabled: on,
            }),
        }
        Ok(())
    }

    pub fn option(&self, m: &AddonManifest, o: &AddonOption) -> serde_json::Value {
        stored(&self.options, m, o).unwrap_or_else(|| o.default_value())
    }

    pub fn set_option(
        &mut self,
        m: &AddonManifest,
        id: &str,
        value: serde_json::Value,
    ) -> crate::Result<bool> {
        if self.is_vanilla() {
            return Err(crate::Error::Other(format!(
                "{VANILLA} is built in and cannot be changed"
            )));
        }
        let o = m
            .options
            .iter()
            .find(|o| o.id == id)
            .ok_or_else(|| crate::Error::Other(format!("{} has no option {id}", m.id)))?;
        let v = o
            .normalise(&value)
            .ok_or_else(|| crate::Error::Other(format!("{value} is not a valid value for {}.{id}", m.id)))?;
        let before = self.option(m, o);
        let values = self.options.entry(m.id.clone()).or_default();
        if v == o.default_value() {
            values.remove(id);
        } else {
            values.insert(id.to_owned(), v.clone());
        }
        if values.is_empty() {
            self.options.remove(&m.id);
        }
        Ok(before != v)
    }

    pub fn reset_options(&mut self, m: &AddonManifest) -> crate::Result<bool> {
        if self.is_vanilla() {
            return Err(crate::Error::Other(format!(
                "{VANILLA} is built in and cannot be changed"
            )));
        }
        Ok(self.options.remove(&m.id).is_some())
    }

    pub fn rebuild_pending(&self, installed: &BTreeMap<String, AddonManifest>) -> Vec<String> {
        let mut out = Vec::new();
        for m in self.enabled().filter_map(|id| installed.get(id)) {
            for o in m.options.iter().filter(|o| o.binds == Binds::Db) {
                let baked = stored(&self.applied, m, o).unwrap_or_else(|| o.default_value());
                if self.option(m, o) != baked {
                    out.push(format!("{}.{}", m.id, o.id));
                }
            }
        }
        out
    }

    pub fn mark_built(&mut self, installed: &BTreeMap<String, AddonManifest>) {
        let mut applied = BTreeMap::new();
        for m in self.enabled().filter_map(|id| installed.get(id)) {
            let values: BTreeMap<String, serde_json::Value> = m
                .options
                .iter()
                .filter(|o| o.binds == Binds::Db)
                .map(|o| (o.id.clone(), self.option(m, o)))
                .filter(|(id, v)| m.options.iter().any(|o| &o.id == id && *v != o.default_value()))
                .collect();
            if !values.is_empty() {
                applied.insert(m.id.clone(), values);
            }
        }
        self.applied = applied;
    }

    pub fn set_state(&mut self, addon: &str, value: serde_json::Value) -> crate::Result<()> {
        if self.is_vanilla() {
            return Err(crate::Error::Other(format!(
                "{VANILLA} is built in and cannot be changed"
            )));
        }
        self.state.insert(addon.into(), value);
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

fn stored(
    map: &BTreeMap<String, BTreeMap<String, serde_json::Value>>,
    m: &AddonManifest,
    o: &AddonOption,
) -> Option<serde_json::Value> {
    map.get(&m.id)?.get(&o.id).and_then(|v| o.normalise(v))
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
