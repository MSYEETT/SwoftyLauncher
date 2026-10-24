//! The core's GPU-removed watchdog record, `<game>/swofty/overlay-gpu-removed.json`, and what a
//! session's `loader.log` says about the in-game overlay.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Deserialize;

use crate::install::LOADER_DIR;
use crate::manifest::{AddonManifest, ExperimentalFeature};
use crate::store::Store;
use crate::{Result, io_err, with_retry};

pub const REMOVED_FILE: &str = "overlay-gpu-removed.json";

/// The fields `hud_guard.hpp`'s `removal_json` writes.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Removal {
    pub when: String,
    pub present_hr: String,
    pub removed_reason: String,
    pub after_draw_ms: u64,
    pub swapchain_by: String,
}

/// The overlay is off for a game copy: the core found the GPU removed within 10 s of an overlay
/// draw and keeps the overlay off while the record exists.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayOff {
    pub path: PathBuf,
    pub at: f64,
    pub removal: Option<Removal>,
}

pub fn path(game: &Path) -> PathBuf {
    game.join(LOADER_DIR).join(REMOVED_FILE)
}

impl OverlayOff {
    pub fn read(game: &Path) -> Option<Self> {
        let path = path(game);
        let meta = std::fs::metadata(&path).ok().filter(|m| m.is_file())?;
        let at = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0.0, |d| d.as_secs_f64());
        let removal = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Removal>(&b).ok());
        Some(Self { path, at, removal })
    }

    pub fn describe(&self) -> String {
        match &self.removal {
            Some(r) => format!(
                "the GPU was removed {:.1} s after the overlay drew ({}, present {}, reason {}, swapchain by {})",
                r.after_draw_ms as f64 / 1000.0,
                or_unknown(&r.when),
                or_unknown(&r.present_hr),
                or_unknown(&r.removed_reason),
                or_unknown(&r.swapchain_by),
            ),
            None => format!("{} exists but could not be read", self.path.display()),
        }
    }
}

fn or_unknown(s: &str) -> &str {
    if s.is_empty() { "unknown" } else { s }
}

/// Deletes the record, so the core draws the overlay again from the next launch. Returns whether
/// there was one.
pub fn reset(game: &Path) -> Result<bool> {
    let path = path(game);
    if !path.is_file() {
        return Ok(false);
    }
    with_retry(&path, || std::fs::remove_file(&path))?;
    let dir = game.join(LOADER_DIR);
    if std::fs::read_dir(&dir).map_err(io_err(&dir))?.next().is_none() {
        std::fs::remove_dir(&dir).ok();
    }
    Ok(true)
}

/// What an addon does in game, from its manifest, as the Addons page tags it.
pub fn effects(m: &AddonManifest) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for f in &m.experimental_features {
        let e = f.effect();
        if !out.contains(&e) {
            out.push(e);
        }
    }
    out
}

pub fn draws_over_the_game(m: &AddonManifest) -> bool {
    m.experimental_features.contains(&ExperimentalFeature::Overlay)
}

/// The addons on in `profile` that draw over the game, as `(id, name)` in load order.
pub fn painters(store: &Store, profile: &str) -> Vec<(String, String)> {
    let (Ok(addons), Ok(p)) = (store.addons(), store.profile(profile)) else {
        return Vec::new();
    };
    p.enabled()
        .filter_map(|id| addons.get(id))
        .filter(|m| draws_over_the_game(m))
        .map(|m| (m.id.clone(), m.name.clone()))
        .collect()
}

/// What a session's `loader.log` says about the overlay, from its `[overlay]` lines.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionOverlay {
    pub painters: Vec<String>,
    pub armed: bool,
    pub drew: bool,
    pub frames: u64,
    pub target: Option<String>,
    pub off: Option<String>,
    pub removed: Option<String>,
}

const TAG: &str = "[overlay] ";

