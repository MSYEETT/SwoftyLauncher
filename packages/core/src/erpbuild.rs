use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use swofty_formats::erp::{self, Entry, Erp, ErpFile, Resource};
use swofty_formats::pkg::{Node, Pkg, Value};

use crate::asset::Source;
use crate::builtin::loader_files;
use crate::manifest::relative_inside;
use crate::ops::{ErpOp, PkgTarget};
use crate::pkgedit::{apply_edits, edit_name, sources, supported};
use crate::runtime::{foreign_argument, layout_change};
use crate::ui::node::component_id;
use crate::{Error, Result};

const SCENE: &str = "UILayout";

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
    // The origin of the op that last wrote each resource, by archive and name,
    // and the origins in the order their ops came.
    writers: HashMap<(String, String), String>,
    origins: Vec<String>,
    sources: HashMap<SourceKey, Node>,
    archives: HashMap<PathBuf, ErpFile>,
    mipmaps: BTreeMap<String, PathBuf>,
}

fn fail(msg: impl Into<String>) -> Error {
    Error::Resolve(msg.into())
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resized {
    pub erp: String,
    pub resources: Vec<(String, String)>,
}

fn scene_ids(data: &[u8]) -> Option<Vec<u64>> {
    let pkg = Pkg::parse(data).ok()?;
    let list = pkg.root.as_obj()?.get("scenelist")?.as_arr()?;
    let ids: Vec<u64> = list
        .items
        .iter()
        .filter_map(|s| match &s.as_obj()?.get("name")?.as_val()?.value {
            Value::Raw(b) if b.len() == 8 => Some(u64::from_le_bytes(b[..].try_into().ok()?)),
            _ => None,
        })
        .collect();
    (!ids.is_empty()).then_some(ids)
}

fn check_scene(uri: &str, data: &[u8]) -> std::result::Result<(), String> {
    let want = component_id(uri.strip_suffix(".idf").unwrap_or(uri));
    match scene_ids(data) {
        None => Err("the source is not a scene: a UILayout resource holds a !pkg with a scenelist".into()),
        Some(ids) if ids.contains(&want) => Ok(()),
        Some(ids) => Err(format!(
            "the source is scene {:#018x}, this resource is scene {want:#018x}; it was saved from another .idf",
            ids[0]
        )),
    }
}

fn write_archive(out: &Path, rel: &str, archive: Archive) -> Result<()> {
    let Archive { file, slots } = archive;
    let path = out.join(rel);
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
        .map_err(|e| fail(format!("{rel}: {e}")))
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
            writers: HashMap::new(),
            origins: Vec::new(),
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

    // Applies an op for an origin, such as an addon or a mod inside one. The
    // runtime route puts each origin's resources in an overlay archive of its own.
    pub fn apply_for(
        &mut self,
        op: &ErpOp,
        origin: &str,
        resolve: &dyn Fn(&Source) -> Result<PathBuf>,
    ) -> Result<()> {
        self.apply(op, resolve)?;
        let written = match op {
            ErpOp::PatchPkg { erp, resource, .. }
            | ErpOp::ReplaceResource { erp, resource, .. }
            | ErpOp::AddResource { erp, resource, .. } => Some((erp, resource.as_str())),
            ErpOp::CopyResource {
                erp,
                resource,
                rename,
                ..
            }
            | ErpOp::PutResource {
                erp,
                resource,
                rename,
                ..
            } => Some((erp, rename.as_deref().unwrap_or(resource))),
            ErpOp::RemoveResource { erp, resource } => {
                self.writers.remove(&(erp.clone(), resource.clone()));
                None
            }
            ErpOp::AddMipmaps { .. } => None,
        };
        if let Some((erp, name)) = written {
            self.writers
                .insert((erp.clone(), name.to_owned()), origin.to_owned());
            if !self.origins.iter().any(|o| o == origin) {
                self.origins.push(origin.to_owned());
            }
        }
        Ok(())
    }

    pub fn origins(&self) -> &[String] {
        &self.origins
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
                let res = e.owned(i)?;
                if res.kind() == SCENE && *fragment == 0 {
                    check_scene(resource, &data).map_err(|err| fail(format!("{erp}: {resource}: {err}")))?;
                }
                res.set_fragment(*fragment, &data)
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

    pub fn resources(&self) -> impl Iterator<Item = (&str, &Resource, &Erp, &str)> {
        self.erps.iter().flat_map(move |(rel, a)| {
            a.slots.iter().filter_map(move |s| match s {
                Slot::Owned(r) => {
                    let origin = self
                        .writers
                        .get(&(rel.clone(), r.name.clone()))
                        .map_or("", String::as_str);
                    Some((rel.as_str(), r, &a.file.erp, origin))
                }
                Slot::Stock(_) => None,
            })
        })
    }

    // The in-game loader reads a replaced resource with the stock entry's
    // layout, so an archive whose changes alter a stock entry's type, argument
    // or fragment sizes is rewritten whole instead, and leaves the runtime set.
    pub fn resized(&self) -> Vec<Resized> {
        let mut found = Vec::new();
        for (rel, a) in &self.erps {
            let stock = &a.file.erp;
            let resources: Vec<(String, String)> = a
                .slots
                .iter()
                .filter_map(|s| match s {
                    Slot::Owned(r) if !foreign_argument(r, stock) => {
                        layout_change(r, stock).map(|why| (r.name.clone(), why))
                    }
                    _ => None,
                })
                .collect();
            if !resources.is_empty() {
                found.push(Resized {
                    erp: rel.clone(),
                    resources,
                });
            }
        }
        found
    }

    pub fn take_resized(&mut self, out: &Path) -> Result<Vec<Resized>> {
        let found = self.resized();
        for r in &found {
            let archive = self.erps.remove(&r.erp).expect("found above");
            write_archive(out, &r.erp, archive)?;
        }
        Ok(found)
    }

    pub fn write(self, out: &Path) -> Result<Vec<String>> {
        let mut written = Vec::new();
        for (rel, archive) in self.erps {
            write_archive(out, &rel, archive)?;
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
        typed_erp("WOInstances", resources)
    }

    fn typed_erp(kind: &str, resources: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(MAGIC);
        header[4..8].copy_from_slice(&4u32.to_le_bytes());
        let resources = resources
            .iter()
            .map(|(name, body)| {
                let packed = compress(body).unwrap();
                let mut res = Resource {
                    name: (*name).into(),
                    kind: erp::kind(kind).unwrap(),
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
    fn each_origin_gets_an_overlay_archive_of_its_own() {
        let root = std::env::temp_dir().join(format!("swofty-erpbuild-origins-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let (game, addon) = (root.join("game"), root.join("addon"));
        std::fs::create_dir_all(game.join("ui")).unwrap();
        std::fs::create_dir_all(addon.join("mods")).unwrap();
        let body = |s: &str| layer(&[s]);
        std::fs::write(game.join("ui/a.erp"), erp(&[("eaid://stock", body("stock"))])).unwrap();
        std::fs::write(game.join("ui/boot.erp"), erp(&[("eaid://boot", body("boot"))])).unwrap();
        std::fs::write(
            addon.join("mods/a.erp"),
            erp(&[("eaid://one", body("a1")), ("eaid://shared", body("from a"))]),
        )
        .unwrap();
        std::fs::write(
            addon.join("mods/b.erp"),
            erp(&[("eaid://two", body("b2")), ("eaid://shared", body("from b"))]),
        )
        .unwrap();
        let put = |res: &str, from: &str| {
            format!(
                r#"{{"op":"put-resource","erp":"ui/a.erp","resource":"{res}","source":"mods/{from}.erp"}}"#
            )
        };
        let ops = |list: &[String]| erp_ops(&format!(r#"{{"schema_version":1,"ops":[{}]}}"#, list.join(",")));
        let original = |rel: &str| game.join(rel);
        let mut b = ErpBuild::new(&original);
        let resolve = |s: &Source| Ok(s.resolve(&addon)?);
        for op in ops(&[put("eaid://one", "a"), put("eaid://shared", "a")]) {
            b.apply_for(&op, "mods (a)", &resolve).unwrap();
        }
        let to_boot =
            r#"{"op":"put-resource","erp":"ui/boot.erp","resource":"eaid://three","source":"mods/b3.erp"}"#;
        std::fs::write(addon.join("mods/b3.erp"), erp(&[("eaid://three", body("b3"))])).unwrap();
        for op in ops(&[
            put("eaid://two", "b"),
            put("eaid://shared", "b"),
            to_boot.to_owned(),
        ]) {
            b.apply_for(&op, "mods (b)", &resolve).unwrap();
        }
        assert_eq!(b.origins(), ["mods (a)", "mods (b)"]);
        let overlay = root.join("overlay");
        let mut rt = Runtime::new(Vec::new());
        assert!(
            rt.resources(b.resources(), b.origins(), &overlay)
                .unwrap()
                .is_empty()
        );
        let rt = rt.finish(&overlay).unwrap().unwrap();
        let erps: Vec<(&str, &str, Vec<String>)> = rt
            .assets
            .resource_erps
            .iter()
            .map(|e| {
                let file = Erp::parse(&std::fs::read(overlay.join("swofty").join(&e.file)).unwrap()).unwrap();
                let names = file.resources.iter().map(|r| r.name.clone()).collect();
                (e.file.as_str(), e.origin.as_str(), names)
            })
            .collect();
        assert_eq!(
            erps,
            [
                (
                    "assets/resources/0.erp",
                    "mods (a)",
                    vec!["eaid://one".to_owned()]
                ),
                (
                    "assets/resources/1.erp",
                    "mods (b)",
                    vec![
                        "eaid://shared".to_owned(),
                        "eaid://three".to_owned(),
                        "eaid://two".to_owned()
                    ]
                ),
            ]
        );
        let targets: Vec<&Vec<String>> = rt.assets.resource_erps.iter().map(|e| &e.erps).collect();
        assert_eq!(
            targets,
            [
                &vec!["ui/a.erp".to_owned()],
                &vec!["ui/a.erp".to_owned(), "ui/boot.erp".to_owned()]
            ]
        );
        std::fs::remove_dir_all(&root).ok();
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
        assert!(
            rt.resources(b.resources(), b.origins(), &overlay)
                .unwrap()
                .is_empty()
        );
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

    fn scene(uri: &str, tiles: &[&str]) -> Vec<u8> {
        let id = component_id(uri.strip_suffix(".idf").unwrap());
        let named = |n: &str| Node::Obj(Obj::new(vec![(Val::str("name"), Node::Val(Val::str(n)))]));
        let root = Obj::new(vec![
            (Val::str("name"), Node::Val(Val::str("Scene Root"))),
            (
                Val::str("children"),
                Node::Arr(Arr::new(tiles.iter().map(|t| named(t)).collect())),
            ),
        ]);
        let entry = Obj::new(vec![
            (Val::str("scene"), Node::Obj(root)),
            (Val::str("name"), Node::Val(Val::raw(*b"ui64", id.to_le_bytes()))),
        ]);
        Pkg {
            name: [0; 4],
            root: Node::Obj(Obj::new(vec![(
                Val::str("scenelist"),
                Node::Arr(Arr::new(vec![Node::Obj(entry)])),
            )])),
            vca: Vec::new(),
        }
        .to_bytes()
        .unwrap()
    }

    #[test]
    fn a_resized_scene_rewrites_its_archive_instead_of_the_runtime_overlay() {
        let root = std::env::temp_dir().join(format!("swofty-erpscene-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let (game, addon) = (root.join("game"), root.join("addon"));
        std::fs::create_dir_all(game.join("ui")).unwrap();
        std::fs::create_dir_all(addon.join("ui")).unwrap();
        let uri = "eaid://ui_package/art/screens/career/rnd_overview.idf";
        let other = "eaid://ui_package/art/screens/career/rnd_dept.idf";
        let four = scene(uri, &["tile_1", "tile_2", "tile_3", "tile_4"]);
        let five = scene(uri, &["tile_1", "tile_2", "tile_3", "tile_4", "tile_5"]);
        let stock = typed_erp(
            "UILayout",
            &[(uri, four.clone()), (other, scene(other, &["dept"]))],
        );
        std::fs::write(game.join("ui/hub.erp"), &stock).unwrap();
        let side = "eaid://ui_package/art/screens/career/rnd_side.idf";
        std::fs::write(
            game.join("ui/hq.erp"),
            typed_erp("UILayout", &[(side, scene(side, &["tile_a"]))]),
        )
        .unwrap();
        std::fs::write(addon.join("ui/side.idf"), scene(side, &["tile_b"])).unwrap();
        std::fs::write(addon.join("ui/same.idf"), &four).unwrap();
        std::fs::write(addon.join("ui/five.idf"), &five).unwrap();
        std::fs::write(addon.join("ui/dept.idf"), scene(other, &["dept", "dept_5"])).unwrap();
        std::fs::write(addon.join("ui/layer.bin"), layer(&["a"])).unwrap();
        std::fs::write(addon.join("ui/text.txt"), b"not a scene").unwrap();
        let replace = |erp: &str, res: &str, src: &str| {
            format!(r#"{{"op":"replace-resource","erp":"{erp}","resource":"{res}","source":"ui/{src}"}}"#)
        };
        let ops = |list: &[String]| erp_ops(&format!(r#"{{"schema_version":1,"ops":[{}]}}"#, list.join(",")));
        let original = |rel: &str| game.join(rel);

        let mut b = ErpBuild::new(&original);
        apply(&mut b, &ops(&[replace("ui/hub.erp", uri, "same.idf")]), &addon).unwrap();
        assert!(b.resized().is_empty());
        assert_eq!(b.resources().count(), 1);

        let mut b = ErpBuild::new(&original);
        apply(
            &mut b,
            &ops(&[
                replace("ui/hub.erp", uri, "five.idf"),
                replace("ui/hq.erp", side, "side.idf"),
            ]),
            &addon,
        )
        .unwrap();
        let why = format!(
            "fragment 0 is {} bytes, the stock one {}; stock archives that bind to the replacement read it with the stock layout",
            five.len(),
            four.len()
        );
        let expected = vec![Resized {
            erp: "ui/hub.erp".into(),
            resources: vec![(uri.into(), why)],
        }];
        assert_eq!(b.resized(), expected);
        let out = root.join("out");
        assert_eq!(b.take_resized(&out).unwrap(), expected);
        let left: Vec<(&str, &str)> = b.resources().map(|(e, r, _, _)| (e, r.name.as_str())).collect();
        assert_eq!(left, [("ui/hq.erp", side)]);
        assert_eq!(b.write(&root.join("rest")).unwrap(), ["ui/hq.erp"]);

        let built = std::fs::read(out.join("ui/hub.erp")).unwrap();
        let written = Erp::parse(&built).unwrap();
        let before = Erp::parse(&stock).unwrap();
        let res = written.find(uri).unwrap();
        assert_eq!(res.fragments[0].data().unwrap(), five);
        assert_eq!(res.fragments[0].size, five.len() as u64);
        assert_eq!(res.hash, res.content_hash().unwrap());
        assert_eq!(
            (res.kind, &res.unk),
            (before.find(uri).unwrap().kind, &before.find(uri).unwrap().unk)
        );
        assert_eq!(
            written.find(other).unwrap().fragments[0].raw,
            before.find(other).unwrap().fragments[0].raw
        );
        let mut again = Erp::parse(&built).unwrap();
        assert_eq!(again.to_bytes(), built);

        let fails = |src: &str, res: &str| {
            apply(
                &mut ErpBuild::new(&original),
                &ops(&[replace("ui/hub.erp", res, src)]),
                &addon,
            )
            .unwrap_err()
            .to_string()
        };
        assert_eq!(
            fails("dept.idf", uri),
            format!(
                "ui/hub.erp: {uri}: the source is scene {:#018x}, this resource is scene {:#018x}; it was saved from another .idf",
                component_id(other.strip_suffix(".idf").unwrap()),
                component_id(uri.strip_suffix(".idf").unwrap())
            )
        );
        for src in ["layer.bin", "text.txt"] {
            assert!(
                fails(src, uri).ends_with(
                    "the source is not a scene: a UILayout resource holds a !pkg with a scenelist"
                )
            );
        }
        apply(
            &mut ErpBuild::new(&original),
            &ops(&[replace("ui/hub.erp", other, "dept.idf")]),
            &addon,
        )
        .unwrap();
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
