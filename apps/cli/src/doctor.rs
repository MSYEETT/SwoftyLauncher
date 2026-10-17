use std::process::ExitCode;

use anyhow::Result;
use swofty_core::store::Store;
use swofty_core::{STEAM_APP_ID, steam};

pub fn doctor(store: &Store) -> Result<ExitCode> {
    let row = |k: &str, v: String| println!("{k:18} {v}");
    row("data dir", store.root.display().to_string());
    row(
        "addons",
        format!(
            "{} installed (including swofty.core)",
            store.addons().map(|m| m.len()).unwrap_or(0)
        ),
    );
    row(
        "maps",
        format!("{} imported", store.maps().map(|m| m.len()).unwrap_or(0)),
    );
    let roots = steam::steam_roots();
    if roots.is_empty() {
        row("steam", "not found".into());
        return Ok(ExitCode::FAILURE);
    }
    for r in &roots {
        row("steam root", r.display().to_string());
        for lib in steam::libraries(r) {
            row("  library", lib.display().to_string());
        }
    }
    let Some(game) = roots.iter().find_map(|r| steam::find_game_in(r)) else {
        row(
            "F1 25",
            format!("app {STEAM_APP_ID} not installed in any library"),
        );
        return Ok(ExitCode::FAILURE);
    };
    row("game dir", game.dir.display().to_string());
    row(
        "build id",
        game.build_id.map_or("unknown".into(), |b| b.to_string()),
    );
    row(
        "exe",
        if game.exe().is_file() {
            "present".into()
        } else {
            "MISSING".into()
        },
    );
    match crate::build::game(store, Some(game.dir.clone()))
        .and_then(|g| Ok(swofty_core::install::Installer::new(&store.root, &g).verify()?))
    {
        Ok(d) if d.is_empty() => row("baseline", "no drift".into()),
        Ok(d) => d.iter().for_each(|d| row("drift", d.to_string())),
        Err(e) => row("baseline", format!("not checked: {e:#}")),
    }
    if !cfg!(windows) {
        row(
            "compat tool",
            steam::compat_tool(&game.steam_root).unwrap_or_else(|| "default".into()),
        );
        match game.prefix() {
            Some(pfx) => {
                let reg = std::fs::read_to_string(pfx.join("user.reg")).unwrap_or_default();
                let key = "[Software\\\\Wine\\\\AppDefaults\\\\F1_25.exe\\\\DllOverrides]";
                row("prefix", pfx.display().to_string());
                row(
                    "dll override",
                    if reg.contains(key) {
                        "present".into()
                    } else {
                        "absent".into()
                    },
                );
            }
            None => row("prefix", "not created yet (launch the game once)".into()),
        }
        let opts = steam::launch_options(&game.steam_root);
        if opts.is_empty() {
            row("launch option", "none".into());
        }
        for (user, o) in opts {
            let state = if steam::wrapper_installed(&o) {
                "wrapper"
            } else {
                "no wrapper"
            };
            row("launch option", format!("user {user} ({state}): {o}"));
        }
    }
    Ok(ExitCode::SUCCESS)
}
