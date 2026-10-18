use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use swofty_formats::gvdb::Gvdb;

use crate::activation::db::{Registry, TrackHost, activate};
use crate::activation::ers::{ErsMap, build_ers};
use crate::activation::grid::grid_size;
use crate::activation::place::{
    PlaceInput, SLOTS, place, slot_asset_group, slot_asset_root, slot_file, track_group,
};
use crate::activation::ui::{MapUi, build_ui, plan_lists, stamp as ui_stamp};
use crate::asset::{
    AssetOp, FileKind, FileOp, GroupOp, ItemOp, LangOp, Op, OpFile, Origin, Root, Source, TrackOp,
};
use crate::baseline::{Baseline, Objects, hex, sha256_file};
use crate::builtin::{CORE_ID, LOADER_ENV, loader_dir, loader_files};
use crate::erpbuild::{ErpBuild, unsupported};
use crate::install::{Game, LOADER_DIR, Plan};
use crate::launcher::BuildCx;
use crate::manifest::{AddonManifest, relative_inside};
use crate::map::TrackPackage;
use crate::owned;
use crate::planner::{Conflict, Features, Kind, Planner, Route, load_features, waiting_lines};
use crate::profile::Profile;
use crate::registry::Host;
use crate::resolver::resolve;
use crate::runtime::{Runtime, db_edits};
use crate::settings::{SessionMod, SessionSettings, SessionUi, render_ops, screen_file, screens};
use crate::store::Store;
use crate::ui::{ERPS as MENU_ERPS, READS as MENU_READS, ui_file};
use crate::{Error, Result, io_err};

pub const GVDB: &str = "game_package/gamedatabases/gamedatabases.2025.bin";
pub const PLAN_FILE: &str = "plan.json";
pub const ADDONS_DIR: &str = "addons";

#[derive(Debug, Clone, Default)]
pub struct Built {
    pub buildhash: String,
    pub overlay: PathBuf,
    pub order: Vec<String>,
    pub plan: Plan,
    pub lines: Vec<String>,
    pub warnings: Vec<String>,
    pub settings: Option<SessionSettings>,
    pub conflicts: Vec<Conflict>,
    pub waiting: Vec<Op>,
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

pub fn track_packages(store: &Store, ops: Vec<Op>) -> Result<Vec<(PathBuf, TrackPackage, Origin)>> {
    let mut out: Vec<(PathBuf, TrackPackage, Origin)> = Vec::new();
    for op in ops {
        let AssetOp::Track(TrackOp::Add { source }) = op.op else {
            continue;
        };
        let dir = source.resolve(&source_dir(store, &op.origin.addon, source.root))?;
        let path = dir.join(crate::map::MANIFEST_FILE);
        let pkg = TrackPackage::parse(&std::fs::read(&path).map_err(io_err(&path))?)?;
        if out.iter().any(|(_, p, _)| p.track_code == pkg.track_code) {
            return Err(Error::Map(format!("track {} is added twice", pkg.track_code)));
        }
        out.push((dir, pkg, op.origin));
    }
    Ok(out)
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

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plugins {
    pub files: BTreeMap<String, PathBuf>,
    pub dlls: Vec<String>,
}

pub fn plugin_files<'a>(
    store: &Store,
    order: impl IntoIterator<Item = &'a AddonManifest>,
    taken: impl IntoIterator<Item = String>,
) -> Result<Plugins> {
    let mut out = Plugins::default();
    let mut names: BTreeMap<String, String> = taken
        .into_iter()
        .map(|n| (n.to_ascii_lowercase(), CORE_ID.to_owned()))
        .collect();
    for m in order
        .into_iter()
        .filter(|m| !m.operations.native.is_empty() && !store.is_builtin(&m.id))
    {
        let dir = store.addons_dir().join(&m.id);
        for n in &m.operations.native {
            let name = n.dll.rsplit('/').next().unwrap_or(&n.dll);
            if let Some(other) = names.insert(name.to_ascii_lowercase(), m.id.clone()) {
                return Err(Error::Other(format!(
                    "{}: plugin {name} has the same file name as a plugin of {other}; rename one",
                    m.id
                )));
            }
            out.dlls.push(format!("{ADDONS_DIR}/{}/{}", m.id, n.dll));
            for rel in std::iter::once(&n.dll).chain(&n.files) {
                let rel = rel.trim_end_matches('/');
                let src = dir.join(rel);
                let found = if src.is_dir() {
                    loader_files(&src)?
                        .into_iter()
                        .filter(|(r, _)| !r.split('/').any(|p| p.starts_with('.')))
                        .map(|(r, p)| (format!("{rel}/{r}"), p))
                        .collect()
                } else if src.is_file() {
                    BTreeMap::from([(rel.to_owned(), src)])
                } else {
                    return Err(Error::Other(format!(
                        "{}: {rel} is missing; build the plugin and install the addon again",
                        m.id
                    )));
                };
                for (rel, src) in found {
                    out.files
                        .insert(format!("{LOADER_DIR}/{ADDONS_DIR}/{}/{rel}", m.id), src);
                }
            }
        }
    }
    Ok(out)
}

pub fn source_dir(store: &Store, addon: &str, root: Root) -> PathBuf {
    match root {
        Root::Addon => store.addons_dir().join(addon),
        Root::Data => store.addon_data_dir(addon),
        Root::Cache => store.addon_cache_dir(addon),
    }
}

fn render_file(
    path: &Path,
    kind: FileKind,
    m: &AddonManifest,
    p: &Profile,
    hasher: &mut Sha256,
) -> Result<OpFile> {
    let json = |source| Error::Json {
        path: path.to_owned(),
        source,
    };
    let raw = std::fs::read(path).map_err(io_err(path))?;
    let mut doc: serde_json::Value = serde_json::from_slice(&raw).map_err(json)?;
    render_ops(&mut doc, m, p)?;
    hasher.update(serde_json::to_vec(&doc).expect("serialisable"));
    OpFile::from_value(doc, kind).map_err(json)
}

fn unbuilt<'a>(ops: impl IntoIterator<Item = &'a AssetOp>) -> Option<&'static str> {
    ops.into_iter().find_map(|op| match op {
        AssetOp::Erp(op) => unsupported(op),
        _ => None,
    })
}

