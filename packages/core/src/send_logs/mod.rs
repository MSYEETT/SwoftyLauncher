use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::store::Store;
use crate::{Error, GAME_EXE, Result};

pub mod webhook;

pub use webhook::{SendError, Stage, Webhook};

pub const UNAVAILABLE: &str = "Sending logs isn't available in this build";
pub const NAME_MIN: usize = 2;
pub const NAME_MAX: usize = 32;
pub const MESSAGE_MAX: usize = 1000;
pub const FILE_CAP: u64 = 5 << 20;
pub const TOTAL_CAP: u64 = 40 << 20;
pub const PART_LIMIT: usize = 9 << 20;
pub const SESSIONS: usize = 3;
pub const SNAPSHOT: &str = "swofty/addons/swofty.career-editor/snapshot.json";

const REDACTED: &str = "[redacted]";
const SESSION_DEPTH: usize = 3;
const MIN_TAIL: u64 = 64 << 10;

/// Where the logs go: the webhook baked in at build time, if any.
pub fn destination() -> Option<Webhook> {
    Webhook::new(option_env!("SWOFTY_LOG_WEBHOOK"))
}

pub fn available() -> bool {
    destination().is_some()
}

pub fn send(bundle: &Bundle, progress: &dyn Fn(Stage)) -> std::result::Result<usize, SendError> {
    destination()
        .ok_or(SendError::Unavailable)?
        .send(bundle, progress)
}

pub fn check_name(raw: &str) -> std::result::Result<String, String> {
    let name = raw.trim();
    match name.chars().count() {
        0 => Err("Enter your Discord username".into()),
        n if n < NAME_MIN => Err(format!("A Discord username has at least {NAME_MIN} characters")),
        n if n > NAME_MAX => Err(format!("A Discord username has at most {NAME_MAX} characters")),
        _ => Ok(name.to_owned()),
    }
}

