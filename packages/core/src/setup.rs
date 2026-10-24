//! The one-time setup before the first modded Play. The player says whether
//! the game folder holds mods Swofty did not add, the launcher puts back what
//! it changed and starts its record of the original game files over, and the
//! player verifies the game files in their store. The originals Swofty records
//! after that are the store's own.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::install::Game;
use crate::recovery::{Restored, SESSION_HELD, SessionLock};
use crate::{Error, Result, read_json, with_retry, write_json};

/// Raised to have every player go through the setup again, which starts each
/// record of original game files over.
pub const VERSION: u32 = 1;

/// Where [`reset`] moves the record of original game files until [`finish`]
/// deletes it.
pub const RETIRED: &str = "baseline-retired";

pub const NEEDED: &str = "Swofty needs a one-time setup before modded play: open Swofty Launcher and press Play, \
                          or run `swofty setup reset`, verify the game files, then run `swofty setup done`";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Setup {
    pub version: u32,
    /// The player said the game folder held mods Swofty did not add.
    pub other_mods: bool,
    pub reset_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done_at: Option<u64>,
}

fn path(data: &Path) -> PathBuf {
    data.join("state/setup.json")
}

pub fn read(data: &Path) -> Option<Setup> {
    read_json(&path(data)).ok()
}

pub fn done(data: &Path) -> bool {
    read(data).is_some_and(|s| s.version >= VERSION && s.done_at.is_some())
}

/// Put the game folder back as `swofty restore` does, then start the record of
/// original game files over: the baselines move to [`RETIRED`] and the built
/// overlays are dropped. The player verifies the game files next.
pub fn reset(data: &Path, game: &Game, other_mods: bool) -> Result<Restored> {
    let Some(lock) = SessionLock::try_acquire(data)? else {
        return Err(Error::Other(SESSION_HELD.into()));
    };
    let restored = crate::recovery::restore_locked(&lock, data, game)?;
    let baseline = data.join("baseline");
    let retired = data.join(RETIRED);
    if baseline.is_dir() {
        if retired.exists() {
            with_retry(&retired, || std::fs::remove_dir_all(&retired))?;
        }
        with_retry(&baseline, || std::fs::rename(&baseline, &retired))?;
    }
    let overlays = data.join("cache/overlay");
    if overlays.is_dir() {
        with_retry(&overlays, || std::fs::remove_dir_all(&overlays))?;
    }
    write_json(
        &path(data),
        &Setup {
            version: VERSION,
            other_mods,
            reset_at: crate::update::now_secs(),
            done_at: None,
        },
    )?;
    drop(lock);
    Ok(restored)
}

/// The player verified the game files: modded play may start, and the old
/// record of original game files is deleted.
pub fn finish(data: &Path) -> Result<Setup> {
    let mut setup = read(data)
        .filter(|s| s.version >= VERSION)
        .ok_or_else(|| Error::Other("the setup hasn't put the game folder back yet; reset first".into()))?;
    setup.done_at = Some(crate::update::now_secs());
    write_json(&path(data), &setup)?;
    let retired = data.join(RETIRED);
    if retired.is_dir() {
        with_retry(&retired, || std::fs::remove_dir_all(&retired))?;
    }
    Ok(setup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baseline::Identity;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("swofty-setup-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn game(root: &Path) -> Game {
        let dir = root.join("F1 25");
        std::fs::create_dir_all(&dir).unwrap();
        Game {
            dir,
            user_reg: None,
            identity: Identity {
                build_id: "1".into(),
                exe_sha256: "0".repeat(64),
            },
        }
    }

    #[test]
    fn reset_moves_the_baselines_aside_and_finish_deletes_them() {
        let root = temp("reset");
        let data = root.join("data");
        let g = game(&root);
        crate::baseline::Baseline::new(&g.identity).save(&data).unwrap();
        std::fs::create_dir_all(data.join("cache/overlay/default/abc")).unwrap();
        assert!(!done(&data));

        reset(&data, &g, true).unwrap();
        assert!(!data.join("baseline").exists());
        assert!(data.join(RETIRED).join("1-000000000000/baseline.json").is_file());
        assert!(!data.join("cache/overlay").exists());
        let s = read(&data).unwrap();
        assert_eq!((s.version, s.other_mods, s.done_at), (VERSION, true, None));
        assert!(!done(&data));

        finish(&data).unwrap();
        assert!(done(&data));
        assert!(!data.join(RETIRED).exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_reset_again_replaces_the_retired_copy() {
        let root = temp("again");
        let data = root.join("data");
        let g = game(&root);
        std::fs::create_dir_all(data.join("baseline/old")).unwrap();
        reset(&data, &g, false).unwrap();
        std::fs::create_dir_all(data.join("baseline/new")).unwrap();
        reset(&data, &g, false).unwrap();
        assert!(data.join(RETIRED).join("new").is_dir());
        assert!(!data.join(RETIRED).join("old").exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn finish_needs_a_reset_of_this_setup_version() {
        let root = temp("finish");
        let data = root.join("data");
        assert!(finish(&data).is_err());
        write_json(
            &path(&data),
            &Setup {
                version: VERSION - 1,
                other_mods: false,
                reset_at: 1,
                done_at: Some(2),
            },
        )
        .unwrap();
        assert!(!done(&data));
        assert!(finish(&data).is_err());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn reset_waits_for_a_running_session() {
        let root = temp("held");
        let data = root.join("data");
        let g = game(&root);
        std::fs::create_dir_all(data.join("baseline/kept")).unwrap();
        let _held = SessionLock::try_acquire(&data).unwrap().unwrap();
        assert!(reset(&data, &g, false).is_err());
        assert!(data.join("baseline/kept").is_dir());
        std::fs::remove_dir_all(&root).ok();
    }
}
