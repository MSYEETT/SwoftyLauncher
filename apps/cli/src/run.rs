use std::io::{IsTerminal, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::Args;
use swofty_core::crash::{Event, LogScan, Symbols, fatal_name};
use swofty_core::install::{Game, Installer};
use swofty_core::process::{descends_from, game_processes, game_running, stop_game, stop_process};
use swofty_core::recovery::{Scope, SessionLock};
use swofty_core::session::{
    Config, DEFAULT_WINEDEBUG, LOADER_LOG, LOADER_MARK, LaunchRequest, LogMark, parse_cpu_list,
    parse_steam_command,
};
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
        help = "Build and install the profile (database, tracks) before the game starts, and roll it back when it exits"
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
    if wrapped {
        return wrapper(store, profile, &args, &g);
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
    {
        let lock = crate::recover::lock(store, &info)?;
        crate::recover::stale(store, &lock, &g, Scope::All, &info)?;
    }
    // Modded sessions run without EA AntiCheat: the terms are accepted once,
    // and declining means the game is not launched.
    crate::offline::gate(store, std::io::stdin().is_terminal())?;
    let id = format!("{:x}{:x}", std::process::id(), (now_secs() * 1e6) as u64);
    let request = LaunchRequest {
        id: id.clone(),
        winedebug: Some(args.winedebug.clone()),
        profile: Some(profile.into()),
        apply: args.apply,
    };
    write_json(&store.root.join("state/launch.json"), &request)?;
    let loader = game.join(LOADER_LOG);
    let before = LogMark::read(&loader);
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
            let err = wrapper.failure(status, "before the game started");
            settle(store, &g, &mut wrapper, None, false);
            return Err(err);
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
    let mut relay = wrapper
        .child
        .as_ref()
        .map(|_| Follower::from_end(wrapper.log.clone()));
    let mut follow = Follow::new(&dir, Some((loader, before)), symbols(&game, args.exe.as_deref()));
    install_ctrl_c();
    let mut quiet = false;
    while !dir.join("exit_code").exists() {
        if stop_requested() {
            if wrapper.child.is_none() {
                say("info", "stopped following (the game keeps running)");
                return Ok(ExitCode::SUCCESS);
            }
            if quiet {
                say("info", LEFT);
                return Ok(ExitCode::FAILURE);
            }
            quiet = true;
            say(
                "info",
                "stopped following; F1 25 keeps running, and its game files are rolled back when it exits (Ctrl-C again to leave now)",
            );
        }
        if !quiet {
            follow.poll();
            if let Some(r) = relay.as_mut() {
                relay_lines(r);
            }
        }
        if let Some(status) = wrapper.exited()
            && !dir.join("exit_code").exists()
        {
            let err = wrapper.failure(status, "while the game was running");
            settle(store, &g, &mut wrapper, relay.as_mut(), false);
            return Err(err);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    if !quiet {
        std::thread::sleep(Duration::from_millis(1500));
        follow.poll();
    }
    let summary = follow.summary();
    settle(store, &g, &mut wrapper, relay.as_mut(), true);
    summary?;
    Ok(ExitCode::SUCCESS)
}

const LEFT: &str = "left without waiting; the next swofty run or launcher start rolls the game files back";
const SETTLE_TIMEOUT: Duration = Duration::from_secs(600);

fn info(line: &str) {
    say("info", line);
}

/// After the game: wait for the wrapper to finish rolling the session back,
/// relaying what it reports, then put back anything it did not (a wrapper
/// killed mid-rollback leaves its journal open).
fn settle(store: &Store, g: &Game, wrapper: &mut Wrapper, mut relay: Option<&mut Follower>, ended: bool) {
    if let Some(child) = wrapper.child.as_mut() {
        loop {
            if let Some(r) = relay.as_deref_mut() {
                relay_lines(r);
            }
            if !matches!(child.try_wait(), Ok(None)) {
                break;
            }
            if stop_requested() {
                say("info", LEFT);
                return;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        if let Some(r) = relay {
            relay_lines(r);
        }
    }
    let started = Instant::now();
    let lock = loop {
        match SessionLock::try_acquire(&store.root) {
            Ok(Some(lock)) => break lock,
            Ok(None) => {}
            Err(e) => {
                say("info", &format!("cannot check the session lock: {e}"));
                return;
            }
        }
        if wrapper.child.is_none() && game_running() {
            say(
                "info",
                "F1 25 is running again; its game files are rolled back when it exits",
            );
            return;
        }
        if started.elapsed() > SETTLE_TIMEOUT || stop_requested() {
            say(
                "info",
                "the session is still rolling back; the next swofty run or launcher start finishes it",
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    match swofty_core::recovery::recover(&lock, &store.root, g, Scope::All) {
        Ok(r) if r.recovered() => {
            say("info", "the wrapper ended without rolling the session back");
            crate::recover::announce(store, g, &r.lines(), &info);
        }
        Ok(r) => {
            r.lines().iter().for_each(|l| info(l));
            if ended {
                info("session over: its game files are rolled back");
            }
        }
        Err(e) => info(&format!("rollback failed, run swofty restore: {e:#}")),
    }
}

fn relay_lines(r: &mut Follower) {
    for line in r.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        match line.strip_prefix('[').and_then(|l| l.split_once("] ")) {
            Some((kind, text)) if !kind.contains(' ') => say(kind, text),
            _ => say("wrapper", line.strip_prefix("swofty: ").unwrap_or(line)),
        }
    }
}

pub fn logs(store: &Store, dir: Option<PathBuf>, exe: Option<PathBuf>) -> Result<ExitCode> {
    let dir = match dir {
        Some(d) => d,
        None => store.latest_session().context("no previous session")?,
    };
    let game = steam::find_game().map(|g| g.dir).unwrap_or_default();
    let mut follow = Follow::new(&dir, None, symbols(&game, exe.as_deref()));
    follow.poll();
    follow.summary()?;
    Ok(ExitCode::SUCCESS)
}

fn wrapper(store: &Store, profile: &str, args: &RunArgs, g: &Game) -> Result<ExitCode> {
    if let Some(running) = game_processes().first() {
        bail!(
            "F1 25 is already running (pid {}); close it before launching it again",
            running.pid
        );
    }
    let lock = crate::recover::lock(store, &info)?;
    crate::recover::stale(store, &lock, g, Scope::All, &info)?;
    crate::offline::gate(store, false)?;
    if let Some(reason) = swofty_core::compat::elevation_blocker(&g.dir) {
        bail!("{reason}");
    }
    end_on_signals();
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
    let game = g.dir.clone();
    let apply = request.apply || args.apply;
    let session = Session {
        store,
        game: g,
        request_id: request.id.clone(),
        _lock: lock,
    };
    if apply {
        say("info", &format!("installing profile {profile}"));
        crate::build::install(store, &profile, g, true, &info)?;
    }
    become_subreaper();
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
    for note in swofty_core::offline::stage(&game, &artifacts, &journal)? {
        say("info", &format!("offline play: {note}"));
    }
    say(
        "info",
        "staged offline play: EA AntiCheat is not started this session; multiplayer is unavailable",
    );
    let result = run_loop(store, args, &config, &request, &profile, g, apply);
    drop(session);
    result
}

/// A wrapper session: it holds the session lock, and however it ends (the
/// game exits or crashes, an error, a panic, a stop signal) dropping it puts
/// the offline play staging and the profile's game files back.
struct Session<'a> {
    store: &'a Store,
    game: &'a Game,
    request_id: String,
    _lock: SessionLock,
}

impl Drop for Session<'_> {
    fn drop(&mut self) {
        match swofty_core::offline::restore_pending(&self.store.root) {
            Ok(true) => info("offline play: the game folder is back to vanilla"),
            Ok(false) => {}
            Err(e) => info(&format!("offline play restore failed, run swofty restore: {e:#}")),
        }
        let inst = Installer::new(&self.store.root, self.game);
        match inst.roll_back(false) {
            Ok(done) => {
                if !done.is_empty() {
                    info("rolled the profile's game files back");
                }
                for d in done.iter().flat_map(|r| &r.drift) {
                    info(&format!("rollback: {d}"));
                }
            }
            Err(e) => info(&format!(
                "rollback failed; the next swofty run or launcher start retries it, or run swofty restore: {e:#}"
            )),
        }
        let logs = self
            .store
            .latest_session()
            .filter(|d| {
                std::fs::read_to_string(d.join("request_id")).is_ok_and(|s| s.trim() == self.request_id)
            })
            .map(|d| d.join("game-logs"))
            .unwrap_or_else(|| self.store.sessions_dir().join("orphaned-logs"));
        match inst.tidy(&logs) {
            Ok(0) => {}
            Ok(n) => info(&format!(
                "moved {n} log file(s) out of the game folder to {}",
                logs.display()
            )),
            Err(e) => info(&format!("the loader folder was left in the game folder: {e:#}")),
        }
    }
}

fn run_loop(
    store: &Store,
    args: &RunArgs,
    config: &Config,
    request: &LaunchRequest,
    profile: &str,
    g: &Game,
    apply: bool,
) -> Result<ExitCode> {
    let profile = profile.to_owned();
    let game = g.dir.clone();
    loop {
        if ending() {
            return Ok(ExitCode::FAILURE);
        }
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
        if !relaunch || ending() {
            return Ok(ExitCode::from(code.clamp(0, 255) as u8));
        }
        say("info", "Apply & restart: starting F1 25 again");
        if apply {
            say("info", &format!("reinstalling profile {profile}"));
            crate::build::install(store, &profile, g, true, &info)?;
        }
    }
}

fn rebuild_if_needed(store: &Store, profile: &str, game: &Path) {
    let pending = match (store.profile(profile), store.addons()) {
        (Ok(p), Ok(addons)) => p.rebuild_pending(&addons),
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
    let loader = game.join(LOADER_LOG);
    let before = LogMark::read(&loader);
    std::fs::write(dir.join(LOADER_MARK), serde_json::to_vec(&before)?)?;
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
    // Proton's steam helper quote-wraps targets whose path contains spaces
    // and then fails to open the quoted name at all, so the game folder's
    // "F1 25" cannot be passed directly. A space-free symlink to the folder
    // gives an absolute target that survives untouched; the game and the
    // loader see the same files through it.
    let game_link = {
        let link = store.root.join("state/game-link");
        #[cfg(unix)]
        {
            match std::fs::symlink_metadata(&link) {
                Ok(m) if m.file_type().is_symlink() => {
                    if std::fs::read_link(&link).ok().as_deref() != Some(game) {
                        std::fs::remove_file(&link).and_then(|_| std::os::unix::fs::symlink(game, &link))?;
                    }
                }
                Ok(_) => bail!(
                    "{} exists and is not the game link; remove it and try again",
                    link.display()
                ),
                Err(_) => {
                    std::fs::create_dir_all(store.root.join("state"))?;
                    std::os::unix::fs::symlink(game, &link)?;
                }
            }
        }
        #[cfg(not(unix))]
        let _ = &link;
        link
    };
    // The game gets a curated environment: Proton's steam helper packs it
    // into a Windows environment block (32767 characters), and a build
    // shell's variables blow past that, which kills the game at startup.
    let (shell_vars, game_env) = {
        let shell: Vec<(String, String)> = std::env::vars().collect();
        let n = shell.len();
        let env = swofty_core::session::game_env(shell, &config.launch.env);
        (n, env)
    };
    if game_env.len() != shell_vars {
        say(
            "info",
            &format!(
                "environment: passing {} of {} variables to the game (the Windows limit is 32K characters)",
                game_env.len(),
                shell_vars
            ),
        );
    }
    let spawn = |exe: &str, log: &str| -> Result<std::process::Child> {
        let out = std::fs::File::create(dir.join(log))?;
        let mut c = match &proton {
            Some(p) => {
                let mut c = Command::new(p);
                c.arg("run").arg(game_link.join(exe));
                c
            }
            None => Command::new(game.join(exe)),
        };
        c.current_dir(game)
            .env_clear()
            .envs(&game_env)
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
    // The offline-play session launcher (staged in place of the EA
    // AntiCheat launcher) creates the game suspended and patches the launch
    // guard before the game's first instruction: the check runs before any
    // window exists, so the in-process hook is too late to act on it.
    let target = if game.join("EAAntiCheat.GameServiceLauncher.exe.bak").is_file() {
        say("info", "offline play: launching via the session launcher");
        "EAAntiCheat.GameServiceLauncher.exe"
    } else {
        GAME_EXE
    };
    let status = wait_game(&mut spawn(target, "game_stdout.log")?)?;
    let code = status.code().unwrap_or(-1);
    std::fs::write(dir.join("exit_code"), format!("{code}\n"))?;
    std::fs::write(dir.join("game_end"), format!("{}\n", now_secs()))?;
    if LogMark::replaced(&loader, before.as_ref()) {
        std::fs::copy(&loader, dir.join("loader.log")).ok();
    }
    for g in game_processes()
        .into_iter()
        .filter(|g| descends_from(g.pid, std::process::id()))
    {
        stop_process(g.pid).ok();
    }
    Ok(code)
}

/// Wait for the game; a stop signal to the wrapper (Steam's Stop button, a
/// kill, a closed terminal) closes the game it started instead of leaving
/// the session's files in the game folder.
fn wait_game(child: &mut Child) -> Result<ExitStatus> {
    let mut asked = false;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if !asked && ending() {
            asked = true;
            info("asked to stop: closing F1 25, then rolling its game files back");
            let ours: Vec<u32> = game_processes()
                .into_iter()
                .filter(|g| descends_from(g.pid, std::process::id()))
                .map(|g| g.pid)
                .collect();
            if ours.is_empty() {
                child.kill().ok();
            }
            for pid in ours {
                stop_process(pid).ok();
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
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
        // The wrapper gets its own process group: a Ctrl-C in this terminal
        // is for this command, and must not kill the wrapper (and with it the
        // session's rollback) along with it.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut c, 0);
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
    fallback: Option<PathBuf>,
    pos: u64,
    buf: String,
    stale: Option<Option<LogMark>>,
}

impl Follower {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            fallback: None,
            pos: 0,
            buf: String::new(),
            stale: None,
        }
    }

    fn from_end(path: PathBuf) -> Self {
        let pos = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        Self {
            pos,
            ..Self::new(path)
        }
    }

    fn open(&self) -> Option<std::fs::File> {
        std::fs::File::open(&self.path)
            .ok()
            .or_else(|| std::fs::File::open(self.fallback.as_ref()?).ok())
    }

    fn replacing(path: PathBuf, before: Option<LogMark>) -> Self {
        Self {
            stale: Some(before),
            ..Self::new(path)
        }
    }

    fn lines(&mut self) -> Vec<String> {
        if let Some(before) = &self.stale {
            if !LogMark::replaced(&self.path, before.as_ref()) {
                return Vec::new();
            }
            self.stale = None;
        }
        let Some(mut f) = self.open() else {
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
    fn new(dir: &Path, loader: Option<(PathBuf, Option<LogMark>)>, symbols: Symbols) -> Self {
        let loader = Some(match loader {
            Some((path, before)) => Follower {
                fallback: Some(dir.join("loader.log")),
                ..Follower::replacing(path, before)
            },
            None => Follower::new(dir.join("loader.log")),
        });
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

    fn summary(&self) -> Result<()> {
        if let Some(failure) = self.scan.start_failure {
            println!();
            say("info", &format!("logs in {}", self.dir.display()));
            bail!("{failure}");
        }
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
        match &self.last_loader {
            Some(l) => say("info", &format!("last loader line: {l}")),
            None => {
                // The follower waits for the loader's rotation at attach, so
                // when the loader never attaches the session launcher's own
                // lines sit unnoticed in the log. Report them directly.
                let stub: Vec<String> = self
                    .loader
                    .as_ref()
                    .and_then(|f| {
                        let mut text = String::new();
                        f.open()?.read_to_string(&mut text).ok()?;
                        Some(text)
                    })
                    .map(|t| {
                        t.lines()
                            .filter(|l| l.contains("[offline]"))
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default();
                if stub.is_empty() {
                    say(
                        "info",
                        "no loader log for this session: the loader never attached",
                    );
                } else {
                    say("info", "the loader never attached; the session launcher reports:");
                    for l in stub.iter().rev().take(8).rev() {
                        say("loader", l);
                    }
                }
            }
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
        Ok(())
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
    Ok(swofty_core::write_json(path, value)?)
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

fn stop_requested() -> bool {
    STOP.swap(false, std::sync::atomic::Ordering::Relaxed)
}

fn end_on_signals() {
    #[cfg(target_os = "linux")]
    unsafe {
        extern "C" fn on_end(_: libc::c_int) {
            ENDING.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        for sig in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            libc::signal(sig, on_end as *const () as libc::sighandler_t);
        }
    }
}

static ENDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn ending() -> bool {
    ENDING.load(std::sync::atomic::Ordering::Relaxed)
}

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

    const STALE: &str = "2026-10-17 21:44:08.228 [  416] swofty loader (version.dll proxy) attached, pid 412\n\
                         2026-10-17 21:45:23.672 [  620] [game] (error) from the previous run\n";

    fn session_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-run-{}-{name}", std::process::id()));
        std::fs::create_dir_all(dir.join("game/swofty")).unwrap();
        dir
    }

    #[test]
    fn ignores_a_stale_loader_log_and_follows_its_replacement() {
        let dir = session_dir("rotate");
        let log = dir.join("game").join(LOADER_LOG);
        std::fs::write(&log, STALE).unwrap();
        let mut follower = Follower::replacing(log.clone(), LogMark::read(&log));
        assert!(follower.lines().is_empty());
        assert!(follower.lines().is_empty());
        std::fs::rename(&log, log.with_file_name("loader.prev.log")).unwrap();
        std::fs::write(
            &log,
            "2026-10-17 22:12:08.004 [  420] swofty loader (version.dll proxy) attached, pid 416\npar",
        )
        .unwrap();
        assert_eq!(
            follower.lines(),
            ["2026-10-17 22:12:08.004 [  420] swofty loader (version.dll proxy) attached, pid 416"]
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reports_a_game_process_proton_could_not_start() {
        let dir = session_dir("run-process");
        let log = dir.join("game").join(LOADER_LOG);
        std::fs::write(&log, STALE).unwrap();
        let before = LogMark::read(&log);
        std::fs::write(
            dir.join(format!("steam-{STEAM_APP_ID}.log")),
            include_str!("../../../packages/core/tests/fixtures/steam-run-process-731.txt"),
        )
        .unwrap();
        std::fs::write(dir.join("exit_code"), "1\n").unwrap();
        let mut follow = Follow::new(&dir, Some((log, before)), Symbols::default());
        follow.poll();
        assert_eq!(follow.last_loader, None);
        let err = follow.summary().unwrap_err().to_string();
        assert!(
            err.starts_with("Proton could not start the game process (CreateProcess error 731). "),
            "{err}"
        );
        assert!(err.ends_with("Nothing from SwoftyMod ran."), "{err}");
        std::fs::remove_dir_all(&dir).ok();
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

    struct Modded {
        root: PathBuf,
        store: Store,
        game: Game,
    }

    impl Modded {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!("swofty-run-{}-{name}", std::process::id()));
            std::fs::remove_dir_all(&root).ok();
            let dir = root.join("game");
            std::fs::create_dir_all(dir.join("game_package")).unwrap();
            std::fs::write(dir.join(GAME_EXE), "exe").unwrap();
            std::fs::write(dir.join("game_package/db.bin"), "db").unwrap();
            std::fs::create_dir_all(root.join("src")).unwrap();
            std::fs::write(root.join("src/db"), "modded").unwrap();
            std::fs::write(root.join("src/core"), "core").unwrap();
            let game = Game {
                identity: swofty_core::baseline::Identity::detect(&dir, Some(1)).unwrap(),
                dir,
                user_reg: None,
            };
            let store = Store::new(root.join("data"));
            let plan = swofty_core::install::Plan {
                profile: "MODDED".into(),
                data: [("game_package/db.bin".to_owned(), root.join("src/db"))].into(),
                loader: [(
                    "swofty/addons/core/swofty_core.dll".to_owned(),
                    root.join("src/core"),
                )]
                .into(),
                ..Default::default()
            };
            Installer {
                session: true,
                ..Installer::new(&store.root, &game)
            }
            .apply(&plan)
            .unwrap();
            std::fs::write(game.dir.join("swofty/loader.log"), "attached\n").unwrap();
            Self { root, store, game }
        }

        fn vanilla(&self) -> bool {
            std::fs::read_to_string(self.game.dir.join("game_package/db.bin")).unwrap() == "db"
                && !self.game.dir.join("swofty").exists()
                && swofty_core::journal::Journal::open(&self.store.root)
                    .unwrap()
                    .is_empty()
                && Installer::new(&self.store.root, &self.game)
                    .verify()
                    .unwrap()
                    .is_empty()
        }
    }

    impl Drop for Modded {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).ok();
        }
    }

    #[test]
    fn rolls_back_a_session_whose_wrapper_was_killed_after_the_game_exited() {
        let m = Modded::new("killed");
        assert!(!m.vanilla());
        let log = m.store.root.join(WRAPPER_LOG);
        let child = spawn_logged(
            Command::new("sh").args(["-c", "echo '[info] in game: nothing changed'; kill -9 $$"]),
            &log,
        )
        .unwrap();
        let mut w = Wrapper {
            child: Some(child),
            log: log.clone(),
            timeout: DIRECT_START_TIMEOUT,
        };
        let mut relay = Follower::new(log);
        settle(&m.store, &m.game, &mut w, Some(&mut relay), true);
        assert!(m.vanilla());
        assert_eq!(
            std::fs::read_to_string(m.store.root.join("sessions/orphaned-logs/loader.log")).unwrap(),
            "attached\n"
        );
        let notice = swofty_core::recovery::Notice::read(&m.store.root).unwrap();
        assert!(notice.lines[0].ends_with("(5 change(s) for profile MODDED) that a session left open"));
    }

    #[test]
    fn waits_for_a_steam_wrapper_to_finish_rolling_back_before_recovering() {
        if !which("flock") || game_running() {
            return;
        }
        let m = Modded::new("steam");
        let lock = swofty_core::recovery::lock_path(&m.store.root);
        let mut holder = Command::new("flock")
            .arg(&lock)
            .args(["sleep", "1"])
            .spawn()
            .unwrap();
        let started = Instant::now();
        while !SessionLock::held_elsewhere(&m.store.root) {
            assert!(started.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(20));
        }
        let mut w = Wrapper {
            child: None,
            log: PathBuf::new(),
            timeout: STEAM_START_TIMEOUT,
        };
        settle(&m.store, &m.game, &mut w, None, true);
        assert!(
            holder.try_wait().unwrap().is_some(),
            "settle ran under a live session"
        );
        holder.wait().unwrap();
        assert!(m.vanilla());
    }

    #[test]
    fn relays_the_wrappers_lines_under_their_own_tags() {
        let log = temp_log("relay");
        std::fs::create_dir_all(log.parent().unwrap()).unwrap();
        std::fs::write(&log, "old\n").unwrap();
        let mut relay = Follower::from_end(log.clone());
        std::fs::write(
            &log,
            "old\n[info] rolled the profile's game files back\nswofty: oops\n",
        )
        .unwrap();
        assert_eq!(
            relay.lines(),
            ["[info] rolled the profile's game files back", "swofty: oops"]
        );
        std::fs::remove_dir_all(log.ancestors().nth(2).unwrap()).ok();
    }
}