fn hash_sources(hasher: &mut Sha256, store: &Store, addon: &str, ops: &[AssetOp]) -> Result<()> {
    for src in ops.iter().flat_map(AssetOp::sources) {
        let path = src.resolve(&source_dir(store, addon, src.root))?;
        let meta = std::fs::metadata(&path).map_err(io_err(&path))?;
        hasher.update(format!("{}|{}|{:?}", src.path, meta.len(), meta.modified().ok()));
        if meta.is_dir() {
            for (rel, p) in loader_files(&path)? {
                let meta = std::fs::metadata(&p).map_err(io_err(&p))?;
                hasher.update(format!("{rel}|{}|{:?}", meta.len(), meta.modified().ok()));
            }
        }
    }
    Ok(())
}

pub fn build(store: &Store, profile: &str, game: &Game) -> Result<Built> {
    let registry = store.registry()?;
    let addons = registry.manifests();
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
    let features = loader_dir()
        .map(|d| load_features(&d))
        .transpose()?
        .unwrap_or_default();
    let config = store.config();
    let mut planner = Planner::new(&features, &config.runtime_assets).build_only(&config.build_assets);
    if features != Features::default() {
        hasher.update(format!("runtime {features:?} {:?};", config.runtime_assets));
        if !config.build_assets.is_empty() {
            hasher.update(format!("build {:?};", config.build_assets));
        }
    }
    let mut runtime = Runtime::new(planner.features());
    for id in &resolved.order {
        let m = &addons[id];
        hasher.update(format!("{id}@{};", m.version));
        let dir = store.addons_dir().join(id);
        let origin = Origin {
            addon: id.clone(),
            item: None,
        };
        let mut files = Vec::new();
        for (kind, rel) in m.operations.op_files() {
            let file = render_file(&dir.join(rel), kind, m, &p, &mut hasher)?;
            hash_sources(&mut hasher, store, id, &file.ops)?;
            files.push((kind, file));
        }
        let declared: Vec<AssetOp> = m.operations.file_ops().collect();
        hash_sources(&mut hasher, store, id, &declared)?;
        if let Some(op) = unbuilt(files.iter().flat_map(|(_, f)| &f.ops)) {
            built
                .warnings
                .push(format!("{id}: ERP operations skipped; {op} is not built yet"));
            for (_, f) in &mut files {
                f.ops
                    .retain(|op| !matches!(op, AssetOp::Erp(_) | AssetOp::Warn { .. }));
            }
        }
        for (kind, file) in files {
            planner.push_file(&origin, kind, file);
        }
        for op in declared {
            planner.push(Op {
                op,
                origin: origin.clone(),
            });
        }
        if let Some(entry) = registry.get(id).filter(|e| !e.declared) {
            let host = Host::new(store, id, &p, Some(game))?;
            let options = m.options.iter().map(|o| (o.id.clone(), p.option(m, o))).collect();
            let batch = entry
                .addon
                .build(&BuildCx {
                    cx: host.cx(),
                    options: &options,
                    features: &features,
                })
                .map_err(|e| Error::Other(format!("building {id}: {e}")))?;
            hasher.update(serde_json::to_vec(&batch.ops).expect("serialisable"));
            let ops: Vec<AssetOp> = batch.ops.iter().map(|op| op.op.clone()).collect();
            hash_sources(&mut hasher, store, id, &ops)?;
            built
                .lines
                .extend(batch.lines.iter().map(|l| format!("{id}: {l}")));
            built
                .warnings
                .extend(batch.warnings.iter().map(|w| format!("{id}: {w}")));
            let mut ops = batch.ops;
            if let Some(op) = unbuilt(ops.iter().map(|o| &o.op)) {
                built
                    .warnings
                    .push(format!("{id}: ERP operations skipped; {op} is not built yet"));
                ops.retain(|o| !matches!(o.op, AssetOp::Erp(_)));
            }
            for op in ops {
                planner.push(op);
            }
        }
        for s in &m.operations.screens {
            let path = dir.join(&s.file);
            hasher.update(std::fs::read(&path).map_err(io_err(&path))?);
        }
        let pending = m.operations.ui.len();
        if pending > 0 {
            built.warnings.push(format!(
                "{id}: {pending} UI operation file(s) skipped; that stage is not built yet"
            ));
        }
    }
    let tracks = track_packages(store, planner.take_tracks())?;
    let maps: Vec<(PathBuf, TrackPackage)> = tracks.iter().map(|(d, p, _)| (d.clone(), p.clone())).collect();
    let mods: Vec<SessionMod> = planner
        .take_items()
        .into_iter()
        .filter_map(|op| match op.op {
            AssetOp::Item(ItemOp::Show {
                id,
                name,
                author,
                version,
                description,
            }) => Some(SessionMod {
                id,
                name,
                version,
                author,
                description,
            }),
            _ => None,
        })
        .collect();
    let natural = planner.route(Kind::Group) == Route::Runtime;
    if !natural && maps.len() > SLOTS.len() {
        return Err(Error::Map(format!(
            "at most {} maps can be enabled at once",
            SLOTS.len()
        )));
    }
    let slots: Vec<Option<&str>> = (0..maps.len()).map(|i| (!natural).then(|| SLOTS[i])).collect();
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
        .zip(&slots)
        .filter(|((_, pkg), _)| pkg.ui.is_some())
        .map(|((dir, pkg), slot)| MapUi {
            pkg,
            dir,
            asset_group: match slot {
                Some(slot) => slot_asset_group(slot),
                None => track_group(&pkg.track_code, &pkg.track_code),
            },
        })
        .collect();
    if !ui_maps.is_empty() {
        hasher.update(ui_stamp(&ui_maps, &resolve)?);
    }
    let mut caches = Vec::new();
    let mut session = store.session_settings(profile)?;
    session.set_maps(maps.iter().map(|(_, pkg)| pkg));
    session.set_mods(mods.into_iter());
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
    let (ui_lists, ui_notes) = if ui_maps.is_empty() {
        Default::default()
    } else {
        plan_lists(&ui_maps, &resolve)?
    };
    built
        .warnings
        .extend(ui_notes.into_iter().map(|n| format!("menus: {n}")));
    if !maps.is_empty() || planner.has_db() {
        let shipped = std::fs::read(&db_src).map_err(io_err(&db_src))?;
        let mut db = Gvdb::parse(&shipped)?;
        for ((_, pkg), slot) in maps.iter().zip(&slots) {
            let names = slot.map(|s| (slot_asset_group(s), format!("{s}.assetgroup")));
            let root = slot_asset_root();
            let route = match &names {
                Some((asset_group, key)) => TrackHost::Slot {
                    asset_group,
                    asset_root: &root,
                    key,
                },
                None => TrackHost::Natural,
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
        let staged = (planner.route(Kind::Db) == Route::Runtime && planner.has_db()).then(|| db.clone());
        let added = planner.apply_db(&mut db)?;
        if !added.is_empty() {
            let found = owned::owned_items(&db, &Gvdb::parse(&shipped)?, &added);
            built.lines.extend(found.lines);
            built.warnings.extend(found.warnings);
            session.owned_items = found.items;
        }
        let cars = grid_size(&db);
        if let Some(staged) = staged {
            match db_edits(&staged, &db) {
                Ok(edits) => {
                    runtime.db(edits);
                    db = staged;
                }
                Err(why) => built
                    .warnings
                    .push(format!("database: built into the game file; {why}")),
            }
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
        for ((dir, pkg, origin), slot) in tracks.iter().zip(&slots) {
            let slot_original = slot.map(|s| original(&store.root, game, &slot_file(s)));
            let placement = place(
                &PlaceInput {
                    pkg,
                    dir,
                    game: &game.dir,
                    slot: slot.zip(slot_original.as_deref()),
                    cars,
                },
                &store.root.join("cache/maps").join(&pkg.track_code),
            )?;
            caches.push(placement.dir.clone());
            let at = match slot {
                Some(slot) => format!("slot {slot}"),
                None => placement.archive.clone(),
            };
            built.lines.push(format!(
                "{}: track archive goes in {at}; {} texture(s) use its own mip files",
                pkg.track_code, placement.mips
            ));
            if !placement.groups.is_empty() {
                built.lines.push(format!(
                    "{}: {} asset group(s) registered in game",
                    pkg.track_code,
                    placement.groups.len()
                ));
            }
            for group in placement.groups {
                planner.push(Op {
                    op: AssetOp::Group(GroupOp::Register {
                        group,
                        erp: Some(placement.archive.clone()),
                        erps: Vec::new(),
                        deps: Vec::new(),
                        template: None,
                    }),
                    origin: origin.clone(),
                });
            }
            if let Some(g) = &placement.grid {
                built.lines.push(format!(
                    "{}: world extended from {} to {} grid slots, {} to {} garages and {} to {} pit boxes for {cars} cars",
                    pkg.track_code,
                    g.before.grid_slots,
                    g.after.grid_slots,
                    g.before.garages,
                    g.after.garages,
                    g.before.pit_boxes,
                    g.after.pit_boxes
                ));
            }
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
        caches.push(ui.dir.clone());
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
        caches.extend(ers.dir.clone());
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
    let erp_ops = planner.take_erp();
    let mut mipmaps = BTreeMap::new();
    if !erp_ops.is_empty() {
        let staged = built.plan.data.clone();
        let base = |rel: &str| staged.get(rel).cloned().unwrap_or_else(|| resolve(rel));
        let mut erps = ErpBuild::new(&base);
        for op in &erp_ops {
            let id = &op.origin.addon;
            let resolve = |s: &Source| Ok(s.resolve(&source_dir(store, id, s.root))?);
            match &op.op {
                AssetOp::Erp(erp) => erps
                    .apply_for(erp, &op.origin.to_string(), &resolve)
                    .map_err(|e| Error::Other(format!("applying {}: {e}", op.origin)))?,
                AssetOp::Warn { message } => built.warnings.push(format!("{}: {message}", op.origin)),
                _ => {}
            }
        }
        mipmaps = erps.take_mipmaps();
        if let Some(note) = planner.route_note(Kind::Erp) {
            built.lines.push(format!("erp: {note}"));
        }
        if planner.route(Kind::Erp) == Route::Runtime {
            let notes = runtime.resources(erps.resources(), erps.origins(), &built.overlay)?;
            built.warnings.extend(notes);
        } else {
            for rel in erps.write(&built.overlay)? {
                let src = built.overlay.join(&rel);
                if built.plan.data.insert(rel.clone(), src).is_some() && !staged.contains_key(&rel) {
                    return Err(Error::Other(format!("{rel} is replaced twice")));
                }
            }
        }
    }
    put_files(&mut built, store, game, mipmaps, planner.take_files())?;
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
                let lang = planner.route(Kind::Lang) == Route::Runtime;
                for (key, value) in build_menus(&mut built, &mut session, &resolve, lang)? {
                    planner.push(Op {
                        op: AssetOp::Lang(LangOp::Set {
                            language: "*".into(),
                            key,
                            value,
                        }),
                        origin: Origin {
                            addon: CORE_ID.into(),
                            item: None,
                        },
                    });
                }
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
            let core = format!("{LOADER_DIR}/{ADDONS_DIR}/core/");
            let taken: Vec<String> = built
                .plan
                .loader
                .keys()
                .filter_map(|rel| rel.strip_prefix(&core))
                .filter(|n| n.ends_with(".dll") && !n.contains('/'))
                .map(str::to_owned)
                .collect();
            let plugins = plugin_files(store, order.iter().copied(), taken)?;
            if !plugins.dlls.is_empty() {
                built.lines.push(format!(
                    "native: {} plugin(s), {} file(s) under {LOADER_DIR}/{ADDONS_DIR}",
                    plugins.dlls.len(),
                    plugins.files.len()
                ));
            }
            for (rel, src) in plugins.files {
                if built.plan.loader.insert(rel.clone(), src).is_some() {
                    return Err(Error::Other(format!("{rel} is placed twice")));
                }
            }
            session.plugins = plugins.dlls;
        }
        None => built.warnings.push(format!(
            "{CORE_ID}: the in-game loader was not found (native/dist, loader/ beside the swofty command, or {LOADER_ENV}); the game starts without it"
        )),
    }
    built.settings = Some(session);
    let planned = planner.finish();
    for op in planned.runtime {
        let resolve = |op: &Op, s: &Source| Ok(s.resolve(&source_dir(store, &op.origin.addon, s.root))?);
        runtime.push(op, &resolve)?;
    }
    if let Some(runtime) = runtime.finish(&built.overlay)? {
        built.lines.push(runtime.summary());
        for (rel, src) in runtime.files {
            if built.plan.loader.insert(rel.clone(), src).is_some() {
                return Err(Error::Other(format!("{rel} is placed twice")));
            }
        }
    }
    built.warnings.extend(waiting_lines(&planned.waiting, &features));
    built
        .warnings
        .extend(planned.conflicts.iter().map(|c| format!("conflict: {c}")));
    built.conflicts = planned.conflicts;
    built.waiting = planned.waiting;
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
    caches.push(built.overlay.clone());
    caches.extend(
        [&built.plan.data, &built.plan.added, &built.plan.loader]
            .into_iter()
            .flat_map(|m| m.values().cloned()),
    );
    let removed = crate::cache::prune(&store.root.join("cache"), profile, &caches);
    if removed > 0 {
        built
            .lines
            .push(format!("cache: {removed} stale build folder(s) removed"));
    }
    Ok(built)
}

fn put_files(
    built: &mut Built,
    store: &Store,
    game: &Game,
    mipmaps: BTreeMap<String, PathBuf>,
    ops: Vec<Op>,
) -> Result<()> {
    let mips = mipmaps.len();
    let mut put = mipmaps;
    for op in ops {
        let AssetOp::File(FileOp::Put { source, target }) = op.op else {
            continue;
        };
        if !relative_inside(&target) {
            return Err(Error::Resolve(format!(
                "{target} must be a relative path inside the game folder"
            )));
        }
        put.insert(
            target,
            source.resolve(&source_dir(store, &op.origin.addon, source.root))?,
        );
    }
    if put.is_empty() {
        return Ok(());
    }
    let baseline = Baseline::load_or_new(&store.root, &game.identity).ok();
    let stock = |rel: &str| {
        baseline.as_ref().is_some_and(|b| b.files.contains_key(rel)) || game.dir.join(rel).is_file()
    };
    let (mut replaced, mut added) = (0, 0);
    for (rel, src) in put {
        let into = if stock(&rel) {
            replaced += 1;
            &mut built.plan.data
        } else {
            added += 1;
            &mut built.plan.added
        };
        if into.insert(rel.clone(), src).is_some() {
            return Err(Error::Other(format!("{rel} is replaced twice")));
        }
    }
    built
        .lines
        .push(format!("files: {replaced} game file(s) replaced, {added} added"));
    if mips > 0 {
        built
            .lines
            .push(format!("mipmaps: {mips} mip file(s) go in the game folder"));
    }
    Ok(())
}

pub fn build_menus(
    built: &mut Built,
    session: &mut SessionSettings,
    original: &dyn Fn(&str) -> PathBuf,
    lang: bool,
) -> Result<Vec<(String, String)>> {
    let input = |rel: &str| built.plan.data.get(rel).cloned().unwrap_or_else(|| original(rel));
    let menus = crate::ui::build(session, &input, &built.overlay, lang)?;
    built.lines.push(format!(
        "ui: ADDON SETTINGS with {} screen(s) under {LOADER_DIR}/ui, {} game archive(s) patched",
        menus.session.len(),
        menus.erps.len()
    ));
    if !menus.strings.is_empty() {
        built.lines.push(format!(
            "ui: the online services popup reads its text from {} language string(s)",
            menus.strings.len()
        ));
    }
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
            "screen {} is reserved for ADDON SETTINGS; rename it in its addon",
            s.name
        )));
    }
    ui.screens.splice(0..0, menus.session);
    Ok(menus.strings)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::asset::{AssetBatch, Op, Origin};
    use crate::baseline::Identity;
    use crate::launcher::{Addon, Builtin};

    struct Hook(AddonManifest);

    impl Addon for Hook {
        fn manifest(&self) -> &AddonManifest {
            &self.0
        }

        fn build(&self, cx: &BuildCx) -> swofty_addon::Result<AssetBatch> {
            let origin = Origin {
                addon: cx.cx.addon.into(),
                item: None,
            };
            Ok(AssetBatch {
                ops: vec![
                    Op {
                        op: AssetOp::Warn {
                            message: format!("{} cars", cx.options["cars"]),
                        },
                        origin: origin.clone(),
                    },
                    Op {
                        op: AssetOp::Erp(crate::ops::ErpOp::PatchPkg {
                            erp: "2025_asset_groups/missing.erp".into(),
                            resource: "eaid://missing".into(),
                            fragment: 0,
                            edits: vec![crate::ops::PkgEdit::SetText {
                                node: "n".into(),
                                value: "v".into(),
                                localise: false,
                            }],
                        }),
                        origin,
                    },
                ],
                lines: vec![format!("profile {}", cx.cx.profile.name)],
                warnings: Vec::new(),
            })
        }
    }

    #[test]
    fn runs_the_build_hooks_of_builtin_addons() {
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-hook-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join(GVDB), "db").unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let manifest = AddonManifest::parse(
            r#"{ "schema_version": 1, "id": "t.hook", "name": "Hook", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" },
                 "options": [{ "id": "cars", "type": "slider", "label": "Cars", "min": 22, "max": 30, "step": 2, "default": 22, "binds": "db" }] }"#,
        )
        .unwrap();
        let store = Store::new(root.join("data")).with_builtins(vec![Builtin {
            addon: Arc::new(Hook(manifest)),
            locked: false,
            default_on: true,
        }]);
        let built = build(&store, "default", &game).unwrap();
        assert_eq!(built.order, [CORE_ID, "t.hook"]);
        assert!(built.lines.contains(&"t.hook: profile default".to_owned()));
        assert!(built.warnings.contains(&"t.hook: 22 cars".to_owned()));
        assert!(
            built
                .warnings
                .contains(&"t.hook: ERP operations skipped; set-text is not built yet".to_owned())
        );
        assert!(store.addon_data_dir("t.hook").is_dir());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn places_screens_under_the_loader_ui_dir() {
        let m = AddonManifest::parse(
            r#"{ "schema_version": 1, "id": "a.pages", "name": "S", "version": "1.0.0", "author": "a",
                 "compat": { "game": "f1_25" },
                 "operations": { "screens": [{ "name": "swofty_addon_0", "file": "ui/page0.ui" }] } }"#,
        )
        .unwrap();
        let files = screen_files(&[&m], Path::new("/addons")).unwrap();
        assert_eq!(
            files,
            BTreeMap::from([(
                "swofty/ui/swofty_addon_0.ui".to_owned(),
                PathBuf::from("/addons/a.pages/ui/page0.ui")
            )])
        );
        assert!(screen_files(&[&m, &m], Path::new("/addons")).is_err());
    }
}
