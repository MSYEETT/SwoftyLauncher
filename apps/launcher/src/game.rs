use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use swofty_core::crash::{LogScan, StartFailure, Symbols, fatal_name};
use swofty_core::install::Game;
use swofty_core::process::{GameProcess, game_processes};
use swofty_core::recovery::{self, Notice, Scope, SessionLock};
use swofty_core::session::{LaunchMode, SessionRecord};
use swofty_core::store::Store;
use swofty_core::{GAME_EXE, steam};

const KB: &str = include_str!("../../../re/db/functions.tsv");

#[derive(Debug, Clone, PartialEq)]
pub struct Install {
    pub dir: PathBuf,
    pub build_id: Option<u64>,
    pub exe_version: Option<String>,
    pub mode: String,
    pub via_steam: bool,
    pub manual: bool,
}

pub struct Probe {
    pub install: Option<Option<Install>>,
    pub processes: Vec<GameProcess>,
    pub session: Option<SessionRecord>,
    pub requests: Option<SystemTime>,
    pub merged: bool,
    pub recovered: Option<Notice>,
}

pub fn probe(store: &Store, rediscover: bool, seen: Option<SystemTime>) -> Probe {
    let processes = game_processes();
    let (requests, merged) = merge_requests(store, processes.is_empty(), seen);
    Probe {
        install: rediscover.then(|| discover(store)),
        processes,
        session: store.latest_session().map(|d| SessionRecord::read(&d)),
        requests,
        merged,
        recovered: Notice::read(&store.root),
    }
}

/// At launcher start: put back what an unfinished `swofty run` session left
/// in the game folder, unless a session is still live. A `swofty apply`
/// install stays.
pub fn recover_stale(store: &Store) -> Result<Vec<String>, String> {
    if !game_processes().is_empty() {
        return Ok(Vec::new());
    }
    let Some(install) = discover(store) else {
        return Ok(Vec::new());
    };
    let Some(lock) = SessionLock::try_acquire(&store.root).map_err(|e| e.to_string())? else {
        return Ok(Vec::new());
    };
    let g = Game::locate(&store.root, install.dir).map_err(|e| e.to_string())?;
    let r = recovery::recover(&lock, &store.root, &g, Scope::Sessions).map_err(|e| e.to_string())?;
    Ok(r.lines())
}

/// The Restore game folder button: what `swofty restore` does.
pub fn restore(store: &Store, dir: PathBuf) -> Result<String, String> {
    let g = Game::locate(&store.root, dir).map_err(|e| e.to_string())?;
    let done = recovery::restore(&store.root, &g).map_err(|e| e.to_string())?;
    if !done.left.is_empty() {
        let left: Vec<String> = done.left.iter().map(ToString::to_string).collect();
        return Err(format!("Not everything could be restored: {}", left.join("; ")));
    }
    let mut text = "The game folder matches the original game files.".to_owned();
    if !done.set_aside.is_empty() {
        text.push_str(&format!(
            " {} moved to {}.",
            match done.set_aside.len() {
                1 => "1 file was".to_owned(),
                n => format!("{n} files were"),
            },
            recovery::set_aside_dir(&store.root).display()
        ));
    }
    Ok(text)
}

fn merge_requests(store: &Store, idle: bool, seen: Option<SystemTime>) -> (Option<SystemTime>, bool) {
    let profile = store.active_profile();
    let Some(written) = std::fs::metadata(store.requests_path(&profile))
        .and_then(|m| m.modified())
        .ok()
    else {
        return (None, false);
    };
    let settled = idle && written.elapsed().is_ok_and(|age| age > Duration::from_secs(10));
    if seen == Some(written) && !settled {
        return (seen, false);
    }
    let merged = store.merge_requests(&profile, settled).ok().flatten();
    (Some(written), merged.is_some_and(|m| !m.changed.is_empty()))
}

