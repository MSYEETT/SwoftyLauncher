mod host;
mod live;
mod pages;
mod store;
mod ui;
mod undo;

use std::path::PathBuf;

use serde_json::{Value, json};

use host::Host;
use live::Live;
use store::Store;

const API: u64 = 1;

pub struct App {
    pub store: Store,
    pub vanilla: bool,
    pub enabled: bool,
    pub launcher: String,
    pub tab: String,
    last_tick: Option<(Option<bool>, Option<u64>, u64)>,
}

impl App {
    fn new(data_dir: PathBuf) -> App {
        App {
            store: Store::new(data_dir),
            vanilla: false,
            enabled: true,
            launcher: String::new(),
            tab: "overview".into(),
            last_tick: None,
        }
    }

    fn locate(&mut self, host: &mut Host) {
        if self.store.mailbox.is_none()
            && let Ok(v) = host.call("addon_game_dir", Value::Null)
            && let Some(p) = v.as_str()
        {
            self.store.mailbox = Some(PathBuf::from(p));
        }
    }

    pub fn live(&mut self, host: &mut Host) -> Live {
        self.locate(host);
        self.store.collect_acks();
        let status = host
            .call("game_status", Value::Null)
            .ok()
            .filter(|v| v.is_object());
        let known = self.store.mailbox.is_some();
        let snap = self.store.snapshot().clone();
        Live::new(status, &snap, known, self.store.snap_error.clone())
    }

    fn tick(&mut self, host: &mut Host) {
        self.locate(host);
        let mut changed = self.store.snapshot_changed();
        if self.store.outbox_changed() && self.store.collect_acks() > 0 {
            changed = true;
        }
        let status = host.call("game_status", Value::Null).ok();
        let running = status.as_ref().and_then(|s| s["running"].as_bool());
        let pid = status.as_ref().and_then(|s| s["pid"].as_u64());
        let written = self.store.snapshot()["written"].as_u64().unwrap_or(0);
        let fresh = written > 0 && store::now_ms().saturating_sub(written) < live::STALE_MS;
        let key = (running, pid, u64::from(fresh));
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
