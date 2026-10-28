//! `swofty test`: plan and run addon tests. A run takes the game lock,
//! builds a throwaway profile per batch, starts the game through
//! `swofty run --apply --direct`, waits for the environment, runs the steps
//! it can, checks the session's logs and writes the reports.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use swofty_core::addon_storage;
use swofty_core::builtin::CORE_ID;
use swofty_core::install::Game;
use swofty_core::journal::Journal;
use swofty_core::process::{descends_from, game_processes, games_in, stop_process};
use swofty_core::recovery::SessionLock;
use swofty_core::session::{LOADER_LOG, LogMark, SessionRecord, request_stop};
use swofty_core::store::Store;
use swofty_core::testsuite::bench;
use swofty_core::testsuite::career;
use swofty_core::testsuite::checks::{self, Evidence, Failure};
use swofty_core::testsuite::fixture::{self, Fixture};
use swofty_core::testsuite::lock::{self, GameLock};
use swofty_core::testsuite::plan::{self, Batch, Plan};
use swofty_core::testsuite::report::{self, BatchReport, Report, Status, StepReport, TestReport, exit};
use swofty_core::testsuite::sandbox::{self, RunDir};
use swofty_core::testsuite::suite::{self, Kind, Selector, Suite, Test};

use crate::time::{now_secs, stamp};

#[derive(Subcommand)]
pub enum TestCommand {
    #[command(
        about = "Show the tests a run would start, grouped by environment, and how many game starts it takes"
    )]
    Plan(PlanArgs),
    #[command(
        about = "Start F1 25 in each test's environment, run its steps, check what happened and write the report"
    )]
    Run(RunArgs),
    #[command(
        subcommand,
        about = "Capture, list and check fixtures: saved games in a known state"
    )]
    Fixture(FixtureCommand),
}

#[derive(Subcommand)]
pub enum FixtureCommand {
    #[command(
        about = "Copy a test run's sandbox saves and addon storage into a new fixture in the fixture store"
    )]
    Capture(CaptureArgs),
    #[command(about = "List the fixtures in the fixture store and check their files")]
    List(StoreArgs),
    #[command(about = "Check a fixture's files against its fixture.json")]
    Check(CheckArgs),
}

#[derive(Args)]
pub struct StoreArgs {
    #[arg(
        long,
        value_name = "DIR",
        help = "The fixture store (default: $SWOFTY_TEST_FIXTURES, else test-fixtures in the data folder)"
    )]
    store: Option<PathBuf>,
}

#[derive(Args)]
pub struct CaptureArgs {
    #[arg(help = "The new fixture's name (letters, digits, - _ .)")]
    name: String,
    #[arg(
        long,
        value_name = "RUN",
        conflicts_with = "from",
        help = "The test run whose sandbox to capture, as named under test-runs/ (default: the newest)"
    )]
    run: Option<String>,
    #[arg(
        long,
        value_name = "DIR",
        help = "Capture a save folder instead of a test run: a Steam remote/ folder or the userdata/<app> folder holding \
                one, such as a copy of a backup. Never the live folder"
    )]
    from: Option<PathBuf>,
    #[arg(long, help = "What the fixture holds, in a sentence")]
    description: Option<String>,
    #[arg(
        long = "addon",
        value_name = "ID=VERSION",
        help = "An addon the fixture's careers need (repeatable; a run's addons are recorded on their own)"
    )]
    addons: Vec<String>,
    #[arg(
        long = "career",
        value_name = "MODE[:SLOT]",
        help = "A career the fixture holds, by its game mode and save slot (repeatable)"
    )]
    careers: Vec<String>,
    #[arg(long, help = "Free text for fixture.json's holds.notes")]
    notes: Option<String>,
    #[command(flatten)]
    store: StoreArgs,
}

#[derive(Args)]
pub struct CheckArgs {
    #[arg(help = "A fixture name in the store or a fixture folder")]
    fixture: String,
    #[command(flatten)]
    store: StoreArgs,
}

#[derive(Args)]
pub struct Select {
    #[arg(
        required = true,
        help = "An addon folder, its tests folder or a .test.json file"
    )]
    paths: Vec<PathBuf>,
    #[arg(
        short = 't',
        long = "tag",
        value_name = "TAG",
        help = "Only tests with this tag (repeatable)"
    )]
    tags: Vec<String>,
    #[arg(
        long = "id",
        value_name = "ID",
        help = "Only this test, by id or suite/id (repeatable)"
    )]
    ids: Vec<String>,
}

#[derive(Args)]
pub struct PlanArgs {
    #[command(flatten)]
    select: Select,
    #[arg(long, help = "Print the plan as JSON")]
    json: bool,
}

#[derive(Args)]
pub struct RunArgs {
    #[command(flatten)]
    select: Select,
    #[arg(
        long,
        help = "Folder for the report (default: swofty-test/<run> in this folder)"
    )]
    out: Option<PathBuf>,
    #[arg(
        long,
        help = "Close a running F1 25 first, by its exact process id, and wait for its game files to be put back"
    )]
    take_over: bool,
    #[arg(
        long,
        value_name = "TIME",
        default_value = "30m",
        value_parser = parse_wait,
        help = "How long to wait for the game to be free: 0, 90s, 30m, 2h"
    )]
    wait: Duration,
    #[arg(long, help = crate::GAME_HELP)]
    game: Option<PathBuf>,
    #[arg(
        long,
        value_name = "DIR",
        help = "The machine's game lock folder (default: $SWOFTY_GAME_LOCK, else /mnt/work/.f1-game.lock.d where /mnt/work exists, else swofty-game.lock.d next to the data folder)"
    )]
    lock: Option<PathBuf>,
    #[arg(long, help = "Print report.json instead of the summary")]
    json: bool,
}

fn parse_wait(s: &str) -> std::result::Result<Duration, String> {
    let s = s.trim();
    let (num, unit) = match s.find(|c: char| !c.is_ascii_digit()) {
        Some(at) => s.split_at(at),
        None => (s, "s"),
    };
    let n: u64 = num
        .parse()
        .map_err(|_| format!("{s}: expected a time such as 0, 90s, 30m or 2h"))?;
    let secs = match unit {
        "s" => n,
        "m" => n * 60,
        "h" => n * 3600,
        _ => return Err(format!("{s}: expected a time such as 0, 90s, 30m or 2h")),
    };
    Ok(Duration::from_secs(secs))
}

pub fn command(store: &Store, cmd: TestCommand) -> Result<ExitCode> {
    match cmd {
        TestCommand::Plan(a) => plan_command(&a),
        TestCommand::Run(a) => run_command(store, a),
        TestCommand::Fixture(c) => fixture_command(store, c),
    }
}

