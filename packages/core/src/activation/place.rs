use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use md5::{Digest as _, Md5};
use swofty_formats::erp::{Entry, ErpFile, Resource, STORED, ZSTD, compress};
use swofty_formats::pkg::{Node, Pkg};

use crate::map::TrackPackage;
use crate::{Error, Result, io_err};

pub const ASSET_ROOT: &str = "2025_asset_groups";
pub const SLOT_DIR: &str = "ui_package/texture_arrays/invitational_events/event_images";
pub const SLOTS: &[&str] = &[
    "invitational_image_4",
    "invitational_image_10",
    "invitational_image_13",
    "invitational_image_14",
    "invitational_image_15",
    "invitational_image_16",
    "invitational_image_17",
    "invitational_image_18",
];
const EAID: &str = "eaid://";
const PLACEMENT_VERSION: u32 = 2;

pub const WORLD_FALLBACKS: &[(&str, &[&str])] = &[
    ("f1", &["f1_26", "f1_myteam"]),
    ("f1_myteam", &["f1"]),
    ("classic", &["f1", "f1_26"]),
    ("f1_story", &["f1"]),
    ("f1_previous_story", &["f1_story", "f1"]),
    ("f1_previous_myteam", &["f1_myteam", "f1"]),
    ("f2", &["f1"]),
    ("f2_previous", &["f2", "f1"]),
    ("f1_26", &["f1"]),
    ("f1_26_myteam", &["f1_26", "f1_myteam", "f1"]),
    ("f2_26", &["f2", "f1_26", "f1"]),
];

pub fn slot_file(slot: &str) -> String {
    format!("{ASSET_ROOT}/{SLOT_DIR}/{slot}.erp")
}

pub fn slot_asset_group(slot: &str) -> String {
    format!("asset_groups/{SLOT_DIR}/{slot}.assetgroup")
}

pub fn slot_asset_root() -> String {
    format!("asset_groups/{SLOT_DIR}/")
}

pub fn mip_dir(code: &str) -> String {
    format!("environment_package/tracks/{code}/mipmaps")
}

fn world_eaid(code: &str, variant: &str) -> String {
    format!("{EAID}environment_package/tracks/{code}/wep/{code}_{variant}.wep")
}

fn world_variant<'a>(code: &str, eaid: &'a str) -> Option<&'a str> {
    let prefix = format!("{EAID}environment_package/tracks/{code}/wep/{code}_");
    eaid.strip_prefix(&prefix)?.strip_suffix(".wep")
}

pub struct PlaceInput<'a> {
    pub pkg: &'a TrackPackage,
    pub dir: &'a Path,
    pub game: &'a Path,
    pub slot: &'a str,
    pub slot_original: &'a Path,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Placement {
    pub replaced: BTreeMap<String, PathBuf>,
    pub added: BTreeMap<String, PathBuf>,
    pub aliases: BTreeMap<String, String>,
    pub mips: usize,
    pub notes: Vec<String>,
}

fn alias_world(
    file: &ErpFile,
    world: &Resource,
    graph: &Resource,
    code: &str,
    to: &str,
) -> Result<[Resource; 2]> {
    let eaid = world_eaid(code, to);
    let mut graph = file.load(graph)?;
    graph.name = format!("{eaid}?event_graph");
    let mut world = file.load(world)?;
    let data = world.fragments[0].data()?;
    let mut pkg = Pkg::parse(&data)?;
    let root = pkg
        .root
        .as_obj_mut()
        .ok_or_else(|| Error::Map(format!("{}: world root is not an object", world.name)))?;
    for (key, value) in [
        ("name", format!("{code}_{to}")),
        ("event_graph", graph.name.clone()),
    ] {
        match root.get_mut(key).and_then(Node::as_val_mut) {
            Some(v) if v.as_str().is_some() => v.set_str(value),
            _ => return Err(Error::Map(format!("{}: world has no {key} string", world.name))),
        }
    }
    set_data(&mut world, 0, &pkg.to_bytes()?)?;
    world.name = eaid;
    Ok([world, graph])
}

