use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use md5::{Digest as _, Md5};
use swofty_formats::erp::{Entry, ErpFile, Resource, STORED, ZSTD, compress};
use swofty_formats::pkg::{Node, Pkg};

use super::grid;
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
const GRID_WORLD: &str = "f1_26_myteam";

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

pub fn track_file(code: &str) -> String {
    format!("{ASSET_ROOT}/environment_package/tracks/{code}/{code}.erp")
}

pub fn track_group(code: &str, rel: &str) -> String {
    format!("asset_groups/environment_package/tracks/{code}/{rel}.assetgroup")
}

pub fn track_groups<'a>(code: &str, names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let prefix = format!("{EAID}environment_package/tracks/{code}/");
    let mut worlds = BTreeSet::new();
    let mut garages = BTreeSet::new();
    let mut ai = BTreeSet::new();
    for name in names {
        let Some(rest) = name.strip_prefix(&prefix) else {
            continue;
        };
        let rest = rest.split('?').next().unwrap_or(rest);
        if let Some(v) = rest
            .strip_prefix("wep/")
            .and_then(|w| w.strip_prefix(code))
            .and_then(|w| w.strip_prefix('_'))
            .and_then(|w| w.strip_suffix(".wep"))
        {
            worlds.insert(format!("wep/{code}_{v}"));
        } else if let Some(n) = rest
            .strip_prefix("probes/probes_garage_")
            .and_then(|p| p.strip_suffix(".shprobes"))
        {
            garages.insert(format!("probes/probes_garage_{n}"));
        } else if let Some([package, team, _]) = rest
            .strip_prefix("ai/")
            .map(|a| a.splitn(3, '/').collect::<Vec<_>>())
            .as_deref()
        {
            ai.insert(format!("ai/{package}/{team}/{team}"));
        }
    }
    let template = [
        format!("bake/{code}_ao.bake"),
        format!("bake/{code}_light_list.bake"),
        format!("bake/{code}_radiance.bake"),
        "probes/config.ddgi".into(),
        "probes/key_2019".into(),
    ];
    std::iter::once(code.to_owned())
        .chain(worlds)
        .chain(template)
        .chain(garages)
        .chain(ai)
        .map(|rel| track_group(code, &rel))
        .collect()
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
    pub slot: Option<(&'a str, &'a Path)>,
    pub cars: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Placement {
    pub dir: PathBuf,
    pub archive: String,
    pub groups: Vec<String>,
    pub replaced: BTreeMap<String, PathBuf>,
    pub added: BTreeMap<String, PathBuf>,
    pub aliases: BTreeMap<String, String>,
    pub mips: usize,
    pub grid: Option<grid::Extension>,
    pub repaired: Vec<String>,
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
    let slot = match input.slot {
        Some((name, original)) => format!("{name}|{}", crate::baseline::sha256_file(original)?),
        None => "natural".into(),
    };
    let manifest = serde_json::to_vec(input.pkg).expect("serialisable");
    let mut h = <sha2::Sha256 as sha2::Digest>::new();
    sha2::Digest::update(&mut h, &manifest);
    sha2::Digest::update(
        &mut h,
        format!(
            "{}|{}|{mtime}|{slot}|{PLACEMENT_VERSION}|{}",
            erp.display(),
            meta.len(),
            input.cars
        ),
    );
    Ok(crate::baseline::hex(&sha2::Digest::finalize(h))[..16].to_owned())
}

