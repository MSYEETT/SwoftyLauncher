use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

pub const SELF_ID: &str = "swofty.driver-manager";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Supplied {
    pub file: PathBuf,
    pub addon: String,
}

// Game files the profile's addons put into the game folder, keyed by their
// path in the game folder, a later addon winning as in a build.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Files {
    pub game_dir: Option<PathBuf>,
    pub supplied: BTreeMap<String, Supplied>,
}

pub fn rel_key(rel: &str) -> String {
    rel.replace('\\', "/")
        .trim_start_matches('/')
        .to_ascii_lowercase()
}

fn inside(rel: &str) -> bool {
    let rel = rel.replace('\\', "/");
    !rel.is_empty()
        && !rel.starts_with('/')
        && !rel.contains(':')
        && rel.split('/').all(|p| !p.is_empty() && p != "..")
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn source_path(source: &Value) -> Option<String> {
    match source {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) if o.get("root").and_then(Value::as_str) == Some("addon") => {
            o.get("path").and_then(Value::as_str).map(str::to_owned)
        }
        _ => None,
    }
}

pub fn puts(dir: &Path) -> Vec<(String, String)> {
    let Some(manifest) = read_json(&dir.join("swofty.json")) else {
        return Vec::new();
    };
    let ops = &manifest["operations"];
    let mut out: Vec<(String, String)> = ops["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| Some((f["target"].as_str()?.to_owned(), f["source"].as_str()?.to_owned())))
        .collect();
    for list in ["assets", "erp"] {
        for rel in ops[list]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !inside(rel) {
                continue;
            }
            let Some(file) = read_json(&dir.join(rel)) else {
                continue;
            };
            for op in file["ops"].as_array().into_iter().flatten() {
                if op["op"] != "file.put" {
                    continue;
                }
                if let (Some(target), Some(source)) = (op["target"].as_str(), source_path(&op["source"])) {
                    out.push((target.to_owned(), source));
                }
            }
        }
    }
    out.retain(|(target, source)| inside(target) && inside(source));
    out
}

impl Files {
    pub fn new(game_dir: Option<PathBuf>, addons: &Value) -> Files {
        let mut supplied = BTreeMap::new();
        for a in addons.as_array().into_iter().flatten() {
            if !a["enabled"].as_bool().unwrap_or(false) || a["id"] == SELF_ID {
                continue;
            }
            let Some(dir) = a["dir"].as_str().map(PathBuf::from) else {
                continue;
            };
            let addon = a["name"]
                .as_str()
                .or(a["id"].as_str())
                .unwrap_or_default()
                .to_owned();
            for (target, source) in puts(&dir) {
                supplied.insert(
                    rel_key(&target),
                    Supplied {
                        file: dir.join(&source),
                        addon: addon.clone(),
                    },
                );
            }
        }
        Files { game_dir, supplied }
    }

    pub fn addon(&self, rel: &str) -> Option<&Supplied> {
        self.supplied.get(&rel_key(rel)).filter(|s| s.file.is_file())
    }

    pub fn find(&self, rel: &str) -> Option<PathBuf> {
        if let Some(s) = self.addon(rel) {
            return Some(s.file.clone());
        }
        let file = self.game_dir.as_ref()?.join(rel);
        file.is_file().then_some(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_file_puts_from_manifests_and_op_files() {
        let dir = std::env::temp_dir().join(format!("dm-assets-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("ops")).unwrap();
        std::fs::create_dir_all(dir.join("files/heads")).unwrap();
        std::fs::write(dir.join("files/heads/daniil_kvyat.erp"), "erp").unwrap();
        std::fs::write(
            dir.join("swofty.json"),
            r#"{"operations":{"files":[{"source":"files/x.dds","target":"x/x.dds"}],"assets":["ops/files.json","../escape.json"],"database":["ops/database.json"]}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("ops/files.json"),
            r#"{"ops":[{"op":"file.put","source":"files/heads/daniil_kvyat.erp","target":"2025_asset_groups/Animation_Package/heads/daniil_kvyat.erp"},{"op":"file.put","source":{"root":"data","path":"y"},"target":"y"},{"op":"file.put","source":"../out","target":"z"},{"op":"overlay","erps":["a.erp"],"source":"erp/overlay.erp"}]}"#,
        )
        .unwrap();
        let mut found = puts(&dir);
        found.sort();
        assert_eq!(
            found,
            [
                (
                    "2025_asset_groups/Animation_Package/heads/daniil_kvyat.erp".to_owned(),
                    "files/heads/daniil_kvyat.erp".to_owned()
                ),
                ("x/x.dds".to_owned(), "files/x.dds".to_owned()),
            ]
        );
        let addons = serde_json::json!([
            { "id": "mod.more-driver-models", "name": "More Driver Models", "dir": dir, "enabled": true },
            { "id": "mod.off", "name": "Off", "dir": "/nowhere", "enabled": false },
        ]);
        let files = Files::new(None, &addons);
        let rel = "2025_asset_groups/animation_package/heads/daniil_kvyat.erp";
        assert_eq!(files.addon(rel).unwrap().addon, "More Driver Models");
        assert_eq!(files.find(rel), Some(dir.join("files/heads/daniil_kvyat.erp")));
        assert!(files.find("x/x.dds").is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