fn set_data(res: &mut Resource, index: usize, data: &[u8]) -> Result<()> {
    let f = res
        .fragments
        .get_mut(index)
        .ok_or_else(|| Error::Map(format!("{}: no fragment {index}", res.name)))?;
    let zstd =
        f.compression == ZSTD || (f.compression != STORED && f.raw.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]));
    f.raw = if zstd { compress(data)? } else { data.to_vec() };
    f.size = data.len() as u64;
    f.packed = f.raw.len() as u64;
    res.hash = res.content_hash()?;
    Ok(())
}

pub fn shipped_name(path: &str) -> String {
    let hex = crate::baseline::hex(&Md5::digest(path.as_bytes()));
    let base = path.rsplit('/').next().unwrap_or(path);
    format!("{}_{base}", &hex[..12])
}

fn repoint_mips(
    file: &ErpFile,
    res: &Resource,
    code: &str,
    shipped: &BTreeSet<String>,
) -> Result<Option<(Resource, String)>> {
    if res.kind() != "GfxSurfaceRes" || res.fragments.len() < 3 {
        return Ok(None);
    }
    let mut loaded = file.load(res)?;
    let data = loaded.fragments[2].data()?;
    let Some(&len) = data.first() else {
        return Ok(None);
    };
    let len = len as usize;
    if len == 0 || data.len() < 1 + len {
        return Ok(None);
    }
    let path = String::from_utf8_lossy(&data[1..1 + len])
        .trim_end_matches('\0')
        .to_owned();
    let base = path.rsplit('/').next().unwrap_or(&path).to_owned();
    let name = if shipped.contains(&base) {
        base
    } else {
        let hashed = shipped_name(&path);
        if !shipped.contains(&hashed) {
            return Ok(None);
        }
        hashed
    };
    let target = format!("{}/{name}", mip_dir(code));
    let nul = data[1..1 + len].ends_with(&[0]);
    let mut text = target.into_bytes();
    if nul {
        text.push(0);
    }
    if text.len() > u8::MAX as usize {
        return Err(Error::Map(format!("{}: mip path is too long", res.name)));
    }
    let mut out = Vec::with_capacity(data.len() + text.len());
    out.push(text.len() as u8);
    out.extend_from_slice(&text);
    out.extend_from_slice(&data[1 + len..]);
    set_data(&mut loaded, 2, &out)?;
    Ok(Some((loaded, name)))
}

fn stamp(input: &PlaceInput) -> Result<String> {
    let erp = input.dir.join(&input.pkg.erp);
    let meta = std::fs::metadata(&erp).map_err(io_err(&erp))?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    let slot = crate::baseline::sha256_file(input.slot_original)?;
    let manifest = serde_json::to_vec(input.pkg).expect("serialisable");
    let mut h = <sha2::Sha256 as sha2::Digest>::new();
    sha2::Digest::update(&mut h, &manifest);
    sha2::Digest::update(
        &mut h,
        format!(
            "{}|{}|{mtime}|{}|{slot}|{PLACEMENT_VERSION}",
            erp.display(),
            meta.len(),
            input.slot
        ),
    );
    Ok(crate::baseline::hex(&sha2::Digest::finalize(h))[..16].to_owned())
}