pub fn discover(store: &Store) -> Option<Install> {
    let found = steam::find_game().filter(|g| g.exe().is_file());
    let manual = store.config().game_dir.filter(|d| d.join(GAME_EXE).is_file());
    let steam_root = found.as_ref().map(|g| g.steam_root.clone());
    let via_steam = steam_root.as_deref().is_some_and(|root| {
        steam::launch_options(root)
            .iter()
            .any(|(_, o)| steam::wrapper_installed(o))
    });
    let mode = if cfg!(windows) {
        "Windows".to_owned()
    } else {
        match steam_root.as_deref().and_then(steam::compat_tool) {
            Some(tool) => format!("Proton · {tool}"),
            None => "Proton".to_owned(),
        }
    };
    match (manual, found) {
        (Some(dir), found) => Some(Install {
            build_id: found.filter(|g| g.dir == dir).and_then(|g| g.build_id),
            exe_version: swofty_core::exe_version::read(&dir.join(GAME_EXE)),
            dir,
            mode,
            via_steam,
            manual: true,
        }),
        (None, Some(g)) => Some(Install {
            exe_version: swofty_core::exe_version::read(&g.dir.join(GAME_EXE)),
            dir: g.dir,
            build_id: g.build_id,
            mode,
            via_steam,
            manual: false,
        }),
        (None, None) => None,
    }
}

pub struct Launch {
    child: Child,
    pub since: Instant,
    pub log: PathBuf,
    pub saw_game: bool,
}

impl Launch {
    pub fn start(store: &Store, profile: &str, install: &Install, mode: LaunchMode) -> Result<Self, String> {
        let cli = cli_path(store).ok_or("the swofty command was not found next to the launcher")?;
        let log = store.root.join("state/launcher-run.log");
        let out = std::fs::create_dir_all(store.root.join("state"))
            .and_then(|_| File::create(&log))
            .map_err(|e| format!("{}: {e}", log.display()))?;
        let err = out.try_clone().map_err(|e| e.to_string())?;
        let mut c = Command::new(cli);
        c.arg("--data-dir")
            .arg(&store.root)
            .arg("--profile")
            .arg(profile)
            .arg("run")
            .arg("--apply");
        if mode.is_vr() {
            c.arg("--vr");
        }
        if install.manual {
            c.arg("--game").arg(&install.dir);
        }
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut c, 0x0800_0000);
        let child = c
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            child,
            since: Instant::now(),
            log,
            saw_game: false,
        })
    }

    pub fn finished(&mut self) -> Option<bool> {
        self.child.try_wait().ok().flatten().map(|s| s.success())
    }

    pub fn failure(&self) -> String {
        std::fs::read_to_string(&self.log)
            .ok()
            .and_then(|s| s.lines().rev().find(|l| !l.trim().is_empty()).map(str::to_owned))
            .map(|l| l.trim_start_matches("swofty: ").to_owned())
            .unwrap_or_else(|| "the launch exited before the game started".into())
    }
}

fn cli_path(store: &Store) -> Option<PathBuf> {
    let name = if cfg!(windows) { "swofty.exe" } else { "swofty" };
    let beside = std::env::current_exe()
        .ok()
        .and_then(|e| Some(e.parent()?.join(name)));
    let path = std::env::var_os("PATH").unwrap_or_default();
    beside
        .into_iter()
        .chain([store.bin_dir().join(name)])
        .chain(std::env::split_paths(&path).map(|d| d.join(name)))
        .find(|p| p.is_file())
}

pub fn crash_summary(session: &SessionRecord, game: Option<&Path>) -> Result<String, StartFailure> {
    let mut symbols = Symbols::parse_kb(KB);
    if let Some(dir) = game {
        symbols.load_pdata(&dir.join(GAME_EXE)).ok();
    }
    let mut scan = LogScan::default();
    if let Ok(f) = File::open(session.wine_log()) {
        for line in BufReader::new(f).split(b'\n').map_while(Result::ok) {
            scan.feed(&String::from_utf8_lossy(&line));
        }
    }
    if let Some(failure) = scan.start_failure {
        return Err(failure);
    }
    let code = session.exit_code.unwrap_or_default();
    Ok(match scan.last_fatal {
        Some((c, addr)) => format!(
            "exit code {code}, last {} at {}",
            fatal_name(c).unwrap_or("exception"),
            scan.describe(addr, &symbols)
        ),
        None => format!("exit code {code}"),
    })
}

pub fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn ago(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    match s {
        0..60 => "just now".into(),
        60..3600 => format!("{} min ago", s / 60),
        3600..86_400 => format!("{} h ago", s / 3600),
        _ => format!("{} d ago", s / 86_400),
    }
}

pub fn duration(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs} s"),
        60..3600 => format!("{} min", secs / 60),
        _ => format!("{} h {} min", secs / 3600, secs % 3600 / 60),
    }
}
