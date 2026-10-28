//! `<game>/swofty/registry/index.json`, written by the build so the core
//! finds the addons without code and every addon's data files. The format
//! (`packages/sdk/std/index.schema.json`):
//!
//! ```json
//! {
//!   "version": 1,
//!   "order": ["swofty.core", "swofty.progressive", "mod.esp"],
//!   "data": { "mod.esp": ["addons/mod.esp/registry/layers.json", "addons/mod.esp/registry/sounds.json"] },
//!   "registry": { "mod.esp": "addons/mod.esp/registry.json" },
//!   "addons": { "mod.esp": { "name": "ESP", "version": "1.0.0", "code": false } }
//! }
//! ```
//!
//! - `order`: every addon on in the profile, in load order.
//! - `data`: each addon's data files, by addon in load order and by path
//!   within one, as paths under `<game>/swofty/`. The build places each file
//!   there.
//! - `registry`: each addon's `registry.json`, placed the same way, for the
//!   guards, `saves` and the `service` and `hook` requirements the core
//!   checks in game.
//! - `addons`: the name, version and whether it has code (native DLLs in
//!   its operations) of every addon in `data` or `registry`.
//!
//! Object keys are written in load order; readers take the order from
//! `order`. A profile with no data files and no `registry.json` gets no
//! index, so its build is the same as on 1.5.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::ser::{Serialize, SerializeMap, SerializeStruct, Serializer};

use super::deps::Std;
use crate::build::ADDONS_DIR;
use crate::install::LOADER_DIR;
use crate::manifest::AddonManifest;

pub const VERSION: u32 = 1;
/// The index's path in the game folder.
pub const PATH: &str = "swofty/registry/index.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub code: bool,
    pub data: Vec<String>,
    pub registry: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Index {
    pub order: Vec<String>,
    pub addons: Vec<Entry>,
}

/// The index and the files the build places for it: game path to source path.
pub struct Placed {
    pub index: Index,
    pub files: BTreeMap<String, PathBuf>,
}

fn under_loader(id: &str, rel: &str) -> String {
    format!("{ADDONS_DIR}/{id}/{rel}")
}

impl Index {
    /// The index for the addons `order` (with their 1.6 files in `std`), or `None` when none of
    /// them has a data file or a `registry.json`.
    pub fn plan(
        order: &[String],
        manifests: &BTreeMap<String, AddonManifest>,
        std: &BTreeMap<String, Std>,
        addons_dir: &std::path::Path,
    ) -> Option<Placed> {
        let mut index = Index {
            order: order.to_vec(),
            addons: Vec::new(),
        };
        let mut files = BTreeMap::new();
        for id in order {
            let (Some(s), Some(m)) = (std.get(id), manifests.get(id)) else {
                continue;
            };
            let has_registry = s.registry.is_some();
            if s.data_files.is_empty() && !has_registry {
                continue;
            }
            let dir = addons_dir.join(id);
            let data: Vec<String> = s.data_files.iter().map(|rel| under_loader(id, rel)).collect();
            for rel in &s.data_files {
                files.insert(format!("{LOADER_DIR}/{}", under_loader(id, rel)), dir.join(rel));
            }
            let registry = has_registry.then(|| under_loader(id, super::manifest::FILE));
            if let Some(r) = &registry {
                files.insert(format!("{LOADER_DIR}/{r}"), dir.join(super::manifest::FILE));
            }
            index.addons.push(Entry {
                id: id.clone(),
                name: m.name.clone(),
                version: m.version.to_string(),
                code: !m.operations.native.is_empty(),
                data,
                registry,
            });
        }
        (!index.addons.is_empty()).then_some(Placed { index, files })
    }

    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).expect("serialisable");
        text.push('\n');
        text
    }
}

struct Ordered<'a, F>(&'a [Entry], F);

impl<F, T> Serialize for Ordered<'_, F>
where
    F: Fn(&Entry) -> Option<T>,
    T: Serialize,
{
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        for e in self.0 {
            if let Some(v) = (self.1)(e) {
                map.serialize_entry(&e.id, &v)?;
            }
        }
        map.end()
    }
}

#[derive(serde::Serialize)]
struct About {
    name: String,
    version: String,
    code: bool,
}

