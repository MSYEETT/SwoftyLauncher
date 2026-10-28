//! The test agent's mailbox: a folder `<game>/swofty/test/<run>/` that the
//! CLI and the core's test agent share. The CLI writes `session.json` (the
//! run's token and the test DLLs) and appends one JSON command per line to
//! `commands.jsonl`; the agent appends one JSON line per result or event to
//! `results.jsonl`. The agent arms only when the game's environment carries
//! the run (`SWOFTY_TEST`) and the token (`SWOFTY_TEST_TOKEN`) that
//! `session.json` holds.

use std::collections::BTreeMap;
use std::hash::{BuildHasher, Hasher};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

use super::Error;
use super::suite::Step;

pub const PROTOCOL: u64 = 1;
pub const DIR: &str = "swofty/test";
pub const SESSION: &str = "session.json";
pub const COMMANDS: &str = "commands.jsonl";
pub const RESULTS: &str = "results.jsonl";
pub const DLL_DIR: &str = "dll";
pub const ENV_RUN: &str = "SWOFTY_TEST";
pub const ENV_TOKEN: &str = "SWOFTY_TEST_TOKEN";

/// The commands the agent knows, in protocol 1.
pub const COMMANDS_V1: &[&str] = &[
    "hello",
    "status",
    "route",
    "skip",
    "advance",
    "ai_drives_player",
    "read",
    "set",
    "capture",
    "wait",
    "at",
    "pad",
    "save",
    "quit",
    "run_test",
];

pub fn dir(game: &Path, run: &str) -> PathBuf {
    game.join(DIR).join(run)
}

/// A run name the agent accepts as a folder name.
pub fn valid_run(run: &str) -> bool {
    !run.is_empty()
        && run.len() <= 64
        && run
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        && !run.starts_with('.')
}

/// A fresh token for one game start: 128 bits from the process's random
/// hasher keys, the time and the pid.
pub fn token() -> String {
    let mut out = String::new();
    for i in 0..2u64 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(now_ms());
        h.write_u32(std::process::id());
        h.write_u64(i);
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub run: String,
    pub token: String,
    pub created_ms: u64,
    /// Test DLLs, relative to the mailbox folder.
    pub dlls: Vec<String>,
}

impl Session {
    pub fn new(run: &str) -> Self {
        Session {
            run: run.to_owned(),
            token: token(),
            created_ms: now_ms(),
            dlls: Vec::new(),
        }
    }

    pub fn json(&self) -> String {
        json!({
            "v": PROTOCOL,
            "run": self.run,
            "token": self.token,
            "created_ms": self.created_ms,
            "dlls": self.dlls,
        })
        .to_string()
    }

    /// The game's environment for this run.
    pub fn env(&self) -> Vec<(String, String)> {
        vec![
            (ENV_RUN.to_owned(), self.run.clone()),
            (ENV_TOKEN.to_owned(), self.token.clone()),
        ]
    }
}

/// The mailbox folder of one game start, made by the CLI.
#[derive(Debug, Clone)]
pub struct Mailbox {
    pub dir: PathBuf,
    pub session: Session,
    dll_sources: Vec<(PathBuf, String)>,
}

impl Mailbox {
    pub fn new(game: &Path, session: Session) -> Self {
        Mailbox {
            dir: dir(game, &session.run),
            session,
            dll_sources: Vec::new(),
        }
    }

    /// Stage a test DLL: it is copied into `dll/<name>` when the mailbox is
    /// written.
    pub fn add_dll(&mut self, source: &Path, name: &str) {
        let rel = format!("{DLL_DIR}/{name}");
        if !self.session.dlls.contains(&rel) {
            self.session.dlls.push(rel.clone());
            self.dll_sources.push((source.to_path_buf(), rel));
        }
    }

    /// Write whatever is missing: the DLLs, an empty command file and, last,
    /// `session.json`. Safe to call again: a launch that clears the folder
    /// gets it back on the next call.
    pub fn write(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.dir.join(DLL_DIR))?;
        for (src, rel) in &self.dll_sources {
            let dest = self.dir.join(rel);
            if !dest.exists() {
                std::fs::copy(src, &dest)?;
            }
        }
        let commands = self.dir.join(COMMANDS);
        if !commands.exists() {
            std::fs::write(&commands, "")?;
        }
        let session = self.dir.join(SESSION);
        if !session.exists() {
            let tmp = self.dir.join("session.json.tmp");
            std::fs::write(&tmp, self.session.json())?;
            std::fs::rename(&tmp, &session)?;
        }
        Ok(())
    }

    pub fn remove(&self) -> std::io::Result<()> {
        match std::fs::remove_dir_all(&self.dir) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    pub seq: u64,
    pub cmd: String,
    pub args: Value,
    pub timeout_ms: Option<u64>,
}

