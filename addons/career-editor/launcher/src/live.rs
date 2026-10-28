use std::path::Path;

use serde_json::Value;

use crate::store::now_ms;

pub const STALE_MS: u64 = 20_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bridge {
    Live,
    Stale,
    Missing,
}

pub fn written(snap: &Value) -> u64 {
    snap["written"].as_u64().unwrap_or(0)
}

pub fn fresh(snap: &Value) -> bool {
    let written = written(snap);
    written > 0 && now_ms().saturating_sub(written) < STALE_MS
}

pub fn says_running(snap: &Value) -> bool {
    fresh(snap) && snap["status"]["game"] == "running"
}

pub fn closed_since(was: Option<u64>, running: Option<bool>, saw_running: bool, now: u64) -> Option<u64> {
    match running {
        Some(false) => Some(was.unwrap_or(if saw_running { now } else { 0 })),
        _ => None,
    }
}

pub fn bridge_of(running: Option<bool>, snap: &Value, closed_since: Option<u64>) -> Bridge {
    let plugin_running = says_running(snap) && written(snap) > closed_since.unwrap_or(0);
    if snap.is_null() {
        Bridge::Missing
    } else if fresh(snap) && (running != Some(false) || plugin_running) {
        Bridge::Live
    } else {
        Bridge::Stale
    }
}

/// The running game's exe version, and whether no binding set was made for its exe.
#[derive(Clone, Debug, PartialEq)]
pub struct GameBuild {
    pub version: Option<String>,
    pub unsupported: bool,
}

/// The SHA-256 of the build each `bindings*.json` set in core's folder was made for.
pub fn binding_builds(core_dir: &Path) -> Vec<String> {
    let Ok(dir) = std::fs::read_dir(core_dir) else {
        return Vec::new();
    };
    dir.flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with("bindings") && name.ends_with(".json")
        })
        .filter_map(|e| std::fs::read(e.path()).ok())
        .filter_map(|b| serde_json::from_slice::<Value>(&b).ok())
        .filter_map(|d| d["build"]["sha256"].as_str().map(str::to_owned))
        .collect()
}

/// A build key ends in the first 12 hex digits of the exe's SHA-256. False when either side is unknown.
pub fn unsupported_build(build_key: Option<&str>, sets: &[String]) -> bool {
    let sha = build_key
        .and_then(|k| k.rsplit('-').next())
        .filter(|s| s.len() == 12);
    sha.is_some_and(|sha| !sets.is_empty() && !sets.iter().any(|s| s.starts_with(sha)))
}

pub struct Live {
    pub running: Option<bool>,
    pub game_session: Option<String>,
    pub pid: Option<u64>,
    pub since: Option<f64>,
    pub mailbox_known: bool,
    pub bridge: Bridge,
    pub snap: Value,
    pub snap_error: Option<String>,
    pub build: Option<GameBuild>,
}

impl Live {
    pub fn new(
        game_status: Option<Value>,
        snap: &Value,
        mailbox_known: bool,
        snap_error: Option<String>,
        closed_since: Option<u64>,
    ) -> Live {
        let running = game_status.as_ref().and_then(|g| g["running"].as_bool());
        let bridge = bridge_of(running, snap, closed_since);
        Live {
            running,
            game_session: game_status
                .as_ref()
                .and_then(|g| g["session"].as_str().map(str::to_owned)),
            pid: game_status.as_ref().and_then(|g| g["pid"].as_u64()),
            since: game_status.as_ref().and_then(|g| g["since"].as_f64()),
            mailbox_known,
            bridge,
            snap: snap.clone(),
            snap_error,
            build: None,
        }
    }

    pub fn game_running(&self) -> bool {
        self.running == Some(true) || self.live()
    }

    pub fn live(&self) -> bool {
        self.bridge == Bridge::Live
    }

    pub fn status(&self) -> &Value {
        &self.snap["status"]
    }