/// The folder of each suite's test file: a test's `fixture` path is relative to it.
fn bases(suites: &[Suite]) -> BTreeMap<String, PathBuf> {
    suites
        .iter()
        .map(|s| {
            (
                s.id.clone(),
                s.file.parent().map(Path::to_path_buf).unwrap_or_default(),
            )
        })
        .collect()
}

fn load(select: &Select) -> std::result::Result<(Vec<Suite>, Plan), String> {
    let mut suites: Vec<Suite> = Vec::new();
    for p in &select.paths {
        for s in suite::load(p).map_err(|e| e.0)? {
            if let Some(o) = suites.iter().find(|o| o.id == s.id) {
                return Err(format!(
                    "{}: suite {} is also in {}",
                    s.file.display(),
                    s.id,
                    o.file.display()
                ));
            }
            suites.push(s);
        }
    }
    let selector = Selector {
        tags: select.tags.clone(),
        ids: select.ids.clone(),
    };
    let plan = plan::plan(&suites, &selector);
    if plan.tests() == 0 {
        let mut want: Vec<String> = select.tags.iter().map(|t| format!("-t {t}")).collect();
        want.extend(select.ids.iter().map(|i| format!("--id {i}")));
        return Err(match want.is_empty() {
            true => "the test files hold no tests".to_owned(),
            false => format!("no test matches {}", want.join(" ")),
        });
    }
    Ok((suites, plan))
}

fn infra(e: impl std::fmt::Display) -> ExitCode {
    eprintln!("swofty: {e}");
    ExitCode::from(exit::INFRA)
}

fn plan_command(a: &PlanArgs) -> Result<ExitCode> {
    let (_, plan) = match load(&a.select) {
        Ok(p) => p,
        Err(e) => return Ok(infra(e)),
    };
    if a.json {
        println!("{}", serde_json::to_string_pretty(&plan.json())?);
    } else {
        print!("{}", plan.text());
    }
    Ok(ExitCode::SUCCESS)
}

fn say(text: &str) {
    eprintln!("[test] {text}");
}

fn run_command(store: &Store, a: RunArgs) -> Result<ExitCode> {
    let started = Instant::now();
    let (suites, plan) = match load(&a.select) {
        Ok(p) => p,
        Err(e) => return Ok(infra(e)),
    };
    let run_id = stamp();
    let out = a
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from("swofty-test").join(&run_id));
    if let Err(e) = std::fs::create_dir_all(&out) {
        return Ok(infra(format!("{}: {e}", out.display())));
    }
    let mut report = Report::new(&run_id, &out.display().to_string());
    report.game_starts = plan.game_starts();
    let busy = if plan.game_starts() == 0 {
        Harness::skip_all(&plan, &mut report);
        false
    } else {
        let ids: Vec<String> = suites.iter().map(|s| s.id.clone()).collect();
        match run_with_game(
            store,
            &a,
            &plan,
            &run_id,
            &ids,
            &bases(&suites),
            &out,
            &mut report,
        ) {
            Ok(()) => false,
            Err(Stop::Busy(why)) => {
                report.errors.push(why);
                Harness::not_run(&plan, &mut report, "the game was not free");
                true
            }
            Err(Stop::Infra(why)) => {
                report.errors.push(why);
                Harness::not_run(&plan, &mut report, "the run could not start the game");
                false
            }
        }
    };
    report.ms = started.elapsed().as_millis() as u64;
    report.exit_code = if busy { exit::BUSY } else { report.compute_exit() };
    if let Err(e) = write_reports(&out, &report) {
        eprintln!("swofty: could not write the report in {}: {e}", out.display());
    }
    if a.json {
        print!("{}", report.json());
    } else {
        print!("{}", report.summary());
    }
    Ok(ExitCode::from(report.exit_code))
}

fn write_reports(out: &Path, r: &Report) -> std::io::Result<()> {
    std::fs::write(out.join(report::JSON_FILE), r.json())?;
    std::fs::write(out.join(report::JUNIT_FILE), r.junit())?;
    std::fs::write(out.join(report::SUMMARY_FILE), r.summary())
}

#[derive(Debug)]
enum Stop {
    Busy(String),
    Infra(String),
}

const MIN_FREE: u64 = 1 << 30;

#[allow(clippy::too_many_arguments)]
fn run_with_game(
    store: &Store,
    a: &RunArgs,
    plan: &Plan,
    run_id: &str,
    suites: &[String],
    bases: &BTreeMap<String, PathBuf>,
    out: &Path,
    report: &mut Report,
) -> std::result::Result<(), Stop> {
    let g = crate::build::game(store, a.game.clone()).map_err(|e| Stop::Infra(format!("{e:#}")))?;
    crate::build::supported(&g).ok();
    report.game_build = swofty_core::exe_version::read(&g.dir.join(swofty_core::GAME_EXE));
    preflight(store, &g, plan, out).map_err(Stop::Infra)?;
    let exe_sha = sandbox_preflight(store, &g, plan, bases).map_err(Stop::Infra)?;
    let live_saves = exe_sha
        .as_ref()
        .and_then(|_| swofty_core::steam::find_game())
        .and_then(|i| swofty_core::steam::save_dir(&i.steam_root));
    let dir = a.lock.clone().unwrap_or_else(|| lock::default_path(&store.root));
    let owner = lock::owner_line(run_id, suites, &hh_mm());
    let lock = take_lock(&dir, &owner, a.wait)?;
    say(&format!("holding the game lock {}", dir.display()));
    free_the_game(store, a.take_over, a.wait)?;
    let launch_json = store.root.join("state/launch.json");
    let saved = std::fs::read(&launch_json).ok();
    let profile = format!("test-{run_id}");
    let mut launcher = SwoftyRun {
        data: store.root.clone(),
        game: g.dir.clone(),
    };
    let mut h = Harness {
        store,
        game: &g.dir,
        out,
        profile: &profile,
        launcher: &mut launcher,
        times: Times::default(),
        run_id,
        bases: bases.clone(),
        exe_sha,
        live_saves,
    };
    h.run(plan, report);
    if store.profile_names().is_ok_and(|n| n.contains(&profile))
        && let Err(e) = store.delete_profile(&profile)
    {
        report
            .errors
            .push(format!("could not delete the test profile {profile}: {e}"));
    }
    let put_back = match saved {
        Some(bytes) => std::fs::write(&launch_json, bytes),
        None => std::fs::remove_file(&launch_json).or_else(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(e),
        }),
    };
    if let Err(e) = put_back {
        report
            .errors
            .push(format!("could not put {} back: {e}", launch_json.display()));
    }
    if let Err(e) = lock.release() {
        report
            .errors
            .push(format!("could not release the game lock {}: {e}", dir.display()));
    }
    Ok(())
}

