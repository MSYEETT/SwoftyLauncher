use std::io::{BufRead, Lines, StdinLock, StdoutLock, Write};

use serde_json::{Value, json};

pub struct Host {
    lines: Lines<StdinLock<'static>>,
    out: StdoutLock<'static>,
    next: u64,
}

impl Default for Host {
    fn default() -> Self {
        Self::new()
    }
}

impl Host {
    pub fn new() -> Self {
        Host {
            lines: std::io::stdin().lock().lines(),
            out: std::io::stdout().lock(),
            next: 1_000_000,
        }
    }

    pub fn read_line(&mut self) -> Option<String> {
        match self.lines.next() {
            Some(Ok(line)) => Some(line),
            _ => None,
        }
    }

    pub fn send(&mut self, message: &Value) {
        writeln!(self.out, "{message}")
            .and_then(|()| self.out.flush())
            .ok();
    }

    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next;
        self.next += 1;
        self.send(&json!({ "id": id, "method": method, "params": params }));
        loop {
            let Some(line) = self.read_line() else {
                std::process::exit(0)
            };
            let Ok(reply) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if reply["id"].as_u64() != Some(id) {
                continue;
            }
            return match reply.get("error") {
                Some(e) => Err(e.as_str().map_or_else(|| e.to_string(), str::to_owned)),
                None => Ok(reply["result"].clone()),
            };
        }
    }

    pub fn invalidate(&mut self, section: Option<&str>) {
        let params = match section {
            Some(s) => json!({ "section": s }),
            None => json!({}),
        };
        self.send(&json!({ "method": "invalidate", "params": params }));
    }
}
