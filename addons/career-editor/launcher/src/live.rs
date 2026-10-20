use serde_json::Value;

use crate::store::now_ms;

pub const STALE_MS: u64 = 20_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bridge {
    Live,
    Stale,
    Missing,
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
}

impl Live {
    pub fn new(
        game_status: Option<Value>,
        snap: &Value,
        mailbox_known: bool,
        snap_error: Option<String>,
    ) -> Live {
        let running = game_status.as_ref().and_then(|g| g["running"].as_bool());
        let written = snap["written"].as_u64().unwrap_or(0);
        let fresh = written > 0 && now_ms().saturating_sub(written) < STALE_MS;
        let bridge = if snap.is_null() {
            Bridge::Missing
        } else if fresh && running != Some(false) {
            Bridge::Live
        } else {
            Bridge::Stale
        };
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
        }
    }

    pub fn game_running(&self) -> bool {
        self.running.unwrap_or(self.bridge == Bridge::Live)
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
