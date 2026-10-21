//! Loader files an antivirus took away. Windows Defender and other scanners
//! delete, quarantine or block version.dll and the core DLLs, often a moment
//! after they are written, and F1 25 then runs with no mods at all. The
//! session checks the placed DLLs before the game starts and watches the
//! game folder's loader.log after it starts; what it finds is recorded here
//! for the launcher to show and for Send Logs.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::baseline::sha256_file;
use crate::builtin::{CORE_DLL, LOADER_ENV};
use crate::install::PROXY;
use crate::placed::Placed;
use crate::session::LogMark;
use crate::{Result, io_err, read_json, write_json};

pub const ERROR_VIRUS_INFECTED: i32 = 225;
pub const ERROR_VIRUS_DELETED: i32 = 226;
/// The loader's first line once version.dll is running in the game.
pub const ATTACHED: &str = "(version.dll proxy) attached";
/// The offline play stub's line when it has the game image and resumes it.
const PATCHING: &str = "game image at ";
/// A second look at the placed DLLs this long after they were written: a
/// scanner often removes a file only once it has been closed.
pub const RECHECK_AFTER: Duration = Duration::from_secs(2);
/// How long after the stub's patching line the loader may take to attach.
pub const ATTACH_WINDOW: Duration = Duration::from_secs(45);
/// The window when no stub line shows up (a launch without offline play).
const ATTACH_WINDOW_NO_STUB: Duration = Duration::from_secs(90);
/// A game that exits sooner than this without the loader failed to start,
/// which the crash report covers.
const SHORTEST_RUN: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_secs(1);

pub fn is_virus_code(code: i32) -> bool {
    matches!(code, ERROR_VIRUS_INFECTED | ERROR_VIRUS_DELETED)
}

/// Windows refuses to copy or open a file it detected as a threat with these
/// two codes; elsewhere the numbers mean nothing.
pub fn is_virus_error(e: &std::io::Error) -> bool {
    cfg!(windows) && e.raw_os_error().is_some_and(is_virus_code)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    BeforeLaunch,
    InGame,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub at: f64,
    pub stage: Stage,
    /// Paths relative to the game folder, or to the launcher folder for a
    /// loader file missing from the install itself.
    pub files: Vec<String>,
    /// In game: version.dll was gone from the game folder when checked.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub removed: bool,
    pub game: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launcher: Option<PathBuf>,
}

impl Report {
    pub fn new(stage: Stage, files: Vec<String>, game: &Path) -> Self {
        Self {
            at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0.0, |d| d.as_secs_f64()),
            stage,
            files,
            removed: false,
            game: game.to_path_buf(),
            launcher: launcher_dir(),
        }
    }

    pub fn path(data: &Path) -> PathBuf {
        data.join("state/mods-blocked.json")
    }

    pub fn write(&self, data: &Path) -> Result<()> {
        write_json(&Self::path(data), self)
    }

    pub fn read(data: &Path) -> Option<Self> {
        read_json(&Self::path(data)).ok()
    }

    pub fn clear(data: &Path) -> Result<()> {
        let path = Self::path(data);
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io_err(&path)(e)),
            _ => Ok(()),
        }
    }

    pub fn title(&self) -> &'static str {
        match self.stage {
            Stage::BeforeLaunch => "Your antivirus removed mod files",
            Stage::InGame => "Mods did not load",
        }
    }

    pub fn text(&self) -> String {
        let names = names(&self.files);
        let one = self.files.len() <= 1;
        let it = if one { "it" } else { "them" };
        let what = match self.stage {
            Stage::BeforeLaunch => format!(
                "An antivirus most likely deleted or quarantined {names}, so F1 25 starts without mods."
            ),
            Stage::InGame if self.removed => format!(
                "F1 25 is running without mods: {names} {} removed from the game folder. An antivirus most likely deleted or quarantined {it}.",
                if one { "was" } else { "were" }
            ),
            Stage::InGame => {
                format!("F1 25 is running without mods. An antivirus most likely blocked {names}.")
            }
        };
        format!("{what} {}", self.fix(true))
    }

    fn fix(&self, running: bool) -> String {
        let it = if self.files.len() <= 1 { "it" } else { "them" };
        let folders = match &self.launcher {
            Some(l) => format!(
                "the game folder ({}) and the launcher folder ({})",
                self.game.display(),
                l.display()
            ),
            None => format!(
                "the game folder ({}) and the launcher folder",
                self.game.display()
            ),
        };
        format!(
            "To fix it, {}restore {it} in Windows Security > Virus & threat protection > \
             Protection history, add exclusions for {folders}, then launch again.",
            if running { "close F1 25, " } else { "" }
        )
    }
}

