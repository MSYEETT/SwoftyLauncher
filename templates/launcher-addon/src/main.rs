use std::io::{BufRead, Lines, StdinLock, StdoutLock, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

const API: u64 = 1;
const EXTENSION: &str = "exmod";

struct Host {
    lines: Lines<StdinLock<'static>>,
    out: StdoutLock<'static>,
    next: u64,
}

impl Host {
    fn send(&mut self, message: &Value) {
        writeln!(self.out, "{message}")
            .and_then(|()| self.out.flush())
            .ok();
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next;
        self.next += 1;
        self.send(&json!({ "id": id, "method": method, "params": params }));
        let line = match self.lines.next() {
            Some(Ok(line)) => line,
            _ => std::process::exit(0),
        };
        let reply: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        match reply.get("error") {
            Some(e) => Err(e.to_string()),
            None => Ok(reply["result"].clone()),
        }
    }

    fn progress(&mut self, done: f32, text: &str) {
        self.send(&json!({ "method": "progress", "params": { "done": done, "text": text } }));
    }
}

struct Addon {
    data_dir: PathBuf,
    cache_dir: PathBuf,
    vanilla: bool,
}

impl Addon {
    fn items_dir(&self) -> PathBuf {
        self.data_dir.join("items")
    }

    fn items(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.items_dir())
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .collect();
        names.sort();
        names
    }

    fn enabled(&self, host: &mut Host) -> Result<Vec<String>, String> {
        let state = host.call("get_state", Value::Null)?;
        Ok(serde_json::from_value(state["on"].clone()).unwrap_or_default())
    }

    fn view(&self, host: &mut Host) -> Result<Value, String> {
        let pick = json!({
            "id": "pick",
            "label": "Import…",
            "icon": "upload",
            "kind": { "pick": { "title": "Import example items", "extensions": [EXTENSION], "folders": false, "multiple": true } }
        });
        let items = self.items();
        if items.is_empty() {
            return Ok(json!({
                "subtitle": "No items",
                "body": { "empty": {
                    "title": "No example items",
                    "text": format!("Drop a .{EXTENSION} file on the launcher, or import one."),
                    "action": pick
                } }
            }));
        }
        let on = self.enabled(host)?;
        let rows: Vec<Value> = items
            .iter()
            .map(|name| {
                let size = std::fs::metadata(self.items_dir().join(name)).map_or(0, |m| m.len());
                json!({
                    "id": name,
                    "title": name,
                    "subtitle": format!("{size} bytes"),
                    "art": { "monogram": name.chars().next().unwrap_or('?').to_uppercase().to_string() },
                    "trailing": { "switch": { "id": name, "on": on.contains(name), "enabled": !self.vanilla } }
                })
            })
            .collect();
        Ok(json!({
            "subtitle": format!("{} items, {} on", items.len(), on.len()),
            "actions": [pick],
            "body": { "list": { "rows": rows, "reorder": false } }
        }))
    }

    fn event(&self, host: &mut Host, event: &Value) -> Result<Value, String> {
        if let Some(toggle) = event.get("toggle") {
            let name = toggle["id"].as_str().unwrap_or_default().to_owned();
            let mut on = self.enabled(host)?;
            on.retain(|n| *n != name);
            if toggle["on"].as_bool() == Some(true) {
                on.push(name);
            }
            host.call("set_state", json!({ "on": on }))?;
            return Ok(json!("reload"));
        }
        if let Some(picked) = event.get("picked") {
            let paths: Vec<PathBuf> = serde_json::from_value(picked["paths"].clone()).unwrap_or_default();
            let lines = self.import(host, &paths)?;
            return Ok(json!({ "notice": lines.join("\n") }));
        }
        Ok(json!("none"))
    }

    fn import(&self, host: &mut Host, paths: &[PathBuf]) -> Result<Vec<String>, String> {
        std::fs::create_dir_all(self.items_dir()).map_err(|e| e.to_string())?;
        let mut lines = Vec::new();
        for (i, path) in paths.iter().enumerate() {
            if !is_item(path) {
                lines.push(format!("Skipped {}: not a .{EXTENSION} file", path.display()));
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| format!("{} has no name", path.display()))?;
            host.progress(i as f32 / paths.len() as f32, &format!("Importing {name}"));
            std::fs::copy(path, self.items_dir().join(name)).map_err(|e| format!("{name}: {e}"))?;
            lines.push(format!("Imported {name}"));
        }
        Ok(lines)
    }

    fn build(&self, host: &mut Host) -> Result<Value, String> {
        let on = self.enabled(host)?;
        let text = on.join("\n");
        let file = self.cache_dir.join("enabled.txt");
        if std::fs::read_to_string(&file).ok().as_deref() != Some(text.as_str()) {
            std::fs::write(&file, &text).map_err(|e| e.to_string())?;
        }
        Ok(json!({
            "ops": [{
                "op": "file.put",
                "source": { "root": "cache", "path": "enabled.txt" },
                "target": "example-launcher-addon/enabled.txt"
            }],
            "lines": [format!("{} items on", on.len())]
        }))
    }

    fn command(&self, host: &mut Host) -> Result<Value, String> {
        let on = self.enabled(host)?;
        let lines: Vec<String> = self
            .items()
            .into_iter()
            .map(|n| format!("{} {n}", if on.contains(&n) { "on " } else { "off" }))
            .collect();
        Ok(json!(lines.join("\n")))
    }

    fn summary(&self, host: &mut Host) -> Result<Value, String> {
        let on = self.enabled(host)?;
        if on.is_empty() {
            return Ok(Value::Null);
        }
        Ok(json!({ "label": "EXAMPLE", "items": on }))
    }
}

fn path(value: &Value) -> PathBuf {
    PathBuf::from(value.as_str().unwrap_or_default())
}

fn main() {
    let stdin = std::io::stdin();
    let mut host = Host {
        lines: stdin.lock().lines(),
        out: std::io::stdout().lock(),
        next: 0,
    };
    let mut addon = Addon {
        data_dir: PathBuf::new(),
        cache_dir: PathBuf::new(),
        vanilla: false,
    };
    while let Some(Ok(line)) = host.lines.next() {
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            eprintln!("not JSON: {line}");
            continue;
        };
        let params = &message["params"];
        let method = message["method"].as_str().unwrap_or_default();
        let Some(id) = message["id"].as_u64() else {
            if method == "profile" {
                addon.vanilla = params["vanilla"].as_bool().unwrap_or(false);
            }
            continue;
        };
        let result = match method {
            "hello" => {
                addon.data_dir = path(&params["data_dir"]);
                addon.cache_dir = path(&params["cache_dir"]);
                addon.vanilla = params["profile"]["vanilla"].as_bool().unwrap_or(false);
                Ok(json!({ "api": API }))
            }
            "view" => addon.view(&mut host),
            "event" => addon.event(&mut host, &params["event"]),
            "import" => {
                let paths: Vec<PathBuf> = serde_json::from_value(params["paths"].clone()).unwrap_or_default();
                addon.import(&mut host, &paths).map(|lines| json!(lines))
            }
            "build" => addon.build(&mut host),
            "command" => addon.command(&mut host),
            "summary" => addon.summary(&mut host),
            other => Err(format!("unknown method {other}")),
        };
        let reply = match result {
            Ok(result) => json!({ "id": id, "result": result }),
            Err(error) => json!({ "id": id, "error": error }),
        };
        host.send(&reply);
    }
}

fn is_item(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION))
}