    pub fn career(&self) -> bool {
        self.live() && self.status()["career"].as_bool().unwrap_or(false)
    }

    pub fn mode(&self) -> &str {
        if !self.career() {
            return "none";
        }
        self.status()["mode"].as_str().unwrap_or("none")
    }

    pub fn my_team(&self) -> bool {
        self.mode() == "my_team"
    }

    pub fn safe(&self) -> bool {
        self.career() && self.status()["safe"].as_bool().unwrap_or(false)
    }

    pub fn anytime(&self) -> bool {
        self.career() && self.status()["anytime"].as_bool().unwrap_or(false)
    }

    pub fn why(&self) -> Option<String> {
        self.status()["why"].as_str().map(str::to_owned)
    }

    pub fn career_id(&self) -> Option<String> {
        if !self.career() {
            return None;
        }
        self.status()["career_id"].as_str().map(str::to_owned)
    }

    pub fn section(&self, name: &str) -> Option<&Value> {
        let v = &self.snap["sections"][name];
        if v.is_null() { None } else { Some(v) }
    }

    pub fn sections_live(&self) -> bool {
        self.live() && self.snap["sections_live"].as_bool().unwrap_or(false)
    }

    pub fn has_op(&self, name: &str) -> bool {
        self.snap["bridge"]["ops"]
            .as_array()
            .is_some_and(|ops| ops.iter().any(|o| o["name"] == name && o["off"].is_null()))
    }

    pub fn op_anytime(&self, name: &str) -> bool {
        self.snap["bridge"]["ops"].as_array().is_some_and(|ops| {
            ops.iter()
                .any(|o| o["name"] == name && o["anytime"].as_bool() == Some(true))
        })
    }

    pub fn gate(&self, op: &str) -> Result<(), String> {
        if !self.mailbox_known {
            return Err(
                "The launcher hasn't found the game folder. Locate F1 25 on the Library page.".into(),
            );
        }
        if !self.game_running() {
            return Err("Start F1 25 and load a career to edit it.".into());
        }
        if !self.live() {
            return Err("The game is running but the editor's game plugin isn't answering.".into());
        }
        if !self.career() {
            return Err("Load a career in the game to edit it.".into());
        }
        if !self.has_op(op) {
            return Err("Not available in this build.".into());
        }
        if self.status()["humans"].as_u64().unwrap_or(1) > 1 {
            return Err("Two-player careers can't be edited.".into());
        }
        Ok(())
    }

    pub fn can(&self, op: &str) -> bool {
        self.gate(op).is_ok()
    }