pub fn check_message(raw: &str) -> std::result::Result<String, String> {
    let message = raw.trim();
    if message.chars().count() > MESSAGE_MAX {
        return Err(format!("Keep it to {MESSAGE_MAX} characters"));
    }
    Ok(message.to_owned())
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GameInfo {
    pub dir: Option<PathBuf>,
    pub build_id: Option<u64>,
    pub version: Option<String>,
}

impl GameInfo {
    /// What the launcher's game detection finds: the folder saved in
    /// `config.json`, else the Steam library that has the game.
    pub fn detect(store: &Store) -> Self {
        let found = crate::steam::find_game().filter(|g| g.exe().is_file());
        let manual = store.config().game_dir.filter(|d| d.join(GAME_EXE).is_file());
        let (dir, build_id) = match (manual, found) {
            (Some(dir), found) => {
                let build = found.filter(|g| g.dir == dir).and_then(|g| g.build_id);
                (dir, build)
            }
            (None, Some(g)) => (g.dir, g.build_id),
            (None, None) => return Self::default(),
        };
        Self {
            version: crate::exe_version::read(&dir.join(GAME_EXE)),
            dir: Some(dir),
            build_id,
        }
    }

    pub fn describe(&self) -> String {
        if self.dir.is_none() {
            return "not found".into();
        }
        match (&self.version, self.build_id) {
            (Some(v), Some(b)) => format!("{v} (Steam build {b})"),
            (Some(v), None) => v.clone(),
            (None, Some(b)) => format!("Steam build {b}"),
            (None, None) => "found, version unknown".into(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub name: String,
    pub message: String,
    pub game: GameInfo,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: String,
    pub data: Vec<u8>,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Bundle {
    pub stem: String,
    pub name: String,
    pub message: String,
    pub launcher: String,
    pub os: String,
    pub game: String,
    pub entries: Vec<Entry>,
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub file_name: String,
    pub data: Vec<u8>,
    pub index: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub file: u64,
    pub total: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            file: FILE_CAP,
            total: TOTAL_CAP,
        }
    }
}

struct Collector {
    limits: Limits,
    used: u64,
    entries: Vec<Entry>,
    skipped: Vec<String>,
}

impl Collector {
    fn budget(&self) -> u64 {
        self.limits.total.saturating_sub(self.used)
    }

    fn push(&mut self, path: String, data: Vec<u8>, note: Option<String>) {
        self.used += data.len() as u64;
        self.entries.push(Entry { path, data, note });
    }

    fn file(&mut self, path: String, from: &Path, redact: Option<fn(&str) -> String>) {
        let cap = self.limits.file.min(self.budget());
        let len = match std::fs::metadata(from) {
            Ok(m) => m.len(),
            Err(e) => return self.skipped.push(format!("{path}: {e}")),
        };
        if cap < len && cap < MIN_TAIL.min(self.limits.file) {
            return self.skipped.push(format!(
                "{path} ({}): the bundle reached its {} limit",
                size(len),
                size(self.limits.total)
            ));
        }
        match read_tail(from, cap) {
            Ok((data, cut)) => {
                let redacted = redact
                    .map(|f| f(&String::from_utf8_lossy(&data)))
                    .filter(|r| r.as_bytes() != data.as_slice());
                let changed = redacted.is_some();
                let mut out = Vec::new();
                if cut {
                    out.extend_from_slice(
                        format!("[swofty: kept the last {} of {}]\n", size(cap), size(len)).as_bytes(),
                    );
                }
                let note = match (cut, changed) {
                    (true, true) => Some(format!("last {} of {}, secrets redacted", size(cap), size(len))),
                    (true, false) => Some(format!("last {} of {}", size(cap), size(len))),
                    (false, true) => Some("secrets redacted".into()),
                    (false, false) => None,
                };
                out.extend_from_slice(redacted.as_ref().map_or(data.as_slice(), |r| r.as_bytes()));
                self.push(path, out, note);
            }
            Err(e) => self.skipped.push(format!("{path}: {e}")),
        }
    }
}

impl Bundle {
    pub fn collect(store: &Store, report: &Report) -> Self {
        Self::collect_with(store, report, Limits::default(), &utc_stamp(now_secs()))
    }

    pub fn collect_with(store: &Store, report: &Report, limits: Limits, stamp: &str) -> Self {
        let mut c = Collector {
            limits,
            used: 0,
            entries: Vec::new(),
            skipped: Vec::new(),
        };
        match std::fs::read(store.config_path()) {
            Ok(data) => {
                let (text, note) = match serde_json::from_slice::<Value>(&data) {
                    Ok(mut v) => {
                        let n = redact_json(&mut v);
                        let text = serde_json::to_string_pretty(&v).unwrap_or_default();
                        (text, (n > 0).then(|| "secrets redacted".to_owned()))
                    }
                    Err(e) => (
                        format!("config.json is not valid JSON ({e}); its contents were left out"),
                        Some("unreadable, left out".to_owned()),
                    ),
                };
                c.push("config.json".into(), text.into_bytes(), note);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => c.skipped.push("config.json: none".into()),
            Err(e) => c.skipped.push(format!("config.json: {e}")),
        }
        let addons = serde_json::to_string_pretty(&addons_summary(store)).unwrap_or_default();
        c.push("addons.json".into(), addons.into_bytes(), None);
        for (id, log) in helper_logs(store) {
            c.file(format!("addons/{id}/{}", crate::helper::LOG_FILE), &log, None);
        }
        if let Some(snapshot) = report.game.dir.as_ref().map(|d| d.join(SNAPSHOT))
            && snapshot.is_file()
        {
            c.file("game/career-editor/snapshot.json".into(), &snapshot, None);
        }
        for log in state_logs(store) {
            let name = file_name(&log);
            c.file(format!("state/{name}"), &log, None);
        }
        for dir in newest_sessions(store, SESSIONS) {
            let session = file_name(&dir);
            for (rel, file) in files_under(&dir, SESSION_DEPTH) {
                let redact = (rel == "env.txt").then_some(redact_env as fn(&str) -> String);
                c.file(format!("sessions/{session}/{rel}"), &file, redact);
            }
        }
        let os = os_description();
        let mut bundle = Self {
            stem: format!("swofty-logs-{}-{stamp}", file_safe(&report.name)),
            name: report.name.clone(),
            message: report.message.clone(),
            launcher: crate::update::current_version().to_string(),
            os,
            game: report.game.describe(),
            entries: c.entries,
            skipped: c.skipped,
        };
        let summary = bundle.summary(store, report, stamp);
        bundle.entries.insert(
            0,
            Entry {
                path: "summary.txt".into(),
                data: summary.into_bytes(),
                note: None,
            },
        );
        bundle
    }

    fn summary(&self, store: &Store, report: &Report, stamp: &str) -> String {
        let mut s = String::new();
        let mut line = |k: &str, v: &str| s.push_str(&format!("{k:16} {v}\n"));
        line("Discord username", &self.name);
        line("Collected", &readable(stamp));
        line("Launcher", &self.launcher);
        line("OS", &self.os);
        line("Hardware", &hardware());
        line("F1 25", &self.game);
        if let Some(dir) = &report.game.dir {
            line("Game folder", &dir.display().to_string());
        }
        line("Data folder", &store.root.display().to_string());
        s.push_str("\nWhat went wrong\n");
        s.push_str(if self.message.is_empty() {
            "(not said)"
        } else {
            &self.message
        });
        s.push_str("\n\nFiles\n");
        for e in &self.entries {
            s.push_str(&format!("  {:60} {:>9}", e.path, size(e.data.len() as u64)));
            if let Some(n) = &e.note {
                s.push_str(&format!("  ({n})"));
            }
            s.push('\n');
        }
        if !self.skipped.is_empty() {
            s.push_str("\nLeft out\n");
            for k in &self.skipped {
                s.push_str(&format!("  {k}\n"));
            }
        }
        s
    }

    pub fn listing(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|e| match &e.note {
                Some(n) => format!("{}  {}  ({n})", e.path, size(e.data.len() as u64)),
                None => format!("{}  {}", e.path, size(e.data.len() as u64)),
            })
            .collect()
    }

    pub fn file_name(&self) -> String {
        format!("{}.zip", self.stem)
    }

    pub fn zip(&self) -> Result<Vec<u8>> {
        zip_entries(self.entries.iter())
    }

    /// The zip files to upload, each at most `limit` bytes where possible:
    /// one file when everything fits, else the entries packed in order into
    /// standalone zips named `-part<n>`.
    pub fn parts(&self, limit: usize) -> Result<Vec<Part>> {
        let whole = self.zip()?;
        if whole.len() <= limit {
            return Ok(vec![Part {
                file_name: self.file_name(),
                data: whole,
                index: 1,
                total: 1,
            }]);
        }
        let mut groups: Vec<Vec<&Entry>> = vec![Vec::new()];
        let mut used = 0usize;
        for e in &self.entries {
            let cost = compressed_len(&e.data) + 2 * e.path.len() + 128;
            let group = groups.last_mut().expect("never empty");
            if !group.is_empty() && used + cost > limit {
                groups.push(Vec::new());
                used = 0;
            }
            groups.last_mut().expect("never empty").push(e);
            used += cost;
        }
        let total = groups.len();
        groups
            .into_iter()
            .enumerate()
            .map(|(i, g)| {
                Ok(Part {
                    file_name: format!("{}-part{}.zip", self.stem, i + 1),
                    data: zip_entries(g.into_iter())?,
                    index: i + 1,
                    total,
                })
            })
            .collect()
    }
}

fn zip_entries<'a>(entries: impl Iterator<Item = &'a Entry>) -> Result<Vec<u8>> {
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(false);
    for e in entries {
        z.start_file(e.path.as_str(), opts)?;
        z.write_all(&e.data)
            .map_err(|e| Error::Other(format!("writing the logs zip: {e}")))?;
    }
    Ok(z.finish()?.into_inner())
}

fn compressed_len(data: &[u8]) -> usize {
    let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(data).ok();
    enc.finish().map_or(data.len(), |v| v.len())
}

fn read_tail(path: &Path, cap: u64) -> std::io::Result<(Vec<u8>, bool)> {
    let mut f = std::fs::File::open(path)?;
    let len = f.metadata()?.len();
    if len <= cap {
        let mut data = Vec::with_capacity(len as usize);
        f.read_to_end(&mut data)?;
        return Ok((data, false));
    }
    f.seek(SeekFrom::Start(len - cap))?;
    let mut data = Vec::with_capacity(cap as usize);
    f.take(cap).read_to_end(&mut data)?;
    if let Some(nl) = data
        .iter()
        .position(|&b| b == b'\n')
        .filter(|&i| i < data.len() / 4)
    {
        data.drain(..=nl);
    }
    Ok((data, true))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

fn state_logs(store: &Store) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(store.root.join("state"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|x| x == "log")
                && std::fs::symlink_metadata(p).is_ok_and(|m| m.is_file())
        })
        .collect();
    out.sort_by_key(|p| {
        let name = file_name(p);
        (
            !matches!(name.as_str(), "launcher-run.log" | "wrapper-run.log"),
            name,
        )
    });
    out
}

pub fn newest_sessions(store: &Store, n: usize) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(store.sessions_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .filter(|p| file_name(p).starts_with(|c: char| c.is_ascii_digit()))
        .collect();
    dirs.sort_by_key(|p| std::cmp::Reverse(file_name(p)));
    dirs.truncate(n);
    dirs
}

fn files_under(dir: &Path, depth: usize) -> Vec<(String, PathBuf)> {
    fn walk(root: &Path, dir: &Path, depth: usize, out: &mut Vec<(String, PathBuf)>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        let mut items: Vec<_> = rd.flatten().collect();
        items.sort_by_key(|e| e.file_name());
        for e in items {
            let Ok(t) = e.file_type() else { continue };
            let path = e.path();
            if t.is_file() {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                let rel = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push((rel, path));
            } else if t.is_dir() && depth > 1 {
                walk(root, &path, depth - 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, depth, &mut out);
    out
}

fn helper_logs(store: &Store) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = std::fs::read_dir(store.root.join("cache/addons"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                e.path().join(crate::helper::LOG_FILE),
            )
        })
        .filter(|(_, log)| log.is_file())
        .collect();
    out.sort();
    out
}

fn addons_summary(store: &Store) -> Value {
    let installed = match store.addons() {
        Ok(addons) => Value::Array(
            addons
                .values()
                .map(|m| {
                    json!({
                        "id": m.id,
                        "name": m.name,
                        "version": m.version.to_string(),
                        "builtin": store.is_builtin(&m.id),
                    })
                })
                .collect(),
        ),
        Err(e) => json!({ "error": e.to_string() }),
    };
    let active = store.active_profile();
    let profiles = match store.profile_names() {
        Ok(names) => Value::Array(
            names
                .iter()
                .map(|name| match store.profile(name) {
                    Ok(p) => json!({
                        "name": name,
                        "active": *name == active,
                        "addons": p.addons.iter().map(|a| json!({ "id": a.id, "enabled": a.enabled })).collect::<Vec<_>>(),
                    }),
                    Err(e) => json!({ "name": name, "error": e.to_string() }),
                })
                .collect(),
        ),
        Err(e) => json!({ "error": e.to_string() }),
    };
    json!({ "installed": installed, "profiles": profiles })
}

/// A field or variable name that may hold a secret.
pub fn secret_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    const PARTS: [&str; 10] = [
        "token",
        "secret",
        "passw",
        "passphrase",
        "key",
        "credential",
        "cookie",
        "private",
        "webhook",
        "bearer",
    ];
    PARTS.iter().any(|p| n.contains(p))
        || n == "auth"
        || n.starts_with("auth_")
        || n.ends_with("_auth")
        || n.contains("_auth_")
        || n.contains("oauth")
}

fn secret_value(value: &str) -> bool {
    let v = value.to_ascii_lowercase();
    v.contains("/api/webhooks/") || v.contains("discord.com/api") && v.contains("token")
}

/// Replace every secret-looking value in a JSON document; returns how many.
pub fn redact_json(v: &mut Value) -> usize {
    match v {
        Value::Object(map) => map
            .iter_mut()
            .map(|(k, v)| {
                if secret_name(k) && !v.is_null() {
                    *v = Value::String(REDACTED.into());
                    1
                } else {
                    redact_json(v)
                }
            })
            .sum(),
        Value::Array(items) => items.iter_mut().map(redact_json).sum(),
        Value::String(s) if secret_value(s) => {
            *s = REDACTED.into();
            1
        }
        _ => 0,
    }
}

/// `NAME=value` lines with a secret-looking name or value get `NAME=[redacted]`.
pub fn redact_env(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\r', '\n']);
        let end = &line[body.len()..];
        match body.split_once('=') {
            Some((name, value)) if secret_name(name) || secret_value(value) => {
                out.push_str(name);
                out.push('=');
                out.push_str(REDACTED);
                out.push_str(end);
            }
            _ => out.push_str(line),
        }
    }
    out
}

