use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use serde_json::Value;
use swofty_core::manifest::{AddonManifest, MANIFEST_FILE};
use swofty_core::store::Store;

use super::lock::LOCK_FILE;

pub const REGISTRY_FILE: &str = "registry.json";

#[derive(Debug, Default)]
pub struct Report {
    pub lines: Vec<String>,
    pub errors: usize,
    pub warnings: usize,
}

impl Report {
    fn error(&mut self, s: impl Into<String>) {
        self.lines.push(format!("error: {}", s.into()));
        self.errors += 1;
    }

    fn warn(&mut self, s: impl Into<String>) {
        self.lines.push(format!("warning: {}", s.into()));
        self.warnings += 1;
    }

    fn ok(&mut self, s: impl Into<String>) {
        self.lines.push(format!("ok: {}", s.into()));
    }
}

const REGISTRY_KEYS: &[&str] = &[
    "requires",
    "recommends",
    "breaks",
    "conflicts",
    "aliases",
    "saves",
    "guards",
];
const NEED_KINDS: &[&str] = &["registry", "service", "hook", "record"];

pub fn check(store: &Store, sdk: &Option<PathBuf>, folder: &Path) -> Result<Report> {
    let mut r = Report::default();
    let id = match AddonManifest::load(folder) {
        Ok(m) => {
            r.ok(format!("{MANIFEST_FILE}: {} {}", m.id, m.version));
            m.id
        }
        Err(e) => {
            r.error(format!("{MANIFEST_FILE}: {e}"));
            return Ok(r);
        }
    };
    let manifest = super::manifest_json(folder)?;
    let mut needs: Vec<(String, bool, String)> = Vec::new();
    match read_json(&folder.join(REGISTRY_FILE)) {
        None => {}
        Some(Err(e)) => r.error(format!("{REGISTRY_FILE}: {e}")),
        Some(Ok(doc)) => registry_json(&doc, &mut r, &mut needs),
    }
    let has_register = match super::register_dlls(folder, &manifest) {
        Ok(dlls) => !dlls.is_empty(),
        Err(e) => {
            r.warn(format!("{e}"));
            false
        }
    };
    let mut own: BTreeSet<String> = BTreeSet::new();
    match read_json(&folder.join(LOCK_FILE)) {
        None if has_register => r.warn(format!(
            "the DLL has a SWOFTY_REGISTER but there's no {LOCK_FILE}: swofty dev pack writes it"
        )),
        None => {}
        Some(Err(e)) => r.error(format!("{LOCK_FILE}: {e}")),
        Some(Ok(lock)) => {
            if !has_register {
                r.warn(format!(
                    "{LOCK_FILE} is there but no DLL has a SWOFTY_REGISTER: swofty dev pack removes it"
                ));
            }
            own.extend(provided(&lock));
            for n in lock["needs"].as_array().into_iter().flatten() {
                if let Some(rec) = n["record"].as_str() {
                    let optional = n["policy"].as_str() == Some("optional");
                    needs.push((rec.to_owned(), !optional, LOCK_FILE.to_owned()));
                }
            }
        }
    }
    let data = data_files(folder);
    own.extend(data_ids(&id, &data));
    if !data.is_empty() {
        validate_data(sdk, folder, &data, &mut r);
    }
    let installed = installed_ids(store, &id);
    for (record, required, from) in needs {
        let Some((registry, rid)) = record.split_once(':') else {
            r.error(format!("{from}: {record:?} isn't <registry>:<id>"));
            continue;
        };
        let key = format!("{registry}:{rid}");
        if rid.starts_with("game/") || own.contains(&key) || installed.contains(&key) {
            continue;
        }
        let msg = format!("{from}: no installed addon provides {key}");
        if required { r.error(msg) } else { r.warn(msg) }
    }
    Ok(r)
}

fn read_json(path: &Path) -> Option<Result<Value, String>> {
    let text = std::fs::read_to_string(path).ok()?;
    Some(serde_json::from_str(&text).map_err(|e| e.to_string()))
}