/// Checks before the lock: nothing here changes anything.
fn preflight(store: &Store, g: &Game, plan: &Plan, out: &Path) -> std::result::Result<(), String> {
    terms(store)?;
    if !swofty_core::setup::done(&store.root) {
        return Err(swofty_core::setup::NEEDED.to_owned());
    }
    if swofty_core::steam::in_steam_library(&g.dir) {
        let status = swofty_core::steam_client::status();
        if !status.state.usable() {
            return Err(format!(
                "Steam is not ready ({status}); start Steam and sign in, then run the tests again"
            ));
        }
        if let Some(state) = swofty_core::steam::find_game()
            .filter(|i| swofty_core::copies::same_dir(&i.dir, &g.dir))
            .and_then(|i| swofty_core::steam::busy(&i))
        {
            return Err(format!("Steam is {state} F1 25; wait for it to finish"));
        }
    }
    if let Some(free) = swofty_core::disk::free_space(out)
        && free < MIN_FREE
    {
        return Err(format!(
            "{} has {} free; a test run needs at least {}",
            out.display(),
            swofty_core::disk::gb(free),
            swofty_core::disk::gb(MIN_FREE)
        ));
    }
    addons_installed(store, plan)
}

/// A career environment runs only in the save sandbox, and only on an exe
/// whose core bindings carry the sandbox's install point (verified in the
/// KB). Its fixture must resolve and match its fixture.json. Returns the
/// exe's sha256 when a batch needs the sandbox.
fn sandbox_preflight(
    store: &Store,
    g: &Game,
    plan: &Plan,
    bases: &BTreeMap<String, PathBuf>,
) -> std::result::Result<Option<String>, String> {
    let need: Vec<&Batch> = plan
        .batches
        .iter()
        .filter(|b| sandboxed(b) && b.starts_game())
        .collect();
    if need.is_empty() {
        return Ok(None);
    }
    let mut hashes = swofty_core::baseline::Hashes::load(&store.root);
    let exe = g.dir.join(swofty_core::GAME_EXE);
    let sha = hashes
        .sha256(&exe)
        .map_err(|e| format!("could not hash {}: {e}", exe.display()))?;
    hashes.save();
    let loader = swofty_core::builtin::loader_dir();
    check_sandbox(
        loader.as_deref(),
        &sha,
        &need,
        bases,
        &fixture::store(&store.root),
    )?;
    Ok(Some(sha))
}

pub(crate) fn sandboxed(b: &Batch) -> bool {
    b.env.kind == Kind::Career
}

pub(crate) fn check_sandbox(
    loader: Option<&Path>,
    exe_sha: &str,
    batches: &[&Batch],
    bases: &BTreeMap<String, PathBuf>,
    store: &Path,
) -> std::result::Result<(), String> {
    let Some(loader) = loader else {
        return Err(
            "a career environment needs the save sandbox, and the launcher's loader (version.dll and the \
                    core) was not found"
                .to_owned(),
        );
    };
    let proof = sandbox::verified(loader, exe_sha)
        .map_err(|e| format!("a career environment needs the save sandbox: {e}"))?;
    say(&format!("save sandbox: verified for this F1_25.exe by {proof}"));
    for b in batches {
        let f = batch_fixture(b, bases, store).map_err(|e| format!("{}: {e}", b.label()))?;
        if f.manifest.exe.sha256 != exe_sha {
            say(&format!(
                "{}: fixture {} was saved on another F1_25.exe ({}); older saves normally load forward",
                b.label(),
                f.manifest.name,
                &f.manifest.exe.sha256[..f.manifest.exe.sha256.len().min(16)]
            ));
        }
    }
    Ok(())
}

fn batch_fixture(
    b: &Batch,
    bases: &BTreeMap<String, PathBuf>,
    store: &Path,
) -> std::result::Result<Fixture, String> {
    let spec = b
        .env
        .str("fixture")
        .ok_or("a career environment names no fixture")?;
    let base = b
        .tests
        .first()
        .and_then(|t| bases.get(&t.suite))
        .map(PathBuf::as_path);
    fixture::load(&fixture::resolve(spec, base, store)?)
}

/// The offline play terms are the player's to accept; a test run never
/// accepts them.
fn terms(store: &Store) -> std::result::Result<(), String> {
    match swofty_core::offline::is_accepted(&store.root) {
        true => Ok(()),
        false => Err(format!(
            "the offline play terms are not accepted in {}; a test run never accepts them for you: run swofty \
             offline accept yourself, then run the tests again",
            store.root.display()
        )),
    }
}

fn addons_installed(store: &Store, plan: &Plan) -> std::result::Result<(), String> {
    let installed = store.addons().map_err(|e| e.to_string())?;
    for b in plan.batches.iter().filter(|b| b.starts_game()) {
        if let Some(id) = b.addons.iter().find(|id| !installed.contains_key(*id)) {
            return Err(format!(
                "{id} is not installed, and the tests in {} need it: install it with swofty addons install <folder>",
                b.label()
            ));
        }
    }
    Ok(())
}

fn take_lock(dir: &Path, owner: &str, wait: Duration) -> std::result::Result<GameLock, Stop> {
    let running = || !game_processes().is_empty();
    match GameLock::acquire(dir, owner, wait, LOCK_POLL, |b| {
        say(&format!("waiting for the game: {}", b.text(running())))
    }) {
        Ok(Ok(lock)) => Ok(lock),
        Ok(Err(busy)) => Err(Stop::Busy(busy.text(running()))),
        Err(e) => Err(Stop::Infra(format!(
            "{e}; set SWOFTY_GAME_LOCK or --lock to a folder this user can create"
        ))),
    }
}

const LOCK_POLL: Duration = Duration::from_secs(5);
const CLOSE_WAIT: Duration = Duration::from_secs(120);
const ROLLBACK_WAIT: Duration = Duration::from_secs(600);