pub fn place(input: &PlaceInput, cache: &Path) -> Result<Placement> {
    let code = &input.pkg.track_code;
    let out = cache.join(stamp(input)?);
    let file = ErpFile::open(&input.dir.join(&input.pkg.erp))?;
    let common = world_eaid(code, "common");
    if !file.erp.resources.iter().any(|r| r.name == common) {
        return Err(Error::Map(format!(
            "{} has no {common}; the track folder must match the track code",
            input.pkg.erp
        )));
    }
    let mut placement = Placement::default();
    let shipped: BTreeMap<String, PathBuf> = input
        .pkg
        .copy_directories
        .iter()
        .flat_map(|c| walk(&input.dir.join(c.source.trim_end_matches('/'))).unwrap_or_default())
        .filter_map(|p| Some((p.file_name()?.to_str()?.to_owned(), p)))
        .collect();
    let names: BTreeSet<String> = shipped.keys().cloned().collect();
    let present: BTreeSet<&str> = file
        .erp
        .resources
        .iter()
        .filter_map(|r| world_variant(code, &r.name))
        .collect();
    let slot_path = out.join(slot_file(input.slot));
    let done = out.join(".done");
    let slot_file_in = ErpFile::open(input.slot_original)?;
    let mut owned = Vec::new();
    for (v, fallbacks) in WORLD_FALLBACKS {
        if present.contains(v) {
            continue;
        }
        let Some(from) = fallbacks.iter().find(|f| present.contains(**f)) else {
            placement
                .notes
                .push(format!("no world can stand in for {code}_{v}"));
            continue;
        };
        let find = |name: &str| file.erp.resources.iter().find(|r| r.name == name);
        let src = world_eaid(code, from);
        let (Some(world), Some(graph)) = (find(&src), find(&format!("{src}?event_graph"))) else {
            continue;
        };
        owned.extend(alias_world(&file, world, graph, code, v)?);
        placement
            .aliases
            .insert(format!("{code}_{v}"), format!("{code}_{from}"));
    }
    let mut repointed = BTreeMap::new();
    let mut used = BTreeSet::new();
    if !names.is_empty() {
        for (i, r) in file.erp.resources.iter().enumerate() {
            if let Some((res, name)) = repoint_mips(&file, r, code, &names)? {
                repointed.insert(i, res);
                used.insert(name);
            }
        }
    }
    placement.mips = repointed.len();
    if !done.is_file() {
        let own: BTreeSet<&str> = file.erp.resources.iter().map(|r| r.name.as_str()).collect();
        let mut entries = Vec::new();
        for r in slot_file_in
            .erp
            .resources
            .iter()
            .filter(|r| !own.contains(r.name.as_str()))
        {
            entries.push(Entry::Owned(slot_file_in.load(r)?));
        }
        let mut repointed = repointed;
        for (i, r) in file.erp.resources.iter().enumerate() {
            entries.push(match repointed.remove(&i) {
                Some(res) => Entry::Owned(res),
                None => Entry::Copy(r),
            });
        }
        entries.extend(owned.into_iter().map(Entry::Owned));
        let dir = slot_path.parent().expect("has parent");
        std::fs::create_dir_all(dir).map_err(io_err(dir))?;
        file.write(entries, &slot_path)?;
        std::fs::write(&done, b"").map_err(io_err(&done))?;
    }
    placement.replaced.insert(slot_file(input.slot), slot_path);
    for name in &used {
        placement
            .added
            .insert(format!("{}/{name}", mip_dir(code)), shipped[name].clone());
    }
    let unused = names.len() - used.len();
    if unused > 0 {
        placement.notes.push(format!(
            "{unused} shipped mip file(s) belong to no texture and are left out"
        ));
    }
    Ok(placement)
}

fn walk(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Ok(out);
    };
    for e in rd {
        let p = e.map_err(io_err(dir))?.path();
        if p.is_dir() {
            out.extend(walk(&p)?);
        } else {
            out.push(p);
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_paths_match_the_game_layout() {
        assert_eq!(
            slot_file("invitational_image_4"),
            "2025_asset_groups/ui_package/texture_arrays/invitational_events/event_images/invitational_image_4.erp"
        );
        assert_eq!(
            slot_asset_group("invitational_image_4"),
            "asset_groups/ui_package/texture_arrays/invitational_events/event_images/invitational_image_4.assetgroup"
        );
        assert_eq!(
            world_variant(
                "okayama",
                "eaid://environment_package/tracks/okayama/wep/okayama_f1_26.wep"
            ),
            Some("f1_26")
        );
        assert_eq!(
            shipped_name("a/b/c.tif.mipmaps").len(),
            12 + 1 + "c.tif.mipmaps".len()
        );
    }
}
