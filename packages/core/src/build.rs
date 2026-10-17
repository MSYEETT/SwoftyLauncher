use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use swofty_formats::gvdb::Gvdb;

use crate::activation::db::{Registry, Slot, activate};
use crate::activation::ers::{ErsMap, build_ers};
use crate::activation::place::{PlaceInput, SLOTS, place, slot_asset_group, slot_asset_root, slot_file};
use crate::activation::ui::{MapUi, build_ui, plan_lists, stamp as ui_stamp};
use crate::baseline::{Baseline, Objects, hex, sha256_file};
use crate::builtin::{CORE_ID, LOADER_ENV, loader_dir, loader_files};
use crate::erpbuild::{ErpBuild, source_archives, unsupported};
use crate::install::{Game, LOADER_DIR, Plan};
use crate::manifest::AddonManifest;
use crate::map::TrackPackage;
use crate::ops::{DbOps, ErpOps, apply_db_ops};
use crate::owned;
use crate::profile::Profile;
use crate::resolver::resolve;
use crate::settings::{SessionSettings, SessionUi, render_ops, screen_file, screens};
use crate::store::Store;
use crate::ui::{ERPS as MENU_ERPS, READS as MENU_READS, ui_file};
use crate::{Error, Result, io_err};

pub const GVDB: &str = "game_package/gamedatabases/gamedatabases.2025.bin";
pub const PLAN_FILE: &str = "plan.json";

#[derive(Debug, Clone, Default)]
pub struct Built {
    pub buildhash: String,
    pub overlay: PathBuf,
    pub order: Vec<String>,
    pub plan: Plan,
    pub lines: Vec<String>,
    pub warnings: Vec<String>,
    pub settings: Option<SessionSettings>,
}

pub fn original(data: &Path, game: &Game, rel: &str) -> PathBuf {
    let baseline = Baseline::load_or_new(data, &game.identity).ok();
    match baseline.and_then(|b| b.files.get(rel).cloned()) {
        Some(rec) => Objects {
            root: data.join("baseline/objects"),
        }
        .path(&rec.sha256),
        None => game.dir.join(rel),
    }
}

pub fn map_packages(store: &Store, codes: &[String]) -> Result<Vec<(PathBuf, TrackPackage)>> {
    let all = store.maps()?;
    codes
        .iter()
        .map(|code| {
            let pkg = all
                .iter()
                .find(|p| &p.track_code == code)
                .cloned()
                .ok_or_else(|| Error::Resolve(format!("map {code} is enabled but not imported")))?;
            Ok((store.maps_dir().join(code), pkg))
        })
        .collect()
}

pub fn screen_files(order: &[&AddonManifest], addons_dir: &Path) -> Result<BTreeMap<String, PathBuf>> {
    Ok(screens(order.iter().copied())?
        .into_iter()
        .map(|(m, s)| {
            (
                format!("{LOADER_DIR}/{}", screen_file(&s.name)),
                addons_dir.join(&m.id).join(&s.file),
            )
        })
        .collect())
}

fn render_file<T: serde::de::DeserializeOwned>(
    path: &Path,
    m: &AddonManifest,
    p: &Profile,
    hasher: &mut Sha256,
) -> Result<T> {
    let json = |source| Error::Json {
        path: path.to_owned(),
        source,
    };
    let raw = std::fs::read(path).map_err(io_err(path))?;
    let mut doc: serde_json::Value = serde_json::from_slice(&raw).map_err(json)?;
    render_ops(&mut doc, m, p)?;
    hasher.update(serde_json::to_vec(&doc).expect("serialisable"));
    serde_json::from_value(doc).map_err(json)
}