/// A game running without the lock is a player's: wait for it to close, or
/// with `--take-over`, close it by its exact pid. Then wait for its session
/// to put the game files back.
fn free_the_game(store: &Store, take_over: bool, wait: Duration) -> std::result::Result<(), Stop> {
    let running = game_processes();
    if let Some(first) = running.first() {
        if take_over {
            request_stop(&store.root);
            for g in &running {
                say(&format!("--take-over: closing F1 25 (pid {})", g.pid));
                stop_process(g.pid).ok();
            }
            if !until(CLOSE_WAIT, || game_processes().is_empty()) {
                return Err(Stop::Infra(format!(
                    "F1 25 (pid {}) did not close within {} s",
                    first.pid,
                    CLOSE_WAIT.as_secs()
                )));
            }
        } else {
            say(&format!(
                "F1 25 is running (pid {}); waiting for it to close (--take-over closes it)",
                first.pid
            ));
            if !until(wait, || game_processes().is_empty()) {
                return Err(Stop::Busy(format!(
                    "F1 25 is running (pid {}); close it, or pass --take-over to close it",
                    first.pid
                )));
            }
        }
    }
    if SessionLock::held_elsewhere(&store.root) {
        say("waiting for the last session to put its game files back");
        let limit = if take_over {
            ROLLBACK_WAIT
        } else {
            wait.min(ROLLBACK_WAIT)
        };
        if !until(limit, || !SessionLock::held_elsewhere(&store.root)) {
            let why =
                "the last session is still putting the game files back (its session lock is held)".to_owned();
            return Err(if take_over {
                Stop::Infra(why)
            } else {
                Stop::Busy(why)
            });
        }
    }
    Ok(())
}

fn until(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    loop {
        if done() {
            return true;
        }
        if start.elapsed() >= limit {
            return false;
        }
        std::thread::sleep(Duration::from_millis(500).min(limit.saturating_sub(start.elapsed())));
    }
}

/// Local `HH:MM` for the lock's owner line.
fn hh_mm() -> String {
    #[cfg(target_os = "linux")]
    {
        let t = now_secs() as libc::time_t;
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        // SAFETY: localtime_r writes only into `tm`.
        if !unsafe { libc::localtime_r(&t, &mut tm) }.is_null() {
            return format!("{:02}:{:02}", tm.tm_hour, tm.tm_min);
        }
    }
    let s = now_secs() as u64 % 86_400;
    format!("{:02}:{:02}", s / 3600, s % 3600 / 60)
}

/// What starts the game for a batch: `swofty run`, or a stand-in in tests.
pub(crate) trait Launcher {
    fn launch(
        &mut self,
        profile: &str,
        game_args: &[String],
        env: &[(String, String)],
        dir: &Path,
    ) -> Result<Box<dyn Running>>;
}

/// A started batch.
pub(crate) trait Running {
    /// The game has exited and its game files are back.
    fn finished(&mut self) -> bool;
    /// Ask the game to close, by its exact pid.
    fn stop(&mut self);
    /// Give up: end the launcher process itself.
    fn kill(&mut self);
    /// The session folder with the session's logs, once known.
    fn session(&mut self) -> Option<PathBuf>;
}

struct SwoftyRun {
    data: PathBuf,
    game: PathBuf,
}

impl Launcher for SwoftyRun {
    fn launch(
        &mut self,
        profile: &str,
        game_args: &[String],
        env: &[(String, String)],
        dir: &Path,
    ) -> Result<Box<dyn Running>> {
        let log = std::fs::File::create(dir.join("launcher.log"))?;
        let mut c = Command::new(std::env::current_exe()?);
        c.arg("--data-dir")
            .arg(&self.data)
            .arg("--profile")
            .arg(profile)
            .arg("run")
            .arg("--apply")
            .arg("--direct")
            .arg("--no-start-store")
            .arg("--game")
            .arg(&self.game);
        for a in game_args {
            c.arg(format!("--game-arg={a}"));
        }
        c.env("SWOFTY_KEEP_FOCUS", "1")
            .envs(env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log);
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut c, 0);
        let before = std::fs::read(self.data.join("state/launch.json")).ok();
        let child = c.spawn()?;
        Ok(Box::new(SwoftyRunning {
            child,
            data: self.data.clone(),
            game: self.game.clone(),
            launched: SystemTime::now(),
            before,
            exited: false,
            session: None,
        }))
    }
}

struct SwoftyRunning {
    child: Child,
    data: PathBuf,
    game: PathBuf,
    launched: SystemTime,
    before: Option<Vec<u8>>,
    exited: bool,
    session: Option<PathBuf>,
}

impl Running for SwoftyRunning {
    fn finished(&mut self) -> bool {
        if !self.exited {
            self.exited = !matches!(self.child.try_wait(), Ok(None));
        }
        self.exited && !SessionLock::held_elsewhere(&self.data)
    }

    fn stop(&mut self) {
        request_stop(&self.data);
        let ours = games_in(&self.game).into_iter().filter(|g| {
            descends_from(g.pid, self.child.id()) || g.started.is_some_and(|s| s >= self.launched)
        });
        for g in ours {
            stop_process(g.pid).ok();
        }
    }

    fn kill(&mut self) {
        stop_process(self.child.id()).ok();
    }

    fn session(&mut self) -> Option<PathBuf> {
        if self.session.is_some() {
            return self.session.clone();
        }
        let now = std::fs::read(self.data.join("state/launch.json")).ok()?;
        if Some(&now) == self.before.as_ref() {
            return None;
        }
        let id = serde_json::from_slice::<serde_json::Value>(&now).ok()?["id"]
            .as_str()?
            .to_owned();
        let store = Store::new(&self.data);
        let dir = store.latest_session()?;
        let record = SessionRecord::read(&dir);
        if record.request_id.as_deref() == Some(id.as_str()) {
            self.session = Some(dir);
        }
        self.session.clone()
    }
}

pub(crate) struct Times {
    pub start: Duration,
    pub step: Duration,
    pub test: Duration,
    pub race: Duration,
    pub finish: Duration,
    pub restop: Duration,
    pub poll: Duration,
}

impl Default for Times {
    fn default() -> Self {
        Times {
            start: Duration::from_secs(900),
            step: Duration::from_secs(30),
            test: Duration::from_secs(300),
            race: Duration::from_secs(900),
            finish: Duration::from_secs(600),
            restop: Duration::from_secs(10),
            poll: Duration::from_millis(250),
        }
    }
}

pub(crate) struct Harness<'a> {
    pub store: &'a Store,
    pub game: &'a Path,
    pub out: &'a Path,
    pub profile: &'a str,
    pub launcher: &'a mut dyn Launcher,
    pub times: Times,
    pub run_id: &'a str,
    /// Each suite's test file folder, for fixture paths.
    pub bases: BTreeMap<String, PathBuf>,
    /// The game exe's sha256 when a batch needs the save sandbox.
    pub exe_sha: Option<String>,
    /// The player's Steam save folder: a sandboxed batch compares its
    /// listing (names, sizes, times; never contents) before and after.
    pub live_saves: Option<PathBuf>,
}

