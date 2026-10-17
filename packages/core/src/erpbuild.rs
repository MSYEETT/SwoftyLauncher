use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use swofty_formats::erp::{Erp, ErpFile};
use swofty_formats::pkg::{Node, Pkg};

use crate::asset::Source;
use crate::manifest::relative_inside;
use crate::ops::{ErpOp, PkgTarget};
use crate::pkgedit::{apply_edits, edit_name, sources, supported};
use crate::{Error, Result, io_err};

type SourceKey = (String, String, usize);

pub struct ErpBuild<'a> {
    original: &'a dyn Fn(&str) -> PathBuf,
    erps: BTreeMap<String, Erp>,
    sources: HashMap<SourceKey, Node>,
    archives: HashMap<PathBuf, ErpFile>,
}

fn fail(msg: impl Into<String>) -> Error {
    Error::Resolve(msg.into())
}

fn key(t: &PkgTarget) -> SourceKey {
    (t.erp.clone(), t.resource.clone(), t.fragment)
}

fn checked(rel: &str) -> Result<&str> {
    if relative_inside(rel) {
        Ok(rel)
    } else {
        Err(fail(format!(
            "{rel} must be a relative path inside the game folder"
        )))
    }
}

fn op_name(op: &ErpOp) -> &'static str {
    match op {
        ErpOp::ReplaceResource { .. } => "replace-resource",
        ErpOp::AddResource { .. } => "add-resource",
        ErpOp::CopyResource { .. } => "copy-resource",
        ErpOp::PutResource { .. } => "put-resource",
        ErpOp::RemoveResource { .. } => "remove-resource",
        ErpOp::AddMipmaps { .. } => "add-mipmaps",
        ErpOp::PatchPkg { .. } => "patch-pkg",
    }
}

pub fn unsupported(op: &ErpOp) -> Option<&'static str> {
    match op {
        ErpOp::PatchPkg { edits, .. } => edits.iter().find(|e| !supported(e)).map(edit_name),
        ErpOp::CopyResource { .. } => None,
        other => Some(op_name(other)),
    }
}

pub fn source_archive(op: &ErpOp) -> Option<&Source> {
    match op {
        ErpOp::CopyResource { source: Some(s), .. } => Some(s),
        _ => None,
    }
}

fn read_resource(path: &Path, t: &PkgTarget) -> Result<Node> {
    let found = Erp::read_matching(path, |n| n == t.resource).map_err(|e| fail(format!("{}: {e}", t.erp)))?;
    let [res] = found.as_slice() else {
        return Err(fail(format!(
            "{}: {} found {} times",
            t.erp,
            t.resource,
            found.len()
        )));
    };
    let frag = res
        .fragments
        .get(t.fragment)
        .ok_or_else(|| fail(format!("{}: no fragment {}", t.resource, t.fragment)))?;
    Ok(Pkg::parse(&frag.data()?)?.root)
}

impl<'a> ErpBuild<'a> {
    pub fn new(original: &'a dyn Fn(&str) -> PathBuf) -> Self {
        Self {
            original,
            erps: BTreeMap::new(),
            sources: HashMap::new(),
            archives: HashMap::new(),
        }
    }

    fn erp(&mut self, rel: &str) -> Result<&mut Erp> {
        if !self.erps.contains_key(rel) {
            let path = (self.original)(checked(rel)?);
            let data = std::fs::read(&path).map_err(io_err(&path))?;
            let erp = Erp::parse(&data).map_err(|e| fail(format!("{rel}: {e}")))?;
            self.erps.insert(rel.to_owned(), erp);
        }
        Ok(self.erps.get_mut(rel).expect("inserted above"))
    }

    pub fn apply(&mut self, op: &ErpOp, resolve: &dyn Fn(&Source) -> Result<PathBuf>) -> Result<()> {
        if let Some(name) = unsupported(op) {
            return Err(fail(format!("{name} is not built yet")));
        }
        match op {
            ErpOp::PatchPkg {
                erp,
                resource,
                fragment,
                edits,
            } => {
                if edits.is_empty() {
                    return Ok(());
                }
                for t in sources(edits) {
                    if !self.sources.contains_key(&key(t)) {
                        let node = read_resource(&(self.original)(checked(&t.erp)?), t)?;
                        self.sources.insert(key(t), node);
                    }
                }
                let srcs = std::mem::take(&mut self.sources);
                let result = self.patch(erp, resource, *fragment, |root| {
                    apply_edits(root, edits, &|t| srcs.get(&key(t)))
                });
                self.sources = srcs;
                result
            }
            ErpOp::CopyResource {
                from_erp,
                source,
                erp,
                resource,
                rename,
            } => {
                let path = match (from_erp, source) {
                    (Some(rel), None) => (self.original)(checked(rel)?),
                    (None, Some(src)) => resolve(src)?,
                    _ => {
                        return Err(fail(format!(
                            "copy-resource {resource}: give exactly one of from_erp and source"
                        )));
                    }
                };
                self.copy(&path, resource, erp, rename.as_deref().unwrap_or(resource))
            }
            _ => unreachable!("checked by unsupported"),
        }
    }

