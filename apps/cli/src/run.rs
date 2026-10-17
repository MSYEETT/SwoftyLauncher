use std::io::{IsTerminal, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::Args;
use swofty_core::crash::{Event, LogScan, Symbols, fatal_name};
use swofty_core::process::{descends_from, game_processes, game_running, stop_game, stop_process};
use swofty_core::session::{Config, DEFAULT_WINEDEBUG, LaunchRequest, parse_cpu_list, parse_steam_command};
use swofty_core::store::Store;
use swofty_core::{GAME_EXE, STEAM_APP_ID, steam};

use crate::time::{now_secs, stamp};

const KB: &str = include_str!("../../../re/db/functions.tsv");

#[derive(Args)]
pub struct RunArgs {
    #[arg(long, default_value = DEFAULT_WINEDEBUG, help = "WINEDEBUG channels for the game process")]
    winedebug: String,
    #[arg(long, help = "Close a running F1 25 first")]
    restart: bool,
    #[arg(long, help = "Launch through Proton directly instead of asking Steam")]
    direct: bool,
    #[arg(
        long,
        help = "Build and install the profile (database, maps) before the game starts, and roll it back when it exits"
    )]
    apply: bool,
    #[arg(long, help = "Game folder (default: discovered from Steam)")]
    game: Option<PathBuf>,
    #[arg(
        long,
        env = "SWOFTY_EXE_COPY",
        help = "A copy of F1_25.exe whose .pdata gives function bounds"
    )]
    exe: Option<PathBuf>,
    #[arg(
        last = true,
        help = "The Steam command (%command%); present when Steam runs the wrapper"
    )]
    steam_command: Vec<String>,
}

