//! Shader effects and `.swfx` shader configs (DESIGN.md §2-§3, §6).
//!
//! An effect is HLSL passes plus declared params; a config lists effect instances with param values.
//! This crate has no store or game access: the launcher, the CLI and the preview all use it.

use std::collections::BTreeMap;
use std::ops::Range;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The most float slots one effect's params may take (`option(0..31)`).
pub const MAX_SLOTS: usize = 32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Number(f64),
    Text(String),
    Vector(Vec<f64>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParamKind {
    Float {
        min: Option<f64>,
        max: Option<f64>,
        step: Option<f64>,
    },
    Int {
        min: Option<i64>,
        max: Option<i64>,
    },
    Bool,
    Enum {
        values: Vec<String>,
    },
    Float2 {
        min: Option<f64>,
        max: Option<f64>,
    },
    Color3 {
        min: Option<f64>,
        max: Option<f64>,
    },
    Color4 {
        min: Option<f64>,
        max: Option<f64>,
    },
}

impl ParamKind {
    /// The float slots a param of this kind takes.
    pub fn width(&self) -> usize {
        match self {
            Self::Float2 { .. } => 2,
            Self::Color3 { .. } => 3,
            Self::Color4 { .. } => 4,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub id: String,
    pub label: String,
    pub doc: String,
    pub kind: ParamKind,
    pub default: Value,
    pub slot: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextureDecl {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub persistent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassDecl {
    pub name: String,
    pub shader: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub raw: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads: Option<[u32; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_ms: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Origin {
    Library,
    Addon { id: String, name: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Effect {
    pub id: String,
    pub name: String,
    pub description: String,
    pub params: Vec<Param>,
    pub textures: Vec<TextureDecl>,
    pub passes: Vec<PassDecl>,
    /// Float slots the params take.
    pub slots: usize,
    pub origin: Origin,
}

/// Reads an `<id>.effect.json` (DESIGN.md §3).
pub fn parse_effect(json: &str, origin: Origin) -> Result<Effect, String> {
    let _ = (json, origin);
    Err("not written yet".into())
}

/// The `swofty/params.hlsli` accessors for an effect (DESIGN.md §3).
pub fn params_hlsl(effect: &Effect) -> String {
    let _ = effect;
    String::new()
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub effects: Vec<Effect>,
}

impl Catalog {
    pub fn get(&self, id: &str) -> Option<&Effect> {
        self.effects.iter().find(|e| e.id == id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    pub effect: String,
    pub label: Option<String>,
    pub enabled: bool,
    pub values: BTreeMap<String, Value>,
    /// The instance's table in the text, header to last line.
    pub span: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Config {
    pub name: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub instances: Vec<Instance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

/// A position for the editor: 0-based line and UTF-16 column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pos {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fix {
    pub title: String,
    pub range: Range<usize>,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub range: Range<usize>,
    pub start: Pos,
    pub end: Pos,
    pub message: String,
    pub fix: Option<Fix>,
}

#[derive(Debug, Clone, Default)]
pub struct Lint {
    /// `None` when any diagnostic is an error.
    pub config: Option<Config>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Lint {
    pub fn errors(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }
}

/// Parses and checks a config against the catalog (DESIGN.md §2).
pub fn lint(text: &str, catalog: &Catalog) -> Lint {
    let _ = (text, catalog);
    Lint::default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Effect,
    Param,
    Value,
    Keyword,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: String,
    pub doc: String,
    pub range: Range<usize>,
    pub insert: String,
}

pub fn completions(text: &str, offset: usize, catalog: &Catalog) -> Vec<Completion> {
    let _ = (text, offset, catalog);
    Vec::new()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hover {
    /// Markdown.
    pub text: String,
    pub range: Range<usize>,
}

pub fn hover(text: &str, offset: usize, catalog: &Catalog) -> Option<Hover> {
    let _ = (text, offset, catalog);
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Comment,
    Header,
    Key,
    Number,
    String,
    Bool,
    Punct,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub range: Range<usize>,
}

/// Highlighting tokens; tolerant of broken text.
pub fn tokens(text: &str) -> Vec<Token> {
    let _ = text;
    Vec::new()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveInstance {
    pub effect: String,
    #[serde(default)]
    pub label: String,
    pub params: Vec<f32>,
}

/// `active.json` (DESIGN.md §5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Active {
    pub version: u32,
    pub serial: u64,
    pub name: String,
    pub source: String,
    #[serde(default)]
    pub forced_by: String,
    pub toggle_key: String,
    pub instances: Vec<ActiveInstance>,
}

/// The enabled instances with their packed, clamped param slots.
pub fn resolve(config: &Config, catalog: &Catalog) -> Vec<ActiveInstance> {
    let _ = (config, catalog);
    Vec::new()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Dxil,
    Spirv,
}

/// Compiles an effect's passes, one blob per pass. `dir` holds the effect's HLSL; `dxc` is the
/// compiler (`swofty_core::postfx::dxc()`); `header` is `postfx.hlsli`.
pub fn compile(
    effect: &Effect,
    dir: &Path,
    target: Target,
    dxc: &Path,
    header: &str,
    cache: &Path,
) -> Result<Vec<Vec<u8>>, String> {
    let _ = (effect, dir, target, dxc, header, cache);
    Err("not written yet".into())
}
