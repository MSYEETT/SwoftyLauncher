use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use swofty_core::baseline::Identity;
use swofty_core::build::GVDB;
use swofty_core::install::{Game, Installer};
use swofty_core::settings::SETTINGS_FILE;
use swofty_core::store::Store;
use swofty_core::{GAME_EXE, steam};
use swofty_formats::erp::Erp;
use swofty_formats::gvdb::Gvdb;
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
    let g = self::game(store, game)?;
    let built = build_profile(store, profile, &g)?;
    report(profile, &built);
    Ok(ExitCode::SUCCESS)
}

fn build_profile(store: &Store, profile: &str, g: &Game) -> Result<swofty_core::build::Built> {
    let addons = store.addons()?;
    let mut p = store.profile(profile)?;
    let mut built = swofty_core::build::build(store, profile, g)?;
    if p.is_vanilla() {
        return Ok(built);
    }
    let settings = built.overlay.join(SETTINGS_FILE);
    std::fs::create_dir_all(settings.parent().expect("has parent"))?;
    let session = match built.settings.take() {
        Some(s) => s,
        None => store.session_settings(profile)?,
    };
    std::fs::write(&settings, serde_json::to_vec_pretty(&session)?)?;
    if !built.plan.loader.is_empty() {
        built.plan.loader.insert(SETTINGS_FILE.into(), settings);
    }
    p.mark_built(&addons);
    store.update_profile(profile, |fresh| {
        fresh.applied = p.applied;
        Ok(())
    })?;
    Ok(built)
}

fn report(profile: &str, built: &swofty_core::build::Built) {
    if built.plan.is_vanilla() {
        println!("{profile} plays without addons; nothing to build");
        return;
    }
    println!("profile {profile}: {}", built.order.join(" -> "));
    for l in &built.lines {
        println!("{l}");
    }
    for w in &built.warnings {
        println!("warning: {w}");
    }
    println!("overlay {}: {}", built.buildhash, built.overlay.display());
    for rel in built.plan.data.keys() {
        println!("  replace {rel}");
    }
    for rel in built.plan.loader.keys() {
        println!("  place {rel}");
    }
    let added = &built.plan.added;
    if !added.is_empty() {
        println!(
            "  add {} file(s) under {}",
            added.len(),
            common_root(added.keys())
        );
    }
}

fn common_root<'a>(paths: impl Iterator<Item = &'a String>) -> String {
    let mut roots: Vec<&str> = paths.filter_map(|p| p.split('/').next()).collect();
    roots.sort();
    roots.dedup();
    roots.join(", ")
}

pub fn install(store: &Store, profile: &str, g: &Game) -> Result<()> {
    let built = build_profile(store, profile, g)?;
    report(profile, &built);
    Installer::new(&store.root, g).apply(&built.plan)?;
    Ok(())
}

pub fn apply(store: &Store, profile: &str, game: Option<PathBuf>) -> Result<ExitCode> {
    let g = self::game(store, game)?;
    install(store, profile, &g)?;
    println!(
        "applied {profile} to {}; swofty restore puts the game back",
        g.dir.display()
    );
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
    match swofty_core::offline::restore_pending(&store.root) {
        Ok(true) => println!("restored: offline play staging"),
        Ok(false) => {}
        Err(e) => bail!("offline play staging is not restorable: {e:#}"),
    }
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use swofty_core::asset::AssetBatch;
    use swofty_core::builtin::loader_dir;
    use swofty_core::launcher::{Addon, BuildCx, Builtin};
    use swofty_core::manifest::AddonManifest;

    use super::*;

    struct Writer(AddonManifest);

    impl Addon for Writer {
        fn manifest(&self) -> &AddonManifest {
            &self.0
        }

        fn build(&self, cx: &BuildCx) -> swofty_addon::Result<AssetBatch> {
            cx.cx.set_state(&serde_json::json!({"built": true}))?;
            Ok(AssetBatch::default())
        }
    }

    #[test]
    fn keeps_profile_changes_made_during_the_build() {
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-cli-build-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join(GVDB), "db").unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let manifest = AddonManifest::parse(
            r#"{"schema_version":1,"id":"t.writer","name":"Writer","version":"1.0.0","author":"t","compat":{"game":"f1_25"}}"#,
        )
        .unwrap();
        let store = Store::new(root.join("data")).with_builtins(vec![Builtin {
            addon: Arc::new(Writer(manifest)),
            locked: false,
            default_on: true,
        }]);
        build_profile(&store, "default", &game).unwrap();
        let p = store.profile("default").unwrap();
        assert_eq!(p.state["t.writer"], serde_json::json!({"built": true}));
        std::fs::remove_dir_all(&root).ok();
    }
}
