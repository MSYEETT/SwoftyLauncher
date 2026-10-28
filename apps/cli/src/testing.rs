//! `swofty test`: plan and run addon tests. A run takes the game lock,
//! builds a throwaway profile per batch, starts the game through
//! `swofty run --apply --direct`, waits for the environment, runs the steps
//! it can, checks the session's logs and writes the reports.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use swofty_core::builtin::CORE_ID;
use swofty_core::install::Game;
use swofty_core::journal::Journal;
use swofty_core::process::{descends_from, game_processes, games_in, stop_process};
use swofty_core::recovery::SessionLock;
use swofty_core::session::{LOADER_LOG, LogMark, SessionRecord, request_stop};
use swofty_core::store::Store;
use swofty_core::testsuite::bench;
use swofty_core::testsuite::checks::{self, Evidence, Failure};
use swofty_core::testsuite::lock::{self, GameLock};
use swofty_core::testsuite::plan::{self, Batch, Plan};
use swofty_core::testsuite::report::{self, BatchReport, Report, Status, StepReport, TestReport, exit};
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
    }
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
        match run_with_game(store, &a, &plan, &run_id, &ids, &out, &mut report) {
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

fn run_with_game(
    store: &Store,
    a: &RunArgs,
    plan: &Plan,
    run_id: &str,
    suites: &[String],
    out: &Path,
    report: &mut Report,
) -> std::result::Result<(), Stop> {
    let g = crate::build::game(store, a.game.clone()).map_err(|e| Stop::Infra(format!("{e:#}")))?;
    crate::build::supported(&g).ok();
    report.game_build = swofty_core::exe_version::read(&g.dir.join(swofty_core::GAME_EXE));
    preflight(store, &g, plan, out).map_err(Stop::Infra)?;
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
    fn launch(&mut self, profile: &str, game_args: &[String], dir: &Path) -> Result<Box<dyn Running>>;
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
    fn launch(&mut self, profile: &str, game_args: &[String], dir: &Path) -> Result<Box<dyn Running>> {
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

    fn profile(&self, b: &Batch) -> Result<()> {
        let installed = self.store.addons()?;
        let registry = self.store.registry()?;
        let mut p = self.store.profile(self.profile)?;
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
        if let Err(e) = self.profile(b) {
            br.error = Some(format!("the test profile: {e:#}"));
            fail_all(&mut reports, "the batch did not start");
            return (br, reports);
        }
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
        let mut run = match self.launcher.launch(self.profile, &args, &dir) {
            Ok(r) => r,
            Err(e) => {
                br.error = Some(format!("could not start the game: {e:#}"));
                fail_all(&mut reports, "the batch did not start");
                return (br, reports);
            }
        };
        br.started_game = true;
        let kind = b.env.kind;
        let mut told_title = false;
        let ready = loop {
            let text = live.text();
            if text.contains(checks::ready_line(kind)) {
                break Ok(());
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
        let mut views: Vec<Option<String>> = vec![None; b.tests.len()];
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
                    views[i] = Some(live.text().to_owned());
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
                            views[i] = None;
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
        if let Some(c) = &crash {
            br.notes.push(c.clone());
        }
        let rel = |f: &str| format!("batches/{index}/{f}");
        for &i in &runnable {
            let r = &mut reports[i];
            if r.status == Status::Error {
                continue;
            }
            let ev = Evidence {
                log: views[i]
                    .clone()
                    .filter(|v| v.len() <= final_log.len())
                    .unwrap_or_else(|| final_log.clone()),
                perf: perf.clone(),
                crash: crash.clone(),
                log_path: Some(rel("loader.log")),
                perf_path: Some(rel("perf.log")),
            };
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
        let (clean, notes) = self.clean(unstaged);
        br.clean = Some(clean);
        br.notes.extend(notes);
        (br, reports)
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
                    "not seen before the game exited".to_owned()
                } else {
                    format!("not seen within {} s", limit.as_secs())
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

#[cfg(test)]
mod tests;