    fn copy(&mut self, from: &Path, resource: &str, erp: &str, name: &str) -> Result<()> {
        if !self.archives.contains_key(from) {
            let file = ErpFile::open(from).map_err(|e| fail(format!("{}: {e}", from.display())))?;
            self.archives.insert(from.to_path_buf(), file);
        }
        let file = &self.archives[from];
        let found = file
            .erp
            .find(resource)
            .map_err(|e| fail(format!("{}: {e}", from.display())))?;
        let mut res = file.load(found)?;
        res.name = name.to_owned();
        let target = self.erp(erp)?;
        if target.resources.iter().any(|r| r.name == name) {
            return Err(fail(format!("{erp}: {name} already exists")));
        }
        target.resources.push(res);
        Ok(())
    }

    fn patch(
        &mut self,
        erp: &str,
        resource: &str,
        fragment: usize,
        edit: impl FnOnce(&mut Node) -> Result<usize>,
    ) -> Result<()> {
        let e = self.erp(erp)?;
        let res = e
            .find_mut(resource)
            .map_err(|err| fail(format!("{erp}: {err}")))?;
        let body = res
            .fragments
            .get(fragment)
            .ok_or_else(|| fail(format!("{resource}: no fragment {fragment}")))?
            .data()?;
        let mut pkg = Pkg::parse(&body).map_err(|err| fail(format!("{resource}: {err}")))?;
        edit(&mut pkg.root).map_err(|err| fail(format!("{resource}: {err}")))?;
        res.set_fragment(fragment, &pkg.to_bytes()?)?;
        Ok(())
    }

    pub fn changed(&self) -> impl Iterator<Item = &str> {
        self.erps.keys().map(String::as_str)
    }

    pub fn resource(&self, erp: &str, resource: &str, fragment: usize) -> Result<Option<Pkg>> {
        let Some(e) = self.erps.get(erp) else {
            return Ok(None);
        };
        let res = e.find(resource).map_err(|err| fail(format!("{erp}: {err}")))?;
        let frag = res
            .fragments
            .get(fragment)
            .ok_or_else(|| fail(format!("{resource}: no fragment {fragment}")))?;
        Ok(Some(Pkg::parse(&frag.data()?)?))
    }

    pub fn write(self, out: &Path) -> Result<Vec<String>> {
        let mut written = Vec::new();
        for (rel, mut erp) in self.erps {
            let path = out.join(&rel);
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(io_err(dir))?;
            }
            std::fs::write(&path, erp.to_bytes()).map_err(io_err(&path))?;
            written.push(rel);
        }
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::{AssetOp, FileKind, OpFile};
    use swofty_formats::erp::{Fragment, HEADER_LEN, MAGIC, Resource, ZSTD, compress};
    use swofty_formats::pkg::{Arr, Obj, Val};

    fn erp_ops(json: &str) -> Vec<ErpOp> {
        let file = OpFile::from_value(serde_json::from_str(json).unwrap(), FileKind::Erp).unwrap();
        file.ops
            .into_iter()
            .filter_map(|op| match op {
                AssetOp::Erp(op) => Some(op),
                _ => None,
            })
            .collect()
    }

    fn apply(b: &mut ErpBuild, ops: &[ErpOp], dir: &Path) -> Result<()> {
        ops.iter()
            .try_for_each(|op| b.apply(op, &|s: &Source| Ok(s.resolve(dir)?)))
    }

    fn layer(names: &[&str]) -> Vec<u8> {
        let items = names
            .iter()
            .map(|n| {
                Node::Obj(Obj::new(vec![
                    (Val::str("name"), Node::Val(Val::str(*n))),
                    (Val::str("id"), Node::Val(Val::u32(1))),
                ]))
            })
            .collect();
        Pkg {
            name: *b"INST",
            root: Node::Obj(Obj::new(vec![(
                Val::str("instances"),
                Node::Arr(Arr::new(items)),
            )])),
            vca: Vec::new(),
        }
        .to_bytes()
        .unwrap()
    }