fn names(files: &[String]) -> String {
    let mut names: Vec<&str> = Vec::new();
    for f in files {
        let name = f.rsplit(['/', '\\']).next().unwrap_or(f);
        if !names.iter().any(|n| n.eq_ignore_ascii_case(name)) {
            names.push(name);
        }
    }
    match names.as_slice() {
        [] => PROXY.to_owned(),
        [a] => (*a).to_owned(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] if rest.len() < 4 => format!("{} and {last}", rest.join(", ")),
        _ => format!("{} and {} more", names[..3].join(", "), names.len() - 3),
    }
}

/// The folder the launcher is installed in, for the exclusion.
fn launcher_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.to_path_buf())
}

/// A failed copy into the game folder; one Windows refused as a threat says
/// so and what to do about it.
pub(crate) fn copy_error(rel: &str, path: &Path, e: std::io::Error) -> crate::Error {
    if !is_virus_error(&e) {
        return io_err(path)(e);
    }
    let game = path
        .ancestors()
        .nth(rel.split(['/', '\\']).count())
        .unwrap_or(path);
    crate::Error::Other(format!(
        "Windows blocked {} as a threat ({e}). {}",
        names(&[rel.to_owned()]),
        Report::new(Stage::BeforeLaunch, vec![rel.to_owned()], game).fix(false)
    ))
}

/// The DLLs the launcher wrote into the game folder, with the bytes it wrote.
pub fn placed_dlls(data: &Path) -> Result<Vec<(String, String)>> {
    Ok(Placed::load(data)?
        .files
        .into_values()
        .filter(|f| !f.replaced && f.path.to_ascii_lowercase().ends_with(".dll"))
        .map(|f| (f.path, f.sha256))
        .collect())
}

/// The placed DLLs that are gone from the game folder, cannot be read, or no
/// longer hold the bytes written there.
pub fn missing(game: &Path, dlls: &[(String, String)]) -> Vec<String> {
    dlls.iter()
        .filter(|(rel, sha)| !sha256_file(&game.join(rel)).is_ok_and(|now| now == *sha))
        .map(|(rel, _)| rel.clone())
        .collect()
}

/// A loader folder of the install that lost version.dll or the core DLL: the
/// build then finds no loader and the game starts without one.
pub fn damaged_install() -> Vec<String> {
    if crate::builtin::loader_dir().is_some() || std::env::var_os(LOADER_ENV).is_some() {
        return Vec::new();
    }
    let Some(bin) = launcher_dir() else {
        return Vec::new();
    };
    let dirs = [bin.parent().map(|p| p.join("loader")), Some(bin.join("loader"))];
    let Some(dir) = dirs.into_iter().flatten().find(|d| d.is_dir()) else {
        return Vec::new();
    };
    damaged_loader(&dir)
}