/// A batch's save sandbox: its run folder, the fixture copied into it and
/// the player's save folder listing before the game started.
struct Sandbox {
    run: RunDir,
    fixture: String,
    storage_ids: BTreeSet<String>,
    live_before: Option<BTreeMap<String, (u64, Option<SystemTime>)>>,
}

/// The benchmark XML staged in the game folder for one batch, and removed
/// when the batch ends.
struct Staged {
    path: PathBuf,
    made_dir: Option<PathBuf>,
}

impl Staged {
    fn write(game: &Path, b: &Batch) -> std::io::Result<Option<Staged>> {
        if b.env.kind != Kind::Benchmark {
            return Ok(None);
        }
        let dir = game.join(bench::DIR);
        let made_dir = match dir.is_dir() {
            true => None,
            false => {
                std::fs::create_dir_all(&dir)?;
                Some(dir.clone())
            }
        };
        let path = dir.join(bench::FILE);
        std::fs::write(&path, bench::xml(&b.env))?;
        Ok(Some(Staged { path, made_dir }))
    }

    fn remove(&mut self) -> std::io::Result<()> {
        match std::fs::remove_file(&self.path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
        if let Some(d) = self.made_dir.take() {
            std::fs::remove_dir(d).ok();
        }
        Ok(())
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        self.remove().ok();
    }
}

fn ms(since: Instant) -> u64 {
    since.elapsed().as_millis() as u64
}

fn read(path: &Path) -> Option<String> {
    std::fs::read(path)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// The live loader.log once this session's game has replaced the last one.
struct Live {
    path: PathBuf,
    mark: Option<LogMark>,
    last: String,
}

impl Live {
    fn text(&mut self) -> &str {
        if LogMark::replaced(&self.path, self.mark.as_ref())
            && let Some(t) = read(&self.path)
            && t.len() >= self.last.len()
        {
            self.last = t;
        }
        &self.last
    }
}

impl Harness<'_> {
    fn blank(b: &Batch, index: usize) -> (BatchReport, Vec<TestReport>) {
        let label = b.label();
        let batch = BatchReport {
            index,
            env: label.clone(),
            key: b.key.clone(),
            addons: b.addons.clone(),
            started_game: false,
            ready_ms: None,
            ms: None,
            session: None,
            end: None,
            error: None,
            clean: None,
            notes: Vec::new(),
        };
        let tests = b
            .tests
            .iter()
            .map(|t| TestReport {
                suite: t.suite.clone(),
                id: t.id.clone(),
                env: label.clone(),
                batch: index,
                status: Status::Skipped,
                ms: None,
                reason: t.skip_reason().map(|r| r.to_owned()),
                failures: Vec::new(),
                steps: Vec::new(),
            })
            .collect();
        (batch, tests)
    }

    fn skip_all(plan: &Plan, report: &mut Report) {
        for (i, b) in plan.batches.iter().enumerate() {
            let (batch, tests) = Self::blank(b, i + 1);
            report.batches.push(batch);
            report.tests.extend(tests);
        }
    }

    fn not_run(plan: &Plan, report: &mut Report, why: &str) {
        Self::skip_all(plan, report);
        for t in report.tests.iter_mut().filter(|t| t.reason.is_none()) {
            t.status = Status::NotRun;
            t.reason = Some(why.to_owned());
        }
    }

    pub fn run(&mut self, plan: &Plan, report: &mut Report) {
        for (i, b) in plan.batches.iter().enumerate() {
            let (batch, tests) = self.batch(i + 1, b);
            report.batches.push(batch);
            report.tests.extend(tests);
        }
    }

    fn profile(&self, b: &Batch, name: &str) -> Result<()> {
        let installed = self.store.addons()?;
        let registry = self.store.registry()?;
        let mut p = self.store.profile(name)?;
        let keep = |id: &str| {
            id == CORE_ID || b.addons.iter().any(|a| a == id) || registry.get(id).is_some_and(|e| e.locked)
        };
        let listed: Vec<String> = p.addons.iter().map(|a| a.id.clone()).collect();
        for id in listed {
            p.set_enabled(&id, keep(&id))?;
        }
        for id in &b.addons {
            if !installed.contains_key(id) {
                bail!("{id} is not installed: install it with swofty addons install <folder>");
            }
            p.set_enabled(id, true)?;
        }
        p.options.clear();
        for (id, options) in &b.settings {
            let Some(m) = installed.get(id) else {
                bail!("the settings name {id}, which is not installed");
            };
            for (option, value) in options {
                p.set_option(m, option, value.clone())?;
            }
        }
        self.store.save_profile(&p)?;
        Ok(())
    }

    fn batch(&mut self, index: usize, b: &Batch) -> (BatchReport, Vec<TestReport>) {
        let (mut br, mut reports) = Self::blank(b, index);
        let runnable: Vec<usize> = (0..b.tests.len())
            .filter(|&i| reports[i].reason.is_none())
            .collect();
        if runnable.is_empty() {
            return (br, reports);
        }
        let fail_all = |reports: &mut [TestReport], why: &str| {
            for &i in &runnable {
                reports[i].status = Status::Error;
                reports[i].reason = Some(why.to_owned());
            }
        };
        let dir = self.out.join("batches").join(index.to_string());
        if let Err(e) = std::fs::create_dir_all(&dir) {
            br.error = Some(format!("{}: {e}", dir.display()));
            fail_all(&mut reports, "the batch did not start");
            return (br, reports);
        }
        let sandboxed = sandboxed(b);
        let profile = match sandboxed {
            true => format!("{}-{index}", self.profile),
            false => self.profile.to_owned(),
        };
        if let Err(e) = self.profile(b, &profile) {
            br.error = Some(format!("the test profile: {e:#}"));
            fail_all(&mut reports, "the batch did not start");
            self.drop_profile(sandboxed, &profile, &mut br);
            return (br, reports);
        }
        let sandbox = match sandboxed {
            false => None,
            true => match self.open_sandbox(index, b, &profile) {
                Ok(s) => {
                    br.notes.push(format!(
                        "save sandbox {} with fixture {}",
                        s.run.saves().display(),
                        s.fixture
                    ));
                    Some(s)
                }
                Err(e) => {
                    br.error = Some(format!("the save sandbox: {e}"));
                    fail_all(&mut reports, "the batch did not start");
                    self.drop_profile(sandboxed, &profile, &mut br);
                    return (br, reports);
                }
            },
        };
        let env = sandbox.as_ref().map(|s| s.run.env()).unwrap_or_default();
        let mut staged = match Staged::write(self.game, b) {
            Ok(s) => s,
            Err(e) => {
                br.error = Some(format!("could not write the benchmark XML: {e}"));
                fail_all(&mut reports, "the batch did not start");
                return (br, reports);
            }
        };
        let mut live = Live {
            path: self.game.join(LOADER_LOG),
            mark: LogMark::read(&self.game.join(LOADER_LOG)),
            last: String::new(),
        };
        let args = match b.env.kind {
            Kind::Benchmark => bench::args(),
            _ => Vec::new(),
        };
        say(&format!("batch {index}: starting F1 25 ({})", br.env));
        let started = Instant::now();
        let mut run = match self.launcher.launch(&profile, &args, &env, &dir) {
            Ok(r) => r,
            Err(e) => {
                br.error = Some(format!("could not start the game: {e:#}"));
                fail_all(&mut reports, "the batch did not start");
                if let Some(s) = sandbox {
                    self.close_sandbox(s, b, &profile, &mut br, &mut reports, &runnable);
                }
                self.drop_profile(sandboxed, &profile, &mut br);
                return (br, reports);
            }
        };
        br.started_game = true;
        let kind = b.env.kind;
        let mut told_title = false;
        let want = match sandboxed {
            true => career::ready_line(&b.env),
            false => checks::ready_line(kind).to_owned(),
        };
        let ready = loop {
            let text = live.text();
            if text.contains(&want) {
                break Ok(());
            }
            if sandboxed && let Some(line) = text.lines().find(|l| l.contains(sandbox::FAILED_LINE)) {
                break Err(format!(
                    "the core's save sandbox did not install and the game quit: {line}"
                ));
            }
            if kind == Kind::Boot && !told_title && text.contains(checks::TITLE) {
                told_title = true;
                say("at the title screen: it waits for a button press before the main menu");
            }
            if run.finished() {
                break Err(format!("the game exited before {}", checks::ready_text(kind)));
            }
            if started.elapsed() >= self.times.start {
                break Err(format!(
                    "the game did not reach {} within {} s",
                    checks::ready_text(kind),
                    self.times.start.as_secs()
                ));
            }
            std::thread::sleep(self.times.poll);
        };
        match &ready {
            Err(why) => {
                br.error = Some(why.clone());
                fail_all(&mut reports, &format!("the batch did not start: {why}"));
            }
            Ok(()) => {
                br.ready_ms = Some(ms(started));
                say(&format!("batch {index}: ready at {}", checks::ready_text(kind)));
                let race_start = Instant::now();
                for &i in &runnable {
                    let t0 = Instant::now();
                    self.steps(&b.tests[i], &mut reports[i], &mut live, &mut *run);
                    reports[i].ms = Some(ms(t0));
                }
                if kind == Kind::Benchmark {
                    let limit = runnable
                        .iter()
                        .filter_map(|&i| b.tests[i].timeout_s)
                        .max()
                        .map_or(self.times.race, Duration::from_secs);
                    if until_polled(limit, self.times.poll, || run.finished()) {
                        for &i in &runnable {
                            reports[i].ms = Some(ms(race_start));
                        }
                    } else {
                        let why = format!("the benchmark race did not end within {} s", limit.as_secs());
                        br.error = Some(why.clone());
                        fail_all(&mut reports, &why);
                    }
                }
            }
        }
        if let Err(why) = self.finish(&mut *run) {
            br.notes.push(why);
        }
        br.ms = Some(ms(started));
        let unstaged = staged.as_mut().map(|s| s.remove());
        drop(staged);
        let session = run.session();
        br.session = session.as_ref().map(|s| s.display().to_string());
        let record = session.as_deref().map(SessionRecord::read);
        let final_log = session
            .as_deref()
            .and_then(|s| read(&s.join("loader.log")))
            .unwrap_or_else(|| live.text().to_owned());
        let perf = session.as_deref().and_then(|s| read(&s.join("perf.log")));
        if let Some(s) = &session {
            for f in [
                "loader.log",
                "perf.log",
                "end.json",
                "exit_code",
                "game_stdout.log",
            ] {
                std::fs::copy(s.join(f), dir.join(f)).ok();
            }
        } else {
            std::fs::write(dir.join("loader.log"), &final_log).ok();
        }
        br.end = record
            .as_ref()
            .and_then(|r| r.end.as_ref())
            .map(|e| format!("{}: {}", e.end.as_str(), e.reason));
        let crash = record.as_ref().filter(|r| r.crashed()).map(|r| match &r.end {
            Some(e) => format!("the game crashed ({})", e.reason),
            None => format!("the game crashed (exit code {})", r.exit_code.unwrap_or(-1)),
        });
        br.notes.extend(crash.clone());
        let rel = |f: &str| format!("batches/{index}/{f}");
        let ev = Evidence {
            log: final_log,
            perf,
            crash,
            log_path: Some(rel("loader.log")),
            perf_path: Some(rel("perf.log")),
        };
        for &i in &runnable {
            let r = &mut reports[i];
            if r.status == Status::Error {
                continue;
            }
            for c in &b.tests[i].checks {
                if let Err(f) = checks::evaluate(c, &ev) {
                    r.failures.push(f);
                }
            }
            r.status = match r.failures.is_empty() {
                true => Status::Pass,
                false => Status::Fail,
            };
        }
        if let Some(s) = sandbox {
            self.close_sandbox(s, b, &profile, &mut br, &mut reports, &runnable);
        }
        self.drop_profile(sandboxed, &profile, &mut br);
        let (clean, notes) = self.clean(unstaged);
        br.clean = Some(clean);
        br.notes.extend(notes);
        (br, reports)
    }

    /// A fresh run folder for the batch, the fixture's saves in its sandbox
    /// and the fixture's addon storage as the batch profile's copy, which
    /// the session loads into the addons' storage when it starts.
    fn open_sandbox(&self, index: usize, b: &Batch, profile: &str) -> std::result::Result<Sandbox, String> {
        let data = &self.store.root;
        let f = batch_fixture(b, &self.bases, &fixture::store(data))?;
        let run = RunDir::create(data, &format!("{}-{index}", self.run_id), self.exe_sha.as_deref())
            .map_err(|e| format!("could not make the run folder: {e}"))?;
        let ids: BTreeSet<String> = f
            .storage_ids()
            .into_iter()
            .chain(b.addons.iter().cloned())
            .collect();
        for id in &ids {
            let d = addon_storage::profile_dir(data, id, profile);
            if d.exists() {
                std::fs::remove_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
            }
        }
        let copied = f.install(&run.remote(), &|id| addon_storage::profile_dir(data, id, profile))?;
        let installed = self.store.addons().map_err(|e| e.to_string())?;
        let addons: BTreeMap<String, String> = b
            .addons
            .iter()
            .filter_map(|id| Some((id.clone(), installed.get(id)?.version.to_string())))
            .collect();
        std::fs::write(
            run.root.join("addons.json"),
            serde_json::to_vec_pretty(&addons).expect("serialisable"),
        )
        .map_err(|e| format!("{}: {e}", run.root.display()))?;
        for id in f.storage_ids().iter().filter(|id| !b.addons.contains(id)) {
            say(&format!(
                "fixture {}: it carries storage for {id}, which the batch does not enable; the game won't see it",
                f.manifest.name
            ));
        }
        say(&format!(
            "save sandbox {}: fixture {} ({} file(s)) copied in",
            run.saves().display(),
            f.manifest.name,
            copied.len()
        ));
        Ok(Sandbox {
            run,
            fixture: f.manifest.name.clone(),
            storage_ids: ids,
            live_before: self.live_saves.as_deref().map(sandbox::listing),
        })
    }

    /// After the batch: keep its addon storage in the run folder for a
    /// capture, check the core armed the sandbox and the player's save folder
    /// did not change, and retire the run token.
    fn close_sandbox(
        &self,
        s: Sandbox,
        b: &Batch,
        profile: &str,
        br: &mut BatchReport,
        reports: &mut [TestReport],
        runnable: &[usize],
    ) {
        let data = &self.store.root;
        for id in &s.storage_ids {
            let from = addon_storage::profile_dir(data, id, profile);
            let to = s.run.storage().join(id);
            if from.is_dir() && copy_tree(&from, &to).is_err() {
                br.notes
                    .push(format!("could not keep {id}'s storage in {}", to.display()));
            }
        }
        let status = s.run.status();
        if let Err(e) = s.run.retire() {
            br.notes.push(format!(
                "could not retire the run token in {}: {e}",
                s.run.root.display()
            ));
        }
        let mut why = match &status {
            None if br.started_game => Some(
                "the core never reported the save sandbox (no sandbox.json): the save device may not have started"
                    .to_owned(),
            ),
            Some(st) if !st.armed() => Some(format!("the save sandbox did not arm: {}", st.why)),
            _ => None,
        };
        if let (Some(before), Some(dir)) = (&s.live_before, &self.live_saves) {
            let changed = sandbox::changed(before, &sandbox::listing(dir));
            if !changed.is_empty() {
                why = Some(format!(
                    "THE PLAYER'S STEAM SAVE FOLDER CHANGED during a sandboxed run ({}): {}",
                    dir.display(),
                    changed.join(", ")
                ));
            }
        }
        if let Some(why) = why {
            say(&format!("{}: {why}", b.label()));
            br.error = Some(why.clone());
            for &i in runnable {
                reports[i].status = Status::Error;
                reports[i].reason = Some(why.clone());
            }
        }
    }

    fn drop_profile(&self, sandboxed: bool, profile: &str, br: &mut BatchReport) {
        if sandboxed
            && self
                .store
                .profile_names()
                .is_ok_and(|n| n.contains(&profile.to_owned()))
            && let Err(e) = self.store.delete_profile(profile)
        {
            br.notes
                .push(format!("could not delete the batch profile {profile}: {e}"));
        }
    }

    fn steps(&self, t: &Test, r: &mut TestReport, live: &mut Live, run: &mut dyn Running) {
        let test_limit = t.timeout_s.map_or(self.times.test, Duration::from_secs);
        let started = Instant::now();
        for s in &t.steps {
            let Some(wait) = &s.host else {
                r.steps.push(StepReport {
                    step: s.text.clone(),
                    status: format!("skipped: {}", suite::NEEDS_AGENT),
                    ms: None,
                });
                continue;
            };
            let limit = s
                .timeout_s
                .map_or(self.times.step, Duration::from_secs)
                .min(test_limit.saturating_sub(started.elapsed()));
            let t0 = Instant::now();
            let seen = loop {
                if checks::waited(live.text(), wait) {
                    break true;
                }
                if t0.elapsed() >= limit || run.finished() {
                    break checks::waited(live.text(), wait);
                }
                std::thread::sleep(self.times.poll);
            };
            r.steps.push(StepReport {
                step: s.text.clone(),
                status: if seen { "ok" } else { "timed out" }.to_owned(),
                ms: Some(ms(t0)),
            });
            if !seen {
                let why = if run.finished() {
                    "none before the game exited".to_owned()
                } else {
                    format!("none within {} s", limit.as_secs())
                };
                r.failures.push(Failure {
                    check: format!("wait {}", checks::wait_text(wait)),
                    expected: checks::wait_text(wait),
                    actual: why,
                    evidence: None,
                });
                return;
            }
        }
    }

    /// Close the game unless it has closed, and wait for the session to put
    /// the game files back, asking again now and then in case the game
    /// started late.
    fn finish(&self, run: &mut dyn Running) -> std::result::Result<(), String> {
        let start = Instant::now();
        let mut asked: Option<Instant> = None;
        while !run.finished() {
            if start.elapsed() >= self.times.finish {
                run.kill();
                return Err(format!(
                    "the session did not end within {} s of being asked to; its launcher was stopped, so run swofty \
                     restore if the game folder is not stock",
                    self.times.finish.as_secs()
                ));
            }
            if asked.is_none_or(|a| a.elapsed() >= self.times.restop) {
                run.stop();
                asked = Some(Instant::now());
            }
            std::thread::sleep(self.times.poll);
        }
        Ok(())
    }

    /// The game folder is back as it was: nothing staged is left, the
    /// session lock is free and no session journal is open.
    fn clean(&self, unstaged: Option<std::io::Result<()>>) -> (bool, Vec<String>) {
        let mut notes = Vec::new();
        if let Some(Err(e)) = unstaged {
            notes.push(format!("could not remove the benchmark XML: {e}"));
        }
        let xml = self.game.join(bench::DIR).join(bench::FILE);
        if xml.exists() {
            notes.push(format!("{} is still in the game folder", xml.display()));
        }
        let data = &self.store.root;
        if SessionLock::held_elsewhere(data) {
            notes.push("the session is still putting the game files back (its session lock is held)".into());
        }
        match Journal::open(data) {
            Ok(open) => {
                let n = open.iter().filter(|j| j.session).count();
                if n > 0 {
                    notes.push(format!(
                        "{n} session journal(s) are still open: the game files were not all put back; run swofty restore"
                    ));
                }
            }
            Err(e) => notes.push(format!("could not read the install journals: {e}")),
        }
        if swofty_core::offline::journal_path(data).exists() {
            notes.push("the offline play staging is still in the game folder; run swofty restore".into());
        }
        (notes.is_empty(), notes)
    }
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if e.file_type()?.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

fn until_polled(limit: Duration, poll: Duration, mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    loop {
        if done() {
            return true;
        }
        if start.elapsed() >= limit {
            return false;
        }
        std::thread::sleep(poll);
    }
}

fn store_dir(store: &Store, a: &StoreArgs) -> PathBuf {
    a.store.clone().unwrap_or_else(|| fixture::store(&store.root))
}

fn fixture_command(store: &Store, cmd: FixtureCommand) -> Result<ExitCode> {
    match cmd {
        FixtureCommand::Capture(a) => capture_command(store, a),
        FixtureCommand::List(a) => {
            let dir = store_dir(store, &a);
            let all = fixture::list(&dir);
            if all.is_empty() {
                println!("no fixtures in {}", dir.display());
            }
            let mut bad = false;
            for f in all {
                match f {
                    Ok(f) => println!(
                        "{:<32} {} file(s), exe {}, {}",
                        f.manifest.name,
                        f.manifest.files.len(),
                        &f.manifest.exe.sha256[..f.manifest.exe.sha256.len().min(16)],
                        f.manifest.description
                    ),
                    Err(e) => {
                        bad = true;
                        println!("BROKEN: {e}");
                    }
                }
            }
            Ok(if bad { ExitCode::FAILURE } else { ExitCode::SUCCESS })
        }
        FixtureCommand::Check(a) => {
            let dir = store_dir(store, &a.store);
            let path = match Path::new(&a.fixture).join(fixture::FILE).is_file() {
                true => PathBuf::from(&a.fixture),
                false => match fixture::resolve(&a.fixture, None, &dir) {
                    Ok(p) => p,
                    Err(e) => return Ok(infra(e)),
                },
            };
            match fixture::load(&path) {
                Ok(f) => {
                    println!(
                        "{}: {} file(s) match fixture.json ({})",
                        f.manifest.name,
                        f.manifest.files.len(),
                        path.display()
                    );
                    Ok(ExitCode::SUCCESS)
                }
                Err(e) => Ok(infra(e)),
            }
        }
    }
}

fn careers(specs: &[String]) -> std::result::Result<Vec<fixture::Career>, String> {
    specs
        .iter()
        .map(|c| {
            let (mode, slot) = match c.split_once(':') {
                Some((m, s)) => (
                    m,
                    Some(
                        s.parse::<u64>()
                            .map_err(|_| format!("--career {c}: the slot is not a number"))?,
                    ),
                ),
                None => (c.as_str(), None),
            };
            Ok(fixture::Career {
                mode: mode.to_owned(),
                slot,
                ..Default::default()
            })
        })
        .collect()
}

/// The saves under a folder given to `--from`: the folder itself when it
/// holds the profile save, else its `remote/`.
fn remote_in(dir: &Path) -> PathBuf {
    match dir.join(fixture::PROFILE_FILES[0]).is_file() {
        true => dir.to_path_buf(),
        false => dir.join(sandbox::REMOTE),
    }
}

fn capture_command(store: &Store, a: CaptureArgs) -> Result<ExitCode> {
    let dest = store_dir(store, &a.store);
    let mut addons: BTreeMap<String, String> = BTreeMap::new();
    let mut exe = fixture::Exe::default();
    let (remote, storage, source) = match &a.from {
        Some(dir) => {
            if let Some(live) =
                swofty_core::steam::find_game().and_then(|i| swofty_core::steam::save_dir(&i.steam_root))
                && let (Ok(x), Ok(y)) = (dir.canonicalize(), live.canonicalize())
                && (x == y || y.starts_with(&x))
            {
                return Ok(infra(format!(
                    "{} is the player's live Steam save folder; capture from a copy of it, never the live folder",
                    dir.display()
                )));
            }
            (remote_in(dir), Vec::new(), format!("folder {}", dir.display()))
        }
        None => {
            let run = match &a.run {
                Some(id) => RunDir::open(&store.root, id),
                None => RunDir::latest(&store.root),
            };
            let Some(run) = run else {
                return Ok(infra(format!(
                    "no test run with saves in {}; name one with --run or a save folder with --from",
                    sandbox::runs_dir(&store.root).display()
                )));
            };
            if let Ok(b) = std::fs::read(run.root.join("addons.json"))
                && let Ok(m) = serde_json::from_slice::<BTreeMap<String, String>>(&b)
            {
                addons = m;
            }
            if let Ok(b) = std::fs::read(run.root.join(sandbox::RUN_FILE))
                && let Ok(v) = serde_json::from_slice::<serde_json::Value>(&b)
                && let Some(sha) = v["exe_sha256"].as_str()
            {
                exe.sha256 = sha.to_owned();
            }
            let storage: Vec<(String, PathBuf)> = std::fs::read_dir(run.storage())
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
                .collect();
            (run.remote(), storage, format!("test run {}", run.id))
        }
    };
    for spec in &a.addons {
        let Some((id, v)) = spec.split_once('=') else {
            return Ok(infra(format!("--addon {spec}: expected ID=VERSION")));
        };
        addons.insert(id.to_owned(), v.to_owned());
    }
    if exe.sha256.is_empty()
        && let Ok(g) = crate::build::game(store, None)
    {
        let mut hashes = swofty_core::baseline::Hashes::load(&store.root);
        if let Ok(sha) = hashes.sha256(&g.dir.join(swofty_core::GAME_EXE)) {
            exe.sha256 = sha;
            exe.version = swofty_core::exe_version::read(&g.dir.join(swofty_core::GAME_EXE));
        }
        hashes.save();
    }
    let holds = fixture::Holds {
        careers: match careers(&a.careers) {
            Ok(c) => c,
            Err(e) => return Ok(infra(e)),
        },
        notes: a.notes.clone().unwrap_or_default(),
    };
    let meta = fixture::Meta {
        description: a.description.clone().unwrap_or_default(),
        created: stamp(),
        exe,
        addons,
        holds,
        source,
    };
    if let Err(e) = std::fs::create_dir_all(&dest) {
        return Ok(infra(format!("{}: {e}", dest.display())));
    }
    match fixture::capture(&a.name, &dest, &remote, &storage, meta) {
        Ok(f) => {
            println!(
                "fixture {} captured: {} file(s) in {}",
                f.manifest.name,
                f.manifest.files.len(),
                f.dir.display()
            );
            for rel in f.manifest.files.keys() {
                println!("  {rel}");
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(e) => Ok(infra(e)),
    }
}

#[cfg(test)]
mod tests;
