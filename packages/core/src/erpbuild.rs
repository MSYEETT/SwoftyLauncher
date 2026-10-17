use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use swofty_formats::erp::{self, Entry, Erp, ErpFile, Resource};
use swofty_formats::pkg::{Node, Pkg};

use crate::asset::Source;
use crate::builtin::loader_files;
use crate::manifest::relative_inside;
use crate::ops::{ErpOp, PkgTarget};
use crate::pkgedit::{apply_edits, edit_name, sources, supported};
use crate::{Error, Result};

type SourceKey = (String, String, usize);

enum Slot {
    Stock(usize),
    Owned(Resource),
}

struct Archive {
    file: ErpFile,
    slots: Vec<Slot>,
}

impl Archive {
    fn entry<'s>(&'s self, slot: &'s Slot) -> &'s Resource {
        match slot {
            Slot::Stock(i) => &self.file.erp.resources[*i],
            Slot::Owned(r) => r,
        }
    }

    fn name<'s>(&'s self, slot: &'s Slot) -> &'s str {
        &self.entry(slot).name
    }

    fn position(&self, name: &str) -> Result<Option<usize>> {
        let mut hits = (0..self.slots.len()).filter(|&i| self.name(&self.slots[i]) == name);
        match (hits.next(), hits.next()) {
            (Some(_), Some(_)) => Err(fail(format!("{name}: more than one resource"))),
            (hit, _) => Ok(hit),
        }
    }

    fn find(&self, name: &str) -> Result<usize> {
        self.position(name)?
            .ok_or_else(|| fail(format!("{name}: not found")))
    }

    fn owned(&mut self, i: usize) -> Result<&mut Resource> {
        if let Slot::Stock(j) = self.slots[i] {
            self.slots[i] = Slot::Owned(self.file.load(&self.file.erp.resources[j])?);
        }
        match &mut self.slots[i] {
            Slot::Owned(r) => Ok(r),
            Slot::Stock(_) => unreachable!("loaded above"),
        }
    }
}

pub struct ErpBuild<'a> {
    original: &'a dyn Fn(&str) -> PathBuf,
    erps: BTreeMap<String, Archive>,
    sources: HashMap<SourceKey, Node>,
    archives: HashMap<PathBuf, ErpFile>,
    mipmaps: BTreeMap<String, PathBuf>,
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

pub fn unsupported(op: &ErpOp) -> Option<&'static str> {
    match op {
        ErpOp::PatchPkg { edits, .. } => edits.iter().find(|e| !supported(e)).map(edit_name),
        _ => None,
    }
}

fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(crate::io_err(path))
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
            mipmaps: BTreeMap::new(),
        }
    }

    fn erp(&mut self, rel: &str) -> Result<&mut Archive> {
        if !self.erps.contains_key(rel) {
            let path = (self.original)(checked(rel)?);
            let file = ErpFile::open(&path).map_err(|e| fail(format!("{rel}: {e}")))?;
            let slots = (0..file.erp.resources.len()).map(Slot::Stock).collect();
            self.erps.insert(rel.to_owned(), Archive { file, slots });
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
                self.copy(&path, resource, erp, rename.as_deref().unwrap_or(resource), false)
            }
            ErpOp::PutResource {
                erp,
                resource,
                source,
                rename,
            } => {
                let path = resolve(source)?;
                self.copy(&path, resource, erp, rename.as_deref().unwrap_or(resource), true)
            }
            ErpOp::ReplaceResource {
                erp,
                resource,
                fragment,
                source,
            } => {
                let data = read(&resolve(source)?)?;
                let e = self.erp(erp)?;
                let i = e.find(resource).map_err(|err| fail(format!("{erp}: {err}")))?;
                e.owned(i)?
                    .set_fragment(*fragment, &data)
                    .map_err(|err| fail(format!("{erp}: {err}")))
            }
            ErpOp::AddResource {
                erp,
                resource,
                kind,
                sources,
            } => {
                let data = sources
                    .iter()
                    .map(|s| read(&resolve(s)?))
                    .collect::<Result<Vec<_>>>()?;
                self.add(erp, resource, kind, &data)
            }
            ErpOp::RemoveResource { erp, resource } => {
                let e = self.erp(erp)?;
                let i = e.find(resource).map_err(|err| fail(format!("{erp}: {err}")))?;
                e.slots.remove(i);
                Ok(())
            }
            ErpOp::AddMipmaps { source, target } => {
                let from = resolve(source)?;
                let target = checked(target.trim_end_matches('/'))?;
                if from.is_file() {
                    self.mipmaps.insert(target.to_owned(), from);
                    return Ok(());
                }
                if !from.is_dir() {
                    return Err(fail(format!("add-mipmaps: {} is missing", source.path)));
                }
                let files = loader_files(&from)?;
                if files.is_empty() {
                    return Err(fail(format!("add-mipmaps: {} has no files", source.path)));
                }
                for (rel, path) in files {
                    self.mipmaps.insert(format!("{target}/{rel}"), path);
                }
                Ok(())
            }
        }
    }

    fn add(&mut self, erp: &str, name: &str, kind: &str, data: &[Vec<u8>]) -> Result<()> {
        let wanted = erp::kind(kind).map_err(|e| fail(format!("add-resource {name}: {e}")))?;
        let target = self.erp(erp)?;
        if target
            .position(name)
            .map_err(|e| fail(format!("{erp}: {e}")))?
            .is_some()
        {
            return Err(fail(format!(
                "{erp}: {name} already exists; put-resource replaces a resource"
            )));
        }
        let like = target
            .slots
            .iter()
            .map(|s| target.entry(s))
            .find(|r| r.kind == wanted && r.fragments.len() == data.len())
            .ok_or_else(|| {
                fail(format!(
                    "{erp}: add-resource {name} takes its layout from a {kind} resource with {} fragment(s) in the archive, and it has none",
                    data.len()
                ))
            })?;
        let res = Resource::like(name, like, data).map_err(|e| fail(format!("{erp}: {e}")))?;
        target.slots.push(Slot::Owned(res));
        Ok(())
    }

    fn copy(&mut self, from: &Path, resource: &str, erp: &str, name: &str, replace: bool) -> Result<()> {
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
        match target.position(name).map_err(|e| fail(format!("{erp}: {e}")))? {
            Some(i) if replace => target.slots[i] = Slot::Owned(res),
            Some(_) => return Err(fail(format!("{erp}: {name} already exists"))),
            None => target.slots.push(Slot::Owned(res)),
        }
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
        let i = e.find(resource).map_err(|err| fail(format!("{erp}: {err}")))?;
        let res = e.owned(i)?;
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

    pub fn take_mipmaps(&mut self) -> BTreeMap<String, PathBuf> {
        std::mem::take(&mut self.mipmaps)
    }

    pub fn resource(&self, erp: &str, resource: &str, fragment: usize) -> Result<Option<Pkg>> {
        let Some(e) = self.erps.get(erp) else {
            return Ok(None);
        };
        let i = e.find(resource).map_err(|err| fail(format!("{erp}: {err}")))?;
        let loaded;
        let res = match &e.slots[i] {
            Slot::Owned(r) => r,
            Slot::Stock(j) => {
                loaded = e.file.load(&e.file.erp.resources[*j])?;
                &loaded
            }
        };
        let frag = res
            .fragments
            .get(fragment)
            .ok_or_else(|| fail(format!("{resource}: no fragment {fragment}")))?;
        Ok(Some(Pkg::parse(&frag.data()?)?))
    }

    pub fn resources(&self) -> impl Iterator<Item = (&str, &Resource, &Erp)> {
        self.erps.iter().flat_map(|(rel, a)| {
            a.slots.iter().filter_map(move |s| match s {
                Slot::Owned(r) => Some((rel.as_str(), r, &a.file.erp)),
                Slot::Stock(_) => None,
            })
        })
    }

    pub fn write(self, out: &Path) -> Result<Vec<String>> {
        let mut written = Vec::new();
        for (rel, Archive { file, slots }) in self.erps {
            let path = out.join(&rel);
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(crate::io_err(dir))?;
            }
            let entries = slots
                .into_iter()
                .map(|s| match s {
                    Slot::Stock(i) => Entry::Copy(&file.erp.resources[i]),
                    Slot::Owned(r) => Entry::Owned(r),
                })
                .collect();
            file.write(entries, &path)
                .map_err(|e| fail(format!("{rel}: {e}")))?;
            written.push(rel);
        }
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::{AssetOp, FileKind, OpFile};
    use crate::runtime::Runtime;
    use swofty_formats::erp::{Fragment, HEADER_LEN, MAGIC, Resource, STORED, ZSTD, compress};
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
        let mut whole = Erp::parse(&world).unwrap();
        let patched = res.fragments[0].data().unwrap();
        whole
            .find_mut("eaid://t-a.layer")
            .unwrap()
            .set_fragment(0, &patched)
            .unwrap();
        assert_eq!(
            whole.to_bytes(),
            std::fs::read(out.join("tracks/t/wep/t_world.erp")).unwrap()
        );
        let untouched = Erp::parse(&world).unwrap();
        assert_eq!(
            written.find("eaid://t-b.layer").unwrap().fragments[0].raw,
            untouched.find("eaid://t-b.layer").unwrap().fragments[0].raw
        );
        let mut bad = ErpBuild::new(&original);
        let text = erp_ops(
            r#"{"schema_version":1,"ops":[{"op":"patch-pkg","erp":"x.erp","resource":"r","edits":[
            {"op":"set-text","node":"n","value":"v"}]}]}"#,
        );
        assert_eq!(unsupported(&text[0]), Some("set-text"));
        assert!(apply(&mut bad, &text, &game).is_err());
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
        let named: Vec<String> = ops
            .iter()
            .flat_map(|op| {
                AssetOp::Erp(op.clone())
                    .sources()
                    .into_iter()
                    .map(|s| s.path.clone())
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(named, ["erp/src.erp", "erp/src.erp"]);
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
        let put = erp_ops(
            r#"{"schema_version":1,"ops":[
            {"op":"put-resource","source":"erp/src.erp","erp":"cars/slot.erp","resource":"eaid://car.wep","as":"eaid://slot-a.layer"},
            {"op":"put-resource","source":"erp/src.erp","erp":"cars/slot.erp","resource":"eaid://car-model.layer"}]}"#,
        );
        let mut b = ErpBuild::new(&original);
        apply(&mut b, &put, &addon_dir).unwrap();
        let out = root.join("put");
        b.write(&out).unwrap();
        let written = Erp::parse(&std::fs::read(out.join("cars/slot.erp")).unwrap()).unwrap();
        let names: Vec<&str> = written.resources.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["eaid://slot-a.layer", "eaid://car-model.layer"]);
        assert_eq!(
            written.resources[0].fragments[0].raw,
            source.find("eaid://car.wep").unwrap().fragments[0].raw
        );
        std::fs::remove_dir_all(&root).ok();
    }

    fn surface(name: &str, head: &[u8], pixels: &[u8]) -> Resource {
        let fragment = |compression: u8, data: &[u8]| Fragment {
            name: *b"temp",
            offset: 0,
            size: data.len() as u64,
            flags: 0x10,
            compression,
            packed: 0,
            raw: match compression {
                ZSTD => compress(data).unwrap(),
                _ => data.to_vec(),
            },
        };
        let mut res = Resource {
            name: name.into(),
            kind: erp::kind("GfxSurfaceRes").unwrap(),
            unk: 9u32.to_le_bytes().to_vec(),
            fragments: vec![fragment(ZSTD, head), fragment(STORED, pixels)],
            hash: [0; 16],
        };
        for f in &mut res.fragments {
            f.packed = f.raw.len() as u64;
        }
        res.hash = res.content_hash().unwrap();
        res
    }

    fn archive(resources: Vec<Resource>) -> Vec<u8> {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(MAGIC);
        header[4..8].copy_from_slice(&3u32.to_le_bytes());
        Erp {
            header,
            version: 3,
            resources,
        }
        .to_bytes()
    }

    fn data(res: &Resource) -> Vec<Vec<u8>> {
        res.fragments.iter().map(|f| f.data().unwrap()).collect()
    }

    #[test]
    fn replaces_adds_and_removes_resources_with_the_same_bytes_on_both_routes() {
        let root = std::env::temp_dir().join(format!("swofty-erpraw-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let game = root.join("game");
        let addon = root.join("addon");
        std::fs::create_dir_all(game.join("ui")).unwrap();
        std::fs::create_dir_all(addon.join("raw")).unwrap();
        let (x, y, z) = (
            "eaid://ui_package/x.tif.image",
            "eaid://ui_package/y.tif.image",
            "eaid://ui_package/z.tif.image",
        );
        let mut l = Erp::parse(&erp(&[("eaid://ui_package/l.layer", layer(&["a"]))]))
            .unwrap()
            .resources
            .remove(0);
        l.unk = 1u32.to_le_bytes().to_vec();
        let stock = archive(vec![
            surface(x, b"HEADER-X", b"PIXELS-X"),
            surface(y, b"HEADER-Y", b"PIXELS-Y"),
            l,
        ]);
        std::fs::write(game.join("ui/a.erp"), &stock).unwrap();
        std::fs::write(addon.join("raw/pixels.bin"), b"PAINTED!").unwrap();
        std::fs::write(addon.join("raw/head.bin"), b"HEADER-Z").unwrap();
        std::fs::write(addon.join("raw/z.bin"), b"NEW PIXELS").unwrap();
        let replace_add = r#"
            {"op":"replace-resource","erp":"ui/a.erp","resource":"eaid://ui_package/x.tif.image","fragment":1,"source":"raw/pixels.bin"},
            {"op":"add-resource","erp":"ui/a.erp","resource":"eaid://ui_package/z.tif.image","type":"GfxSurfaceRes",
             "sources":["raw/head.bin","raw/z.bin"]}"#;
        let ops = erp_ops(&format!(
            r#"{{"schema_version":1,"ops":[{replace_add},
            {{"op":"remove-resource","erp":"ui/a.erp","resource":"eaid://ui_package/y.tif.image"}}]}}"#
        ));
        assert!(ops.iter().all(|op| unsupported(op).is_none()));
        let original = |rel: &str| game.join(rel);
        let mut b = ErpBuild::new(&original);
        apply(&mut b, &ops, &addon).unwrap();
        let out = root.join("build");
        assert_eq!(b.write(&out).unwrap(), ["ui/a.erp"]);
        let built = std::fs::read(out.join("ui/a.erp")).unwrap();

        let mut expected = Erp::parse(&stock).unwrap();
        expected
            .find_mut(x)
            .unwrap()
            .set_fragment(1, b"PAINTED!")
            .unwrap();
        let like = expected.find(x).unwrap().clone();
        expected.resources.retain(|r| r.name != y);
        expected
            .resources
            .push(Resource::like(z, &like, &[b"HEADER-Z".to_vec(), b"NEW PIXELS".to_vec()]).unwrap());
        assert_eq!(built, expected.to_bytes());

        let written = Erp::parse(&built).unwrap();
        let names: Vec<&str> = written.resources.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, [x, "eaid://ui_package/l.layer", z]);
        let before = Erp::parse(&stock).unwrap();
        let (old, new) = (before.find(x).unwrap(), written.find(x).unwrap());
        assert_eq!(new.fragments[0].raw, old.fragments[0].raw);
        assert_eq!(new.fragments[1].raw, b"PAINTED!");
        assert_eq!(new.fragments[1].compression, STORED);
        assert_eq!(new.hash, new.content_hash().unwrap());
        assert_ne!(new.hash, old.hash);
        let added = written.find(z).unwrap();
        assert_eq!((added.kind(), &added.unk), ("GfxSurfaceRes".into(), &old.unk));
        assert_eq!(data(added), [b"HEADER-Z".to_vec(), b"NEW PIXELS".to_vec()]);
        assert_eq!(
            added
                .fragments
                .iter()
                .map(|f| (f.name, f.flags, f.compression))
                .collect::<Vec<_>>(),
            old.fragments
                .iter()
                .map(|f| (f.name, f.flags, f.compression))
                .collect::<Vec<_>>()
        );
        assert_eq!(added.hash, added.content_hash().unwrap());

        let runtime_ops = erp_ops(&format!(r#"{{"schema_version":1,"ops":[{replace_add}]}}"#));
        let mut b = ErpBuild::new(&original);
        apply(&mut b, &runtime_ops, &addon).unwrap();
        let overlay = root.join("overlay");
        let mut rt = Runtime::new(Vec::new());
        assert!(rt.resources(b.resources(), &overlay).unwrap().is_empty());
        let rt = rt.finish(&overlay).unwrap().unwrap();
        assert_eq!(rt.assets.resource_erps.len(), 1);
        assert_eq!(rt.assets.resource_erps[0].erps, ["ui/a.erp"]);
        let lay = Erp::parse(
            &std::fs::read(overlay.join("swofty").join(&rt.assets.resource_erps[0].file)).unwrap(),
        )
        .unwrap();
        let names: Vec<&str> = lay.resources.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, [x, z]);
        for r in &lay.resources {
            let b = written.find(&r.name).unwrap();
            assert_eq!((r.kind, &r.unk, r.hash), (b.kind, &b.unk, b.hash));
            for (f, g) in r.fragments.iter().zip(&b.fragments) {
                assert_eq!(
                    (f.name, f.size, f.flags, f.compression, f.packed, &f.raw),
                    (g.name, g.size, g.flags, g.compression, g.packed, &g.raw)
                );
            }
        }

        let fails = |json: &str| {
            let ops = erp_ops(&format!(r#"{{"schema_version":1,"ops":[{json}]}}"#));
            apply(&mut ErpBuild::new(&original), &ops, &addon)
                .unwrap_err()
                .to_string()
        };
        assert_eq!(
            fails(
                r#"{"op":"replace-resource","erp":"ui/a.erp","resource":"eaid://ui_package/w","source":"raw/head.bin"}"#
            ),
            "ui/a.erp: eaid://ui_package/w: not found"
        );
        assert_eq!(
            fails(&format!(
                r#"{{"op":"replace-resource","erp":"ui/a.erp","resource":"{x}","fragment":2,"source":"raw/head.bin"}}"#
            )),
            format!("ui/a.erp: {x}: no fragment 2")
        );
        assert!(
            fails(&format!(
                r#"{{"op":"replace-resource","erp":"ui/a.erp","resource":"{x}","source":"raw/missing.bin"}}"#
            ))
            .contains("missing.bin")
        );
        assert_eq!(
            fails(&format!(
                r#"{{"op":"add-resource","erp":"ui/a.erp","resource":"{y}","type":"GfxSurfaceRes","sources":["raw/head.bin","raw/z.bin"]}}"#
            )),
            format!("ui/a.erp: {y} already exists; put-resource replaces a resource")
        );
        assert!(
            fails(&format!(
                r#"{{"op":"add-resource","erp":"ui/a.erp","resource":"{z}","type":"GfxSurfaceRes","sources":["raw/head.bin"]}}"#
            ))
            .contains("takes its layout from a GfxSurfaceRes resource with 1 fragment(s)")
        );
        assert!(
            fails(&format!(
                r#"{{"op":"add-resource","erp":"ui/a.erp","resource":"{z}","type":"GfxMeshRes","sources":["raw/head.bin"]}}"#
            ))
            .contains("GfxMeshRes")
        );
        assert!(
            fails(&format!(
                r#"{{"op":"add-resource","erp":"ui/a.erp","resource":"{z}","type":"SixteenBytesLong","sources":["raw/head.bin"]}}"#
            ))
            .contains("is not a resource type")
        );
        assert_eq!(
            fails(r#"{"op":"remove-resource","erp":"ui/a.erp","resource":"eaid://ui_package/w"}"#),
            "ui/a.erp: eaid://ui_package/w: not found"
        );
        assert!(
            fails(&format!(
                r#"{{"op":"remove-resource","erp":"ui/a.erp","resource":"{y}"}},{{"op":"remove-resource","erp":"ui/a.erp","resource":"{y}"}}"#
            ))
            .contains("not found")
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn add_mipmaps_places_files_and_folders_at_game_paths() {
        let root = std::env::temp_dir().join(format!("swofty-erpmips-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let addon = root.join("addon");
        std::fs::create_dir_all(addon.join("mips/sub")).unwrap();
        std::fs::write(addon.join("mips/a.tif.mipmaps"), b"a").unwrap();
        std::fs::write(addon.join("mips/sub/b.tif.mipmaps"), b"b").unwrap();
        std::fs::write(addon.join("one.tif.mipmaps"), b"1").unwrap();
        let ops = erp_ops(
            r#"{"schema_version":1,"ops":[
            {"op":"add-mipmaps","source":"mips/","target":"livery_package/textures/"},
            {"op":"add-mipmaps","source":"one.tif.mipmaps","target":"livery_package/one.tif.mipmaps"}]}"#,
        );
        assert!(ops.iter().all(|op| unsupported(op).is_none()));
        let original = |rel: &str| root.join("game").join(rel);
        let mut b = ErpBuild::new(&original);
        apply(&mut b, &ops, &addon).unwrap();
        assert_eq!(b.changed().count(), 0);
        assert_eq!(
            b.take_mipmaps(),
            BTreeMap::from([
                (
                    "livery_package/one.tif.mipmaps".to_owned(),
                    addon.join("one.tif.mipmaps")
                ),
                (
                    "livery_package/textures/a.tif.mipmaps".to_owned(),
                    addon.join("mips/").join("a.tif.mipmaps")
                ),
                (
                    "livery_package/textures/sub/b.tif.mipmaps".to_owned(),
                    addon.join("mips/").join("sub/b.tif.mipmaps")
                ),
            ])
        );
        for bad in [
            r#"{"op":"add-mipmaps","source":"mips","target":"../outside"}"#,
            r#"{"op":"add-mipmaps","source":"missing","target":"x"}"#,
        ] {
            let ops = erp_ops(&format!(r#"{{"schema_version":1,"ops":[{bad}]}}"#));
            assert!(apply(&mut ErpBuild::new(&original), &ops, &addon).is_err());
        }
        std::fs::remove_dir_all(&root).ok();
    }
}