impl Command {
    pub fn line(&self) -> String {
        let mut o = Map::new();
        o.insert("seq".into(), json!(self.seq));
        o.insert("cmd".into(), json!(self.cmd));
        if !self.args.is_null() {
            o.insert("args".into(), self.args.clone());
        }
        if let Some(t) = self.timeout_ms {
            o.insert("timeout_ms".into(), json!(t));
        }
        Value::Object(o).to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    Error,
    Unavailable,
    Timeout,
}

impl Outcome {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "ok" => Outcome::Ok,
            "error" => Outcome::Error,
            "unavailable" => Outcome::Unavailable,
            "timeout" => Outcome::Timeout,
            _ => return None,
        })
    }

    pub fn word(self) -> &'static str {
        match self {
            Outcome::Ok => "ok",
            Outcome::Error => "error",
            Outcome::Unavailable => "unavailable",
            Outcome::Timeout => "timed out",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub seq: u64,
    pub outcome: Outcome,
    pub value: Value,
    pub error: String,
    pub ms: u64,
}

impl Reply {
    pub fn text(&self) -> String {
        match self.outcome {
            Outcome::Ok if self.value.is_null() => "ok".into(),
            Outcome::Ok => format!("ok {}", self.value),
            o if self.error.is_empty() => o.word().into(),
            o => format!("{}: {}", o.word(), self.error),
        }
    }
}

/// One line of `results.jsonl`.
#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    Hello(Value),
    Refused(String),
    Reply(Reply),
    /// `{"event": <name>, ...}`: an event fire, a test's expectation or log
    /// line, a pad or capture notice.
    Event(String, Value),
}

pub fn parse_line(text: &str) -> Option<Line> {
    let v: Value = serde_json::from_str(text.trim()).ok()?;
    let o = v.as_object()?;
    if let Some(seq) = o.get("seq").and_then(Value::as_u64) {
        let outcome = Outcome::parse(o.get("status")?.as_str()?)?;
        return Some(Line::Reply(Reply {
            seq,
            outcome,
            value: o.get("value").cloned().unwrap_or(Value::Null),
            error: o.get("error").and_then(Value::as_str).unwrap_or("").to_owned(),
            ms: o.get("ms").and_then(Value::as_u64).unwrap_or(0),
        }));
    }
    let name = o.get("event")?.as_str()?.to_owned();
    Some(match name.as_str() {
        "hello" => Line::Hello(v.clone()),
        "refused" => Line::Refused(o.get("why").and_then(Value::as_str).unwrap_or("").to_owned()),
        _ => Line::Event(name, v.clone()),
    })
}

/// The agent command a test step becomes, or `None` for the steps the CLI
/// runs itself (`wait` for a log line, `sample`).
pub fn step_commands(step: &Step) -> Result<Vec<(String, Value)>, Error> {
    let v: Value = serde_json::from_str(&step.text).map_err(|e| Error(e.to_string()))?;
    let arg = v.get(&step.action).cloned().unwrap_or(Value::Null);
    let one = |cmd: &str, args: Value| Ok(vec![(cmd.to_owned(), args)]);
    match step.action.as_str() {
        "route" => one("route", json!({ "route": arg })),
        "skip" => one("skip", json!({ "to": arg })),
        "advance" => one("advance", arg),
        "ai_drives_player" => one("ai_drives_player", json!({ "on": arg })),
        "pad" => one("pad", arg),
        "read" => one("read", json!({ "path": arg })),
        "save" => one("save", json!({ "what": arg })),
        "capture" => {
            let name = v.get("name").and_then(Value::as_str).unwrap_or("frame");
            one("capture", json!({ "name": name }))
        }
        "wait" => match arg.get("event") {
            Some(e) => one("wait", json!({ "event": e })),
            None => Ok(Vec::new()),
        },
        "set" => {
            let o = arg
                .as_object()
                .ok_or_else(|| Error("set takes an object of paths and values".into()))?;
            Ok(o.iter()
                .map(|(path, value)| ("set".to_owned(), json!({ "path": path, "value": value })))
                .collect())
        }
        _ => Ok(Vec::new()),
    }
}

