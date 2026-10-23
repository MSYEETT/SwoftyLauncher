use std::collections::BTreeMap;
use std::path::Path;

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

use crate::launcher::CommandSpec;
use crate::view::SectionSpec;
use crate::{Error, Result};

pub const MANIFEST_FILE: &str = "swofty.json";
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AddonManifest {
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kind: Vec<Kind>,
    /// Experimental engines of the core the addon uses; the build turns them on while it is on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub experimental_features: Vec<ExperimentalFeature>,
    pub compat: Compat,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<Requirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<Conflict>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub load_after: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<AddonOption>,
    #[serde(default)]
    pub operations: Operations,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launcher: Option<Launcher>,
    #[serde(default, skip_serializing_if = "Saves::is_unset")]
    pub saves: Saves,
    /// What the addon is for, for grouping and filtering; `kind` says what it touches.
    #[serde(
        default,
        deserialize_with = "Category::read",
        skip_serializing_if = "Option::is_none"
    )]
    pub category: Option<Category>,
    /// The tool that generated the addon and the file it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
    /// A colour look applied to the game's picture, before the HUD draws.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shader: Option<Shader>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    Seasons,
    Tracks,
    #[serde(alias = "liveries")]
    Cars,
    Helmets,
    Shaders,
}

impl Category {
    pub const ALL: [Category; 5] = [
        Self::Seasons,
        Self::Tracks,
        Self::Cars,
        Self::Helmets,
        Self::Shaders,
    ];

