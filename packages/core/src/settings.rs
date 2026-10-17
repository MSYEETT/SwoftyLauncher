use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::manifest::{Binds, ModManifest, ModOption, OptionType, ScreenOp};
use crate::map::TrackPackage;
use crate::profile::Profile;
use crate::{Error, Result};

pub const SCHEMA: u32 = 1;
pub const SETTINGS_FILE: &str = "swofty/settings.json";
pub const RESTART_COMMAND: &str = "swofty_restart";
pub const UI_DIR: &str = "swofty/ui";
pub const MAX_ID_LEN: usize = 40;
pub const MAX_SLIDER_STEPS: usize = 100;
pub const MAX_CHOICES: usize = 16;

pub fn djb2(s: &str) -> u32 {
    s.bytes()
        .fold(5381u32, |h, b| h.wrapping_mul(33).wrapping_add(u32::from(b)))
}

pub fn valid_option_id(id: &str) -> bool {
    id.len() <= MAX_ID_LEN
        && id.starts_with(|c: char| c.is_ascii_lowercase())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub value: Value,
    pub text: String,
}

struct Grid {
    min: f64,
    step: f64,
    steps: usize,
    integer: bool,
    decimals: usize,
}

fn decimals(x: f64) -> usize {
    format!("{x}").split_once('.').map_or(0, |(_, f)| f.len())
}

impl Grid {
    fn value(&self, k: usize) -> f64 {
        let v = self.min + k as f64 * self.step;
        if self.integer {
            return v.round();
        }
        let scale = 10f64.powi(self.decimals as i32);
        (v * scale).round() / scale
    }

    fn json(&self, v: f64) -> Value {
        if self.integer {
            Value::from(v as i64)
        } else {
            serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)
        }
    }

    fn text(&self, v: f64) -> String {
        if self.integer {
            format!("{}", v as i64)
        } else {
            format!("{v:.*}", self.decimals)
        }
    }
}