pub fn place(input: &PlaceInput, cache: &Path) -> Result<Placement> {
    let code = &input.pkg.track_code;
    let file = ErpFile::open(&input.dir.join(&input.pkg.erp))?;
    let common = world_eaid(code, "common");
    if !file.erp.resources.iter().any(|r| r.name == common) {
        return Err(Error::Map(format!(
            "{} has no {common}; the track folder must match the track code",
            input.pkg.erp
        )));
    }
    let (fixed, broken) = crate::erpbuild::repair_stats_in(&file)?;
    if !broken.is_empty() {
        let who = input.pkg.display_name();
        let fix = format!(
            "Turn {who} off in this profile, and set any race weekend that uses it back to another track."
        );
        return Err(Error::Other(crate::erpbuild::unrepairable_message(
            who, &broken, &fix,
        )));
    }
    let stamp = stamp(input)?;
    let out = cache.join(match fixed.is_empty() {
        true => stamp,
        false => format!("{stamp}-stats"),
    });
    let mut placement = Placement {
        dir: out.clone(),
        repaired: fixed.iter().map(|(_, r)| r.name.clone()).collect(),
        ..Placement::default()
    };
    let shipped: BTreeMap<String, PathBuf> = input
        .pkg
        .copy_directories
        .iter()
        .flat_map(|c| walk(&input.dir.join(c.source.trim_end_matches('/'))).unwrap_or_default())
        .filter_map(|p| Some((p.file_name()?.to_str()?.to_owned(), p)))
        .collect();
    let names: BTreeSet<String> = shipped.keys().cloned().collect();
    let present = variants(&file, code);
    let archive = match input.slot {
        Some((slot, _)) => slot_file(slot),
        None => track_file(code),
    };
    let archive_path = out.join(&archive);
    let done = out.join(".done");
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
    for (i, res) in fixed {
        repointed.entry(i).or_insert(res);
    }
    match extend_grid(&file, code, input.cars) {
        Ok(Some((i, res, ext))) => {
            repointed.insert(i, res);
            placement.grid = Some(ext);
        }
        Ok(None) => {}
        Err(e) => placement.notes.push(format!(
            "its world could not be given room for {0} cars ({e}); a Grand Prix there with {0} cars crashes",
            input.cars
        )),
    }
    if input.slot.is_none() {
        placement.groups = track_groups(
            code,
            file.erp.resources.iter().chain(&owned).map(|r| r.name.as_str()),
        );
    }
    if !done.is_file() {
        let own: BTreeSet<&str> = file.erp.resources.iter().map(|r| r.name.as_str()).collect();
        let mut entries = Vec::new();
        if let Some((_, original)) = input.slot {
            let slot_file_in = ErpFile::open(original)?;
            for r in slot_file_in
                .erp
                .resources
                .iter()
                .filter(|r| !own.contains(r.name.as_str()))
            {
                entries.push(Entry::Owned(slot_file_in.load(r)?));
            }
        }
        let mut repointed = repointed;
        for (i, r) in file.erp.resources.iter().enumerate() {
            entries.push(match repointed.remove(&i) {
                Some(res) => Entry::Owned(res),
                None => Entry::Copy(r),
            });
        }
        entries.extend(owned.into_iter().map(Entry::Owned));
        let dir = archive_path.parent().expect("has parent");
        std::fs::create_dir_all(dir).map_err(io_err(dir))?;
        file.write(entries, &archive_path)?;
        std::fs::write(&done, b"").map_err(io_err(&done))?;
    }
    if input.slot.is_none() && !input.game.join(&archive).is_file() {
        placement.added.insert(archive.clone(), archive_path);
    } else {
        placement.replaced.insert(archive.clone(), archive_path);
    }
    placement.archive = archive;
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

fn world_layers(file: &ErpFile, eaid: &str) -> Result<Vec<String>> {
    let Some(world) = file.erp.resources.iter().find(|r| r.name == eaid) else {
        return Ok(Vec::new());
    };
    let pkg = Pkg::parse(&file.load(world)?.fragments[0].data()?)?;
    let layers = pkg
        .root
        .as_obj()
        .and_then(|o| o.get("layers"))
        .and_then(Node::as_arr)
        .map(|a| {
            a.items
                .iter()
                .filter_map(|n| Some(n.as_val()?.as_str()?.to_owned()))
                .collect()
        })
        .unwrap_or_default();
    Ok(layers)
}

fn variants<'a>(file: &'a ErpFile, code: &str) -> BTreeSet<&'a str> {
    file.erp
        .resources
        .iter()
        .filter_map(|r| world_variant(code, &r.name))
        .collect()
}

