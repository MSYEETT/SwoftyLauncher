mod calendar;
mod host;
mod live;
mod pages;
mod store;
mod tracks;
mod ui;
mod undo;

use std::io::Write;
use std::path::PathBuf;

use serde_json::{Value, json};

use host::Host;
use live::{Bridge, Live};
use store::Store;

const API: u64 = 1;

pub struct App {
    pub store: Store,
    pub vanilla: bool,
    pub enabled: bool,
    pub launcher: String,
    pub tab: String,
    pub profile_tracks: Option<Result<Value, String>>,
    pub cal_move: (Option<i64>, Option<i64>),
    last_tick: Option<(Option<bool>, Option<u64>, Bridge)>,
    closed_since: Option<u64>,
    saw_running: bool,
    logged_state: Option<String>,
    logged_disagree: bool,
    logged_error: Option<String>,
}

pub fn log(line: std::fmt::Arguments) {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    writeln!(std::io::stderr(), "{now} {line}").ok();
}

impl App {
    fn new(data_dir: PathBuf) -> App {
        App {
            store: Store::new(data_dir),
            vanilla: false,
            enabled: true,
            launcher: String::new(),
            tab: "overview".into(),
            profile_tracks: None,
            cal_move: (None, None),
            last_tick: None,
            closed_since: None,
            saw_running: false,
            logged_state: None,
            logged_disagree: false,
            logged_error: None,
        }
    }

    /// The launcher names the game copy that is running, or the one Play
    /// uses, so the mailbox follows a switch between copies.
    fn locate(&mut self, host: &mut Host) {
        if let Ok(v) = host.call("addon_game_dir", Value::Null)
            && let Some(p) = v.as_str()
        {
            self.store.set_mailbox(PathBuf::from(p));
        }
    }

    fn running(&self) -> bool {
        self.saw_running && self.closed_since.is_none()
    }

    fn game_status(&mut self, host: &mut Host) -> Option<Value> {
        let status = host
            .call("game_status", Value::Null)
            .ok()
            .filter(Value::is_object);
        let running = status.as_ref().and_then(|s| s["running"].as_bool());
        self.closed_since = live::closed_since(self.closed_since, running, self.saw_running, store::now_ms());
        self.saw_running |= running == Some(true);
        status
    }

    fn note(&mut self, status: Option<&Value>) -> Bridge {
        let running = status.and_then(|s| s["running"].as_bool());
        let host_pid = status.and_then(|s| s["pid"].as_u64());
        let closed_since = self.closed_since;
        let error = self.store.snap_error.clone();
        let snap = self.store.snapshot();
        let bridge = live::bridge_of(running, snap, closed_since);
        let disagree = running == Some(false) && live::says_running(snap);
        let written = live::written(snap);
        let age = if written > 0 {
            format!("{} ms old", store::now_ms().saturating_sub(written))
        } else {
            "missing".to_owned()
        };
        let st = &snap["status"];
        let plugin = format!(
            "snapshot {age}, pid {}, game {}, career {}, mode {}, phase {}",
            snap["pid"], st["game"], st["career"], st["mode"], st["phase"]
        );
        let shown = if bridge == Bridge::Live && st["career"] == true {
            st["mode"].as_str().unwrap_or("none").to_owned()
        } else {
            "none".to_owned()
        };
        if error != self.logged_error {
            match &error {
                Some(e) => log(format_args!("snapshot.json is unreadable: {e}")),
                None => log(format_args!("snapshot.json is readable again")),
            }
            self.logged_error = error;
        }
        if disagree != self.logged_disagree {
            if disagree {
                log(format_args!(
                    "the launcher's game_status says F1 25 is not running ({}) but the game plugin says it is: {plugin}",
                    status.map_or_else(|| "no answer".to_owned(), Value::to_string)
                ));
            } else {
                log(format_args!(
                    "the launcher's game_status and the game plugin agree again (running {running:?}, {plugin})"
                ));
            }
            self.logged_disagree = disagree;
        }
        let state = format!("{running:?} {bridge:?} {shown}");
        if self.logged_state.as_ref() != Some(&state) {
            log(format_args!(
                "page: game_status running {running:?} pid {host_pid:?}; {plugin}; bridge {bridge:?}; shows mode {shown}"
            ));
            self.logged_state = Some(state);
        }
        bridge
    }

    pub fn live(&mut self, host: &mut Host) -> Live {
        self.locate(host);
        self.store.collect_acks();
        let status = self.game_status(host);
        let known = self.store.mailbox.is_some();
        if self.running() {
            self.store.touch_viewer();
        }
        self.note(status.as_ref());
        let snap = self.store.snapshot().clone();
        Live::new(
            status,
            &snap,
            known,
            self.store.snap_error.clone(),
            self.closed_since,
        )
    }

    fn tick(&mut self, host: &mut Host) {
        self.locate(host);
        if self.running() {
            self.store.touch_viewer();
        }
        let mut changed = self.store.snapshot_changed();
        if self.store.outbox_changed() && self.store.collect_acks() > 0 {
            changed = true;
        }
        let status = self.game_status(host);
        let bridge = self.note(status.as_ref());
        let running = status.as_ref().and_then(|s| s["running"].as_bool());
        let pid = status.as_ref().and_then(|s| s["pid"].as_u64());
        let key = (running, pid, bridge);
        if self.last_tick != Some(key) {
            changed = true;
            self.last_tick = Some(key);
        }
        if changed {
            host.invalidate(None);
        }
    }
}

fn main() {
    let mut host = Host::new();
    let mut app = App::new(PathBuf::new());
    while let Some(line) = host.read_line() {
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            eprintln!("not JSON: {line}");
            continue;
        };
        let params = &message["params"];
        let method = message["method"].as_str().unwrap_or_default();
        let Some(id) = message["id"].as_u64() else {
            if method == "profile" {
                app.vanilla = params["vanilla"].as_bool().unwrap_or(false);
                app.enabled = params["enabled"].as_bool().unwrap_or(true);
            }
            continue;
        };
        let section = params["section"].as_str().unwrap_or("overview").to_owned();
        let result = match method {
            "hello" => {
                let mut fresh = App::new(PathBuf::from(params["data_dir"].as_str().unwrap_or_default()));
                fresh.vanilla = params["profile"]["vanilla"].as_bool().unwrap_or(false);
                fresh.enabled = params["profile"]["enabled"].as_bool().unwrap_or(true);
                fresh.launcher = params["launcher"].as_str().unwrap_or_default().to_owned();
                fresh.tab = std::mem::take(&mut app.tab);
                app = fresh;
                Ok(json!({ "api": API }))
            }
            "view" => Ok(pages::view(&mut app, &mut host, &section)),
            "event" => pages::event(&mut app, &mut host, &section, &params["event"]),
            "tick" => {
                app.tick(&mut host);
                Ok(Value::Null)
            }
            "summary" => Ok(pages::summary(&mut app)),
            other => Err(format!("unknown method {other}")),
        };
        let reply = match result {
            Ok(result) => json!({ "id": id, "result": result }),
            Err(error) => json!({ "id": id, "error": error }),
        };
        host.send(&reply);
    }
}