/// A command a person types in `swofty test shell`: `route career`,
/// `read car[0].chassis.mass`, `wait race.start 90`, or a JSON object.
pub fn parse_shell(line: &str) -> Result<(String, Value, Option<u64>), String> {
    let line = line.trim();
    if line.starts_with('{') {
        let v: Value = serde_json::from_str(line).map_err(|e| format!("not valid JSON: {e}"))?;
        let cmd = v
            .get("cmd")
            .and_then(Value::as_str)
            .ok_or("a JSON command needs \"cmd\"")?
            .to_owned();
        let timeout = v.get("timeout_ms").and_then(Value::as_u64);
        return Ok((cmd, v.get("args").cloned().unwrap_or(Value::Null), timeout));
    }
    let mut words = line.split_whitespace();
    let cmd = words.next().ok_or("empty command")?.to_owned();
    let rest: Vec<&str> = words.collect();
    let need = |n: usize, usage: &str| match rest.len() >= n {
        true => Ok(()),
        false => Err(format!("usage: {usage}")),
    };
    let secs = |s: Option<&&str>| -> Result<Option<u64>, String> {
        s.map(|s| {
            s.parse::<u64>()
                .map(|n| n * 1000)
                .map_err(|_| format!("{s}: expected seconds"))
        })
        .transpose()
    };
    let value = |s: &str| serde_json::from_str::<Value>(s).unwrap_or_else(|_| Value::String(s.to_owned()));
    Ok(match cmd.as_str() {
        "hello" | "status" | "quit" => (cmd, Value::Null, None),
        "route" => {
            need(1, "route <name>")?;
            (cmd, json!({ "route": rest[0] }), None)
        }
        "skip" => {
            need(1, "skip next|simulate|session|quick_practice [session type]")?;
            let mut a = json!({ "to": rest[0] });
            if let Some(s) = rest.get(1) {
                let n: u64 = s
                    .parse()
                    .map_err(|_| format!("{s}: expected a session type number"))?;
                a["session"] = json!(n);
            }
            (cmd, a, None)
        }
        "advance" => {
            need(1, "advance <days>")?;
            let d: u64 = rest[0]
                .parse()
                .map_err(|_| format!("{}: expected days", rest[0]))?;
            (cmd, json!({ "days": d }), None)
        }
        "ai_drives_player" | "ai" => {
            need(1, "ai_drives_player on|off")?;
            let on = matches!(rest[0], "on" | "true" | "1");
            ("ai_drives_player".into(), json!({ "on": on }), None)
        }
        "read" => {
            need(1, "read <path>")?;
            (cmd, json!({ "path": rest[0] }), None)
        }
        "set" => {
            need(2, "set <path> <value>")?;
            (
                cmd,
                json!({ "path": rest[0], "value": value(&rest[1..].join(" ")) }),
                None,
            )
        }
        "capture" => (
            cmd,
            json!({ "name": rest.first().copied().unwrap_or("frame") }),
            None,
        ),
        "wait" => {
            need(1, "wait <event> [seconds]")?;
            let t = secs(rest.get(1))?;
            (cmd, json!({ "event": rest[0] }), t)
        }
        "at" => {
            need(1, "at <screen or flow state> [seconds]")?;
            let t = secs(rest.get(1))?;
            (cmd, json!({ "phase": rest[0] }), t)
        }
        "pad" => {
            need(1, "pad <button> [then <button>] [ms]")?;
            let mut a = json!({ "press": rest[0] });
            let mut i = 1;
            if rest.get(1) == Some(&"then") && rest.len() > 2 {
                a["then"] = json!(rest[2]);
                i = 3;
            }
            if let Some(ms) = rest.get(i) {
                a["ms"] = json!(
                    ms.parse::<u64>()
                        .map_err(|_| format!("{ms}: expected milliseconds"))?
                );
            }
            (cmd, a, None)
        }
        "save" => (
            cmd,
            json!({ "what": rest.first().copied().unwrap_or("now") }),
            None,
        ),
        "run_test" | "test" => {
            need(1, "run_test <fn>")?;
            ("run_test".into(), json!({ "fn": rest[0] }), None)
        }
        other => {
            return Err(format!(
                "unknown command {other} (one of: {})",
                COMMANDS_V1.join(", ")
            ));
        }
    })
}

/// Why the CLI stopped waiting for a reply.
#[derive(Debug, Clone, PartialEq)]
pub enum WaitError {
    /// No reply within the time.
    Timeout(Duration),
    /// The game exited first.
    Gone,
    Io(String),
}

impl std::fmt::Display for WaitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WaitError::Timeout(d) => write!(f, "no answer from the test agent within {} s", d.as_secs()),
            WaitError::Gone => f.write_str("the game exited before the test agent answered"),
            WaitError::Io(e) => write!(f, "the mailbox: {e}"),
        }
    }
}

/// The CLI's end of a mailbox.
#[derive(Debug)]
pub struct Client {
    pub dir: PathBuf,
    offset: u64,
    partial: String,
    next_seq: u64,
    pub hello: Option<Value>,
    pub refused: Option<String>,
    replies: BTreeMap<u64, Reply>,
    /// Every event line read so far, in order.
    pub events: Vec<(String, Value)>,
    /// Every line read so far, as the agent wrote it.
    pub lines: Vec<String>,
}