pub fn build(store: &Store, profile: &str, game: &Game) -> Result<Built> {
    let addons = store.addons()?;
    let p = store.profile(profile)?;
    let resolved = resolve(&p, &addons)?;
    let mut built = Built {
        order: resolved.order.clone(),
        warnings: resolved.warnings.clone(),
        plan: Plan {
            profile: profile.into(),
            ..Plan::default()
        },
        ..Built::default()
    };
    if p.is_vanilla() {
        built.plan = Plan::vanilla();
        return Ok(built);
    }
    let mut hasher = Sha256::new();
    let mut db_ops = Vec::new();
    let mut erp_ops = Vec::new();
    for id in &resolved.order {
        let m = &addons[id];
        hasher.update(format!("{id}@{};", m.version));
        let dir = store.addons_dir().join(id);
        for rel in &m.operations.database {
            let ops: DbOps = render_file(&dir.join(rel), m, &p, &mut hasher)?;
            db_ops.push((id.clone(), ops));
        }
        let mut erps = Vec::new();
        for rel in &m.operations.erp {
            let ops: ErpOps = render_file(&dir.join(rel), m, &p, &mut hasher)?;
            for src in source_archives(&ops) {
                let path = dir.join(src);
                let meta = std::fs::metadata(&path).map_err(io_err(&path))?;
                hasher.update(format!("{src}|{}|{:?}", meta.len(), meta.modified().ok()));
            }
            erps.push(ops);
        }
        match erps.iter().find_map(unsupported) {
            Some(op) => built
                .warnings
                .push(format!("{id}: ERP operations skipped; {op} is not built yet")),
            None if !erps.is_empty() => erp_ops.push((id.clone(), dir.clone(), erps)),
            None => {}
        }
        for s in &m.operations.screens {
            let path = dir.join(&s.file);
            hasher.update(std::fs::read(&path).map_err(io_err(&path))?);
        }
        let pending = m.operations.ui.len() + m.operations.files.len();
        if pending > 0 {
            built.warnings.push(format!(
                "{id}: {pending} UI/file operation file(s) skipped; that stage is not built yet"
            ));
        }
    }
    let maps = map_packages(store, &p.maps)?;
    let reg = Registry::builtin();
    hasher.update(crate::activation::db::REGISTRY_JSON);
    for (dir, pkg) in &maps {
        hasher.update(serde_json::to_vec(pkg).expect("serialisable"));
        let erp = dir.join(&pkg.erp);
        let meta = std::fs::metadata(&erp).map_err(io_err(&erp))?;
        hasher.update(format!("{}|{:?}", meta.len(), meta.modified().ok()));
    }
    let resolve = |rel: &str| original(&store.root, game, rel);
    let ui_maps: Vec<MapUi> = maps
        .iter()
        .zip(SLOTS)
        .filter(|((_, pkg), _)| pkg.ui.is_some())
        .map(|((dir, pkg), slot)| MapUi {
            pkg,
            dir,
            track_slot: slot,
        })
        .collect();
    if !ui_maps.is_empty() {
        hasher.update(ui_stamp(&ui_maps, &resolve)?);
    }
    let mut session = store.session_settings(profile)?;
    let menus = loader_dir().is_some();
    if menus {
        hasher.update(serde_json::to_vec(&session).expect("serialisable"));
        for rel in MENU_ERPS.iter().chain(MENU_READS) {
            let p = resolve(&ui_file(rel));
            let meta = std::fs::metadata(&p).map_err(io_err(&p))?;
            hasher.update(format!("{rel}|{}|{:?}", meta.len(), meta.modified().ok()));
        }
    }
    let db_src = original(&store.root, game, GVDB);
    hasher.update(sha256_file(&db_src)?);
    built.buildhash = hex(&hasher.finalize())[..16].to_owned();
    built.overlay = store.overlay_dir(profile, &built.buildhash);
    if maps.len() > SLOTS.len() {
        return Err(Error::Map(format!(
            "at most {} maps can be enabled at once",
            SLOTS.len()
        )));
    }
    let (ui_lists, ui_notes) = if ui_maps.is_empty() {
        Default::default()
    } else {
        plan_lists(&ui_maps, &resolve)?
    };
    built
        .warnings
        .extend(ui_notes.into_iter().map(|n| format!("menus: {n}")));
    if !maps.is_empty() || !db_ops.is_empty() {
        let shipped = std::fs::read(&db_src).map_err(io_err(&db_src))?;
        let mut db = Gvdb::parse(&shipped)?;
        for ((_, pkg), slot) in maps.iter().zip(SLOTS) {
            let group = slot_asset_group(slot);
            let root = slot_asset_root();
            let key = format!("{slot}.assetgroup");
            let route = Slot {
                asset_group: &group,
                asset_root: &root,
                key: &key,
            };
            let lists = ui_lists.get(&pkg.track_code).cloned().unwrap_or_default();
            let report = activate(&mut db, pkg, &reg, Some(&route), &lists)?;
            for l in report.lines {
                built.lines.push(format!("{}: {l}", pkg.track_code));
            }
            for w in report.warnings {
                built.warnings.push(format!("{}: {w}", pkg.track_code));
            }
        }
        let names: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut added = Vec::new();
        for (id, ops) in &db_ops {
            let watched = owned::watch(&db, ops);
            apply_db_ops(&mut db, ops, &names).map_err(|e| Error::Other(format!("applying {id}: {e}")))?;
            added.extend(owned::added(&db, id, watched));
        }
        if !added.is_empty() {
            let found = owned::owned_items(&db, &Gvdb::parse(&shipped)?, &added);
            built.lines.extend(found.lines);
            built.warnings.extend(found.warnings);
            session.owned_items = found.items;
        }
        let bytes = db.to_bytes();
        if bytes == shipped {
            built.lines.push("database: unchanged".into());
        } else {
            let out = built.overlay.join(GVDB);
            let dir = out.parent().expect("has parent");
            std::fs::create_dir_all(dir).map_err(io_err(dir))?;
            std::fs::write(&out, bytes).map_err(io_err(&out))?;
            built.plan.data.insert(GVDB.into(), out);
        }
        for ((dir, pkg), slot) in maps.iter().zip(SLOTS) {
            let slot_original = original(&store.root, game, &slot_file(slot));
            let placement = place(
                &PlaceInput {
                    pkg,
                    dir,
                    game: &game.dir,
                    slot,
                    slot_original: &slot_original,
                },
                &store.root.join("cache/maps").join(&pkg.track_code),
            )?;
            built.lines.push(format!(
                "{}: track archive goes in slot {slot}; {} texture(s) use its own mip files",
                pkg.track_code, placement.mips
            ));
            for (alias, to) in &placement.aliases {
                built
                    .lines
                    .push(format!("{}: world {alias} stands in with {to}", pkg.track_code));
            }
            built
                .warnings
                .extend(placement.notes.iter().map(|n| format!("{}: {n}", pkg.track_code)));
            for (rel, src) in placement.replaced {
                if built.plan.data.insert(rel.clone(), src).is_some() {
                    return Err(Error::Map(format!("{rel} is replaced twice")));
                }
            }
            for (rel, src) in placement.added {
                if built.plan.added.insert(rel.clone(), src).is_some() {
                    return Err(Error::Map(format!("{rel} is placed by two maps")));
                }
            }
        }
    }
    if !ui_maps.is_empty() {
        let ui = build_ui(&ui_maps, &resolve, &store.root.join("cache/menus"))?;
        for (code, lists) in &ui.lists {
            built.lines.push(format!(
                "{code}: menu pictures in {} list(s), minimap and track-select scenes",
                lists.len()
            ));
        }
        for (rel, src) in ui.replaced {
            if built.plan.data.insert(rel.clone(), src).is_some() {
                return Err(Error::Map(format!("{rel} is replaced twice")));
            }
        }
    }
    let ers_maps: Vec<ErsMap> = maps
        .iter()
        .filter(|(_, pkg)| !pkg.ers.is_empty())
        .map(|(_, pkg)| ErsMap {
            code: &pkg.track_code,
            base: pkg
                .activation
                .copy_from_track
                .as_deref()
                .unwrap_or(&pkg.track_code),
            values: &pkg.ers,
        })
        .collect();
    if !ers_maps.is_empty() {
        let ers = build_ers(&ers_maps, &game.dir, &resolve, &store.root.join("cache/ers"))?;
        built.lines.push(format!(
            "ers: {} handling file(s) in {} car archive(s) get the tracks' settings",
            ers.handling_files,
            ers.replaced.len()
        ));
        built
            .warnings
            .extend(ers.notes.iter().map(|n| format!("ers: {n}")));
        for (rel, src) in ers.replaced {
            if built.plan.data.insert(rel.clone(), src).is_some() {
                return Err(Error::Map(format!("{rel} is replaced twice")));
            }
        }
    }
    if !erp_ops.is_empty() {
        let staged = built.plan.data.clone();
        let base = |rel: &str| staged.get(rel).cloned().unwrap_or_else(|| resolve(rel));
        let mut erps = ErpBuild::new(&base);
        for (id, dir, files) in &erp_ops {
            let seen = erps.warnings.len();
            for ops in files {
                erps.apply(ops, dir)
                    .map_err(|e| Error::Other(format!("applying {id}: {e}")))?;
            }
            built
                .warnings
                .extend(erps.warnings[seen..].iter().map(|w| format!("{id}: {w}")));
        }
        for rel in erps.write(&built.overlay)? {
            let src = built.overlay.join(&rel);
            if built.plan.data.insert(rel.clone(), src).is_some() && !staged.contains_key(&rel) {
                return Err(Error::Other(format!("{rel} is replaced twice")));
            }
        }
    }
    match loader_dir() {
        Some(dir) => {
            built.plan.loader = loader_files(&dir)?;
            built.plan.registry_override = true;
            built.lines.push(format!(
                "loader: {} file(s) from {}",
                built.plan.loader.len(),
                dir.display()
            ));
            let order: Vec<&AddonManifest> = resolved.order.iter().map(|id| &addons[id]).collect();
            if menus {
                build_menus(&mut built, &mut session, &resolve)?;
            }
            let files = screen_files(&order, &store.addons_dir())?;
            if !files.is_empty() {
                built.lines.push(format!("ui: {} screen(s) under {LOADER_DIR}/ui", files.len()));
            }
            for (rel, src) in files {
                if built.plan.loader.insert(rel.clone(), src).is_some() {
                    return Err(Error::Other(format!("{rel} is placed twice")));
                }
            }
        }
        None => built.warnings.push(format!(
            "{CORE_ID}: the in-game loader was not found (native/dist, loader/ beside the swofty command, or {LOADER_ENV}); the game starts without it"
        )),
    }
    built.settings = Some(session);
    if !built.plan.is_vanilla() {
        let listing = serde_json::json!({
            "profile": profile,
            "buildhash": built.buildhash,
            "replace": built.plan.data,
            "add": built.plan.added,
            "loader": built.plan.loader,
        });
        crate::write_json(&built.overlay.join(PLAN_FILE), &listing)?;
    }
    Ok(built)
}