    fn erp(resources: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(MAGIC);
        header[4..8].copy_from_slice(&4u32.to_le_bytes());
        let resources = resources
            .iter()
            .map(|(name, body)| {
                let packed = compress(body).unwrap();
                let mut res = Resource {
                    name: (*name).into(),
                    kind: *b"WOInstances\0\0\0\0\0",
                    unk: vec![0; 6],
                    fragments: vec![Fragment {
                        name: *b"temp",
                        offset: 0,
                        size: body.len() as u64,
                        flags: 0,
                        compression: ZSTD,
                        packed: packed.len() as u64,
                        raw: packed,
                    }],
                    hash: [0; 16],
                };
                res.hash = res.content_hash().unwrap();
                res
            })
            .collect();
        Erp {
            header,
            version: 4,
            resources,
        }
        .to_bytes()
    }

    #[test]
    fn patches_layers_and_writes_only_changed_archives() {
        let game = std::env::temp_dir().join(format!("swofty-erpbuild-{}", std::process::id()));
        let wep = game.join("tracks/t/wep");
        std::fs::create_dir_all(&wep).unwrap();
        let world = erp(&[
            ("eaid://t-a.layer", layer(&["a$PitBox12"])),
            ("eaid://t-b.layer", layer(&["b$X"])),
        ]);
        std::fs::write(wep.join("t_world.erp"), &world).unwrap();
        std::fs::write(
            wep.join("t_common.erp"),
            erp(&[("eaid://t-c.layer", layer(&["c$GridSlot23"]))]),
        )
        .unwrap();
        let ops = erp_ops(
            r#"{"schema_version":1,"ops":[
            {"op":"patch-pkg","erp":"tracks/t/wep/t_world.erp","resource":"eaid://t-a.layer","edits":[
              {"op":"clone-entry","at":"instances","src":"[name=a$PitBox12]","as":"a$PitBox13","set":{"id":13}},
              {"op":"clone-entry","at":"instances","from":{"erp":"tracks/t/wep/t_common.erp","resource":"eaid://t-c.layer"},
               "src":"[name=c$GridSlot23]","as":"a$GridSlot25","set":{"id":25}}]},
            {"op":"patch-pkg","erp":"tracks/t/wep/t_common.erp","resource":"eaid://t-c.layer","edits":[]},
            {"op":"warn","message":"room for 26 cars"}]}"#,
        );
        let original = |rel: &str| game.join(rel);
        let mut b = ErpBuild::new(&original);
        apply(&mut b, &ops, &game).unwrap();
        assert_eq!(b.changed().collect::<Vec<_>>(), ["tracks/t/wep/t_world.erp"]);
        let out = game.join("out");
        assert_eq!(b.write(&out).unwrap(), ["tracks/t/wep/t_world.erp"]);
        let written = Erp::parse(&std::fs::read(out.join("tracks/t/wep/t_world.erp")).unwrap()).unwrap();
        let res = written.find("eaid://t-a.layer").unwrap();
        assert_eq!(res.content_hash().unwrap(), res.hash);
        let pkg = Pkg::parse(&res.fragments[0].data().unwrap()).unwrap();
        let names: Vec<String> = pkg
            .root
            .as_obj()
            .unwrap()
            .get("instances")
            .unwrap()
            .as_arr()
            .unwrap()
            .items
            .iter()
            .map(|n| {
                n.as_obj()
                    .unwrap()
                    .get("name")
                    .unwrap()
                    .as_val()
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        assert_eq!(names, ["a$PitBox12", "a$PitBox13", "a$GridSlot25"]);
        let untouched = Erp::parse(&world).unwrap();
        assert_eq!(
            written.find("eaid://t-b.layer").unwrap().fragments[0].raw,
            untouched.find("eaid://t-b.layer").unwrap().fragments[0].raw
        );
        let mut bad = ErpBuild::new(&original);
        let replace = erp_ops(
            r#"{"schema_version":1,"ops":[{"op":"replace-resource","erp":"x.erp","resource":"r","source":"s"}]}"#,
        );
        assert_eq!(unsupported(&replace[0]), Some("replace-resource"));
        assert!(apply(&mut bad, &replace, &game).is_err());
        let escape = erp_ops(
            r#"{"schema_version":1,"ops":[{"op":"patch-pkg","erp":"../x.erp","resource":"r","edits":[
            {"op":"set-value","path":"a","value":1}]}]}"#,
        );
        assert!(apply(&mut bad, &escape, &game).is_err());
        std::fs::remove_dir_all(&game).ok();
    }

    #[test]
    fn copies_resources_from_game_and_mod_archives() {
        let root = std::env::temp_dir().join(format!("swofty-erpcopy-{}", std::process::id()));
        let game = root.join("game");
        let addon_dir = root.join("addon");
        std::fs::create_dir_all(game.join("cars")).unwrap();
        std::fs::create_dir_all(addon_dir.join("erp")).unwrap();
        std::fs::write(
            game.join("cars/slot.erp"),
            erp(&[("eaid://slot-a.layer", layer(&["a"]))]),
        )
        .unwrap();
        std::fs::write(
            game.join("cars/stock.erp"),
            erp(&[("eaid://stock-b.layer", layer(&["b"]))]),
        )
        .unwrap();
        let src = erp(&[
            ("eaid://car.wep", layer(&["w"])),
            ("eaid://car-model.layer", layer(&["m"])),
        ]);
        std::fs::write(addon_dir.join("erp/src.erp"), &src).unwrap();
        let ops = erp_ops(
            r#"{"schema_version":1,"ops":[
            {"op":"copy-resource","source":"erp/src.erp","erp":"cars/slot.erp","resource":"eaid://car.wep","as":"eaid://car_alt.wep"},
            {"op":"copy-resource","source":"erp/src.erp","erp":"cars/slot.erp","resource":"eaid://car-model.layer"},
            {"op":"copy-resource","from_erp":"cars/stock.erp","erp":"cars/slot.erp","resource":"eaid://stock-b.layer"},
            {"op":"patch-pkg","erp":"cars/slot.erp","resource":"eaid://car_alt.wep","edits":[
              {"op":"set-value","path":"instances[0].name","value":"w_alt"}]}]}"#,
        );
        assert!(ops.iter().all(|op| unsupported(op).is_none()));
        assert_eq!(
            ops.iter()
                .filter_map(source_archive)
                .map(|s| s.path.as_str())
                .collect::<Vec<_>>(),
            ["erp/src.erp", "erp/src.erp"]
        );
        let original = |rel: &str| game.join(rel);
        let mut b = ErpBuild::new(&original);
        apply(&mut b, &ops, &addon_dir).unwrap();
        let out = root.join("out");
        assert_eq!(b.write(&out).unwrap(), ["cars/slot.erp"]);
        let written = Erp::parse(&std::fs::read(out.join("cars/slot.erp")).unwrap()).unwrap();
        let names: Vec<&str> = written.resources.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "eaid://slot-a.layer",
                "eaid://car_alt.wep",
                "eaid://car-model.layer",
                "eaid://stock-b.layer"
            ]
        );
        let source = Erp::parse(&src).unwrap();
        let copied = written.find("eaid://car-model.layer").unwrap();
        assert_eq!(
            copied.fragments[0].raw,
            source.find("eaid://car-model.layer").unwrap().fragments[0].raw
        );
        assert_eq!(copied.hash, copied.content_hash().unwrap());
        let wep = written.find("eaid://car_alt.wep").unwrap();
        assert_eq!(wep.hash, wep.content_hash().unwrap());
        let pkg = Pkg::parse(&wep.fragments[0].data().unwrap()).unwrap();
        let name = pkg
            .root
            .as_obj()
            .unwrap()
            .get("instances")
            .unwrap()
            .as_arr()
            .unwrap()
            .items[0]
            .as_obj()
            .unwrap()
            .get("name")
            .unwrap()
            .as_val()
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(name, "w_alt");
        let twice = erp_ops(
            r#"{"schema_version":1,"ops":[
            {"op":"copy-resource","source":"erp/src.erp","erp":"cars/slot.erp","resource":"eaid://car.wep"},
            {"op":"copy-resource","source":"erp/src.erp","erp":"cars/slot.erp","resource":"eaid://car.wep"}]}"#,
        );
        assert!(apply(&mut ErpBuild::new(&original), &twice, &addon_dir).is_err());
        let both = erp_ops(
            r#"{"schema_version":1,"ops":[
            {"op":"copy-resource","source":"erp/src.erp","from_erp":"cars/stock.erp","erp":"cars/slot.erp","resource":"eaid://car.wep"}]}"#,
        );
        assert!(apply(&mut ErpBuild::new(&original), &both, &addon_dir).is_err());
        let escape = erp_ops(
            r#"{"schema_version":1,"ops":[
            {"op":"copy-resource","source":"../game/cars/stock.erp","erp":"cars/slot.erp","resource":"eaid://stock-b.layer"}]}"#,
        );
        assert!(apply(&mut ErpBuild::new(&original), &escape, &addon_dir).is_err());
        std::fs::remove_dir_all(&root).ok();
    }
}