fn file_safe(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim_matches(['.', '_']).to_owned();
    if s.is_empty() { "player".into() } else { s }
}

pub fn size(bytes: u64) -> String {
    match bytes {
        b if b < 1024 => format!("{b} B"),
        b if b < 1 << 20 => format!("{:.1} KB", b as f64 / 1024.),
        b => format!("{:.1} MB", b as f64 / f64::from(1 << 20)),
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// `YYYYMMDD-HHMMSS` in UTC.
pub fn utc_stamp(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn readable(stamp: &str) -> String {
    match (
        stamp.get(..4),
        stamp.get(4..6),
        stamp.get(6..8),
        stamp.get(9..11),
        stamp.get(11..13),
        stamp.get(13..15),
    ) {
        (Some(y), Some(mo), Some(d), Some(h), Some(mi), Some(s)) if stamp.len() == 15 => {
            format!("{y}-{mo}-{d} {h}:{mi}:{s} UTC")
        }
        _ => stamp.to_owned(),
    }
}

pub fn os_description() -> String {
    if cfg!(windows) {
        return windows_version();
    }
    let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let field = |k: &str| {
        release
            .lines()
            .find_map(|l| l.strip_prefix(k)?.strip_prefix('='))
            .map(|v| v.trim().trim_matches('"').to_owned())
            .filter(|v| !v.is_empty())
    };
    let distro = field("PRETTY_NAME")
        .or_else(|| field("NAME"))
        .unwrap_or_else(|| std::env::consts::OS.to_owned());
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|k| k.trim().to_owned())
        .unwrap_or_default();
    let flatpak = std::path::Path::new("/.flatpak-info").exists();
    let mut s = format!("Linux · {distro}");
    if !kernel.is_empty() {
        s.push_str(&format!(" (kernel {kernel})"));
    }
    if flatpak {
        s.push_str(" · Flatpak");
    }
    s
}

fn windows_version() -> String {
    let ver = crate::compat::windows_tool("cmd", &["/C", "ver"]).unwrap_or_default();
    let number = ver
        .split_once("Version ")
        .map(|(_, v)| v.trim().trim_end_matches(']').trim().to_owned())
        .unwrap_or_default();
    let build: u32 = number.split('.').nth(2).and_then(|b| b.parse().ok()).unwrap_or(0);
    let display = crate::compat::windows_tool(
        "reg",
        &[
            "query",
            r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "/v",
            "DisplayVersion",
        ],
    )
    .and_then(|out| {
        out.lines()
            .find_map(|l| l.split("REG_SZ").nth(1).map(|v| v.trim().to_owned()))
    })
    .filter(|v| !v.is_empty());
    let name = if build >= 22_000 {
        "Windows 11"
    } else {
        "Windows 10"
    };
    match (number.is_empty(), display) {
        (true, _) => "Windows".into(),
        (false, Some(d)) => format!("{name} {d} ({number})"),
        (false, None) => format!("{name} ({number})"),
    }
}

fn hardware() -> String {
    let threads = std::thread::available_parallelism().map_or(0, |n| n.get());
    let cpu = if cfg!(windows) {
        std::env::var("PROCESSOR_IDENTIFIER").ok()
    } else {
        std::fs::read_to_string("/proc/cpuinfo").ok().and_then(|c| {
            c.lines().find_map(|l| {
                l.strip_prefix("model name")?
                    .split_once(':')
                    .map(|(_, v)| v.trim().to_owned())
            })
        })
    }
    .unwrap_or_else(|| "unknown CPU".into());
    let memory = std::fs::read_to_string("/proc/meminfo").ok().and_then(|m| {
        let kb: u64 = m
            .lines()
            .find_map(|l| l.strip_prefix("MemTotal:"))?
            .trim()
            .trim_end_matches("kB")
            .trim()
            .parse()
            .ok()?;
        Some(format!(", {:.0} GB memory", kb as f64 / 1024. / 1024.))
    });
    format!("{cpu} ({threads} threads){}", memory.unwrap_or_default())
}

#[cfg(test)]
mod tests;