pub fn build_menus(
    built: &mut Built,
    session: &mut SessionSettings,
    original: &dyn Fn(&str) -> PathBuf,
) -> Result<()> {
    let input = |rel: &str| built.plan.data.get(rel).cloned().unwrap_or_else(|| original(rel));
    let menus = crate::ui::build(session, &input, &built.overlay)?;
    built.lines.push(format!(
        "ui: ADDON SETTINGS with {} screen(s) under {LOADER_DIR}/ui, {} game archive(s) patched",
        menus.session.len(),
        menus.erps.len()
    ));
    built.plan.data.extend(menus.erps);
    for (rel, src) in menus.screens {
        if built.plan.loader.insert(rel.clone(), src).is_some() {
            return Err(Error::Other(format!("{rel} is placed twice")));
        }
    }
    let ui = session.ui.get_or_insert_with(|| SessionUi {
        register: true,
        strict: false,
        recover: true,
        screens: Vec::new(),
    });
    if let Some(s) = ui
        .screens
        .iter()
        .find(|s| menus.session.iter().any(|m| m.name == s.name))
    {
        return Err(Error::Other(format!(
            "screen {} is reserved for ADDON SETTINGS; rename it in its mod",
            s.name
        )));
    }
    ui.screens.splice(0..0, menus.session);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_screens_under_the_loader_ui_dir() {
        let m = AddonManifest::parse(
            r#"{ "schema_version": 1, "id": "a.pages", "name": "S", "version": "1.0.0", "author": "a",
                 "compat": { "game": "f1_25" },
                 "operations": { "screens": [{ "name": "swofty_mod_0", "file": "ui/page0.ui" }] } }"#,
        )
        .unwrap();
        let files = screen_files(&[&m], Path::new("/addons")).unwrap();
        assert_eq!(
            files,
            BTreeMap::from([(
                "swofty/ui/swofty_mod_0.ui".to_owned(),
                PathBuf::from("/addons/a.pages/ui/page0.ui")
            )])
        );
        assert!(screen_files(&[&m, &m], Path::new("/addons")).is_err());
    }
}