pub fn extend_grid(
    file: &ErpFile,
    code: &str,
    cars: u32,
) -> Result<Option<(usize, Resource, grid::Extension)>> {
    let present = variants(file, code);
    let fallbacks = WORLD_FALLBACKS
        .iter()
        .find(|(v, _)| *v == GRID_WORLD)
        .map_or(&[][..], |(_, f)| *f);
    let Some(variant) = std::iter::once(GRID_WORLD)
        .chain(fallbacks.iter().copied())
        .find(|v| present.contains(v))
    else {
        return Ok(None);
    };
    let mut names = world_layers(file, &world_eaid(code, "common"))?;
    for name in world_layers(file, &world_eaid(code, variant))? {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    let mut found = Vec::new();
    let mut roots = Vec::new();
    for (i, r) in file.erp.resources.iter().enumerate() {
        if names.contains(&r.name) {
            let res = file.load(r)?;
            roots.push(Pkg::parse(&res.fragments[0].data()?)?);
            found.push((i, res));
        }
    }
    let mut layers: Vec<Node> = roots.iter().map(|p| p.root.clone()).collect();
    let Some(ext) = grid::extend(code, &mut layers, cars)? else {
        return Ok(None);
    };
    let mut pkg = roots.swap_remove(ext.layer);
    pkg.root = layers.swap_remove(ext.layer);
    let (i, mut res) = found.swap_remove(ext.layer);
    set_data(&mut res, 0, &pkg.to_bytes()?)?;
    Ok(Some((i, res, ext)))
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

    #[test]
    fn a_package_with_stale_tsmd_stats_is_repaired_or_refused() {
        use swofty_formats::bxml::{Document, Element, Kind};
        use swofty_formats::erp::{Erp, Fragment, HEADER_LEN, MAGIC, STORED};
        let dir = std::env::temp_dir().join(format!("swofty-place-tsmd-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let pkg = TrackPackage::parse(crate::map::tests::OKAYAMA_LIKE.as_bytes()).unwrap();
        let line = |gate: &str| Document {
            root: Element {
                kind: Kind::Element,
                pad: 0,
                name: b"track".to_vec(),
                attrs: vec![(b"name".to_vec(), gate.as_bytes().to_vec())],
                children: Vec::new(),
                end_pad: 0,
            },
            tail: Vec::new(),
        };
        let fragment = |raw: Vec<u8>| Fragment {
            name: *b"temp",
            offset: 0,
            size: raw.len() as u64,
            flags: 0x10,
            compression: STORED,
            packed: raw.len() as u64,
            raw,
        };
        let resource = |name: &str, kind: &str, fragments: Vec<Fragment>| Resource {
            name: name.into(),
            kind: swofty_formats::erp::kind(kind).unwrap(),
            unk: vec![0; 4],
            fragments,
            hash: [0; 16],
        };
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(MAGIC);
        header[4..8].copy_from_slice(&3u32.to_le_bytes());
        let spline = "eaid://environment_package/tracks/testring/ai/testring_ai_track.aispline";
        let archive = |data: Vec<u8>| {
            Erp {
                header,
                version: 3,
                resources: vec![
                    resource(&world_eaid("testring", "common"), "WEP", vec![fragment(vec![0])]),
                    resource(
                        spline,
                        "AISplineData",
                        vec![fragment(data), fragment(line("default").string_stats(1))],
                    ),
                ],
            }
            .to_bytes()
        };
        let input = PlaceInput {
            pkg: &pkg,
            dir: &dir,
            game: &dir,
            slot: None,
            cars: 22,
        };
        let edited = line("a longer racing line");
        std::fs::write(dir.join(&pkg.erp), archive(edited.to_bytes())).unwrap();
        let placement = place(&input, &dir.join("cache")).unwrap();
        assert_eq!(placement.repaired, [spline]);
        assert!(placement.dir.to_string_lossy().ends_with("-stats"));
        let placed = ErpFile::open(&placement.dir.join(&placement.archive)).unwrap();
        let res = placed.load(placed.erp.find(spline).unwrap()).unwrap();
        assert_eq!(res.fragments[1].data().unwrap(), edited.string_stats(1));
        assert!(crate::erpbuild::stale_stats_in(&placed).unwrap().is_empty());

        let mut cut = line("default").to_bytes();
        cut.truncate(cut.len() - 3);
        std::fs::write(dir.join(&pkg.erp), archive(cut)).unwrap();
        let Err(e) = place(&input, &dir.join("cache")) else {
            panic!("the package should be refused");
        };
        assert_eq!(
            e.to_string(),
            "Test Ring would crash F1 25: its testring_ai_track.aispline doesn't match the size header the game \
             reads with it, and the launcher can't read the file to fix the header. Turn Test Ring off in this \
             profile, and set any race weekend that uses it back to another track."
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn natural_groups_come_from_the_archive() {
        let t = "eaid://environment_package/tracks/okayama";
        let names = [
            format!("{t}/wep/okayama_f1.wep"),
            format!("{t}/wep/okayama_common.wep?event_graph"),
            format!("{t}/wep/okayama_common.wep"),
            format!("{t}/probes/probes_garage_02.shprobes"),
            format!("{t}/probes/key0_2019.diffuse.dds"),
            format!("{t}/ai/f1_2025_vehicle_package/mclaren/mclaren.acpd"),
            format!("{t}/ai/f1_2025_vehicle_package/mclaren/mclaren.brpd"),
            format!("{t}/ai/okayama_ai_track.aispline"),
            "eaid://environment_package/tracks/sepang/wep/sepang_f1.wep".into(),
        ];
        let g = "asset_groups/environment_package/tracks/okayama";
        assert_eq!(
            track_groups("okayama", names.iter().map(String::as_str)),
            [
                format!("{g}/okayama.assetgroup"),
                format!("{g}/wep/okayama_common.assetgroup"),
                format!("{g}/wep/okayama_f1.assetgroup"),
                format!("{g}/bake/okayama_ao.bake.assetgroup"),
                format!("{g}/bake/okayama_light_list.bake.assetgroup"),
                format!("{g}/bake/okayama_radiance.bake.assetgroup"),
                format!("{g}/probes/config.ddgi.assetgroup"),
                format!("{g}/probes/key_2019.assetgroup"),
                format!("{g}/probes/probes_garage_02.assetgroup"),
                format!("{g}/ai/f1_2025_vehicle_package/mclaren/mclaren.assetgroup"),
            ]
        );
        assert_eq!(
            track_file("okayama"),
            "2025_asset_groups/environment_package/tracks/okayama/okayama.erp"
        );
    }
}