fn registry_json(doc: &Value, r: &mut Report, needs: &mut Vec<(String, bool, String)>) {
    let Some(obj) = doc.as_object() else {
        r.error(format!("{REGISTRY_FILE}: not a JSON object"));
        return;
    };
    let before = r.errors;
    for k in obj.keys() {
        if !REGISTRY_KEYS.contains(&k.as_str()) {
            r.error(format!("{REGISTRY_FILE}: unknown field {k:?}"));
        }
    }
    for (field, required) in [("requires", true), ("recommends", false)] {
        for (i, e) in doc[field].as_array().into_iter().flatten().enumerate() {
            let at = format!("{REGISTRY_FILE}: {field}[{i}]");
            let kinds: Vec<&&str> = NEED_KINDS.iter().filter(|k| e.get(**k).is_some()).collect();
            if kinds.len() != 1 {
                r.error(format!("{at}: needs one of registry, service, hook or record"));
                continue;
            }
            if !required && !e["for"].is_string() {
                r.warn(format!("{at}: add a \"for\" for players"));
            }
            if let Some(rec) = e["record"].as_str() {
                needs.push((rec.to_owned(), required, REGISTRY_FILE.to_owned()));
            }
        }
        if !doc[field].is_null() && !doc[field].is_array() {
            r.error(format!("{REGISTRY_FILE}: {field} isn't a list"));
        }
    }
    for field in ["breaks", "conflicts", "aliases"] {
        for (i, e) in doc[field].as_array().into_iter().flatten().enumerate() {
            if !e["id"].is_string() {
                r.error(format!("{REGISTRY_FILE}: {field}[{i}] has no id"));
            }
        }
    }
    if r.errors == before {
        r.ok(REGISTRY_FILE);
    }
}

fn provided(lock: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for (registry, list) in lock["provides"].as_object().into_iter().flatten() {
        for p in list.as_array().into_iter().flatten() {
            if let Some(id) = p.as_str().or_else(|| p["id"].as_str()) {
                out.push(format!("{registry}:{id}"));
            }
        }
    }
    out
}

fn data_files(folder: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(folder.join("registry"))
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "json"))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn data_ids(addon: &str, files: &[PathBuf]) -> Vec<String> {
    let mut out = Vec::new();
    for f in files {
        let Some(Ok(doc)) = read_json(f) else { continue };
        for (registry, records) in doc.as_object().into_iter().flatten() {
            for key in records.as_object().into_iter().flatten().map(|(k, _)| k) {
                if key.starts_with('$') {
                    continue;
                }
                let id = if key.contains('/') {
                    key.clone()
                } else {
                    format!("{addon}/{key}")
                };
                if id.starts_with(&format!("{addon}/")) {
                    out.push(format!("{registry}:{id}"));
                }
            }
        }
    }
    out
}

fn installed_ids(store: &Store, except: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Ok(addons) = store.addons() else { return out };
    for id in addons.keys().filter(|id| *id != except) {
        let dir = store.addons_dir().join(id);
        if let Some(Ok(lock)) = read_json(&dir.join(LOCK_FILE)) {
            out.extend(provided(&lock));
        }
        out.extend(data_ids(id, &data_files(&dir)));
    }
    out
}