    /// Categories from earlier launchers that read as none.
    pub const RETIRED: [&str; 7] = [
        "drivers-teams",
        "database",
        "gameplay",
        "ui",
        "audio",
        "tools",
        "other",
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Seasons => "Seasons",
            Self::Tracks => "Tracks",
            Self::Cars => "Cars",
            Self::Helmets => "Helmets",
            Self::Shaders => "Shaders",
        }
    }

    /// Reads an optional category, taking `liveries` as cars and a
    /// [retired](Category::RETIRED) one as none.
    pub fn read<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Option<Self>, D::Error> {
        use serde::de::IntoDeserializer;
        match Option::<String>::deserialize(d)? {
            Some(name) if Self::RETIRED.contains(&name.as_str()) => Ok(None),
            Some(name) => Self::deserialize(name.into_deserializer()).map(Some),
            None => Ok(None),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    /// `db-editor`, or the id of the addon that generated it (`teamsimplified.mods`).
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_version: Option<String>,
    /// The file name the addon was made from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Saves {
    #[serde(default)]
    pub affects: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl Saves {
    fn is_unset(&self) -> bool {
        *self == Self::default()
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref().map(str::trim).filter(|m| !m.is_empty())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Launcher {
    pub program: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<SectionSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub build: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<CommandSpec>,
    // Ask the helper which dropped files and folders it takes, by running it as
    // `<program> claims <path>`, instead of matching `imports` against extensions.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub claims: bool,
    // Send the addon's first section a `startup` event once per launcher start.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub startup: bool,
}

impl Launcher {
    pub fn program(&self) -> Option<&str> {
        self.program.get(std::env::consts::OS).map(String::as_str)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Database,
    Erp,
    Text,
    Ui,
    Files,
    Native,
    /// Merged into the game's shader packages, as the Simplified Suite loads `.serps` mods.
    Simplified,
}

/// An experimental engine of the in-game core, named as its standard library page's
/// Feature row names it, that an addon turns on by listing it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ExperimentalFeature {
    /// Drawing over the game with `swofty/overlay.hpp`.
    Overlay,
    /// Grading the game's picture with a [`Shader`] look.
    Shaders,
}

impl ExperimentalFeature {
    /// Its name in `swofty/assets.json`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Overlay => "overlay",
            Self::Shaders => "shaders",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Overlay => "Overlay",
            Self::Shaders => "Shaders",
        }
    }
}

/// A colour look: a `.cube` 3D LUT graded in display colour, as a screenshot is, applied to
/// the game's picture after its tonemap and before the HUD.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Shader {
    /// The `.cube` file inside the addon. `{{id}}` names a `db` option, so a choice can pick
    /// between looks.
    pub lut: String,
    /// How much of the look applies, from 0 to 100, or `{{id}}` naming a `db` slider over that
    /// range. Defaults to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<Strength>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Strength {
    Percent(f64),
    Option(String),
}

/// The option ids a `{{id}}` template names, in order.
pub fn placeholders(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let Some(end) = rest[start..].find("}}") else {
            break;
        };
        out.push(rest[start + 2..start + end].trim());
        rest = &rest[start + end + 2..];
    }
    out
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

    pub fn accepts_launcher(&self, version: &Version) -> bool {
        self.launcher.as_ref().is_none_or(|r| r.matches(version))
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
pub struct AddonOption {
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
    pub files: Vec<FileEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub native: Vec<NativePlugin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub screens: Vec<ScreenOp>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NativePlugin {
    pub dll: String,
    // Accepted for compatibility; declaring a capability has no effect.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub early: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScreenOp {
    pub name: String,
    pub file: String,
    #[serde(default = "default_flow")]
    pub flow: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(default)]
    pub view_model: bool,
}

fn default_flow() -> String {
    "flow_frontend".into()
}

pub fn valid_screen_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

impl AddonManifest {
    pub fn load(dir: &Path) -> Result<Self> {
        let path = dir.join(MANIFEST_FILE);
        let data = std::fs::read(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        let m: Self = serde_json::from_slice(&data).map_err(|source| Error::Json { path, source })?;
        m.validate()?;
        Ok(m)
    }

    pub fn parse(json: &str) -> Result<Self> {
        let m: Self = serde_json::from_str(json).map_err(|e| Error::Manifest(e.to_string()))?;
        m.validate()?;
        Ok(m)
    }

    pub fn is_builtin(&self) -> bool {
        self.id == crate::CORE_ID
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
        let mut screens = BTreeMap::new();
        for s in &self.operations.screens {
            if !valid_screen_name(&s.name) || !valid_screen_name(&s.flow) {
                return fail(format!("screen {}: name and flow must be [a-z0-9_]", s.name));
            }
            if s.anchor.as_deref().is_some_and(|a| !valid_screen_name(a)) {
                return fail(format!("screen {}: anchor must be [a-z0-9_]", s.name));
            }
            if screens.insert(s.name.as_str(), ()).is_some() {
                return fail(format!("screen {} is declared twice", s.name));
            }
        }
        let paths = self
            .operations
            .database
            .iter()
            .chain(&self.operations.erp)
            .chain(&self.operations.ui)
            .chain(&self.operations.assets);
        let paths = paths
            .chain(self.operations.files.iter().map(|f| &f.source))
            .chain(
                self.operations
                    .native
                    .iter()
                    .flat_map(|n| std::iter::once(&n.dll).chain(&n.files)),
            )
            .chain(self.operations.screens.iter().map(|s| &s.file))
            .chain(self.launcher.iter().flat_map(|l| l.program.values()))
            .chain(&self.icon);
        if let Some(p) = paths.into_iter().find(|p| !relative_inside(p)) {
            return fail(format!("{p} must be a relative path inside the addon"));
        }
        if let Some(s) = &self.shader {
            self.validate_shader(s)
                .or_else(|e| fail(format!("shader: {e}")))?;
        }
        if let Some(l) = &self.launcher {
            if l.program.is_empty() {
                return fail("launcher.program names no program".into());
            }
            if let Some(e) = l.imports.iter().find(|e| e.is_empty() || e.contains('.')) {
                return fail(format!(
                    "launcher.imports: {e:?} must be an extension without the dot"
                ));
            }
        }
        Ok(())
    }

    fn validate_shader(&self, s: &Shader) -> std::result::Result<(), String> {
        if !self.experimental_features.contains(&ExperimentalFeature::Shaders) {
            return Err("list \"shaders\" in experimental_features to use it".into());
        }
        if !relative_inside(&s.lut) {
            return Err(format!("lut {} must be a relative path inside the addon", s.lut));
        }
        if !s.lut.to_ascii_lowercase().ends_with(".cube") {
            return Err(format!("lut {} must be a .cube file", s.lut));
        }
        if s.lut.matches("{{").count() != placeholders(&s.lut).len() {
            return Err(format!("lut {} has an unclosed {{{{", s.lut));
        }
        let db_option = |id: &str, slider: bool| {
            let o = self.options.iter().find(|o| o.id == id);
            match o {
                None => Err(format!("{{{{{id}}}}} names no option")),
                Some(o) if o.binds != Binds::Db => Err(format!("option {id} must bind db")),
                Some(o) if slider && o.kind != OptionType::Slider => {
                    Err(format!("option {id} must be a slider"))
                }
                Some(o) if slider && (o.min.unwrap_or(0.) < 0. || o.max.unwrap_or(0.) > 100.) => {
                    Err(format!("option {id} must stay within 0 to 100"))
                }
                Some(_) => Ok(()),
            }
        };
        for id in placeholders(&s.lut) {
            db_option(id, false)?;
        }
        match &s.strength {
            Some(Strength::Percent(p)) if !(0.0..=100.0).contains(p) => {
                Err(format!("strength {p} is not from 0 to 100"))
            }
            Some(Strength::Option(t)) => match placeholders(t)[..] {
                [id] if t.trim().starts_with("{{") && t.trim().ends_with("}}") => db_option(id, true),
                _ => Err(format!("strength {t:?} must be a number or one {{{{option}}}}")),
            },
            _ => Ok(()),
        }
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
        let m = AddonManifest::parse(GRID24).unwrap();
        assert_eq!(m.version, Version::new(1, 0, 0));
        assert!(m.compat.accepts_build(25_251_167));
        assert!(!m.compat.accepts_build(25_000_000));
        assert!(m.compat.accepts_launcher(&Version::new(1, 0, 9)));
        let newer = Compat {
            launcher: Some(VersionReq::parse(">=1.0.9").unwrap()),
            ..m.compat.clone()
        };
        assert!(newer.accepts_launcher(&Version::new(1, 0, 9)));
        assert!(!newer.accepts_launcher(&Version::new(1, 0, 8)));
        assert_eq!(m.options[0].binds, Binds::Db);
    }

    #[test]
    fn kind_takes_text_beside_the_older_values() {
        assert_eq!(AddonManifest::parse(GRID24).unwrap().kind, [Kind::Database]);
        let m = AddonManifest::parse(&GRID24.replace(
            r#""kind": ["database"]"#,
            r#""kind": ["database", "erp", "text", "ui", "files", "native", "simplified"]"#,
        ))
        .unwrap();
        assert_eq!(m.kind[2], Kind::Text);
        assert_eq!(m.kind[6], Kind::Simplified);
        assert_eq!(serde_json::to_value(&m).unwrap()["kind"][2], "text");
        assert_eq!(serde_json::to_value(&m).unwrap()["kind"][6], "simplified");
        assert!(
            AddonManifest::parse(&GRID24.replace(r#""kind": ["database"]"#, r#""kind": ["lang"]"#)).is_err()
        );
    }

    #[test]
    fn parses_early_native_plugins() {
        let with = |native: &str| {
            GRID24.replace(
                r#""operations": { "database": ["db/grid24.json"] }"#,
                &format!(r#""operations": {{ "native": [{native}] }}"#),
            )
        };
        let m = AddonManifest::parse(&with(
            r#"{ "dll": "native/a.dll", "early": true }, { "dll": "native/b.dll" }"#,
        ))
        .unwrap();
        let n = &m.operations.native;
        assert!(n[0].early && !n[1].early);
        let json = serde_json::to_value(&m.operations).unwrap();
        assert_eq!(json["native"][0]["early"], true);
        assert!(json["native"][1].get("early").is_none());
        let m = AddonManifest::parse(&with(
            r#"{ "dll": "native/a.dll", "files": ["lang/", "erp/a.bin"] }"#,
        ))
        .unwrap();
        assert_eq!(m.operations.native[0].files, ["lang/", "erp/a.bin"]);
        assert!(json["native"][1].get("files").is_none());
        assert!(AddonManifest::parse(&with(r#"{ "dll": "native/a.dll", "files": ["../x"] }"#)).is_err());
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
        assert!(AddonManifest::parse(&bad).is_err());
    }

    #[test]
    fn parses_and_checks_screens() {
        let with = |screens: &str| {
            GRID24.replace(
                r#""operations": { "database": ["db/grid24.json"] }"#,
                &format!(r#""operations": {{ "screens": [{screens}] }}"#),
            )
        };
        let m = AddonManifest::parse(&with(
            r#"{ "name": "grid_page", "file": "ui/grid_page.ui", "flow": "flow_options", "anchor": "settings_container", "view_model": true },
               { "name": "grid_info", "file": "ui/grid_info.ui" }"#,
        ))
        .unwrap();
        let s = &m.operations.screens;
        assert_eq!(
            (s[0].flow.as_str(), s[0].anchor.as_deref(), s[0].view_model),
            ("flow_options", Some("settings_container"), true)
        );
        assert_eq!(
            (s[1].flow.as_str(), s[1].anchor.as_deref(), s[1].view_model),
            ("flow_frontend", None, false)
        );
        for bad in [
            r#"{ "name": "a", "file": "../a.ui" }"#,
            r#"{ "name": "Bad", "file": "ui/a.ui" }"#,
            r#"{ "name": "a", "file": "ui/a.ui", "anchor": "x.y" }"#,
            r#"{ "name": "a", "file": "ui/a.ui" }, { "name": "a", "file": "ui/b.ui" }"#,
            r#"{ "name": "a", "file": "ui/a.ui", "extra": 1 }"#,
        ] {
            assert!(AddonManifest::parse(&with(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn parses_the_saves_flag() {
        let with = |saves: &str| {
            GRID24.replace(
                r#""author": "swofty","#,
                &format!(r#""author": "swofty", {saves}"#),
            )
        };
        let m = AddonManifest::parse(GRID24).unwrap();
        assert_eq!(m.saves, Saves::default());
        assert!(!m.saves.affects);
        assert!(serde_json::to_value(&m).unwrap().get("saves").is_none());
        let m = AddonManifest::parse(&with(r#""saves": { "affects": true },"#)).unwrap();
        assert!(m.saves.affects);
        assert_eq!(m.saves.message(), None);
        let m = AddonManifest::parse(&with(
            r#""saves": { "affects": true, "message": "Retire drivers instead of removing the addon." },"#,
        ))
        .unwrap();
        assert!(m.saves.affects);
        assert_eq!(
            m.saves.message(),
            Some("Retire drivers instead of removing the addon.")
        );
        let json = serde_json::to_value(&m).unwrap();
        assert_eq!(json["saves"]["affects"], true);
        assert_eq!(AddonManifest::parse(&json.to_string()).unwrap(), m);
        let m = AddonManifest::parse(&with(r#""saves": {},"#)).unwrap();
        assert!(!m.saves.affects);
        assert_eq!(
            AddonManifest::parse(&with(r#""saves": { "affects": true, "message": " " },"#))
                .unwrap()
                .saves
                .message(),
            None
        );
        for bad in [
            r#""saves": { "affects": true, "extra": 1 },"#,
            r#""saves": { "affects": "yes" },"#,
            r#""saves": true,"#,
        ] {
            assert!(AddonManifest::parse(&with(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn category_and_origin_are_optional_and_round_trip() {
        let m = AddonManifest::parse(GRID24).unwrap();
        assert_eq!((m.category, &m.origin), (None, &None));
        let json = serde_json::to_value(&m).unwrap();
        assert!(json.get("category").is_none() && json.get("origin").is_none());
        let with = GRID24.replace(
            r#""author": "swofty","#,
            r#""author": "swofty", "category": "helmets",
               "origin": { "tool": "teamsimplified.mods", "tool_version": "2.0.0", "file": "grid.zip", "sha256": "ab" },"#,
        );
        let m = AddonManifest::parse(&with).unwrap();
        assert_eq!(m.category, Some(Category::Helmets));
        assert_eq!(m.category.unwrap().label(), "Helmets");
        let origin = m.origin.as_ref().unwrap();
        assert_eq!(
            (origin.tool.as_str(), origin.file.as_deref()),
            ("teamsimplified.mods", Some("grid.zip"))
        );
        let json = serde_json::to_value(&m).unwrap();
        assert_eq!(json["category"], "helmets");
        assert_eq!(AddonManifest::parse(&json.to_string()).unwrap(), m);
        let bare = with.replace(
            r#", "tool_version": "2.0.0", "file": "grid.zip", "sha256": "ab""#,
            "",
        );
        let json = serde_json::to_value(AddonManifest::parse(&bare).unwrap()).unwrap();
        assert_eq!(
            json["origin"],
            serde_json::json!({ "tool": "teamsimplified.mods" })
        );
        for c in Category::ALL {
            let v = serde_json::to_value(c).unwrap();
            assert_eq!(serde_json::from_value::<Category>(v).unwrap(), c);
        }
        for bad in [
            r#""category": "maps","#,
            r#""origin": { "tool": "x", "extra": 1 },"#,
        ] {
            let bad = GRID24.replace(r#""author": "swofty","#, &format!(r#""author": "swofty", {bad}"#));
            assert!(AddonManifest::parse(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn earlier_categories_read_as_the_nearest_or_none() {
        let read = |c: &str| {
            let with = GRID24.replace(
                r#""author": "swofty","#,
                &format!(r#""author": "swofty", "category": {c},"#),
            );
            AddonManifest::parse(&with).unwrap().category
        };
        assert_eq!(read(r#""liveries""#), Some(Category::Cars));
        assert_eq!(read(r#""tracks""#), Some(Category::Tracks));
        assert_eq!(read("null"), None);
        for c in Category::RETIRED {
            assert_eq!(read(&format!("{c:?}")), None, "{c}");
        }
        let m = AddonManifest::parse(&GRID24.replace(
            r#""author": "swofty","#,
            r#""author": "swofty", "category": "liveries","#,
        ))
        .unwrap();
        assert_eq!(serde_json::to_value(&m).unwrap()["category"], "cars");
    }

    #[test]
    fn experimental_features_name_the_core_engines_an_addon_turns_on() {
        let m = AddonManifest::parse(GRID24).unwrap();
        assert!(m.experimental_features.is_empty());
        assert!(
            serde_json::to_value(&m)
                .unwrap()
                .get("experimental_features")
                .is_none()
        );
        let with = |f: &str| {
            GRID24.replace(
                r#""author": "swofty","#,
                &format!(r#""author": "swofty", "experimental_features": {f},"#),
            )
        };
        let m = AddonManifest::parse(&with(r#"["overlay"]"#)).unwrap();
        assert_eq!(m.experimental_features, [ExperimentalFeature::Overlay]);
        let f = m.experimental_features[0];
        assert_eq!((f.name(), f.label()), ("overlay", "Overlay"));
        assert_eq!(
            serde_json::to_value(&m).unwrap()["experimental_features"][0],
            "overlay"
        );
        assert!(AddonManifest::parse(&with(r#"["hud"]"#)).is_err());
        let m = AddonManifest::parse(&with(r#"["shaders"]"#)).unwrap();
        let f = m.experimental_features[0];
        assert_eq!((f.name(), f.label()), ("shaders", "Shaders"));
    }

    const LOOKS: &str = r#"{
        "schema_version": 1, "id": "author.looks", "name": "Looks", "version": "1.0.0", "author": "a",
        "category": "shaders", "experimental_features": ["shaders"], "compat": { "game": "f1_25" },
        "options": [
            { "id": "look", "type": "choice", "default": "mono", "values": ["mono", "noir"], "binds": "db" },
            { "id": "strength", "type": "slider", "min": 0, "max": 100, "step": 10, "default": 100, "binds": "db" },
            { "id": "live", "type": "toggle", "default": true, "binds": "native" }
        ],
        "shader": SHADER
    }"#;

    fn looks(shader: &str) -> std::result::Result<AddonManifest, String> {
        AddonManifest::parse(&LOOKS.replace("SHADER", shader)).map_err(|e| e.to_string())
    }

    #[test]
    fn a_shader_pack_names_its_lut_and_strength() {
        let m = looks(r#"{ "lut": "looks/{{look}}.cube", "strength": "{{ strength }}" }"#).unwrap();
        assert_eq!(m.category, Some(Category::Shaders));
        assert_eq!(Category::Shaders.label(), "Shaders");
        let s = m.shader.as_ref().unwrap();
        assert_eq!(s.lut, "looks/{{look}}.cube");
        assert_eq!(s.strength, Some(Strength::Option("{{ strength }}".into())));
        assert_eq!(placeholders(&s.lut), ["look"]);
        let m = looks(r#"{ "lut": "looks/mono.CUBE", "strength": 40 }"#).unwrap();
        assert_eq!(m.shader.unwrap().strength, Some(Strength::Percent(40.0)));
        assert_eq!(
            looks(r#"{ "lut": "mono.cube" }"#)
                .unwrap()
                .shader
                .unwrap()
                .strength,
            None
        );
    }

    #[test]
    fn a_shader_pack_is_checked_before_it_installs() {
        let err = |shader: &str| looks(shader).unwrap_err();
        let at = |msg: &str| format!("invalid manifest: author.looks: shader: {msg}");
        assert_eq!(
            err(r#"{ "lut": "mono.png" }"#),
            at("lut mono.png must be a .cube file")
        );
        assert_eq!(
            err(r#"{ "lut": "../mono.cube" }"#),
            at("lut ../mono.cube must be a relative path inside the addon")
        );
        assert_eq!(
            err(r#"{ "lut": "{{look.cube" }"#),
            at("lut {{look.cube has an unclosed {{")
        );
        assert_eq!(
            err(r#"{ "lut": "{{shade}}.cube" }"#),
            at("{{shade}} names no option")
        );
        assert_eq!(
            err(r#"{ "lut": "{{live}}.cube" }"#),
            at("option live must bind db")
        );
        assert_eq!(
            err(r#"{ "lut": "a.cube", "strength": 150 }"#),
            at("strength 150 is not from 0 to 100")
        );
        assert_eq!(
            err(r#"{ "lut": "a.cube", "strength": "{{look}}" }"#),
            at("option look must be a slider")
        );
        assert_eq!(
            err(r#"{ "lut": "a.cube", "strength": "x{{strength}}" }"#),
            at(r#"strength "x{{strength}}" must be a number or one {{option}}"#)
        );
        let no_feature = LOOKS
            .replace(r#""experimental_features": ["shaders"],"#, "")
            .replace("SHADER", r#"{ "lut": "a.cube" }"#);
        assert_eq!(
            AddonManifest::parse(&no_feature).unwrap_err().to_string(),
            at(r#"list "shaders" in experimental_features to use it"#)
        );
    }

    #[test]
    fn rejects_unknown_fields() {
        let bad = GRID24.replace("\"kind\"", "\"kinds\"");
        assert!(AddonManifest::parse(&bad).is_err());
    }
}
