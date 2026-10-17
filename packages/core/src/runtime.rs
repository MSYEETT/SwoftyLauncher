use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::Serialize;
use swofty_formats::erp::Resource;
use swofty_formats::gvdb::Gvdb;
use swofty_formats::lng::Lng;

use crate::asset::{AssetOp, FileOp, GroupOp, Kind, LangOp, Op, Source};
use crate::manifest::relative_inside;
use crate::{Error, Result, io_err};

pub const ASSETS_FILE: &str = "swofty/assets.json";
const SWOFTY: &str = "swofty";
const RESOURCES_DIR: &str = "assets/resources";
const LANG_DIR: &str = "assets/lang";
const OVERLAY_DIR: &str = "overlay";
const NO_VALUE_TAGS: [u8; 3] = [10, 15, 16];

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Assets {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub early_stage: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<Kind>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub asset_groups: Vec<Group>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<Res>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub db_edits: Vec<DbEdit>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub lang: BTreeMap<String, LangTable>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub overlay: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Group {
    pub group: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub erps: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    pub origin: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Res {
    pub uri: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arg: Option<u32>,
    pub fragments: Vec<Fragment>,
    pub erps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fragment {
    pub file: String,
    pub fourcc: String,
    pub flags: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DbEdit {
    pub hash: String,
    #[serde(rename = "type")]
    pub tag: u8,
    pub value: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub retype: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LangTable {
    pub table: String,
}

#[derive(Debug, Default)]
pub struct Runtime {
    pub assets: Assets,
    pub files: BTreeMap<String, PathBuf>,
    tables: BTreeMap<String, Lng>,
}

fn lang_file(language: &str) -> String {
    match language {
        "*" => format!("{LANG_DIR}/all.lng"),
        code => format!("{LANG_DIR}/{code}.lng"),
    }
}

pub fn db_edits(base: &Gvdb, built: &Gvdb) -> std::result::Result<Vec<DbEdit>, String> {
    let stock: HashMap<u64, _> = base.records.iter().map(|r| (r.key, r)).collect();
    let kept: std::collections::HashSet<u64> = built.records.iter().map(|r| r.key).collect();
    let removed = stock.keys().filter(|k| !kept.contains(k)).count();
    if removed > 0 {
        return Err(format!(
            "{removed} record(s) are removed, which runtime database edits cannot do"
        ));
    }
    let mut edits = Vec::new();
    for r in &built.records {
        let before = stock.get(&r.key);
        if before.is_some_and(|b| *b == r) {
            continue;
        }
        if NO_VALUE_TAGS.contains(&r.tag) {
            return Err(format!(
                "record {:#018x} has tag {}, which stores nothing",
                r.key, r.tag
            ));
        }
        let value = String::from_utf8(r.value.clone())
            .map_err(|_| format!("record {:#018x} does not hold text", r.key))?;
        edits.push(DbEdit {
            hash: format!("{:016x}", r.key),
            tag: r.tag,
            value,
            retype: before.is_some_and(|b| b.tag != r.tag),
        });
    }
    Ok(edits)
}

impl Runtime {
    pub fn new(features: Vec<Kind>) -> Self {
        Self {
            assets: Assets {
                features,
                ..Assets::default()
            },
            ..Self::default()
        }
    }

    fn place(&mut self, rel: String, src: PathBuf) -> Result<()> {
        let rel = format!("{SWOFTY}/{rel}");
        if self.files.insert(rel.clone(), src).is_some() {
            return Err(Error::Other(format!("{rel} is placed twice")));
        }
        Ok(())
    }

    pub fn push(&mut self, op: Op, resolve: &dyn Fn(&Op, &Source) -> Result<PathBuf>) -> Result<()> {
        match &op.op {
            AssetOp::Group(GroupOp::Register {
                group,
                erp,
                erps,
                deps,
                template,
            }) => self.assets.asset_groups.push(Group {
                group: group.clone(),
                erps: erp.iter().chain(erps).cloned().collect(),
                deps: deps.clone(),
                template: template.clone(),
                origin: op.origin.to_string(),
            }),
            AssetOp::Lang(LangOp::Set { language, key, value }) => {
                self.tables
                    .entry(language.clone())
                    .or_default()
                    .set(key, value.clone());
            }
            AssetOp::Lang(LangOp::Table { language, source }) => {
                let path = resolve(&op, source)?;
                let bytes = std::fs::read(&path).map_err(io_err(&path))?;
                let read =
                    Lng::parse(&bytes).map_err(|e| Error::Other(format!("{}: {e}", path.display())))?;
                let table = self.tables.entry(language.clone()).or_default();
                for e in read.entries {
                    table.set(&e.key, e.value);
                }
            }
            AssetOp::File(FileOp::Put { source, target }) => {
                if !relative_inside(target) {
                    return Err(Error::Resolve(format!(
                        "{target} must be a relative path inside the game folder"
                    )));
                }
                let src = resolve(&op, source)?;
                let rel = format!("{OVERLAY_DIR}/{target}");
                self.files.remove(&format!("{SWOFTY}/{rel}"));
                self.place(rel, src)?;
                self.assets.overlay = true;
            }
            AssetOp::Erp(_) | AssetOp::Db(_) | AssetOp::Track(_) | AssetOp::Warn { .. } => {}
        }
        Ok(())
    }

    pub fn resources<'r>(
        &mut self,
        resources: impl IntoIterator<Item = (&'r str, &'r Resource)>,
        out: &Path,
    ) -> Result<()> {
        for (erp, res) in resources {
            let n = self.assets.resources.len();
            let mut fragments = Vec::new();
            for (i, f) in res.fragments.iter().enumerate() {
                let rel = format!("{RESOURCES_DIR}/{n}.{i}.bin");
                let path = out.join(SWOFTY).join(&rel);
                let dir = path.parent().expect("has parent");
                std::fs::create_dir_all(dir).map_err(io_err(dir))?;
                std::fs::write(&path, f.data()?).map_err(io_err(&path))?;
                self.place(rel.clone(), path)?;
                fragments.push(Fragment {
                    file: rel,
                    fourcc: f.name(),
                    flags: f.flags,
                });
            }
            self.assets.resources.push(Res {
                uri: res.name.clone(),
                kind: res.kind(),
                arg: res
                    .unk
                    .get(..4)
                    .map(|b| u32::from_le_bytes(b.try_into().expect("four bytes"))),
                fragments,
                erps: vec![erp.to_owned()],
            });
        }
        Ok(())
    }

    pub fn db(&mut self, edits: Vec<DbEdit>) {
        self.assets.early_stage |= !edits.is_empty();
        self.assets.db_edits.extend(edits);
    }

    pub fn summary(&self) -> String {
        let a = &self.assets;
        let overlay = self
            .files
            .keys()
            .filter(|k| k.starts_with(&format!("{SWOFTY}/{OVERLAY_DIR}/")))
            .count();
        format!(
            "runtime: {ASSETS_FILE} with {} asset group(s), {} resource(s), {} database edit(s), {} language table(s), {overlay} overlay file(s)",
            a.asset_groups.len(),
            a.resources.len(),
            a.db_edits.len(),
            a.lang.len(),
        )
    }

    pub fn finish(mut self, out: &Path) -> Result<Option<Self>> {
        for (language, table) in std::mem::take(&mut self.tables) {
            let rel = lang_file(&language);
            let path = out.join(SWOFTY).join(&rel);
            let dir = path.parent().expect("has parent");
            std::fs::create_dir_all(dir).map_err(io_err(dir))?;
            std::fs::write(&path, table.to_bytes()).map_err(io_err(&path))?;
            self.place(rel.clone(), path)?;
            self.assets.lang.insert(language, LangTable { table: rel });
        }
        let empty = Assets {
            features: self.assets.features.clone(),
            ..Assets::default()
        };
        if self.assets == empty {
            return Ok(None);
        }
        let path = out.join(ASSETS_FILE);
        crate::write_json(&path, &self.assets)?;
        self.files.insert(ASSETS_FILE.into(), path);
        Ok(Some(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::Origin;
    use swofty_formats::erp::{Fragment as ErpFragment, STORED};
    use swofty_formats::gvdb::Record;

    fn op(addon: &str, json: &str) -> Op {
        Op {
            op: serde_json::from_str(json).unwrap(),
            origin: Origin {
                addon: addon.into(),
                item: None,
            },
        }
    }

    fn fragment(name: &[u8; 4], flags: u32, data: &[u8]) -> ErpFragment {
        ErpFragment {
            name: *name,
            offset: 0,
            size: data.len() as u64,
            flags,
            compression: STORED,
            packed: data.len() as u64,
            raw: data.to_vec(),
        }
    }

    #[test]
    fn writes_assets_json_in_the_core_schema() {
        let dir = std::env::temp_dir().join(format!("swofty-runtime-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let addon = dir.join("addon");
        std::fs::create_dir_all(&addon).unwrap();
        let mut table = Lng::default();
        table.set("lng_a", "from table");
        table.set("lng_b", "kept");
        std::fs::write(addon.join("eng.lng"), table.to_bytes()).unwrap();
        std::fs::write(addon.join("one.dds"), b"one").unwrap();
        std::fs::write(addon.join("two.dds"), b"two").unwrap();
        let resolve = |_: &Op, s: &Source| Ok(s.resolve(&addon)?);
        let mut rt = Runtime::new(vec![Kind::File]);
        for (id, json) in [
            (
                "a",
                r#"{"op":"group.register","group":"asset_groups/x/y.assetgroup","erp":"2025_asset_groups/x/y.erp","deps":["asset_groups/z.assetgroup"]}"#,
            ),
            ("a", r#"{"op":"lang.table","language":"eng","source":"eng.lng"}"#),
            (
                "b",
                r#"{"op":"lang.set","language":"eng","key":"lng_a","value":"from set"}"#,
            ),
            (
                "b",
                r#"{"op":"lang.set","language":"*","key":"lng_c","value":"every language"}"#,
            ),
            (
                "a",
                r#"{"op":"file.put","source":"one.dds","target":"livery/s.dds"}"#,
            ),
            (
                "b",
                r#"{"op":"file.put","source":"two.dds","target":"livery/s.dds"}"#,
            ),
        ] {
            rt.push(op(id, json), &resolve).unwrap();
        }
        let mut kind = [0; 16];
        kind[..13].copy_from_slice(b"GfxSurfaceRes");
        let res = Resource {
            name: "eaid://ui_package/x.tif.image".into(),
            kind,
            unk: 1u32.to_le_bytes().to_vec(),
            fragments: vec![fragment(b"temp", 16, b"HEAD"), fragment(b"pixl", 0, b"PIXELS")],
            hash: [0; 16],
        };
        let out = dir.join("overlay");
        rt.resources([("2025_asset_groups/ui_package/boot.erp", &res)], &out)
            .unwrap();
        let mut base = Gvdb::default();
        base.records.push(Record::new("a.int", 0, "1"));
        let mut built = base.clone();
        built.set("a.int", "2", None).unwrap();
        built.set("a.new", "hello", Some(2)).unwrap();
        rt.db(db_edits(&base, &built).unwrap());
        let rt = rt.finish(&out).unwrap().unwrap();
        let written: serde_json::Value =
            serde_json::from_slice(&std::fs::read(out.join(ASSETS_FILE)).unwrap()).unwrap();
        let hash = |p: &str| format!("{:016x}", swofty_formats::hash::fnv1a64(p));
        assert_eq!(
            written,
            serde_json::json!({
                "early_stage": true,
                "features": ["file"],
                "asset_groups": [{
                    "group": "asset_groups/x/y.assetgroup",
                    "erps": ["2025_asset_groups/x/y.erp"],
                    "deps": ["asset_groups/z.assetgroup"],
                    "origin": "a",
                }],
                "resources": [{
                    "uri": "eaid://ui_package/x.tif.image",
                    "type": "GfxSurfaceRes",
                    "arg": 1,
                    "fragments": [
                        { "file": "assets/resources/0.0.bin", "fourcc": "temp", "flags": 16 },
                        { "file": "assets/resources/0.1.bin", "fourcc": "pixl", "flags": 0 },
                    ],
                    "erps": ["2025_asset_groups/ui_package/boot.erp"],
                }],
                "db_edits": [
                    { "hash": hash("a.int"), "type": 0, "value": "2" },
                    { "hash": hash("a.new"), "type": 2, "value": "hello" },
                ],
                "lang": {
                    "*": { "table": "assets/lang/all.lng" },
                    "eng": { "table": "assets/lang/eng.lng" },
                },
                "overlay": true,
            })
        );
        let eng = Lng::parse(&std::fs::read(out.join("swofty/assets/lang/eng.lng")).unwrap()).unwrap();
        assert_eq!(eng.get("lng_a"), Some("from set"));
        assert_eq!(eng.get("lng_b"), Some("kept"));
        assert_eq!(
            std::fs::read(out.join("swofty/assets/resources/0.1.bin")).unwrap(),
            b"PIXELS"
        );
        let placed: Vec<&str> = rt.files.keys().map(String::as_str).collect();
        assert_eq!(
            placed,
            [
                "swofty/assets.json",
                "swofty/assets/lang/all.lng",
                "swofty/assets/lang/eng.lng",
                "swofty/assets/resources/0.0.bin",
                "swofty/assets/resources/0.1.bin",
                "swofty/overlay/livery/s.dds",
            ]
        );
        assert_eq!(rt.files["swofty/overlay/livery/s.dds"], addon.join("two.dds"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn database_edits_retype_and_refuse_removals() {
        let mut base = Gvdb::default();
        base.records.push(Record::new("a.x", 0, "1"));
        base.records.push(Record::new("a.y", 0, "1"));
        let mut built = base.clone();
        built.set("a.x", "1.5", Some(1)).unwrap();
        let edits = db_edits(&base, &built).unwrap();
        assert_eq!(edits.len(), 1);
        assert!(edits[0].retype);
        built.remove("a.y");
        assert!(
            db_edits(&base, &built)
                .unwrap_err()
                .contains("1 record(s) are removed")
        );
        let mut empty = Runtime::new(vec![Kind::File]);
        empty.db(Vec::new());
        assert!(empty.finish(Path::new("/nonexistent")).unwrap().is_none());
    }
}