fn damaged_loader(dir: &Path) -> Vec<String> {
    [PROXY, CORE_DLL]
        .into_iter()
        .filter(|rel| !dir.join(rel).is_file())
        .map(|rel| format!("loader/{rel}"))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attach {
    Waiting,
    Attached,
    Missed,
}

/// Whether a loader.log shows the loader running: `Some(false)` for a log
/// with lines but no attach line (only the offline play stub's), `None` for
/// an empty one.
pub fn attached(text: &str) -> Option<bool> {
    if text.contains(ATTACHED) {
        return Some(true);
    }
    (!text.trim().is_empty()).then_some(false)
}

/// Follows the game folder's loader.log after the game starts. The stub
/// writes its lines first; the loader moves them to loader.prev.log when it
/// attaches and starts a fresh log with its attach line, so a log still
/// equal to the one from before the launch says nothing.
pub struct AttachWatch {
    log: PathBuf,
    before: Option<LogMark>,
    started: Instant,
    patched: Option<Instant>,
    next: Instant,
    state: Attach,
}

impl AttachWatch {
    pub fn new(log: PathBuf, before: Option<LogMark>, now: Instant) -> Self {
        Self {
            log,
            before,
            started: now,
            patched: None,
            next: now,
            state: Attach::Waiting,
        }
    }

    pub fn state(&self) -> Attach {
        self.state
    }

    /// The state after a look at the log; a verdict, once reached, stays.
    pub fn poll(&mut self, now: Instant) -> Attach {
        if self.state != Attach::Waiting || now < self.next {
            return self.state;
        }
        self.next = now + POLL;
        if self.read(now) == Some(true) {
            self.state = Attach::Attached;
            return self.state;
        }
        let (from, window) = match self.patched {
            Some(at) => (at, ATTACH_WINDOW),
            None => (self.started, ATTACH_WINDOW_NO_STUB),
        };
        if now.duration_since(from) > window {
            self.state = Attach::Missed;
        }
        self.state
    }

    /// The verdict when the game has exited: a game that ran a while and
    /// never got the loader missed it.
    pub fn ended(&mut self, now: Instant) -> Attach {
        if self.state == Attach::Waiting {
            self.state = if self.read(now) == Some(true) {
                Attach::Attached
            } else if now.duration_since(self.started) >= SHORTEST_RUN {
                Attach::Missed
            } else {
                Attach::Waiting
            };
        }
        self.state
    }

    fn read(&mut self, now: Instant) -> Option<bool> {
        let text = std::fs::read(&self.log).ok()?;
        let text = String::from_utf8_lossy(&text);
        if self.patched.is_none() && text.contains(PATCHING) {
            self.patched = Some(now);
        }
        if !LogMark::replaced(&self.log, self.before.as_ref()) {
            return None;
        }
        attached(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STUB: &str = "01:12:12.575 [stub] [offline] session launcher invoked, args: <none>\n\
                        01:12:13.901 [stub] [offline] game image at 0x140000000, patching\n";
    const LOADED: &str =
        "2026-10-20 21:16:05.789 [ 2548] swofty loader (version.dll proxy) attached, pid 14088\n";

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-quarantine-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn knows_the_two_windows_virus_codes() {
        assert!(is_virus_code(225));
        assert!(is_virus_code(226));
        assert!(!is_virus_code(5));
        assert!(!is_virus_code(32));
    }

    #[test]
    fn finds_a_placed_dll_that_was_deleted_or_changed() {
        let game = temp("missing");
        std::fs::create_dir_all(game.join("swofty/addons/core")).unwrap();
        std::fs::write(game.join("version.dll"), b"proxy").unwrap();
        std::fs::write(game.join(CORE_DLL), b"core").unwrap();
        let dlls = vec![
            (
                "version.dll".to_owned(),
                sha256_file(&game.join("version.dll")).unwrap(),
            ),
            (CORE_DLL.to_owned(), sha256_file(&game.join(CORE_DLL)).unwrap()),
        ];
        assert!(missing(&game, &dlls).is_empty());
        std::fs::remove_file(game.join("version.dll")).unwrap();
        std::fs::write(game.join(CORE_DLL), b"emptied by a scanner").unwrap();
        assert_eq!(missing(&game, &dlls), ["version.dll", CORE_DLL]);
        std::fs::remove_dir_all(&game).ok();
    }

    #[test]
    fn lists_only_the_dlls_it_placed() {
        let data = temp("placed");
        let mut placed = Placed::load(&data).unwrap();
        placed.insert("version.dll", "a".into(), false);
        placed.insert(CORE_DLL, "b".into(), false);
        placed.insert("swofty/addons/core/symbols.tsv", "c".into(), false);
        placed.insert("2025_asset_groups/ui_package/databases.erp", "d".into(), true);
        placed.save(&data).unwrap();
        let mut got = placed_dlls(&data).unwrap();
        got.sort();
        assert_eq!(
            got,
            [
                (CORE_DLL.to_owned(), "b".to_owned()),
                ("version.dll".to_owned(), "a".to_owned())
            ]
        );
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn names_what_an_install_loader_folder_lost() {
        let dir = temp("install");
        std::fs::create_dir_all(dir.join("swofty/addons/core")).unwrap();
        std::fs::write(dir.join(CORE_DLL), b"core").unwrap();
        assert_eq!(damaged_loader(&dir), ["loader/version.dll"]);
        std::fs::write(dir.join(PROXY), b"proxy").unwrap();
        assert!(damaged_loader(&dir).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tells_a_stub_only_log_from_a_loaded_one() {
        assert_eq!(attached(STUB), Some(false));
        assert_eq!(
            attached(&format!(
                "{LOADED}2026-10-20 21:16:05.789 [ 2548] host: F1_25.exe\n"
            )),
            Some(true)
        );
        assert_eq!(attached(""), None);
    }

    #[test]
    fn misses_the_loader_a_while_after_the_stub_patched_the_game() {
        let dir = temp("watch-miss");
        let log = dir.join("loader.log");
        let t0 = Instant::now();
        let mut w = AttachWatch::new(log.clone(), None, t0);
        assert_eq!(w.poll(t0), Attach::Waiting);
        std::fs::write(&log, STUB).unwrap();
        assert_eq!(w.poll(t0 + Duration::from_secs(2)), Attach::Waiting);
        assert_eq!(w.poll(t0 + Duration::from_secs(20)), Attach::Waiting);
        let late = t0 + Duration::from_secs(3) + ATTACH_WINDOW;
        assert_eq!(w.poll(late), Attach::Missed);
        std::fs::write(&log, LOADED).unwrap();
        assert_eq!(w.poll(late + Duration::from_secs(5)), Attach::Missed);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sees_the_loader_attach_and_ignores_a_stale_log() {
        let dir = temp("watch-ok");
        let log = dir.join("loader.log");
        std::fs::write(&log, LOADED).unwrap();
        let t0 = Instant::now();
        let mut w = AttachWatch::new(log.clone(), LogMark::read(&log), t0);
        assert_eq!(w.poll(t0), Attach::Waiting);
        std::fs::remove_file(&log).unwrap();
        std::fs::write(&log, STUB).unwrap();
        assert_eq!(w.poll(t0 + Duration::from_secs(2)), Attach::Waiting);
        std::fs::write(
            &log,
            "2026-10-21 01:12:14.001 [  420] swofty loader (version.dll proxy) attached, pid 1976\n",
        )
        .unwrap();
        assert_eq!(w.poll(t0 + Duration::from_secs(4)), Attach::Attached);
        assert_eq!(w.ended(t0 + Duration::from_secs(600)), Attach::Attached);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_session_that_ends_without_the_loader_missed_it() {
        let dir = temp("watch-end");
        let log = dir.join("loader.log");
        std::fs::write(&log, STUB).unwrap();
        let t0 = Instant::now();
        let mut quick = AttachWatch::new(log.clone(), None, t0);
        assert_eq!(quick.ended(t0 + Duration::from_secs(3)), Attach::Waiting);
        let mut long = AttachWatch::new(log, None, t0);
        assert_eq!(long.ended(t0 + Duration::from_secs(13 * 60)), Attach::Missed);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn says_what_happened_and_how_to_fix_it() {
        let mut r = Report {
            at: 1.0,
            stage: Stage::BeforeLaunch,
            files: vec!["version.dll".into(), CORE_DLL.into()],
            removed: false,
            game: PathBuf::from(r"C:\Steam\steamapps\common\F1 25"),
            launcher: Some(PathBuf::from(r"C:\Programs\SwoftyLauncher")),
        };
        assert_eq!(
            r.text(),
            "An antivirus most likely deleted or quarantined version.dll and swofty_core.dll, so F1 25 \
             starts without mods. To fix it, close F1 25, restore them in Windows Security > Virus & \
             threat protection > Protection history, add exclusions for the game folder \
             (C:\\Steam\\steamapps\\common\\F1 25) and the launcher folder (C:\\Programs\\SwoftyLauncher), \
             then launch again."
        );
        r.stage = Stage::InGame;
        r.files = vec!["version.dll".into()];
        r.removed = true;
        assert!(r.text().starts_with(
            "F1 25 is running without mods: version.dll was removed from the game folder. An antivirus \
             most likely deleted or quarantined it. To fix it, close F1 25, restore it in"
        ));
        r.removed = false;
        assert!(r.text().starts_with(
            "F1 25 is running without mods. An antivirus most likely blocked version.dll. To fix it"
        ));
        assert_eq!(r.title(), "Mods did not load");
    }

    #[test]
    fn a_report_survives_a_round_trip_and_clears() {
        let data = temp("report");
        let r = Report::new(
            Stage::InGame,
            vec!["version.dll".into()],
            Path::new("/games/F1 25"),
        );
        r.write(&data).unwrap();
        let back = Report::read(&data).unwrap();
        assert!((back.at - r.at).abs() < 1e-3);
        assert_eq!(Report { at: r.at, ..back }, r);
        Report::clear(&data).unwrap();
        assert_eq!(Report::read(&data), None);
        Report::clear(&data).unwrap();
        std::fs::remove_dir_all(&data).ok();
    }
}