    pub fn applies_now(&self, op: &str) -> bool {
        if self.op_anytime(op) {
            self.anytime()
        } else {
            self.safe()
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{Bridge, Live, STALE_MS, binding_builds, closed_since, unsupported_build};
    use crate::store::now_ms;

    fn snapshot(age_ms: u64) -> Value {
        json!({
            "written": now_ms() - age_ms,
            "pid": 7504,
            "status": { "game": "running", "career": true, "mode": "my_team", "phase": "team_hq", "safe": true },
        })
    }

    fn closed() -> Option<Value> {
        Some(json!({ "running": false, "pid": null, "since": null, "session": null }))
    }

    #[test]
    fn a_fresh_snapshot_outranks_a_launcher_that_says_the_game_is_closed() {
        let live = Live::new(closed(), &snapshot(500), true, None, None);
        assert_eq!(live.bridge, Bridge::Live);
        assert!(live.live() && live.game_running() && live.career() && live.my_team());
        assert!(live.safe());
        assert_eq!(
            live.gate("economy.cash.set").err().as_deref(),
            Some("Not available in this build.")
        );
    }

    #[test]
    fn an_old_snapshot_stays_stale_and_the_game_reads_as_closed() {
        let live = Live::new(closed(), &snapshot(STALE_MS + 1_000), true, None, None);
        assert_eq!(live.bridge, Bridge::Stale);
        assert!(!live.game_running() && !live.career() && !live.my_team());
        let live = Live::new(None, &snapshot(STALE_MS + 1_000), true, None, None);
        assert_eq!(live.bridge, Bridge::Stale);
        assert!(!live.game_running());
    }

    #[test]
    fn the_last_snapshot_of_a_game_that_just_closed_does_not_count() {
        let snap = snapshot(2_000);
        let live = Live::new(closed(), &snap, true, None, Some(now_ms() - 1_000));
        assert_eq!(live.bridge, Bridge::Stale);
        assert!(!live.game_running());
        let live = Live::new(closed(), &snap, true, None, Some(now_ms() - 3_000));
        assert!(live.live() && live.game_running() && live.my_team());
    }

    #[test]
    fn a_running_or_silent_launcher_keeps_the_old_rules() {
        let running = Some(json!({ "running": true, "pid": 7504 }));
        let live = Live::new(running.clone(), &snapshot(500), true, None, None);
        assert!(live.live() && live.game_running() && live.my_team());
        let live = Live::new(running, &snapshot(STALE_MS + 1_000), true, None, None);
        assert_eq!(live.bridge, Bridge::Stale);
        assert!(live.game_running() && !live.career());
        let live = Live::new(None, &snapshot(500), true, None, None);
        assert!(live.live() && live.game_running());
        let live = Live::new(closed(), &Value::Null, true, None, None);
        assert_eq!(live.bridge, Bridge::Missing);
        assert!(!live.game_running());
    }

    #[test]
    fn a_snapshot_that_does_not_say_running_needs_the_launcher() {
        let mut snap = snapshot(500);
        snap["status"]["game"] = json!("closing");
        let live = Live::new(closed(), &snap, true, None, None);
        assert_eq!(live.bridge, Bridge::Stale);
        assert!(!live.game_running());
    }

    #[test]
    fn a_build_no_binding_set_was_made_for_is_unsupported() {
        let dir = std::env::temp_dir().join(format!("ce-bindings-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let steam = "d873a1c8d2044cef2fd76c99c9d5a97649c5b815d055540995105f46619865ae";
        let ea = "cc506879c1c834aa54d3478b2f2133d2ec243f2f2d313397631145ea618ff7f4";
        for (name, sha) in [
            ("bindings.json", steam),
            ("bindings.ea.json", ea),
            ("symbols.json", "ffff"),
        ] {
            std::fs::write(
                dir.join(name),
                json!({ "build": { "sha256": sha }, "bindings": [] }).to_string(),
            )
            .unwrap();
        }
        let mut sets = binding_builds(&dir);
        sets.sort();
        assert_eq!(sets, [ea, steam]);
        std::fs::remove_dir_all(&dir).ok();

        assert!(unsupported_build(Some("exe-5c1f0a9e77b2-5c1f0a9e77b2"), &sets));
        assert!(!unsupported_build(Some("25251167-d873a1c8d204"), &sets));
        assert!(!unsupported_build(Some("exe-cc506879c1c8-cc506879c1c8"), &sets));
        assert!(!unsupported_build(None, &sets));
        assert!(!unsupported_build(Some("exe-5c1f0a9e77b2-5c1f0a9e77b2"), &[]));
    }

    #[test]
    fn the_game_counts_as_closed_from_the_first_false_after_a_true() {
        assert_eq!(closed_since(None, Some(false), true, 500), Some(500));
        assert_eq!(closed_since(Some(500), Some(false), true, 900), Some(500));
        assert_eq!(closed_since(Some(500), Some(true), true, 900), None);
        assert_eq!(closed_since(Some(500), None, true, 900), None);
        let never = closed_since(None, Some(false), false, 900);
        assert_eq!(never, Some(0));
        let live = Live::new(closed(), &snapshot(4_000), true, None, never);
        assert!(live.live() && live.game_running() && live.my_team());
    }
}