impl SessionOverlay {
    /// `None` when the log has no `[overlay]` line.
    pub fn scan(log: &str) -> Option<Self> {
        let mut s = Self::default();
        let mut any = false;
        for line in log.lines() {
            let Some(at) = line.find(TAG) else {
                continue;
            };
            any = true;
            let rest = &line[at + TAG.len()..];
            if let Some(api) = rest.strip_prefix("api: ") {
                if let Some((id, _)) = api.split_once(" on_paint -> ok")
                    && !id.is_empty()
                    && !s.painters.iter().any(|p| p == id)
                {
                    s.painters.push(id.to_owned());
                }
            } else if rest.starts_with("present hook armed") {
                s.armed = true;
            } else if let Some(up) = rest.strip_prefix("renderer up: ") {
                s.drew = true;
                s.target = Some(up.trim().to_owned());
            } else if rest.starts_with("first overlay frame submitted") {
                s.drew = true;
            } else if let Some(on) = rest.strip_prefix("drawing on swapchain ") {
                s.target.get_or_insert_with(|| clip(on, 120));
            } else if let Some(n) = drawn_frames(rest) {
                s.frames += n;
                s.drew |= n > 0;
            } else if rest.starts_with("off: ")
                || rest.starts_with("the overlay feature is experimental and off")
            {
                s.off = Some(clip(rest.strip_prefix("off: ").unwrap_or(rest), 200));
            } else if rest.starts_with("the GPU was removed") {
                s.removed = Some(clip(rest, 200));
            }
        }
        any.then_some(s)
    }

    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(off) = &self.off {
            parts.push(format!("off ({off})"));
        } else if self.drew {
            let mut drew = "drew".to_owned();
            if self.frames > 0 {
                drew.push_str(&format!(" {} frame(s)", self.frames));
            }
            if let Some(t) = &self.target {
                drew.push_str(&format!(" on {t}"));
            }
            parts.push(drew);
        } else if self.armed {
            parts.push("armed, never drew".to_owned());
        } else {
            parts.push("did not arm".to_owned());
        }
        if !self.painters.is_empty() {
            parts.push(format!("on_paint by {}", self.painters.join(", ")));
        }
        if let Some(r) = &self.removed {
            parts.push(r.clone());
        }
        parts.join("; ")
    }
}

fn drawn_frames(rest: &str) -> Option<u64> {
    let tail = rest.strip_prefix("last ")?;
    let (_, after) = tail.split_once(": ")?;
    let (n, more) = after.split_once(' ')?;
    more.starts_with("frame(s) drawn").then(|| n.parse().ok())?
}

