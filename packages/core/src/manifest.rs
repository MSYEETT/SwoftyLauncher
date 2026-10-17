use std::collections::BTreeMap;
use std::path::Path;

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use swofty_sdk::Capability;

use crate::{Error, Result};

pub const MANIFEST_FILE: &str = "swofty.json";
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: Version,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kind: Vec<Kind>,
    pub compat: Compat,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<Requirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<Conflict>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub load_after: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<ModOption>,
    #[serde(default)]
    pub operations: Operations,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Database,
    Erp,
    Ui,
    Files,
    Native,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Compat {
    pub game: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub game_builds: Vec<VersionReq>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launcher: Option<VersionReq>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seasons: Vec<String>,
}

impl Compat {
    pub fn accepts_build(&self, build_id: u64) -> bool {
        let v = Version::new(build_id, 0, 0);
        self.game_builds.is_empty() || self.game_builds.iter().any(|r| r.matches(&v))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub id: String,
    #[serde(default = "any_version")]
    pub version: VersionReq,
}

fn any_version() -> VersionReq {
    VersionReq::STAR
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Conflict {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub touches: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OptionType {
    Toggle,
    Slider,
    Choice,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Binds {
    Db,
    Native,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "ChoiceRepr", into = "ChoiceRepr")]
pub struct Choice {
    pub value: String,
    pub label: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum ChoiceRepr {
    Plain(String),
    Labelled { value: String, label: String },
}

impl From<ChoiceRepr> for Choice {
    fn from(r: ChoiceRepr) -> Self {
        match r {
            ChoiceRepr::Plain(value) => Self { value, label: None },
            ChoiceRepr::Labelled { value, label } => Self {
                value,
                label: Some(label),
            },
        }
    }
}

impl From<Choice> for ChoiceRepr {
    fn from(c: Choice) -> Self {
        match c.label {
            None => Self::Plain(c.value),
            Some(label) => Self::Labelled {
                value: c.value,
                label,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModOption {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: OptionType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub default: serde_json::Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<Choice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    pub binds: Binds,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Operations {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub database: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub erp: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ui: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileOp>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub native: Vec<NativePlugin>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FileOp {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NativePlugin {
    pub dll: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl ModManifest {
    pub fn load(dir: &Path) -> Result<Self> {
        let m: Self = crate::read_json(&dir.join(MANIFEST_FILE))?;
        m.validate()?;
        Ok(m)
    }

    pub fn parse(json: &str) -> Result<Self> {
        let m: Self = serde_json::from_str(json).map_err(|e| Error::Manifest(e.to_string()))?;
        m.validate()?;
        Ok(m)
    }

    pub fn is_builtin(&self) -> bool {
        self.id == crate::builtin::CORE_ID
    }

    pub fn validate(&self) -> Result<()> {
        let fail = |m: String| Err(Error::Manifest(format!("{}: {m}", self.id)));
        if self.schema_version != SCHEMA_VERSION {
            return fail(format!("schema_version {} is not supported", self.schema_version));
        }
        if !valid_id(&self.id) {
            return fail("id must be lowercase dotted [a-z0-9._-] with at least two parts".into());
        }
        if self.compat.game != crate::GAME_ID {
            return fail(format!(
                "compat.game {} is not {}",
                self.compat.game,
                crate::GAME_ID
            ));
        }
        let mut seen = BTreeMap::new();
        for o in &self.options {
            if seen.insert(o.id.as_str(), ()).is_some() {
                return fail(format!("option {} is declared twice", o.id));
            }
            if let Err(e) = o.validate() {
                return fail(format!("option {}: {e}", o.id));
            }
        }
        for p in &self.operations.native {
            if let Some(c) = p.capabilities.iter().find(|c| Capability::parse(c).is_none()) {
                return fail(format!("{}: unknown capability {c}", p.dll));
            }
        }
        let paths = self
            .operations
            .database
            .iter()
            .chain(&self.operations.erp)
            .chain(&self.operations.ui);
        let paths = paths
            .chain(self.operations.files.iter().map(|f| &f.source))
            .chain(self.operations.native.iter().map(|n| &n.dll));
        if let Some(p) = paths.into_iter().find(|p| !relative_inside(p)) {
            return fail(format!("{p} must be a relative path inside the mod"));
        }
        Ok(())
    }
}

pub fn valid_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    parts.len() >= 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        })
}

pub fn relative_inside(path: &str) -> bool {
    let p = Path::new(path);
    !path.is_empty()
        && !path.contains('\\')
        && p.components()
            .all(|c| matches!(c, std::path::Component::Normal(_) | std::path::Component::CurDir))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRID24: &str = r#"{
      "schema_version": 1,
      "id": "swofty.f1-2026-grid24",
      "name": "F1 2026 - 24-Car Grid",
      "version": "1.0.0",
      "author": "swofty",
      "kind": ["database"],
      "compat": { "game": "f1_25", "game_builds": [">=25251167"], "launcher": ">=0.1.0", "seasons": ["f1_2026"] },
      "requires": [{ "id": "swofty.core", "version": ">=0.1.0" }],
      "conflicts": [{ "id": "*", "touches": "db:entrants.entrant_lists.entrant_list_34.*" }],
      "load_after": ["swofty.track.*"],
      "options": [{ "id": "team_livery", "type": "choice", "default": "titlesponsor",
                    "values": ["titlesponsor", { "value": "tier01", "label": "Tier 1" }, "replica"], "binds": "db" }],
      "operations": { "database": ["db/grid24.json"] }
    }"#;

    #[test]
    fn parses_design_example() {
        let m = ModManifest::parse(GRID24).unwrap();
        assert_eq!(m.version, Version::new(1, 0, 0));
        assert!(m.compat.accepts_build(25_251_167));
        assert!(!m.compat.accepts_build(25_000_000));
        assert_eq!(m.options[0].binds, Binds::Db);
    }

    #[test]
    fn rejects_bad_ids_and_paths() {
        assert!(!valid_id("Grid24"));
        assert!(!valid_id("single"));
        assert!(valid_id("author.mod-name"));
        assert!(!relative_inside("../escape.json"));
        assert!(!relative_inside("/abs.json"));
        assert!(relative_inside("db/ops.json"));
        let bad = GRID24.replace("db/grid24.json", "../grid24.json");
        assert!(ModManifest::parse(&bad).is_err());
    }

    #[test]
    fn rejects_unknown_fields() {
        let bad = GRID24.replace("\"kind\"", "\"kinds\"");
        assert!(ModManifest::parse(&bad).is_err());
    }
}