impl Serialize for Index {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("Index", 5)?;
        st.serialize_field("version", &VERSION)?;
        st.serialize_field("order", &self.order)?;
        st.serialize_field(
            "data",
            &Ordered(&self.addons, |e: &Entry| {
                (!e.data.is_empty()).then(|| e.data.clone())
            }),
        )?;
        st.serialize_field("registry", &Ordered(&self.addons, |e: &Entry| e.registry.clone()))?;
        st.serialize_field(
            "addons",
            &Ordered(&self.addons, |e: &Entry| {
                Some(About {
                    name: e.name.clone(),
                    version: e.version.clone(),
                    code: e.code,
                })
            }),
        )?;
        st.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(id: &str, native: bool) -> AddonManifest {
        let ops = if native {
            r#""operations": { "native": [ { "dll": "native/x.dll" } ] },"#
        } else {
            ""
        };
        serde_json::from_str(&format!(
            r#"{{ "schema_version": 1, "id": "{id}", "name": "Name {id}", "version": "1.2.0", "author": "a",
                 {ops} "compat": {{ "game": "f1_25" }} }}"#
        ))
        .unwrap()
    }

    fn std_of(m: &AddonManifest, data: &[&str], registry: bool) -> Std {
        let mut s = Std::bare(m);
        s.data_files = data.iter().map(|d| d.to_string()).collect();
        s.registry = registry.then(Default::default);
        s
    }

    #[test]
    fn lists_data_files_and_registry_files_in_load_order() {
        let order: Vec<String> = ["swofty.core", "z.code", "a.data", "m.old"]
            .map(String::from)
            .to_vec();
        let ms: BTreeMap<String, AddonManifest> = [
            ("swofty.core", false),
            ("z.code", true),
            ("a.data", false),
            ("m.old", false),
        ]
        .into_iter()
        .map(|(id, n)| (id.to_owned(), manifest(id, n)))
        .collect();
        let std: BTreeMap<String, Std> = [
            ("swofty.core", std_of(&ms["swofty.core"], &[], false)),
            ("z.code", std_of(&ms["z.code"], &[], true)),
            (
                "a.data",
                std_of(
                    &ms["a.data"],
                    &["registry/layers.json", "registry/sounds.json"],
                    true,
                ),
            ),
            ("m.old", std_of(&ms["m.old"], &[], false)),
        ]
        .into_iter()
        .map(|(id, s)| (id.to_owned(), s))
        .collect();
        let placed = Index::plan(&order, &ms, &std, std::path::Path::new("/store/addons")).unwrap();
        assert_eq!(
            placed.index.to_json(),
            r#"{
  "version": 1,
  "order": [
    "swofty.core",
    "z.code",
    "a.data",
    "m.old"
  ],
  "data": {
    "a.data": [
      "addons/a.data/registry/layers.json",
      "addons/a.data/registry/sounds.json"
    ]
  },
  "registry": {
    "z.code": "addons/z.code/registry.json",
    "a.data": "addons/a.data/registry.json"
  },
  "addons": {
    "z.code": {
      "name": "Name z.code",
      "version": "1.2.0",
      "code": true
    },
    "a.data": {
      "name": "Name a.data",
      "version": "1.2.0",
      "code": false
    }
  }
}
"#
        );
        assert_eq!(
            placed.files.keys().collect::<Vec<_>>(),
            [
                "swofty/addons/a.data/registry.json",
                "swofty/addons/a.data/registry/layers.json",
                "swofty/addons/a.data/registry/sounds.json",
                "swofty/addons/z.code/registry.json",
            ]
        );
        assert_eq!(
            placed.files["swofty/addons/a.data/registry/sounds.json"],
            std::path::Path::new("/store/addons/a.data/registry/sounds.json")
        );
    }

    #[test]
    fn a_profile_without_std_files_gets_no_index() {
        let order: Vec<String> = vec!["swofty.core".into(), "m.old".into()];
        let ms: BTreeMap<String, AddonManifest> = ["swofty.core", "m.old"]
            .map(|id| (id.to_owned(), manifest(id, false)))
            .into_iter()
            .collect();
        let std: BTreeMap<String, Std> = ms.iter().map(|(id, m)| (id.clone(), Std::bare(m))).collect();
        assert!(Index::plan(&order, &ms, &std, std::path::Path::new("/s")).is_none());
    }

    #[test]
    fn matches_its_schema_file() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../../sdk/std/index.schema.json")).unwrap();
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(required, ["version", "order", "data"]);
        let props = schema["properties"].as_object().unwrap();
        for key in ["version", "order", "data", "registry", "addons"] {
            assert!(props.contains_key(key), "{key}");
        }
        assert_eq!(schema["properties"]["version"]["const"], VERSION);
    }
}