impl ModOption {
    pub fn title(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.id)
    }

    pub(crate) fn validate(&self) -> std::result::Result<(), String> {
        if !valid_option_id(&self.id) {
            return Err(format!(
                "id must match [a-z][a-z0-9_]* and be at most {MAX_ID_LEN} characters"
            ));
        }
        let range = self.min.is_some() || self.max.is_some() || self.step.is_some();
        match self.kind {
            OptionType::Toggle => {
                if range || !self.values.is_empty() {
                    return Err("a toggle takes no values, min, max or step".into());
                }
                if !self.default.is_boolean() {
                    return Err("default must be true or false".into());
                }
            }
            OptionType::Slider => {
                if !self.values.is_empty() {
                    return Err("a slider takes min, max and step, not values".into());
                }
                self.grid()?;
                if self.index(&self.default).is_none() {
                    return Err(format!("default {} is not on the slider's grid", self.default));
                }
            }
            OptionType::Choice => {
                if range {
                    return Err("a choice takes values, not min, max or step".into());
                }
                if self.values.is_empty() || self.values.len() > MAX_CHOICES {
                    return Err(format!("a choice needs 1 to {MAX_CHOICES} values"));
                }
                for (i, c) in self.values.iter().enumerate() {
                    if c.value.is_empty() {
                        return Err("choice values must not be empty".into());
                    }
                    if self.values[..i].iter().any(|d| d.value == c.value) {
                        return Err(format!("value {} is listed twice", c.value));
                    }
                }
                if self.index(&self.default).is_none() {
                    return Err(format!("default {} is not one of the values", self.default));
                }
            }
        }
        Ok(())
    }

    fn grid(&self) -> std::result::Result<Grid, String> {
        let (Some(min), Some(max), Some(step)) = (self.min, self.max, self.step) else {
            return Err("a slider needs min, max and step".into());
        };
        if !(min.is_finite() && max.is_finite() && step.is_finite()) || min >= max || step <= 0.0 {
            return Err("a slider needs min < max and step > 0".into());
        }
        let span = (max - min) / step;
        let steps = span.round();
        if (span - steps).abs() > 1e-6 {
            return Err("max - min must be a whole number of steps".into());
        }
        if steps > MAX_SLIDER_STEPS as f64 {
            return Err(format!("a slider has at most {MAX_SLIDER_STEPS} steps"));
        }
        Ok(Grid {
            min,
            step,
            steps: steps as usize,
            integer: [min, max, step].iter().all(|v| v.fract() == 0.0),
            decimals: decimals(step).max(decimals(min)),
        })
    }

    pub fn range(&self) -> Option<(f64, f64, f64)> {
        self.grid().ok().map(|g| (g.min, g.value(g.steps), g.step))
    }

    pub fn states(&self) -> Vec<State> {
        match self.kind {
            OptionType::Toggle => vec![
                State {
                    value: false.into(),
                    text: "OFF".into(),
                },
                State {
                    value: true.into(),
                    text: "ON".into(),
                },
            ],
            OptionType::Slider => {
                let Ok(g) = self.grid() else {
                    return Vec::new();
                };
                (0..=g.steps)
                    .map(|k| {
                        let v = g.value(k);
                        State {
                            value: g.json(v),
                            text: g.text(v),
                        }
                    })
                    .collect()
            }
            OptionType::Choice => self
                .values
                .iter()
                .map(|c| State {
                    value: c.value.clone().into(),
                    text: c.label.clone().unwrap_or_else(|| c.value.clone()),
                })
                .collect(),
        }
    }

    pub fn index(&self, v: &Value) -> Option<usize> {
        match self.kind {
            OptionType::Toggle => v.as_bool().map(usize::from),
            OptionType::Slider => {
                let g = self.grid().ok()?;
                let x = v.as_f64()?;
                let k = ((x - g.min) / g.step).round();
                let ok = k >= 0.0 && k <= g.steps as f64 && (g.value(k as usize) - x).abs() <= g.step * 1e-6;
                ok.then_some(k as usize)
            }
            OptionType::Choice => {
                let s = v.as_str()?;
                self.values.iter().position(|c| c.value == s)
            }
        }
    }

    pub fn normalise(&self, v: &Value) -> Option<Value> {
        let i = self.index(v)?;
        match self.kind {
            OptionType::Toggle => Some(Value::Bool(i == 1)),
            OptionType::Slider => self.grid().ok().map(|g| g.json(g.value(i))),
            OptionType::Choice => Some(Value::String(self.values[i].value.clone())),
        }
    }

    pub fn default_value(&self) -> Value {
        self.normalise(&self.default)
            .unwrap_or_else(|| self.default.clone())
    }

    pub fn text(&self, v: &Value) -> String {
        match (self.kind, self.index(v)) {
            (OptionType::Toggle, Some(i)) => (if i == 1 { "On" } else { "Off" }).into(),
            (_, Some(i)) => self.states().swap_remove(i).text,
            (_, None) => v.to_string(),
        }
    }

    fn placeholder(&self, v: &Value) -> String {
        match v {
            Value::String(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            _ => self.text(v),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionSettings {
    pub schema: u32,
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requests: Option<String>,
    pub restart_command: String,
    pub mods: Vec<SessionMod>,
    #[serde(default)]
    pub maps: Vec<SessionMap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<SessionUi>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionUi {
    pub register: bool,
    pub strict: bool,
    pub recover: bool,
    pub screens: Vec<SessionScreen>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionScreen {
    pub name: String,
    pub file: String,
    pub flow: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(default)]
    pub view_model: bool,
}

pub fn screen_file(name: &str) -> String {
    format!("ui/{name}.ui")
}

pub fn screens<'a>(
    order: impl IntoIterator<Item = &'a ModManifest>,
) -> Result<Vec<(&'a ModManifest, &'a ScreenOp)>> {
    let mut out: Vec<(&ModManifest, &ScreenOp)> = Vec::new();
    for m in order {
        for s in &m.operations.screens {
            if let Some((other, _)) = out.iter().find(|(_, o)| o.name == s.name) {
                return Err(Error::Other(format!(
                    "screen {} is declared by {} and {}",
                    s.name, other.id, m.id
                )));
            }
            out.push((m, s));
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionMap {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionMod {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    pub enabled: bool,
    pub locked: bool,
    #[serde(default)]
    pub dlls: Vec<String>,
    pub enable_commands: [String; 2],
    #[serde(default)]
    pub options: Vec<SessionOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionOption {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: OptionType,
    pub binds: Binds,
    pub label: String,
    #[serde(default)]
    pub description: String,
    pub current: usize,
    pub states: Vec<SessionState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionState {
    pub value: Value,
    pub text: String,
    pub command: String,
}

impl SessionSettings {
    pub fn build(
        profile: &Profile,
        installed: &BTreeMap<String, ModManifest>,
        packages: &[TrackPackage],
        requests: Option<String>,
    ) -> Result<Self> {
        if profile.is_vanilla() {
            return Err(Error::Other(format!(
                "{} plays without mods and has no mod settings",
                profile.name
            )));
        }
        let mut order: Vec<&ModManifest> = profile.mods.iter().filter_map(|m| installed.get(&m.id)).collect();
        let mut rest: Vec<&ModManifest> = installed
            .values()
            .filter(|m| !order.iter().any(|o| o.id == m.id))
            .collect();
        rest.sort_by_key(|m| m.name.to_lowercase());
        order.extend(rest);
        let enabled: Vec<&str> = profile.enabled().collect();
        let screens: Vec<SessionScreen> =
            screens(order.iter().copied().filter(|m| enabled.contains(&m.id.as_str())))?
                .into_iter()
                .map(|(_, s)| SessionScreen {
                    name: s.name.clone(),
                    file: screen_file(&s.name),
                    flow: s.flow.clone(),
                    anchor: s.anchor.clone(),
                    view_model: s.view_model,
                })
                .collect();
        let ui = (!screens.is_empty()).then_some(SessionUi {
            register: true,
            strict: false,
            recover: true,
            screens,
        });
        let mods = order
            .into_iter()
            .map(|m| SessionMod {
                id: m.id.clone(),
                name: m.name.clone(),
                version: m.version.to_string(),
                author: m.author.clone(),
                description: m.description.clone().unwrap_or_default(),
                enabled: enabled.contains(&m.id.as_str()),
                locked: m.is_builtin(),
                dlls: m
                    .operations
                    .native
                    .iter()
                    .map(|n| n.dll.rsplit('/').next().unwrap_or(&n.dll).to_owned())
                    .collect(),
                enable_commands: [0, 1].map(|b| format!("swofty_enable:{}:{b}", m.id)),
                options: m
                    .options
                    .iter()
                    .map(|o| SessionOption {
                        id: o.id.clone(),
                        kind: o.kind,
                        binds: o.binds,
                        label: o.title().to_owned(),
                        description: o.description.clone().unwrap_or_default(),
                        current: o.index(&profile.option(m, o)).unwrap_or(0),
                        states: o
                            .states()
                            .into_iter()
                            .enumerate()
                            .map(|(i, s)| SessionState {
                                value: s.value,
                                text: s.text,
                                command: format!("swofty_set:{}:{}:{i}", m.id, o.id),
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect();
        let maps = profile
            .maps
            .iter()
            .filter_map(|code| packages.iter().find(|p| &p.track_code == code))
            .map(|p| SessionMap {
                code: p.track_code.clone(),
                name: p.track_name.clone().unwrap_or_else(|| p.track_code.clone()),
                version: p.version.clone().unwrap_or_default(),
                author: p.author_name.clone().unwrap_or_default(),
                description: p
                    .extra
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            })
            .collect();
        let out = Self {
            schema: SCHEMA,
            profile: profile.name.clone(),
            requests,
            restart_command: RESTART_COMMAND.into(),
            mods,
            maps,
            ui,
        };
        out.check_commands()?;
        Ok(out)
    }

    pub fn commands(&self) -> impl Iterator<Item = &str> {
        let mods = self.mods.iter().flat_map(|m| {
            let options = m
                .options
                .iter()
                .flat_map(|o| o.states.iter().map(|s| s.command.as_str()));
            m.enable_commands.iter().map(String::as_str).chain(options)
        });
        std::iter::once(self.restart_command.as_str()).chain(mods)
    }

    fn check_commands(&self) -> Result<()> {
        let mut seen: HashMap<u32, &str> = HashMap::new();
        for c in self.commands() {
            if let Some(other) = seen.insert(djb2(c), c) {
                return Err(Error::Other(format!(
                    "commands {other} and {c} share the djb2 hash {:08x}; rename an option",
                    djb2(c)
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Requests {
    #[serde(default)]
    pub schema: u32,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub seq: u64,
    #[serde(default)]
    pub options: BTreeMap<String, BTreeMap<String, Value>>,
    #[serde(default)]
    pub enabled: BTreeMap<String, bool>,
    #[serde(default)]
    pub relaunch: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Merge {
    pub changed: Vec<String>,
    pub skipped: Vec<String>,
    pub relaunch: bool,
}

pub fn merge(profile: &mut Profile, installed: &BTreeMap<String, ModManifest>, req: &Requests) -> Merge {
    let mut out = Merge {
        relaunch: req.relaunch,
        ..Merge::default()
    };
    for (mod_id, values) in &req.options {
        let Some(m) = installed.get(mod_id) else {
            out.skipped.push(format!("{mod_id} is not installed"));
            continue;
        };
        for (id, v) in values {
            match profile.set_option(m, id, v.clone()) {
                Ok(true) => {
                    let text = m.options.iter().find(|o| &o.id == id).map(|o| o.text(v));
                    out.changed
                        .push(format!("{mod_id}.{id} = {}", text.unwrap_or_default()));
                }
                Ok(false) => {}
                Err(e) => out.skipped.push(e.to_string()),
            }
        }
    }
    for (mod_id, &on) in &req.enabled {
        if !installed.contains_key(mod_id) {
            out.skipped.push(format!("{mod_id} is not installed"));
            continue;
        }
        if profile.enabled().any(|e| e == mod_id) == on {
            continue;
        }
        match profile.set_enabled(mod_id, on) {
            Ok(()) => out
                .changed
                .push(format!("{mod_id} {}", if on { "enabled" } else { "disabled" })),
            Err(e) => out.skipped.push(e.to_string()),
        }
    }
    out
}

pub fn render_ops(doc: &mut Value, m: &ModManifest, profile: &Profile) -> Result<()> {
    let fail = |msg: String| Error::Resolve(format!("{}: {msg}", m.id));
    let option = |id: &str| {
        m.options
            .iter()
            .find(|o| o.id == id)
            .ok_or_else(|| fail(format!("unknown option {id}")))
    };
    let Some(ops) = doc.get_mut("ops").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    let mut keep = Vec::with_capacity(ops.len());
    for mut op in ops.drain(..) {
        if let Some(when) = op.as_object_mut().and_then(|o| o.remove("when")) {
            let Value::Object(when) = when else {
                return Err(fail("when must be an object of option values".into()));
            };
            let mut hit = true;
            for (id, want) in &when {
                let o = option(id)?;
                let want = o
                    .normalise(want)
                    .ok_or_else(|| fail(format!("when: {want} is not a value of {id}")))?;
                hit &= profile.option(m, o) == want;
            }
            if !hit {
                continue;
            }
        }
        substitute(&mut op, &|id| {
            let o = option(id)?;
            Ok(o.placeholder(&profile.option(m, o)))
        })?;
        keep.push(op);
    }
    *ops = keep;
    Ok(())
}

fn substitute(v: &mut Value, lookup: &dyn Fn(&str) -> Result<String>) -> Result<()> {
    match v {
        Value::String(s) if s.contains("{{") => {
            let mut out = String::with_capacity(s.len());
            let mut rest = s.as_str();
            while let Some(start) = rest.find("{{") {
                let end = rest[start..]
                    .find("}}")
                    .ok_or_else(|| Error::Resolve(format!("unclosed {{{{ in {s}")))?;
                out.push_str(&rest[..start]);
                out.push_str(&lookup(rest[start + 2..start + end].trim())?);
                rest = &rest[start + end + 2..];
            }
            out.push_str(rest);
            *s = out;
        }
        Value::Array(a) => a.iter_mut().try_for_each(|x| substitute(x, lookup))?,
        Value::Object(o) => o.values_mut().try_for_each(|x| substitute(x, lookup))?,
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ModManifest;

    fn manifest() -> ModManifest {
        ModManifest::parse(
            r#"{
              "schema_version": 1, "id": "a.grid", "name": "Grid", "version": "1.0.0", "author": "a",
              "compat": { "game": "f1_25" },
              "options": [
                { "id": "livery", "type": "choice", "default": "title", "binds": "db",
                  "values": ["title", { "value": "tier01", "label": "Tier 1" }, "replica"] },
                { "id": "audi", "type": "toggle", "default": false, "binds": "db" },
                { "id": "offset", "type": "slider", "min": -1, "max": 1, "step": 0.25, "default": 0, "binds": "native" },
                { "id": "count", "type": "slider", "min": 0, "max": 10, "step": 2, "default": 4, "binds": "native" }
              ],
              "operations": { "native": [{ "dll": "native/grid.dll" }] }
            }"#,
        )
        .unwrap()
    }

    fn installed() -> BTreeMap<String, ModManifest> {
        let mut m = BTreeMap::from([(
            crate::builtin::CORE_ID.to_owned(),
            crate::builtin::core_manifest(),
        )]);
        m.insert("a.grid".into(), manifest());
        m
    }

    #[test]
    fn djb2_matches_the_game() {
        assert_eq!(djb2("swofty_set"), 0x5fdf279c);
        assert_eq!(djb2("swofty_settings"), 0x90c2d901);
        assert_eq!(djb2(""), 0x1505);
    }

    #[test]
    fn slider_grids_and_text() {
        let m = manifest();
        let offset = &m.options[2];
        let states = offset.states();
        assert_eq!(states.len(), 9);
        assert_eq!(states[0].text, "-1.00");
        assert_eq!(states[5].value, serde_json::json!(0.25));
        assert_eq!(offset.normalise(&serde_json::json!(0.26)), None);
        assert_eq!(
            offset.normalise(&serde_json::json!(0.5000000001)),
            Some(serde_json::json!(0.5))
        );
        let count = &m.options[3];
        assert_eq!(
            count.states().iter().map(|s| s.text.as_str()).collect::<Vec<_>>(),
            ["0", "2", "4", "6", "8", "10"]
        );
        assert_eq!(
            count.normalise(&serde_json::json!(6.0)),
            Some(serde_json::json!(6))
        );
        assert_eq!(count.text(&serde_json::json!(8)), "8");
        assert_eq!(m.options[0].text(&serde_json::json!("tier01")), "Tier 1");
    }

    #[test]
    fn rejects_bad_options() {
        let base = |opt: &str| {
            format!(
                r#"{{"schema_version":1,"id":"a.b","name":"B","version":"1.0.0","author":"a",
                "compat":{{"game":"f1_25"}},"options":[{opt}]}}"#
            )
        };
        for bad in [
            r#"{"id":"Bad","type":"toggle","default":true,"binds":"native"}"#,
            r#"{"id":"t","type":"toggle","default":1,"binds":"native"}"#,
            r#"{"id":"s","type":"slider","min":0,"max":10,"default":0,"binds":"native"}"#,
            r#"{"id":"s","type":"slider","min":0,"max":10,"step":3,"default":0,"binds":"native"}"#,
            r#"{"id":"s","type":"slider","min":0,"max":1000,"step":1,"default":0,"binds":"native"}"#,
            r#"{"id":"s","type":"slider","min":0,"max":10,"step":1,"default":0.5,"binds":"native"}"#,
            r#"{"id":"c","type":"choice","values":[],"default":"a","binds":"db"}"#,
            r#"{"id":"c","type":"choice","values":["a","a"],"default":"a","binds":"db"}"#,
            r#"{"id":"c","type":"choice","values":["a"],"default":"b","binds":"db"}"#,
            r#"{"id":"c","type":"string","default":"b","binds":"db"}"#,
        ] {
            assert!(
                ModManifest::parse(&base(bad)).is_err(),
                "{bad} should be rejected"
            );
        }
    }

    #[test]
    fn session_file_lists_enabled_maps_in_profile_order() {
        let mut p = Profile::new("x");
        p.maps = vec!["okayama".into(), "bahrain24".into()];
        let pkg = |code: &str, name: &str| {
            TrackPackage::parse(
                format!(
                    r#"{{"track_code":"{code}","track_name":"{name}","version":"1.0.0","author_name":"a","erp":"{code}.erp","description":"d"}}"#
                )
                .as_bytes(),
            )
            .unwrap()
        };
        let packages = [
            pkg("bahrain24", "Sepang"),
            pkg("okayama", "Okayama"),
            pkg("spare", "Spare"),
        ];
        let s = SessionSettings::build(&p, &installed(), &packages, None).unwrap();
        let codes: Vec<&str> = s.maps.iter().map(|m| m.code.as_str()).collect();
        assert_eq!(codes, ["okayama", "bahrain24"]);
        assert_eq!(s.maps[1].name, "Sepang");
        assert_eq!(
            (s.maps[0].author.as_str(), s.maps[0].description.as_str()),
            ("a", "d")
        );
    }

    #[test]
    fn session_file_lists_every_mod_with_states_and_commands() {
        let mut p = Profile::new("x");
        let m = manifest();
        p.set_option(&m, "livery", serde_json::json!("replica")).unwrap();
        let s = SessionSettings::build(&p, &installed(), &[], Some("Z:\\r.json".into())).unwrap();
        assert_eq!(s.mods[0].id, crate::builtin::CORE_ID);
        assert!(s.mods[0].locked && s.mods[0].enabled);
        let grid = &s.mods[1];
        assert!(!grid.enabled && !grid.locked);
        assert_eq!(grid.author, "a");
        assert_eq!(grid.dlls, ["grid.dll"]);
        assert_eq!(grid.enable_commands[1], "swofty_enable:a.grid:1");
        assert_eq!(grid.options[0].current, 2);
        assert_eq!(grid.options[0].states[2].command, "swofty_set:a.grid:livery:2");
        assert_eq!(grid.options[2].current, 4);
        assert!(s.commands().any(|c| c == RESTART_COMMAND));
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["mods"][1]["options"][0]["type"], "choice");
        assert!(SessionSettings::build(&Profile::vanilla(), &installed(), &[], None).is_err());
    }

    fn screens_manifest(id: &str, screens: &str) -> ModManifest {
        ModManifest::parse(&format!(
            r#"{{ "schema_version": 1, "id": "{id}", "name": "S", "version": "1.0.0", "author": "a",
                 "compat": {{ "game": "f1_25" }}, "operations": {{ "screens": [{screens}] }} }}"#
        ))
        .unwrap()
    }

    #[test]
    fn session_file_lists_enabled_screens_and_omits_an_empty_ui_block() {
        let mut mods = installed();
        mods.insert(
            "a.pages".into(),
            screens_manifest(
                "a.pages",
                r#"{ "name": "swofty_mod_0", "file": "ui/page0.ui", "flow": "flow_options", "anchor": "settings_container", "view_model": true },
                   { "name": "swofty_mod_1", "file": "ui/page1.ui", "flow": "flow_options" }"#,
            ),
        );
        mods.insert(
            "b.off".into(),
            screens_manifest("b.off", r#"{ "name": "off_page", "file": "ui/off.ui" }"#),
        );
        let mut p = Profile::new("x");
        let none = SessionSettings::build(&p, &mods, &[], None).unwrap();
        assert_eq!(none.ui, None);
        assert!(serde_json::to_value(&none).unwrap().get("ui").is_none());
        p.set_enabled("a.pages", true).unwrap();
        let s = SessionSettings::build(&p, &mods, &[], None).unwrap();
        let ui = s.ui.as_ref().unwrap();
        assert!(ui.register && !ui.strict && ui.recover);
        let names: Vec<&str> = ui.screens.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["swofty_mod_0", "swofty_mod_1"]);
        assert_eq!(ui.screens[0].file, "ui/swofty_mod_0.ui");
        assert_eq!(ui.screens[0].anchor.as_deref(), Some("settings_container"));
        assert!(ui.screens[0].view_model && !ui.screens[1].view_model);
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["ui"]["screens"][1]["flow"], "flow_options");
        assert!(json["ui"]["screens"][1].get("anchor").is_none());
        let back: SessionSettings = serde_json::from_value(json).unwrap();
        assert_eq!(back, s);
        mods.insert(
            "c.clash".into(),
            screens_manifest("c.clash", r#"{ "name": "swofty_mod_0", "file": "ui/x.ui" }"#),
        );
        p.set_enabled("c.clash", true).unwrap();
        assert!(SessionSettings::build(&p, &mods, &[], None).is_err());
    }

    #[test]
    fn merges_requests_and_tracks_rebuilds() {
        let mods = installed();
        let mut p = Profile::new("x");
        p.set_enabled("a.grid", true).unwrap();
        assert!(p.rebuild_pending(&mods).is_empty());
        let req: Requests = serde_json::from_str(
            r#"{"schema":1,"profile":"x","seq":2,
                "options":{"a.grid":{"livery":"tier01","offset":0.5,"nope":1},"z.missing":{"a":1}},
                "enabled":{"a.grid":false,"swofty.core":false},"relaunch":true}"#,
        )
        .unwrap();
        let out = merge(&mut p, &mods, &req);
        assert_eq!(
            out.changed,
            [
                "a.grid.livery = Tier 1",
                "a.grid.offset = 0.50",
                "a.grid disabled"
            ]
        );
        assert_eq!(out.skipped.len(), 3);
        assert!(out.relaunch);
        assert_eq!(merge(&mut p, &mods, &req).changed, Vec::<String>::new());
        assert!(p.rebuild_pending(&mods).is_empty());
        p.set_enabled("a.grid", true).unwrap();
        assert_eq!(p.rebuild_pending(&mods), ["a.grid.livery"]);
        p.mark_built(&mods);
        assert!(p.rebuild_pending(&mods).is_empty());
        p.set_option(&mods["a.grid"], "livery", serde_json::json!("title"))
            .unwrap();
        assert_eq!(p.rebuild_pending(&mods), ["a.grid.livery"]);
        assert!(!p.options.contains_key("a.grid") || !p.options["a.grid"].contains_key("livery"));
    }

    #[test]
    fn renders_database_ops_from_options() {
        let m = manifest();
        let mut p = Profile::new("x");
        p.set_option(&m, "count", serde_json::json!(8)).unwrap();
        let mut doc = serde_json::json!({"schema_version": 1, "ops": [
            {"op": "set", "select": "t.r", "field": "n", "value": "{{count}}"},
            {"op": "set", "select": "t.r", "field": "l", "value": "{{ livery }}-{{audi}}", "when": {"livery": "title"}},
            {"op": "set", "select": "t.r", "field": "a", "value": "1", "when": {"audi": true}}
        ]});
        render_ops(&mut doc, &m, &p).unwrap();
        let ops = doc["ops"].as_array().unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0]["value"], "8");
        assert_eq!(ops[1]["value"], "title-false");
        assert!(ops[1].get("when").is_none());
        let mut bad = serde_json::json!({"ops": [{"op": "set", "value": "{{missing}}"}]});
        assert!(render_ops(&mut bad, &m, &p).is_err());
        let mut bad = serde_json::json!({"ops": [{"op": "set", "value": "x", "when": {"livery": "nope"}}]});
        assert!(render_ops(&mut bad, &m, &p).is_err());
    }
}