pub fn run(store: &Store, profile: &str, args: RunArgs) -> Result<ExitCode> {
    let wrapped = !args.steam_command.is_empty();
    let explicit = args
        .game
        .clone()
        .or_else(|| wrapped.then(|| std::env::current_dir().ok()).flatten());
    let g = crate::build::game(store, explicit)?;
    for d in swofty_core::install::Installer::new(&store.root, &g).recover()? {
        say("info", &format!("rollback: {d}"));
    }
    match swofty_core::offline::restore_pending(&store.root) {
        Ok(true) => say("info", "rollback: offline play staging"),
        Ok(false) => {}
        Err(e) => say("info", &format!("offline play rollback failed: {e:#}")),
    }
    // Modded sessions run without EA AntiCheat: the terms are accepted once,
    // and declining means the game is not launched.
    crate::offline::gate(store, !wrapped && std::io::stdin().is_terminal())?;
    if wrapped {
        return wrapper(store, profile, &args);
    }
    let game = g.dir.clone();
    if let Some(running) = game_processes().first() {
        if !args.restart {
            bail!(
                "F1 25 is already running (pid {}); close it first, or pass --restart to close it",
                running.pid
            );
        }
        stop_game().ok();
        while game_running() {
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    let id = format!("{:x}{:x}", std::process::id(), (now_secs() * 1e6) as u64);
    let request = LaunchRequest {
        id: id.clone(),
        winedebug: Some(args.winedebug.clone()),
        profile: Some(profile.into()),
        apply: args.apply,
    };
    write_json(&store.root.join("state/launch.json"), &request)?;
    let mut wrapper = launch(store, &game, args.direct)?;
    let started = Instant::now();
    let dir = loop {
        if let Some(d) = store
            .latest_session()
            .filter(|d| std::fs::read_to_string(d.join("request_id")).is_ok_and(|s| s.trim() == id))
        {
            break d;
        }
        if let Some(status) = wrapper.exited() {
            return Err(wrapper.failure(status, "before the game started"));
        }
        if started.elapsed() > wrapper.timeout {
            match &wrapper.child {
                Some(_) => bail!(
                    "the wrapper did not start the game within {} min and is still running; its output is in {}",
                    wrapper.timeout.as_secs() / 60,
                    wrapper.log.display()
                ),
                None => bail!(
                    "the wrapper did not start within {} min; check the Steam launch option (swofty doctor)",
                    wrapper.timeout.as_secs() / 60
                ),
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    say("info", &format!("wrapper started, logging to {}", dir.display()));
    let mut follow = Follow::new(
        &dir,
        Some(game.join("swofty/loader.log")),
        symbols(&game, args.exe.as_deref()),
    );
    install_ctrl_c();
    while !dir.join("exit_code").exists() {
        if STOP.load(std::sync::atomic::Ordering::Relaxed) {
            say("info", "stopped following (the game keeps running)");
            break;
        }
        follow.poll();
        if let Some(status) = wrapper.exited()
            && !dir.join("exit_code").exists()
        {
            return Err(wrapper.failure(status, "while the game was running"));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    std::thread::sleep(Duration::from_millis(1500));
    follow.poll();
    follow.summary();
    Ok(ExitCode::SUCCESS)
}

pub fn logs(store: &Store, dir: Option<PathBuf>, exe: Option<PathBuf>) -> Result<ExitCode> {
    let dir = match dir {
        Some(d) => d,
        None => store.latest_session().context("no previous session")?,
    };
    let game = steam::find_game().map(|g| g.dir).unwrap_or_default();
    let mut follow = Follow::new(&dir, None, symbols(&game, exe.as_deref()));
    follow.poll();
    follow.summary();
    Ok(ExitCode::SUCCESS)
}

fn wrapper(store: &Store, profile: &str, args: &RunArgs) -> Result<ExitCode> {
    if let Some(running) = game_processes().first() {
        bail!(
            "F1 25 is already running (pid {}); close it before launching it again",
            running.pid
        );
    }
    let config: Config = std::fs::read(store.config_path())
        .ok()
        .and_then(|d| serde_json::from_slice(&d).ok())
        .unwrap_or_default();
    let request: LaunchRequest = std::fs::read(store.root.join("state/launch.json"))
        .ok()
        .and_then(|d| serde_json::from_slice(&d).ok())
        .unwrap_or_default();
    let profile = request.profile.clone().unwrap_or_else(|| profile.to_owned());
    if let Some(cpus) = config.launch.cpu_affinity.as_deref().and_then(parse_cpu_list) {
        set_affinity(&cpus);
    }
    let game = args.game.clone().unwrap_or(std::env::current_dir()?);
    let installed = if request.apply || args.apply {
        let g = crate::build::game(store, Some(game.clone()))?;
        say("info", &format!("installing profile {profile}"));
        crate::build::install(store, &profile, &g)?;
        Some(g)
    } else {
        None
    };
    become_subreaper();
    let offline = (|| -> Result<()> {
        let artifacts = crate::offline::ensure_extension(store)?;
        let probe = swofty_core::offline::probe(&game, &artifacts)?;
        if !probe.supported {
            bail!(
                "this F1 25 build is not supported by the offline play extension yet ({}); \
                 the modded session was refused",
                probe.report
            );
        }
        say("info", &format!("game build supported: {}", probe.report));
        let journal = swofty_core::offline::journal_path(&store.root);
        swofty_core::offline::stage(&game, &artifacts, &journal)?;
        say(
            "info",
            "staged offline play: EA AntiCheat is not started this session; multiplayer is unavailable",
        );
        Ok(())
    })();
    if let Err(e) = offline {
        rollback_profile(store, installed.as_ref());
        return Err(e);
    }
    let result = run_loop(
        store,
        args,
        &config,
        &request,
        &profile,
        &game,
        installed.as_ref(),
    );
    match swofty_core::offline::restore_pending(&store.root) {
        Ok(true) => say("info", "offline play: the game folder is back to vanilla"),
        Ok(false) => {}
        Err(e) => say(
            "info",
            &format!("offline play restore failed, run swofty restore: {e:#}"),
        ),
    }
    rollback_profile(store, installed.as_ref());
    result
}

/// Put the profile's game files back; safe to call when nothing is staged.
fn rollback_profile(store: &Store, installed: Option<&swofty_core::install::Game>) {
    let Some(g) = installed else {
        return;
    };
    match swofty_core::install::Installer::new(&store.root, g).apply(&swofty_core::install::Plan::vanilla()) {
        Ok(()) => say("info", "rolled the profile's game files back"),
        Err(e) => say("info", &format!("rollback failed, run swofty restore: {e:#}")),
    }
}

fn run_loop(
    store: &Store,
    args: &RunArgs,
    config: &Config,
    request: &LaunchRequest,
    profile: &str,
    game: &Path,
    installed: Option<&swofty_core::install::Game>,
) -> Result<ExitCode> {
    let profile = profile.to_owned();
    let game = game.to_path_buf();
    loop {
        rebuild_if_needed(store, &profile, &game);
        let code = play(store, args, config, request, &game)?;
        let relaunch = match store.merge_requests(&profile, true) {
            Ok(Some(m)) => {
                for c in &m.changed {
                    say("info", &format!("in game: {c}"));
                }
                for c in &m.skipped {
                    say("info", &format!("in game, skipped: {c}"));
                }
                m.relaunch
            }
            Ok(None) => false,
            Err(e) => {
                say("info", &format!("in-game changes not merged: {e}"));
                false
            }
        };
        if !relaunch {
            return Ok(ExitCode::from(code.clamp(0, 255) as u8));
        }
        say("info", "Apply & restart: starting F1 25 again");
        if let Some(g) = installed {
            say("info", &format!("reinstalling profile {profile}"));
            crate::build::install(store, &profile, g)?;
        }
    }
}

fn rebuild_if_needed(store: &Store, profile: &str, game: &Path) {
    let pending = match (store.profile(profile), store.mods()) {
        (Ok(p), Ok(mods)) => p.rebuild_pending(&mods),
        _ => return,
    };
    if pending.is_empty() {
        return;
    }
    say(
        "info",
        &format!("rebuilding {profile} for {}", pending.join(", ")),
    );
    if let Err(e) = crate::build::build(store, profile, Some(game.to_path_buf())) {
        say("info", &format!("rebuild failed, launching anyway: {e:#}"));
    }
}

fn play(store: &Store, args: &RunArgs, config: &Config, request: &LaunchRequest, game: &Path) -> Result<i32> {
    let dir = store.sessions_dir().join(stamp());
    std::fs::create_dir_all(&dir)?;
    let latest = store.sessions_dir().join("latest");
    std::fs::remove_file(&latest).ok();
    link(&dir, &latest);
    std::fs::write(store.sessions_dir().join("latest.txt"), dir.display().to_string())?;
    std::fs::write(dir.join("request_id"), format!("{}\n", request.id))?;
    std::fs::write(dir.join("steam_command.txt"), shell_join(&args.steam_command))?;
    let mut env: Vec<String> = std::env::vars().map(|(k, v)| format!("{k}={v}")).collect();
    env.sort();
    std::fs::write(dir.join("env.txt"), env.join("\n") + "\n")?;
    let cmd = parse_steam_command(&args.steam_command);
    let proton = cmd.proton.map(PathBuf::from).or_else(|| {
        let tools = std::env::var("STEAM_COMPAT_TOOL_PATHS").ok()?;
        Some(PathBuf::from(tools.split(':').next()?).join("proton"))
    });
    let winedebug = request
        .winedebug
        .clone()
        .unwrap_or(config.launch.winedebug.clone());
    let spawn = |exe: &str, log: &str| -> Result<std::process::Child> {
        let out = std::fs::File::create(dir.join(log))?;
        let mut c = match &proton {
            Some(p) => {
                let mut c = Command::new(p);
                c.arg("run").arg(game.join(exe));
                c
            }
            None => Command::new(game.join(exe)),
        };
        c.current_dir(game)
            .envs(&config.launch.env)
            .env("PROTON_LOG", "1")
            .env("PROTON_LOG_DIR", &dir)
            .env("WINEDEBUG", &winedebug)
            .env("SteamAppId", STEAM_APP_ID.to_string())
            .env("SteamGameId", STEAM_APP_ID.to_string())
            .stdout(out.try_clone()?)
            .stderr(out);
        Ok(c.spawn()?)
    };
    std::fs::write(dir.join("game_start"), format!("{}\n", now_secs()))?;
    let status = spawn(GAME_EXE, "game_stdout.log")?.wait()?;
    let code = status.code().unwrap_or(-1);
    std::fs::write(dir.join("exit_code"), format!("{code}\n"))?;
    std::fs::write(dir.join("game_end"), format!("{}\n", now_secs()))?;
    std::fs::copy(game.join("swofty/loader.log"), dir.join("loader.log")).ok();
    for g in game_processes()
        .into_iter()
        .filter(|g| descends_from(g.pid, std::process::id()))
    {
        stop_process(g.pid).ok();
    }
    Ok(code)
}

const DIRECT_START_TIMEOUT: Duration = Duration::from_secs(600);
const STEAM_START_TIMEOUT: Duration = Duration::from_secs(120);
const WRAPPER_LOG: &str = "state/wrapper-run.log";
const TAIL_LINES: usize = 12;

struct Wrapper {
    child: Option<Child>,
    log: PathBuf,
    timeout: Duration,
}

impl Wrapper {
    fn exited(&mut self) -> Option<ExitStatus> {
        self.child.as_mut()?.try_wait().ok().flatten()
    }

    fn failure(&self, status: ExitStatus, when: &str) -> anyhow::Error {
        let mut lines = tail(&self.log, TAIL_LINES);
        let reason = lines
            .pop()
            .map(|l| l.strip_prefix("swofty: ").map(str::to_owned).unwrap_or(l));
        for l in &lines {
            say("wrapper", l);
        }
        match reason {
            Some(r) => anyhow::anyhow!("the wrapper exited ({status}) {when}: {r}"),
            None => anyhow::anyhow!(
                "the wrapper exited ({status}) {when} without output; see {}",
                self.log.display()
            ),
        }
    }
}

fn tail(path: &Path, n: usize) -> Vec<String> {
    let text = std::fs::read(path)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();
    let lines: Vec<String> = text
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    lines[lines.len().saturating_sub(n)..].to_vec()
}

fn spawn_logged(c: &mut Command, log: &Path) -> Result<Child> {
    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let out = std::fs::File::create(log).with_context(|| log.display().to_string())?;
    Ok(c.stdin(Stdio::null())
        .stdout(out.try_clone()?)
        .stderr(out)
        .spawn()?)
}

fn launch(store: &Store, game: &Path, direct: bool) -> Result<Wrapper> {
    let install = steam::find_game();
    let wrapped = install
        .as_ref()
        .map(|g| {
            steam::launch_options(&g.steam_root)
                .iter()
                .any(|(_, o)| steam::wrapper_installed(o))
        })
        .unwrap_or(false);
    if cfg!(windows) || direct || !wrapped {
        let me = std::env::current_exe()?;
        let mut c = Command::new(&me);
        c.arg("--data-dir")
            .arg(&store.root)
            .arg("run")
            .arg("--game")
            .arg(game);
        if let Some(g) = install.as_ref().filter(|_| !cfg!(windows)) {
            let tool = steam::compat_tool(&g.steam_root).unwrap_or_default();
            let proton = [
                g.steam_root.join("compatibilitytools.d").join(&tool),
                g.library.join("steamapps/common").join(&tool),
            ]
            .into_iter()
            .find(|p| p.join("proton").is_file())
            .context("could not find the Proton build Steam uses for F1 25")?;
            let runtime =
                [&g.library, &g.steam_root].map(|l| l.join("steamapps/common/SteamLinuxRuntime_sniper"));
            let runtime = runtime.iter().find(|p| p.is_dir()).unwrap_or(&runtime[0]);
            c.env(
                "STEAM_COMPAT_TOOL_PATHS",
                std::env::join_paths([&proton, runtime])?,
            )
            .env("STEAM_COMPAT_DATA_PATH", g.compatdata.clone().unwrap_or_default())
            .env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &g.steam_root)
            .env("STEAM_COMPAT_INSTALL_PATH", game)
            .env("STEAM_COMPAT_LIBRARY_PATHS", g.library.join("steamapps"))
            .env("STEAM_COMPAT_APP_ID", STEAM_APP_ID.to_string())
            .env("SteamAppId", STEAM_APP_ID.to_string())
            .env("SteamGameId", STEAM_APP_ID.to_string());
            c.arg("--")
                .arg(proton.join("proton"))
                .arg("waitforexitandrun")
                .arg(game.join(GAME_EXE));
        } else {
            c.arg("--").arg(game.join(GAME_EXE));
        }
        if which("steam-run") {
            let mut fhs = Command::new("steam-run");
            fhs.arg(c.get_program())
                .args(c.get_args())
                .envs(c.get_envs().filter_map(|(k, v)| Some((k, v?))));
            c = fhs;
        }
        let log = store.root.join(WRAPPER_LOG);
        let child = spawn_logged(c.current_dir(game), &log)?;
        say(
            "info",
            &format!(
                "launching F1 25 directly (Steam must be running); waiting for the wrapper, output in {}",
                log.display()
            ),
        );
        Ok(Wrapper {
            child: Some(child),
            log,
            timeout: DIRECT_START_TIMEOUT,
        })
    } else {
        Command::new("steam")
            .args(["-applaunch", &STEAM_APP_ID.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        say("info", "asked Steam to launch F1 25; waiting for the wrapper");
        Ok(Wrapper {
            child: None,
            log: PathBuf::new(),
            timeout: STEAM_START_TIMEOUT,
        })
    }
}

struct Follower {
    path: PathBuf,
    pos: u64,
    buf: String,
}

impl Follower {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            pos: 0,
            buf: String::new(),
        }
    }

    fn lines(&mut self) -> Vec<String> {
        let Ok(mut f) = std::fs::File::open(&self.path) else {
            return Vec::new();
        };
        let size = f.metadata().map(|m| m.len()).unwrap_or(0);
        if size < self.pos {
            self.pos = 0;
        }
        let mut chunk = Vec::new();
        if f.seek(SeekFrom::Start(self.pos)).is_err() || f.read_to_end(&mut chunk).is_err() {
            return Vec::new();
        }
        self.pos += chunk.len() as u64;
        self.buf.push_str(&String::from_utf8_lossy(&chunk));
        let mut out: Vec<String> = self.buf.split('\n').map(str::to_owned).collect();
        self.buf = out.pop().unwrap_or_default();
        out
    }
}

struct Follow {
    dir: PathBuf,
    loader: Option<Follower>,
    wine: Follower,
    scan: LogScan,
    symbols: Symbols,
    last_loader: Option<String>,
}

impl Follow {
    fn new(dir: &Path, loader: Option<PathBuf>, symbols: Symbols) -> Self {
        let loader = loader.or_else(|| Some(dir.join("loader.log"))).map(Follower::new);
        let wine = Follower::new(dir.join(format!("steam-{STEAM_APP_ID}.log")));
        Self {
            dir: dir.into(),
            loader,
            wine,
            scan: LogScan::default(),
            symbols,
            last_loader: None,
        }
    }

    fn poll(&mut self) {
        for line in self.loader.as_mut().map(Follower::lines).unwrap_or_default() {
            if line.trim().is_empty() || line.contains("   section ") {
                continue;
            }
            let text = line.split_once("] ").map_or(line.as_str(), |(_, t)| t).to_owned();
            say(
                if text.starts_with("[core]") {
                    "core"
                } else {
                    "loader"
                },
                &text,
            );
            self.last_loader = Some(text);
        }
        for line in self.wine.lines() {
            match self.scan.feed(&line) {
                Some(Event::Fatal {
                    code,
                    addr,
                    first: true,
                }) => {
                    let name = fatal_name(code).unwrap_or("?");
                    say(
                        "crash",
                        &format!(
                            "{name} ({code:08x}) at {}",
                            self.scan.describe(addr, &self.symbols)
                        ),
                    );
                }
                Some(Event::Crash(text)) => say("crash", &text),
                Some(Event::Error(text)) => say("wine", &text.chars().take(240).collect::<String>()),
                _ => {}
            }
        }
    }

    fn summary(&self) {
        let read = |f: &str| {
            std::fs::read_to_string(self.dir.join(f))
                .ok()
                .map(|s| s.trim().to_owned())
        };
        let code = read("exit_code").unwrap_or_else(|| "?".into());
        let secs = |f| read(f).and_then(|s| s.parse::<f64>().ok());
        let dur = match (secs("game_start"), secs("game_end")) {
            (Some(a), Some(b)) => format!("{:.1}s", b - a),
            _ => "?".into(),
        };
        println!();
        say(
            "info",
            &format!(
                "game exited with code {code} after {dur}; logs in {}",
                self.dir.display()
            ),
        );
        if let Some(l) = &self.last_loader {
            say("info", &format!("last loader line: {l}"));
        }
        for (i, line) in self.scan.summary(&self.symbols).into_iter().enumerate() {
            say(
                if self.scan.exceptions.is_empty() && i == 0 {
                    "info"
                } else {
                    "crash"
                },
                &line,
            );
        }
        if !self.scan.errors.is_empty() {
            let total: usize = self.scan.errors.values().sum();
            say(
                "info",
                &format!(
                    "{} distinct wine err: lines ({total} total)",
                    self.scan.errors.len()
                ),
            );
        }
    }
}

fn symbols(game: &Path, exe: Option<&Path>) -> Symbols {
    let mut s = Symbols::parse_kb(KB);
    let exe = exe.map(Path::to_path_buf).unwrap_or_else(|| game.join(GAME_EXE));
    s.load_pdata(&exe).ok();
    s
}

fn say(kind: &str, text: &str) {
    let colour = match kind {
        "loader" => "36",
        "core" => "32",
        "wine" => "33",
        "crash" => "31;1",
        _ => "90",
    };
    if std::io::stdout().is_terminal() {
        println!("\x1b[{colour}m[{kind}]\x1b[0m {text}");
    } else {
        println!("[{kind}] {text}");
    }
}

fn which(bin: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
}

fn shell_join(args: &[String]) -> String {
    args.iter()
        .map(|a| {
            if a.chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_./=:,+%@".contains(c))
            {
                a.clone()
            } else {
                format!("'{}'", a.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    std::fs::create_dir_all(path.parent().expect("has parent"))?;
    std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn install_ctrl_c() {
    #[cfg(target_os = "linux")]
    unsafe {
        extern "C" fn on_int(_: libc::c_int) {
            STOP.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        libc::signal(libc::SIGINT, on_int as *const () as libc::sighandler_t);
    }
}

static STOP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(target_os = "linux")]
fn set_affinity(cpus: &[usize]) {
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        for &c in cpus {
            libc::CPU_SET(c, &mut set);
        }
        libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set);
    }
}

#[cfg(not(target_os = "linux"))]
fn set_affinity(_: &[usize]) {}

#[cfg(target_os = "linux")]
fn become_subreaper() {
    unsafe {
        libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0);
    }
}

#[cfg(not(target_os = "linux"))]
fn become_subreaper() {}

#[cfg(unix)]
fn link(target: &Path, at: &Path) {
    std::os::unix::fs::symlink(target, at).ok();
}

#[cfg(not(unix))]
fn link(_: &Path, _: &Path) {}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn temp_log(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("swofty-run-{}-{name}", std::process::id()))
            .join(WRAPPER_LOG)
    }

    #[test]
    fn reports_a_wrapper_that_exits_before_the_game_starts() {
        let log = temp_log("early");
        let child = spawn_logged(
            Command::new("sh").args([
                "-c",
                "echo '[info] installing profile Default'; \
                 echo 'swofty: version.dll already exists in the game folder; run swofty restore' >&2; exit 3",
            ]),
            &log,
        )
        .unwrap();
        let mut w = Wrapper {
            child: Some(child),
            log: log.clone(),
            timeout: DIRECT_START_TIMEOUT,
        };
        let status = loop {
            if let Some(s) = w.exited() {
                break s;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(3));
        assert_eq!(
            w.failure(status, "before the game started").to_string(),
            "the wrapper exited (exit status: 3) before the game started: \
             version.dll already exists in the game folder; run swofty restore"
        );
        std::fs::remove_dir_all(log.ancestors().nth(2).unwrap()).ok();
    }

    #[test]
    fn names_the_log_when_the_wrapper_says_nothing() {
        let log = temp_log("silent");
        let child = spawn_logged(Command::new("sh").args(["-c", "exit 1"]), &log).unwrap();
        let mut w = Wrapper {
            child: Some(child),
            log: log.clone(),
            timeout: DIRECT_START_TIMEOUT,
        };
        let status = loop {
            if let Some(s) = w.exited() {
                break s;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let msg = w.failure(status, "before the game started").to_string();
        assert!(
            msg.ends_with(&format!("without output; see {}", log.display())),
            "{msg}"
        );
        std::fs::remove_dir_all(log.ancestors().nth(2).unwrap()).ok();
    }

    #[test]
    fn keeps_the_last_lines() {
        let log = temp_log("tail");
        std::fs::create_dir_all(log.parent().unwrap()).unwrap();
        std::fs::write(&log, "a\n\nb\r\nc\n").unwrap();
        assert_eq!(tail(&log, 2), ["b", "c"]);
        assert_eq!(tail(&log, 9), ["a", "b", "c"]);
        assert!(tail(&log.with_extension("missing"), 3).is_empty());
        std::fs::remove_dir_all(log.ancestors().nth(2).unwrap()).ok();
    }
}