fn clip(s: &str, max: usize) -> String {
    let s = s.trim();
    match s.char_indices().nth(max) {
        Some((at, _)) => format!("{}…", &s[..at]),
        None => s.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const V150: &str = "\
2026-10-24 02:37:54.717 [31308] [assets] resources: 104 prebuilt overlay ERP(s) in swofty/assets.json
2026-10-24 02:37:56.256 [31308] [overlay] present hook armed: Present (vt+0x40), ResizeBuffers (vt+0x68), ExecuteCommandLists (vt+0x50) on the game's D3D12 swapchain
2026-10-24 02:38:01.760 [31308] [overlay] api: kizi.pitlane on_paint -> ok (token 1)
2026-10-24 02:38:01.800 [31308] [overlay] api: kizi.canned on_paint -> ok (token 2)
2026-10-24 02:38:01.800 [31308] [overlay] api: kizi.canned on_paint -> ok (token 3)
2026-10-24 02:38:06.370 [35760] [overlay] captured the game's direct command queue 00000000D1AE5A30
2026-10-24 02:38:06.753 [31308] [overlay] renderer up: 2560x1440, 4 buffer(s), format 24
";

    #[test]
    fn a_150_session_that_drew_names_its_painters() {
        let s = SessionOverlay::scan(V150).unwrap();
        assert!(s.armed && s.drew);
        assert_eq!(s.painters, ["kizi.pitlane", "kizi.canned"]);
        assert_eq!(
            s.describe(),
            "drew on 2560x1440, 4 buffer(s), format 24; on_paint by kizi.pitlane, kizi.canned"
        );
    }

    #[test]
    fn frame_counts_and_the_removal_line_are_read() {
        let log = "\
1 [overlay] api: kizi.pitlane on_paint -> ok (token 1)
2 [overlay] drawing on swapchain 0000000012345678 with the DIRECT queue 00000000AAAA (2560x1440, format 24, 3 buffer(s))
3 [overlay] first overlay frame submitted: 96 vertices
4 [overlay] last 60 s: 3600 frame(s) drawn, 2 skipped while the GPU still held the slot
5 [overlay] the GPU was removed (present 0x887a0005, reason 0x887a0006) 4.8 s after the overlay last drew; the overlay stops drawing
";
        let s = SessionOverlay::scan(log).unwrap();
        assert!(s.drew);
        assert_eq!(s.frames, 3600);
        assert!(
            s.removed
                .as_deref()
                .unwrap()
                .starts_with("the GPU was removed (present 0x887a0005")
        );
        assert!(
            s.describe().starts_with("drew 3600 frame(s) on 0000000012345678"),
            "{}",
            s.describe()
        );
    }

    #[test]
    fn an_overlay_kept_off_by_the_record_says_so() {
        let log = "1 [overlay] off: swofty/overlay-gpu-removed.json records the GPU removed 4.8 s after the overlay drew (x); the overlay stays off while that file exists\n";
        let s = SessionOverlay::scan(log).unwrap();
        assert!(!s.drew);
        assert!(
            s.describe()
                .starts_with("off (swofty/overlay-gpu-removed.json records")
        );
    }

    #[test]
    fn erp_overlay_lines_are_not_the_overlay() {
        assert_eq!(
            SessionOverlay::scan("1 [assets] resources: overlay ERP swofty/assets/resources/p2.erp\n"),
            None
        );
        let armed = SessionOverlay::scan("1 [overlay] present hook armed: Present (vt+0x40)\n").unwrap();
        assert_eq!(armed.describe(), "armed, never drew");
    }

    #[test]
    fn the_record_is_read_and_reset() {
        let game = std::env::temp_dir().join(format!("swofty-overlay-off-{}", std::process::id()));
        std::fs::remove_dir_all(&game).ok();
        assert_eq!(OverlayOff::read(&game), None);
        assert!(!reset(&game).unwrap());
        std::fs::create_dir_all(game.join(LOADER_DIR)).unwrap();
        std::fs::write(
            path(&game),
            "{\n  \"schema\": 1,\n  \"when\": \"2026-10-24 02:39:47\",\n  \"present_hr\": \"0x887a0005\",\n  \
             \"removed_reason\": \"0x887a0006\",\n  \"after_draw_ms\": 4812,\n  \"swapchain_by\": \"sl.interposer.dll <- F1_25.exe\"\n}\n",
        )
        .unwrap();
        let off = OverlayOff::read(&game).unwrap();
        assert_eq!(off.removal.as_ref().unwrap().after_draw_ms, 4812);
        assert_eq!(
            off.describe(),
            "the GPU was removed 4.8 s after the overlay drew (2026-10-24 02:39:47, present 0x887a0005, \
             reason 0x887a0006, swapchain by sl.interposer.dll <- F1_25.exe)"
        );
        assert!(reset(&game).unwrap());
        assert!(!game.join(LOADER_DIR).exists());
        std::fs::create_dir_all(game.join(LOADER_DIR)).unwrap();
        std::fs::write(path(&game), "not json").unwrap();
        assert!(
            OverlayOff::read(&game)
                .unwrap()
                .describe()
                .ends_with("exists but could not be read")
        );
        std::fs::remove_dir_all(&game).ok();
    }
}