impl Client {
    pub fn new(dir: &Path) -> Self {
        Client {
            dir: dir.to_path_buf(),
            offset: 0,
            partial: String::new(),
            next_seq: 1,
            hello: None,
            refused: None,
            replies: BTreeMap::new(),
            events: Vec::new(),
            lines: Vec::new(),
        }
    }

    /// Append a command; returns its seq.
    pub fn send(&mut self, cmd: &str, args: Value, timeout_ms: Option<u64>) -> std::io::Result<u64> {
        let c = Command {
            seq: self.next_seq,
            cmd: cmd.to_owned(),
            args,
            timeout_ms,
        };
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join(COMMANDS))?;
        f.write_all(format!("{}\n", c.line()).as_bytes())?;
        f.flush()?;
        self.next_seq += 1;
        Ok(c.seq)
    }

    /// Read the lines the agent added since the last call. Returns the new
    /// lines; replies, the hello and events are also kept.
    pub fn poll(&mut self) -> Vec<Line> {
        let mut out = Vec::new();
        let Ok(mut f) = std::fs::File::open(self.dir.join(RESULTS)) else {
            return out;
        };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.offset {
            self.offset = 0;
            self.partial.clear();
        }
        if len == self.offset || f.seek(SeekFrom::Start(self.offset)).is_err() {
            return out;
        }
        let mut buf = Vec::new();
        if f.read_to_end(&mut buf).is_err() {
            return out;
        }
        self.offset += buf.len() as u64;
        self.partial.push_str(&String::from_utf8_lossy(&buf));
        while let Some(at) = self.partial.find('\n') {
            let line: String = self.partial.drain(..=at).collect();
            self.lines.push(line.trim_end().to_owned());
            let Some(l) = parse_line(&line) else { continue };
            match &l {
                Line::Hello(v) => self.hello = Some(v.clone()),
                Line::Refused(why) => self.refused = Some(why.clone()),
                Line::Reply(r) => {
                    self.replies.insert(r.seq, r.clone());
                }
                Line::Event(name, v) => self.events.push((name.clone(), v.clone())),
            }
            out.push(l);
        }
        out
    }

    pub fn take_reply(&mut self, seq: u64) -> Option<Reply> {
        self.replies.remove(&seq)
    }

    /// Wait for the reply to `seq`, polling every `poll`, while `alive`
    /// says the game still runs.
    pub fn wait_reply(
        &mut self,
        seq: u64,
        limit: Duration,
        poll: Duration,
        mut alive: impl FnMut() -> bool,
    ) -> Result<Reply, WaitError> {
        let start = Instant::now();
        loop {
            self.poll();
            if let Some(r) = self.take_reply(seq) {
                return Ok(r);
            }
            if !alive() {
                self.poll();
                return self.take_reply(seq).ok_or(WaitError::Gone);
            }
            if start.elapsed() >= limit {
                return Err(WaitError::Timeout(limit));
            }
            std::thread::sleep(
                poll.min(limit.saturating_sub(start.elapsed()))
                    .max(Duration::from_millis(1)),
            );
        }
    }

    /// Send a command and wait for its reply. The agent's own time limit is
    /// `timeout`; the CLI waits that long plus `grace` for the answer.
    pub fn call(
        &mut self,
        cmd: &str,
        args: Value,
        timeout: Option<Duration>,
        grace: Duration,
        poll: Duration,
        alive: impl FnMut() -> bool,
    ) -> Result<Reply, WaitError> {
        let seq = self
            .send(cmd, args, timeout.map(|t| t.as_millis() as u64))
            .map_err(|e| WaitError::Io(e.to_string()))?;
        self.wait_reply(seq, timeout.unwrap_or_default() + grace, poll, alive)
    }

    /// Wait for the agent's hello (or its refusal).
    pub fn wait_hello(
        &mut self,
        limit: Duration,
        poll: Duration,
        mut alive: impl FnMut() -> bool,
        mut each: impl FnMut(),
    ) -> Result<Value, String> {
        let start = Instant::now();
        loop {
            self.poll();
            if let Some(h) = &self.hello {
                return Ok(h.clone());
            }
            if let Some(why) = &self.refused {
                return Err(format!("the test agent refused the run: {why}"));
            }
            if !alive() {
                return Err("the game exited before the test agent said hello".into());
            }
            if start.elapsed() >= limit {
                return Err(format!(
                    "the test agent did not say hello within {} s (is the core built with it, and did the game reach its window?)",
                    limit.as_secs()
                ));
            }
            each();
            std::thread::sleep(poll);
        }
    }
}

/// A value read from the agent as a number, for checks.
pub fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
        Value::Object(o) => o.get("value").and_then(number),
        _ => None,
    }
}
