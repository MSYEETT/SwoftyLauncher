//! Exercises offline staging against a real installed extension.
//!
//! Skipped unless SWOFTY_OFFLINE_TEST_DIR points at an extension directory
//! (CI has no extension to run). Everything works on throwaway folders.

use std::path::PathBuf;

use swofty_core::offline::{self, Artifacts};

fn artifacts() -> Option<Artifacts> {
    let dir = PathBuf::from(std::env::var("SWOFTY_OFFLINE_TEST_DIR").ok()?);
    let (prelaunch, probe) = if cfg!(windows) {
        ("f1op-prelaunch.exe", "f1op-probe.exe")
    } else {
        ("f1op-prelaunch-linux-x86_64", "f1op-probe-linux-x86_64")
    };
    Some(Artifacts {
        hook: dir.join("f1op_hook.dll"),
        stub: dir.join("f1op_stub.exe"),
        prelaunch: dir.join(prelaunch),
        probe: dir.join(probe),
        dir,
    })
}

#[test]
fn stage_creates_the_journal_directory() {
    let Some(a) = artifacts() else {
        eprintln!("skipping: set SWOFTY_OFFLINE_TEST_DIR to an installed extension");
        return;
    };
    let tmp = std::env::temp_dir().join(format!("swofty-stage-{}", std::process::id()));
    std::fs::remove_dir_all(&tmp).ok();
    let game = tmp.join("game");
    std::fs::create_dir_all(&game).unwrap();
    std::fs::write(game.join("EAAntiCheat.GameServiceLauncher.exe"), "original").unwrap();

    // The parent directory does not exist yet: this is the regression for the
    // first real run failing with "cannot write journal".
    let journal = tmp.join("state/offline/staging.jsonl");
    offline::stage(&game, &a, &journal).expect("staging with a nested journal path");
    assert!(journal.is_file());

    // And the session unrolls back to vanilla.
    offline::restore(&journal, true).unwrap();
    assert_eq!(
        std::fs::read_to_string(game.join("EAAntiCheat.GameServiceLauncher.exe")).unwrap(),
        "original"
    );
    assert!(!game.join("EAAntiCheat.GameServiceLauncher.exe.bak").exists());
    assert!(!game.join("swofty/mods/core/f1op_hook.dll").exists());
    std::fs::remove_dir_all(&tmp).ok();
}
