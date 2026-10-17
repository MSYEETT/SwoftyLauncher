use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use swofty_core::baseline::Identity;
use swofty_core::install::{Game, Installer};
use swofty_core::ops::{DbOps, apply_db_ops};
use swofty_core::resolver::resolve;
use swofty_core::settings::{SETTINGS_FILE, render_ops};
use swofty_core::store::Store;
use swofty_core::{GAME_EXE, steam};
use swofty_formats::erp::Erp;
use swofty_formats::gvdb::Gvdb;

pub const GVDB: &str = "game_package/gamedatabases/gamedatabases.2025.bin";
pub const UI_ERPS: &[&str] = &[
    "2025_asset_groups/ui_package/flow_frontend.erp",
    "2025_asset_groups/ui_package/persistent.erp",
    "2025_asset_groups/ui_package/databases.erp",
    "2025_asset_groups/ui_package/screens/fe_home.erp",
];
pub fn game(store: &Store, explicit: Option<PathBuf>) -> Result<Game> {
    let install = steam::find_game();
    let dir = game_dir(explicit)?;
    let found = install.filter(|g| g.dir == dir);
    let user_reg = found
        .as_ref()
        .and_then(|g| g.prefix())
        .map(|p| p.join("user.reg"))
        .filter(|p| p.is_file());
    let identity = Identity::detect_cached(&store.root, &dir, found.and_then(|g| g.build_id))?;
    Ok(Game {
        dir,
        user_reg,
        identity,
    })
}

fn game_dir(explicit: Option<PathBuf>) -> Result<PathBuf> {
    explicit
        .or_else(|| steam::find_game().map(|g| g.dir))
        .context("F1 25 not found; pass --game or run swofty doctor")
}

pub fn build(store: &Store, profile: &str, game: Option<PathBuf>) -> Result<ExitCode> {
    let mods = store.mods()?;
    let mut p = store.profile(profile)?;
    let plan = resolve(&p, &mods)?;
    println!("profile {profile}: {}", plan.order.join(" -> "));
    for w in &plan.warnings {
        println!("warning: {w}");
    }
    let mut hasher = Sha256::new();
    let mut db_ops = Vec::new();
    for id in &plan.order {
        let m = &mods[id];
        hasher.update(format!("{id}@{};", m.version));
        let dir = store.mods_dir().join(id);
        for rel in &m.operations.database {
            let path = dir.join(rel);
            let mut doc: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).with_context(|| format!("{id}: {rel}"))?)
                    .with_context(|| format!("{id}: {rel}"))?;
            render_ops(&mut doc, m, &p).with_context(|| format!("{id}: {rel}"))?;
            hasher.update(serde_json::to_vec(&doc)?);
            let ops: DbOps = serde_json::from_value(doc).with_context(|| format!("{id}: {rel}"))?;
            db_ops.push((id.clone(), ops));
        }
        let pending = m.operations.erp.len() + m.operations.ui.len() + m.operations.files.len();
        if pending > 0 {
            println!("{id}: {pending} ERP/UI/file operation file(s) skipped; that stage is not built yet");
        }
    }
    let buildhash = format!("{:x}", hasher.finalize())[..16].to_owned();
    let overlay = store.overlay_dir(profile, &buildhash);
    if p.is_vanilla() {
        println!("{profile} plays without mods; nothing to build");
        return Ok(ExitCode::SUCCESS);
    }
    let settings = overlay.join(SETTINGS_FILE);
    std::fs::create_dir_all(settings.parent().expect("has parent"))?;
    std::fs::write(
        &settings,
        serde_json::to_vec_pretty(&store.session_settings(profile)?)?,
    )?;
    println!("overlay {buildhash}: wrote {}", settings.display());
    if !db_ops.is_empty() {
        let game = game_dir(game)?;
        let mut db = Gvdb::parse(&std::fs::read(game.join(GVDB)).context("reading the game database")?)?;
        let names: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (id, ops) in &db_ops {
            apply_db_ops(&mut db, ops, &names).with_context(|| format!("applying {id}"))?;
        }
        let out = overlay.join(GVDB);
        std::fs::create_dir_all(out.parent().expect("has parent"))?;
        std::fs::write(&out, db.to_bytes())?;
        println!("overlay {buildhash}: wrote {}", out.display());
    }
    p.mark_built(&mods);
    store.save_profile(&p)?;
    Ok(ExitCode::SUCCESS)
}

pub fn verify(store: &Store, game: Option<PathBuf>) -> Result<ExitCode> {
    let g = self::game(store, game)?;
    let drift = Installer::new(&store.root, &g).verify()?;
    let game = g.dir;
    let mut ok = drift.is_empty();
    for d in &drift {
        println!("DRIFT {d}");
    }
    let mut check = |name: &str, result: Result<bool>| match result {
        Ok(true) => println!("ok    {name}"),
        Ok(false) => {
            ok = false;
            println!("FAIL  {name}: round trip differs");
        }
        Err(e) => {
            ok = false;
            println!("FAIL  {name}: {e:#}");
        }
    };
    check(
        GVDB,
        read(&game, GVDB).and_then(|d| Ok(Gvdb::parse(&d)?.to_bytes() == d)),
    );
    for rel in UI_ERPS {
        check(
            rel,
            read(&game, rel).and_then(|d| Ok(Erp::parse(&d)?.to_bytes() == d)),
        );
    }
    if !game.join(GAME_EXE).is_file() {
        ok = false;
        println!("FAIL  {GAME_EXE} not found in {}", game.display());
    }
    Ok(if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

fn read(game: &Path, rel: &str) -> Result<Vec<u8>> {
    std::fs::read(game.join(rel)).with_context(|| rel.to_owned())
}

pub fn restore(store: &Store, game: Option<PathBuf>) -> Result<ExitCode> {
    let g = self::game(store, game)?;
    let left = Installer::new(&store.root, &g).restore()?;
    if left.is_empty() {
        println!(
            "restored: the game folder matches the baseline for build {}",
            g.identity.build_id
        );
        return Ok(ExitCode::SUCCESS);
    }
    for d in &left {
        println!("not restored: {d}");
    }
    Ok(ExitCode::FAILURE)
}