fn validate_data(sdk: &Option<PathBuf>, folder: &Path, files: &[PathBuf], r: &mut Report) {
    let (Some(sdk), Ok(py)) = (sdk, super::python()) else {
        r.warn("the data files weren't checked: dev check needs python3 and the SDK's tools/std/validate.py");
        return;
    };
    let out = Command::new(py)
        .arg(sdk.join("tools/std/validate.py"))
        .arg("--repo")
        .arg(sdk)
        .arg("--data")
        .args(files)
        .current_dir(folder)
        .output();
    let Ok(out) = out else {
        r.warn("the data files weren't checked: validate.py didn't start");
        return;
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut current_failed = false;
    for line in text.lines() {
        let shown = line.replace(&format!("{}/", folder.display()), "");
        if let Some(rest) = shown.strip_prefix("ok  ") {
            r.ok(rest.trim().to_owned());
            current_failed = false;
        } else if let Some(rest) = shown.strip_prefix("FAIL") {
            r.error(rest.trim().to_owned());
            current_failed = true;
        } else if current_failed && !shown.trim().is_empty() {
            r.lines.push(format!("    {}", shown.trim()));
        }
    }
    if !out.status.success() && r.errors == 0 {
        r.error(format!(
            "validate.py failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("swofty-check-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn addon(dir: &Path, id: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join(MANIFEST_FILE),
            format!(r#"{{"schema_version":1,"id":"{id}","name":"X","version":"1.0.0","author":"t","kind":["native"],"compat":{{"game":"f1_25"}}}}"#),
        )
        .unwrap();
    }

    #[test]
    fn a_broken_manifest_stops_the_check() {
        let root = scratch("manifest");
        std::fs::write(root.join(MANIFEST_FILE), "{").unwrap();
        let r = check(&Store::new(root.join("data")), &None, &root).unwrap();
        assert_eq!(r.errors, 1, "{:?}", r.lines);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn registry_json_fields_and_references() {
        let root = scratch("refs");
        let store = Store::new(root.join("data"));
        let dir = root.join("mod.esp");
        addon(&dir, "mod.esp");
        std::fs::write(
            dir.join(REGISTRY_FILE),
            r#"{
              "requires": [ { "record": "vehicle:swofty.classic-seasons-2017/ferrari" },
                            { "registry": "career.season", "version": "^1" },
                            { "record": "vehicle:game/ferrari_2025" },
                            { "registry": "x", "service": "y" } ],
              "recommends": [ { "record": "track:venue/sochi" } ],
              "breaks": [ { "version": "1" } ],
              "colour": "red"
            }"#,
        )
        .unwrap();
        let r = check(&store, &None, &dir).unwrap();
        let text = r.lines.join("\n");
        assert!(text.contains("unknown field \"colour\""), "{text}");
        assert!(text.contains("requires[3]: needs one of"), "{text}");
        assert!(text.contains("breaks[0] has no id"), "{text}");
        assert!(text.contains("recommends[0]: add a \"for\""), "{text}");
        assert!(text.contains(
            "error: registry.json: no installed addon provides vehicle:swofty.classic-seasons-2017/ferrari"
        ));
        assert!(text.contains("warning: registry.json: no installed addon provides track:venue/sochi"));
        assert!(!text.contains("game/ferrari_2025"));
        assert_eq!((r.errors, r.warnings), (4, 2), "{text}");

        let cs = store.addons_dir().join("swofty.classic-seasons-2017");
        addon(&cs, "swofty.classic-seasons-2017");
        std::fs::write(
            cs.join(LOCK_FILE),
            r#"{"generated_by":"x","provides":{"vehicle":[{"id":"swofty.classic-seasons-2017/ferrari","label":"Ferrari"}]},"needs":[]}"#,
        )
        .unwrap();
        let sochi = store.addons_dir().join("mod.sochi");
        addon(&sochi, "mod.sochi");
        std::fs::create_dir_all(sochi.join("registry")).unwrap();
        std::fs::write(
            sochi.join("registry/tracks.json"),
            r#"{"track":{"venue/sochi":{}}}"#,
        )
        .unwrap();
        let r = check(&store, &None, &dir).unwrap();
        let text = r.lines.join("\n");
        assert!(!text.contains("no installed addon provides vehicle"), "{text}");
        assert!(
            text.contains("track:venue/sochi"),
            "an id outside mod.sochi's own isn't provided by it: {text}"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn lock_needs_and_data_files() {
        let root = scratch("data");
        let store = Store::new(root.join("data"));
        let dir = root.join("mod.esp");
        addon(&dir, "mod.esp");
        std::fs::write(
            dir.join(LOCK_FILE),
            r#"{"generated_by":"x","provides":{"engine_sound":["mod.esp/v10"]},
                "needs":[{"record":"engine_sound:mod.esp/v10","policy":"required"},
                         {"record":"livery:mod.l/red","policy":"optional"}]}"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("registry")).unwrap();
        std::fs::write(
            dir.join("registry/layers.json"),
            r#"{"vehicle":{"swofty.classic-seasons-2017/ferrari":{"engine_sound":"mod.esp/v10","livery":null}}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("registry/bad-layers.json"),
            r#"{"vehicle":{"game/ferrari_2025":{"tyre_wear_scale":"fast"}}}"#,
        )
        .unwrap();
        let sdk = super::super::sdk_root().ok();
        let r = check(&store, &sdk, &dir).unwrap();
        let text = r.lines.join("\n");
        assert!(
            text.contains("warning: registry.lock.json is there but no DLL"),
            "{text}"
        );
        assert!(
            text.contains("warning: registry.lock.json: no installed addon provides livery:mod.l/red"),
            "{text}"
        );
        assert!(!text.contains("engine_sound:mod.esp/v10"), "{text}");
        if super::super::python().is_ok() {
            assert!(text.contains("ok: registry/layers.json"), "{text}");
            assert!(text.contains("error: registry/bad-layers.json"), "{text}");
            assert_eq!(r.errors, 1, "{text}");
        }
        std::fs::remove_dir_all(&root).ok();
    }
}
