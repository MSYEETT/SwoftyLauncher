use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::time::Duration;

use clap::{CommandFactory, Parser};
use swofty_core::session::{Ending, LOADER_LOG, PERF_LOG, SessionEnd};
use swofty_core::store::Store;
use swofty_core::testsuite::plan;
use swofty_core::testsuite::report::{Report, Status, exit};
use swofty_core::testsuite::suite::{self, Selector};

use super::*;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("swofty-cli-test-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A stand-in for `swofty run`: a shell script plays the game, writing
/// loader.log lines into the game folder, and its end does what the
/// wrapper does when the game exits (keep the logs, record the end).
struct Fake {
    scripts: Vec<&'static str>,
    root: PathBuf,
    game: PathBuf,
    launches: Vec<(String, Vec<String>)>,
    envs: Vec<Vec<(String, String)>>,
}

impl Launcher for Fake {
    fn launch(
        &mut self,
        profile: &str,
        game_args: &[String],
        env: &[(String, String)],
        dir: &Path,
    ) -> anyhow::Result<Box<dyn Running>> {
        self.launches.push((profile.to_owned(), game_args.to_vec()));
        self.envs.push(env.to_vec());
        let script = self.scripts.remove(0);
        let session = self.root.join(format!("sessions/{}", self.launches.len()));
        std::fs::create_dir_all(&session)?;
        let log = std::fs::File::create(dir.join("launcher.log"))?;
        let child = Command::new("sh")
            .arg("-c")
            .arg(script)
            .arg("sh")
            .args(game_args)
            .env("GAME", &self.game)
            .envs(env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?;
        Ok(Box::new(FakeRun {
            child,
            game: self.game.clone(),
            session,
            done: false,
            stops: 0,
        }))
    }
}

struct FakeRun {
    child: Child,
    game: PathBuf,
    session: PathBuf,
    done: bool,
    stops: u32,
}

impl Running for FakeRun {
    fn finished(&mut self) -> bool {
        if self.done {
            return true;
        }
        let Ok(Some(status)) = self.child.try_wait() else {
            return false;
        };
        std::fs::copy(self.game.join(LOADER_LOG), self.session.join("loader.log")).ok();
        std::fs::copy(self.game.join(PERF_LOG), self.session.join("perf.log")).ok();
        std::fs::remove_dir_all(self.game.join("swofty")).ok();
        let code = status.code().unwrap_or(-1);
        let (end, reason) = match code {
            3 => (Ending::Crash, "exception 0xc0000005 in F1_25.exe"),
            _ if self.stops > 0 => (Ending::Stopped, "the test run closed F1 25"),
            _ => (Ending::Quit, "the game quit"),
        };
        SessionEnd {
            end,
            exit_code: Some(code),
            crash_code: None,
            reason: reason.into(),
        }
        .write(&self.session)
        .unwrap();
        std::fs::write(self.session.join("exit_code"), format!("{code}\n")).unwrap();
        self.done = true;
        true
    }

    fn stop(&mut self) {
        self.stops += 1;
        stop_process(self.child.id()).ok();
    }

    fn kill(&mut self) {
        self.child.kill().ok();
    }

    fn session(&mut self) -> Option<PathBuf> {
        self.done.then(|| self.session.clone())
    }
}

#[cfg(unix)]
const BOOT: &str = r##"
L="$GAME/swofty/loader.log"; mkdir -p "$GAME/swofty"
echo "[10:00:00.000] swofty loader (version.dll proxy) attached, pid $$" > "$L"
echo "[10:00:00.100] [my] hello" >> "$L"
trap 'echo "[10:00:09.000] [core] closing" >> "$L"; exit 0' TERM
sleep 0.3
echo '[10:00:01.000] [career] flow "#957880c6" Entering -> Active' >> "$L"
echo "[10:00:01.100] [events] game.ready" >> "$L"
echo "frame times: p50 8.00 ms, p95 12.00 ms, p99 30.00 ms, max 90 ms" > "$GAME/swofty/perf.log"
while :; do sleep 0.05; done
"##;

#[cfg(unix)]
const CRASH: &str = r##"
L="$GAME/swofty/loader.log"; mkdir -p "$GAME/swofty"
echo "[10:00:00.000] swofty loader (version.dll proxy) attached, pid $$" > "$L"
echo '[10:00:01.000] [career] flow "#957880c6" Entering -> Active' >> "$L"
sleep 0.3
echo "[10:00:02.000] [crash] unhandled exception 0xc0000005" >> "$L"
exit 3
"##;

#[cfg(unix)]
const BENCH: &str = r##"
L="$GAME/swofty/loader.log"; mkdir -p "$GAME/swofty"
echo "$*" > "$GAME/args.txt"
cp "$GAME/benchmark/swofty_test.xml" "$GAME/seen.xml"
echo "[10:00:00.000] swofty loader (version.dll proxy) attached, pid $$" > "$L"
sleep 0.2
echo "[10:00:25.000] [events] race.start (bahrain, 1 lap)" >> "$L"
echo "frame times: p50 8.00 ms, p95 12.00 ms, p99 22.50 ms, max 90 ms" > "$GAME/swofty/perf.log"
sleep 0.2
exit 0
"##;

#[cfg(unix)]
const STUCK: &str = r##"
L="$GAME/swofty/loader.log"; mkdir -p "$GAME/swofty"
echo "[10:00:00.000] swofty loader (version.dll proxy) attached, pid $$" > "$L"
trap 'exit 0' TERM
while :; do sleep 0.05; done
"##;

#[cfg(unix)]
const SUITE: &str = r#"{
  "schema": 1,
  "suite": "me.fake",
  "tests": [
    { "id": "boot_ok", "env": { "kind": "boot" },
      "steps": [ { "wait": { "event": "game.ready" }, "timeout_s": 10 } ],
      "assert": [ { "log": "\\[my\\] hello" }, { "event": "game.ready", "count": 1 }, { "no_crash": true },
                  { "perf": { "p99_ms": { "max": 40 } } } ] },
    { "id": "boot_fail", "env": { "kind": "boot" }, "assert": [ { "log": "\\[my\\] goodbye" } ] },
    { "id": "needs_agent", "env": { "kind": "boot" }, "steps": [ { "route": "contracts" } ] },
    { "id": "crashes", "fresh": true, "env": { "kind": "boot" },
      "steps": [ { "wait": { "log": "never" }, "timeout_s": 10 } ], "assert": [ { "no_crash": true } ] },
    { "id": "race", "env": { "kind": "benchmark", "track": "bahrain", "laps": 1, "cars": 4 },
      "steps": [ { "wait": { "event": "race.start" }, "timeout_s": 10 } ],
      "assert": [ { "event": "race.start" }, { "no_crash": true }, { "perf": { "p99_ms": { "max": 20 } } } ] },
    { "id": "career", "env": { "kind": "career", "fixture": "fixtures/f" }, "assert": [ { "no_crash": true } ] }
  ]
}"#;

fn quick() -> Times {
    Times {
        start: Duration::from_secs(10),
        step: Duration::from_secs(5),
        test: Duration::from_secs(20),
        race: Duration::from_secs(10),
        finish: Duration::from_secs(10),
        restop: Duration::from_secs(2),
        poll: Duration::from_millis(20),
    }
}

#[cfg(unix)]
fn status_of<'r>(r: &'r Report, id: &str) -> &'r TestReport {
    r.tests.iter().find(|t| t.id == id).unwrap()
}

#[cfg(unix)]
#[test]
fn a_fake_game_runs_every_batch_and_puts_the_folder_back() {
    let root = temp("fake-run");
    let (data, game, out) = (root.join("data"), root.join("game"), root.join("out"));
    std::fs::create_dir_all(&game).unwrap();
    let store = Store::new(&data);
    let s = suite::parse(SUITE, Path::new("tests/fake.test.json"), None).unwrap();
    let plan = plan::plan(&[s], &Selector::default());
    assert_eq!(plan.game_starts(), 3);
    let mut fake = Fake {
        scripts: vec![BOOT, CRASH, BENCH],
        root: root.clone(),
        game: game.clone(),
        launches: Vec::new(),
        envs: Vec::new(),
    };
    let mut report = Report::new("r1", &out.display().to_string());
    report.game_starts = plan.game_starts();
    Harness {
        store: &store,
        game: &game,
        out: &out,
        profile: "test-r1",
        launcher: &mut fake,
        times: quick(),
        run_id: "r1",
        bases: BTreeMap::new(),
        exe_sha: None,
        live_saves: None,
    }
    .run(&plan, &mut report);
    report.exit_code = report.compute_exit();

    let summary = report.summary();
    assert_eq!(
        status_of(&report, "boot_ok").status,
        Status::Pass,
        "{summary}\n{:#?}",
        report.tests
    );
    let fail = status_of(&report, "boot_fail");
    assert_eq!(fail.status, Status::Fail);
    assert_eq!(
        fail.failures[0].text(),
        "expected log /\\[my\\] goodbye/ at least 1 time(s), got 0 times"
    );
    assert_eq!(fail.failures[0].evidence.as_deref(), Some("batches/1/loader.log"));
    assert_eq!(status_of(&report, "needs_agent").status, Status::Skipped);
    assert_eq!(
        status_of(&report, "needs_agent").reason.as_deref(),
        Some("needs the test agent")
    );
    assert_eq!(
        status_of(&report, "career").reason.as_deref(),
        Some(career::NEEDS)
    );

    let crash = status_of(&report, "crashes");
    assert_eq!(crash.status, Status::Fail);
    assert_eq!(
        crash.failures[0].text(),
        "expected a log line /never/, got none before the game exited"
    );
    assert_eq!(
        crash.failures[1].text(),
        "expected no crash, got the game crashed (exception 0xc0000005 in F1_25.exe)"
    );
    let race = status_of(&report, "race");
    assert_eq!(race.status, Status::Fail);
    assert_eq!(race.failures.len(), 1, "{:?}", race.failures);
    assert_eq!(
        race.failures[0].text(),
        "expected p99 frame time at most 20 ms, got 22.5 ms"
    );

    assert_eq!(
        fake.launches.len(),
        3,
        "the agent and career batches start no game"
    );
    assert_eq!(fake.launches[0], ("test-r1".to_owned(), vec![]));
    assert_eq!(fake.launches[2].1, ["-benchmark", "swofty_test.xml"]);
    assert_eq!(
        std::fs::read_to_string(game.join("args.txt")).unwrap().trim(),
        "-benchmark swofty_test.xml"
    );
    let seen = std::fs::read_to_string(game.join("seen.xml")).unwrap();
    assert!(
        seen.contains("<track name=\"bahrain\" laps=\"1\" weather=\"clear\" num_cars=\"4\""),
        "{seen}"
    );
    assert!(
        !game.join("benchmark").exists(),
        "the staged XML and its folder are gone"
    );

    let batches: Vec<(&str, bool, Option<bool>)> = report
        .batches
        .iter()
        .map(|b| (b.env.as_str(), b.started_game, b.clean))
        .collect();
    assert_eq!(
        batches,
        [
            ("boot", true, Some(true)),
            ("boot", true, Some(true)),
            ("benchmark bahrain 1 lap 4 cars", true, Some(true)),
            ("career fixtures/f", false, None),
        ]
    );
    assert_eq!(
        report.batches[0].end.as_deref(),
        Some("stopped: the test run closed F1 25")
    );
    assert!(
        report.batches[1]
            .notes
            .iter()
            .any(|n| n.contains("the game crashed"))
    );
    assert!(out.join("batches/1/loader.log").is_file());
    assert!(out.join("batches/3/perf.log").is_file());
    assert!(out.join("batches/1/launcher.log").is_file());
    let profile = store.profile("test-r1").unwrap();
    assert_eq!(
        profile.enabled().collect::<Vec<_>>(),
        [CORE_ID],
        "only the suite's addons, none here"
    );
    assert_eq!(report.exit_code, exit::FAIL);
    assert!(
        summary.contains("2 passed") || summary.contains("1 passed"),
        "{summary}"
    );
    assert!(summary.contains("boot_ok"), "{summary}");
    std::fs::remove_dir_all(&root).ok();
}

#[cfg(unix)]
#[test]
fn a_game_that_never_reaches_the_menu_is_an_error_and_is_closed() {
    let root = temp("fake-stuck");
    let (data, game, out) = (root.join("data"), root.join("game"), root.join("out"));
    std::fs::create_dir_all(&game).unwrap();
    let store = Store::new(&data);
    let s = suite::parse(
        r#"{"schema":1,"suite":"s","tests":[{"id":"menu","env":{"kind":"boot"},"assert":[{"no_crash":true}]}]}"#,
        Path::new("s.test.json"),
        None,
    )
    .unwrap();
    let plan = plan::plan(&[s], &Selector::default());
    let mut fake = Fake {
        scripts: vec![STUCK],
        root: root.clone(),
        game: game.clone(),
        launches: Vec::new(),
        envs: Vec::new(),
    };
    let mut report = Report::new("r2", "out");
    let mut times = quick();
    times.start = Duration::from_millis(400);
    Harness {
        store: &store,
        game: &game,
        out: &out,
        profile: "test-r2",
        launcher: &mut fake,
        times,
        run_id: "r2",
        bases: BTreeMap::new(),
        exe_sha: None,
        live_saves: None,
    }
    .run(&plan, &mut report);
    let t = &report.tests[0];
    assert_eq!(t.status, Status::Error);
    assert_eq!(
        t.reason.as_deref(),
        Some("the batch did not start: the game did not reach the main menu within 0 s")
    );
    assert_eq!(
        report.batches[0].end.as_deref(),
        Some("stopped: the test run closed F1 25")
    );
    assert_eq!(report.batches[0].clean, Some(true));
    assert_eq!(report.compute_exit(), exit::INFRA);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_missing_addon_stops_the_batch_before_the_game_starts() {
    let root = temp("fake-addon");
    let (data, game, out) = (root.join("data"), root.join("game"), root.join("out"));
    std::fs::create_dir_all(&game).unwrap();
    let store = Store::new(&data);
    let s = suite::parse(
        r#"{"schema":1,"suite":"s","defaults":{"addons":["not.here"]},
            "tests":[{"id":"menu","env":{"kind":"benchmark","track":"monza"},"assert":[{"no_crash":true}]}]}"#,
        Path::new("s.test.json"),
        None,
    )
    .unwrap();
    let plan = plan::plan(&[s], &Selector::default());
    assert!(
        addons_installed(&store, &plan)
            .unwrap_err()
            .contains("not.here is not installed")
    );
    let mut fake = Fake {
        scripts: vec![],
        root: root.clone(),
        game: game.clone(),
        launches: Vec::new(),
        envs: Vec::new(),
    };
    let mut report = Report::new("r3", "out");
    Harness {
        store: &store,
        game: &game,
        out: &out,
        profile: "test-r3",
        launcher: &mut fake,
        times: quick(),
        run_id: "r1",
        bases: BTreeMap::new(),
        exe_sha: None,
        live_saves: None,
    }
    .run(&plan, &mut report);
    assert!(fake.launches.is_empty());
    assert_eq!(report.tests[0].status, Status::Error);
    assert!(
        report.batches[0]
            .error
            .as_deref()
            .unwrap()
            .contains("not.here is not installed"),
        "{:?}",
        report.batches[0].error
    );
    assert!(!game.join("benchmark").exists(), "nothing staged");
    assert_eq!(report.compute_exit(), exit::INFRA);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_held_lock_is_busy_and_unaccepted_terms_are_infra() {
    let root = temp("lock-busy");
    let dir = root.join("game.lock.d");
    let held = GameLock::try_acquire(&dir, "someone else 10:00")
        .unwrap()
        .unwrap();
    match take_lock(&dir, "swofty-test r s 10:01", Duration::ZERO) {
        Err(Stop::Busy(why)) => assert!(why.contains("held by \"someone else 10:00\""), "{why}"),
        other => panic!("{other:?}"),
    }
    assert!(dir.is_dir());
    drop(held);
    let lock = take_lock(&dir, "swofty-test r s 10:01", Duration::ZERO).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.join("owner")).unwrap(),
        "swofty-test r s 10:01\n"
    );
    drop(lock);
    match take_lock(&root.join("missing/x.lock.d"), "x", Duration::ZERO) {
        Err(Stop::Infra(why)) => assert!(why.contains("SWOFTY_GAME_LOCK"), "{why}"),
        other => panic!("{other:?}"),
    }
    let store = Store::new(root.join("data"));
    let why = terms(&store).unwrap_err();
    assert!(why.contains("a test run never accepts them for you"), "{why}");
    swofty_core::offline::record_accept(&store.root).unwrap();
    assert!(terms(&store).is_ok());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn waits_parse_as_seconds_minutes_or_hours() {
    assert_eq!(parse_wait("0"), Ok(Duration::ZERO));
    assert_eq!(parse_wait("90s"), Ok(Duration::from_secs(90)));
    assert_eq!(parse_wait("30m"), Ok(Duration::from_secs(1800)));
    assert_eq!(parse_wait("2h"), Ok(Duration::from_secs(7200)));
    assert!(parse_wait("soon").is_err());
    assert!(parse_wait("5d").is_err());
}

#[test]
fn the_cli_takes_the_documented_forms() {
    let parse = |args: &[&str]| crate::Cli::try_parse_from(args).map(|c| c.command);
    let Ok(crate::Command::Test(TestCommand::Run(a))) = parse(&[
        "swofty",
        "test",
        "run",
        "./my-addon",
        "-t",
        "smoke",
        "--take-over",
        "--wait",
        "0",
    ]) else {
        panic!("test run");
    };
    assert_eq!(a.select.paths, [PathBuf::from("./my-addon")]);
    assert_eq!(a.select.tags, ["smoke"]);
    assert!(a.take_over);
    assert_eq!(a.wait, Duration::ZERO);
    let Ok(crate::Command::Test(TestCommand::Run(a))) = parse(&["swofty", "test", "run", "x"]) else {
        panic!("defaults");
    };
    assert_eq!(
        a.wait,
        Duration::from_secs(1800),
        "a run waits for the game by default"
    );
    let Ok(crate::Command::Test(TestCommand::Plan(p))) =
        parse(&["swofty", "test", "plan", "./my-addon", "--id", "a", "--id", "s/b"])
    else {
        panic!("test plan");
    };
    assert_eq!(p.select.ids, ["a", "s/b"]);
    assert!(parse(&["swofty", "test", "plan"]).is_err(), "a path is required");
    assert!(
        parse(&["swofty", "run", "--game-arg=-benchmark"]).is_err(),
        "game arguments need a direct launch"
    );
    assert!(
        parse(&[
            "swofty",
            "run",
            "--direct",
            "--game-arg=-benchmark",
            "--game-arg",
            "x.xml"
        ])
        .is_ok()
    );
    crate::Cli::command().debug_assert();
}

#[test]
fn a_run_of_only_skipped_tests_starts_nothing_and_passes() {
    let root = temp("skipped-only");
    let addon = root.join("addon");
    std::fs::create_dir_all(addon.join("tests")).unwrap();
    std::fs::write(
        addon.join("tests/a.test.json"),
        r#"{"schema":1,"suite":"s","tests":[{"id":"c","env":{"kind":"career","fixture":"f"},"assert":[{"no_crash":true}]},
            {"id":"r","env":{"kind":"route","routes":["career"]},"assert":[{"no_crash":true}]}]}"#,
    )
    .unwrap();
    let out = root.join("out");
    let store = Store::new(root.join("data"));
    let cli = crate::Cli::try_parse_from([
        "swofty",
        "test",
        "run",
        addon.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--lock",
        root.join("never/used.lock.d").to_str().unwrap(),
    ])
    .unwrap();
    let crate::Command::Test(cmd) = cli.command else {
        panic!("test");
    };
    assert_eq!(command(&store, cmd).unwrap(), ExitCode::from(exit::PASS));
    let r: Report = serde_json::from_str(&std::fs::read_to_string(out.join("report.json")).unwrap()).unwrap();
    assert_eq!(r.game_starts, 0);
    assert!(r.tests.iter().all(|t| t.status == Status::Skipped));
    assert!(out.join("junit.xml").is_file() && out.join("summary.txt").is_file());
    assert!(
        !root.join("never").exists(),
        "no lock is taken when no game starts"
    );

    let none = crate::Cli::try_parse_from(["swofty", "test", "plan", addon.to_str().unwrap(), "-t", "nope"])
        .unwrap();
    let crate::Command::Test(cmd) = none.command else {
        panic!("test");
    };
    assert_eq!(command(&store, cmd).unwrap(), ExitCode::from(exit::INFRA));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn the_owner_line_has_a_clock_time() {
    let t = hh_mm();
    assert_eq!(t.len(), 5, "{t}");
    assert_eq!(&t[2..3], ":");
}

const CAREER_SUITE: &str = r#"{
  "schema": 1,
  "suite": "my.career",
  "tests": [
    { "id": "hq", "env": { "kind": "career", "fixture": "fixtures/r3", "at": "team_hq" }, "fn": "hq" }
  ]
}"#;

fn sandbox_bindings(loader: &Path, sha: &str, ids: &[&str]) {
    let core = loader.join("swofty/addons/core");
    std::fs::create_dir_all(&core).unwrap();
    let b: Vec<serde_json::Value> = ids
        .iter()
        .map(|id| serde_json::json!({"id": id, "kind": "func", "status": "experimental"}))
        .collect();
    std::fs::write(
        core.join("bindings.json"),
        serde_json::to_vec(&serde_json::json!({"build": {"sha256": sha}, "bindings": b})).unwrap(),
    )
    .unwrap();
}

fn fixture_at(tests_dir: &Path, name: &str, sha: &str) {
    let remote = tests_dir.join("captured-remote");
    std::fs::create_dir_all(&remote).unwrap();
    std::fs::write(remote.join(fixture::PROFILE_FILES[0]), [7u8; 900]).unwrap();
    std::fs::write(remote.join(fixture::PROFILE_FILES[1]), [8u8; 416]).unwrap();
    let meta = fixture::Meta {
        exe: fixture::Exe {
            sha256: sha.into(),
            version: None,
        },
        created: "now".into(),
        ..Default::default()
    };
    fixture::capture(name, &tests_dir.join("fixtures"), &remote, &[], meta).unwrap();
}

#[test]
fn a_career_run_is_refused_unless_the_sandbox_is_verified_for_the_exe() {
    let root = temp("sandbox-preflight");
    let tests_dir = root.join("addon/tests");
    std::fs::create_dir_all(&tests_dir).unwrap();
    let file = tests_dir.join("c.test.json");
    let s = suite::parse(CAREER_SUITE, &file, None).unwrap();
    let suites = vec![s];
    let plan = plan::plan(&suites, &Selector::default());
    let batches: Vec<&Batch> = plan.batches.iter().collect();
    let b = bases(&suites);
    let store = root.join("store");
    let none = check_sandbox(None, "aa", &batches, &b, &store).unwrap_err();
    assert!(none.contains("loader"), "{none}");
    let loader = root.join("loader");
    sandbox_bindings(&loader, "aa", &["Ne_SaveSystem2Win_Start"]);
    let lacking = check_sandbox(Some(&loader), "aa", &batches, &b, &store).unwrap_err();
    assert!(
        lacking.starts_with("a career environment needs the save sandbox")
            && lacking.contains("lack Ne_SaveSystem2Win_Mount"),
        "{lacking}"
    );
    sandbox_bindings(&loader, "aa", sandbox::BINDINGS);
    let other = check_sandbox(Some(&loader), "bb", &batches, &b, &store).unwrap_err();
    assert!(other.contains("not verified for this build"), "{other}");
    let missing = check_sandbox(Some(&loader), "aa", &batches, &b, &store).unwrap_err();
    assert!(missing.contains("fixture \"fixtures/r3\" not found"), "{missing}");
    fixture_at(&tests_dir, "r3", "aa");
    check_sandbox(Some(&loader), "aa", &batches, &b, &store).unwrap();
    std::fs::write(
        tests_dir.join("fixtures/r3/save").join(fixture::PROFILE_FILES[1]),
        [0u8; 416],
    )
    .unwrap();
    let edited = check_sandbox(Some(&loader), "aa", &batches, &b, &store).unwrap_err();
    assert!(edited.contains("does not match fixture.json"), "{edited}");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_career_batch_runs_in_its_own_sandbox_and_fails_closed_on_any_doubt() {
    let root = temp("sandbox-batch");
    let (data, game, out) = (root.join("data"), root.join("game"), root.join("out"));
    std::fs::create_dir_all(&game).unwrap();
    let tests_dir = root.join("addon/tests");
    std::fs::create_dir_all(&tests_dir).unwrap();
    fixture_at(&tests_dir, "r3", "aa");
    let storage = tests_dir.join("fixtures/r3/storage/my.addon");
    std::fs::create_dir_all(&storage).unwrap();
    std::fs::write(storage.join("careers.json"), "{}").unwrap();
    let f = tests_dir.join("fixtures/r3/fixture.json");
    let mut m: fixture::Manifest = serde_json::from_slice(&std::fs::read(&f).unwrap()).unwrap();
    m.files.insert(
        "storage/my.addon/careers.json".into(),
        fixture::FileSum {
            size: 2,
            sha256: swofty_core::baseline::sha256_bytes(b"{}"),
        },
    );
    std::fs::write(&f, serde_json::to_vec(&m).unwrap()).unwrap();
    let s = suite::parse(CAREER_SUITE, &tests_dir.join("c.test.json"), None).unwrap();
    let suites = vec![s];
    let plan = plan::plan(&suites, &Selector::default());
    let live = root.join("live-remote");
    std::fs::create_dir_all(&live).unwrap();
    std::fs::write(live.join(fixture::PROFILE_FILES[1]), [1u8; 416]).unwrap();
    let store = Store::new(&data);
    let mut fake = Fake {
        scripts: Vec::new(),
        root: root.clone(),
        game: game.clone(),
        launches: Vec::new(),
        envs: Vec::new(),
    };
    let h = Harness {
        store: &store,
        game: &game,
        out: &out,
        profile: "test-r9",
        launcher: &mut fake,
        times: quick(),
        run_id: "r9",
        bases: bases(&suites),
        exe_sha: Some("aa".into()),
        live_saves: Some(live.clone()),
    };
    let b = &plan.batches[0];
    let sb = h.open_sandbox(1, b, "test-r9-1").unwrap();
    assert_eq!(sb.run.root, data.join("test-runs/r9-1"));
    assert_eq!(
        std::fs::read(sb.run.remote().join(fixture::PROFILE_FILES[1])).unwrap(),
        [8u8; 416],
        "the fixture's saves are in the sandbox"
    );
    assert_eq!(
        std::fs::read_to_string(
            addon_storage::profile_dir(&data, "my.addon", "test-r9-1").join("careers.json")
        )
        .unwrap(),
        "{}",
        "the fixture's storage is the batch profile's copy"
    );
    let env = sb.run.env();
    assert!(
        env.iter()
            .any(|(k, v)| k == sandbox::RUN_ENV && Path::new(v) == sb.run.root)
    );
    assert!(env.iter().any(|(k, v)| k == sandbox::TOKEN_ENV && v.len() == 32));

    let mut br = Harness::blank(b, 1).0;
    br.started_game = true;
    let mut reports = Harness::blank(b, 1).1;
    reports[0].status = Status::Pass;
    h.close_sandbox(sb, b, "test-r9-1", &mut br, &mut reports, &[0]);
    assert_eq!(
        br.error.as_deref(),
        Some(
            "the core never reported the save sandbox (no sandbox.json): the save device may not have started"
        )
    );
    assert_eq!(reports[0].status, Status::Error);
    let doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(data.join("test-runs/r9-1/run.json")).unwrap()).unwrap();
    assert!(
        doc.get("token").is_none(),
        "the run token is retired after the batch"
    );

    let sb = h.open_sandbox(2, b, "test-r9-2").unwrap();
    std::fs::write(
        sb.run.root.join(sandbox::STATUS_FILE),
        r#"{"state":"failed","why":"Ne_SaveSystem2Win_Start does not resolve on this build"}"#,
    )
    .unwrap();
    let mut br = Harness::blank(b, 2).0;
    let mut reports = Harness::blank(b, 2).1;
    h.close_sandbox(sb, b, "test-r9-2", &mut br, &mut reports, &[0]);
    assert_eq!(
        br.error.as_deref(),
        Some("the save sandbox did not arm: Ne_SaveSystem2Win_Start does not resolve on this build")
    );

    let sb = h.open_sandbox(3, b, "test-r9-3").unwrap();
    std::fs::write(sb.run.root.join(sandbox::STATUS_FILE), r#"{"state":"armed"}"#).unwrap();
    std::fs::write(sb.run.remote().join("savegame@profile#QKRHMYXE"), [5u8; 416]).unwrap();
    let mut br = Harness::blank(b, 3).0;
    let mut reports = Harness::blank(b, 3).1;
    reports[0].status = Status::Pass;
    h.close_sandbox(sb, b, "test-r9-3", &mut br, &mut reports, &[0]);
    assert_eq!(
        br.error, None,
        "an armed sandbox and an unchanged live folder pass"
    );
    assert_eq!(reports[0].status, Status::Pass);
    assert_eq!(
        std::fs::read_to_string(data.join("test-runs/r9-3/storage/my.addon/careers.json")).unwrap(),
        "{}",
        "the batch's addon storage is kept in the run folder for a capture"
    );

    let sb = h.open_sandbox(4, b, "test-r9-4").unwrap();
    std::fs::write(sb.run.root.join(sandbox::STATUS_FILE), r#"{"state":"armed"}"#).unwrap();
    std::fs::write(live.join(fixture::PROFILE_FILES[1]), [2u8; 417]).unwrap();
    let mut br = Harness::blank(b, 4).0;
    let mut reports = Harness::blank(b, 4).1;
    h.close_sandbox(sb, b, "test-r9-4", &mut br, &mut reports, &[0]);
    let why = br.error.unwrap();
    assert!(
        why.starts_with("THE PLAYER'S STEAM SAVE FOLDER CHANGED")
            && why.contains("savegame@profile#QKRHMYXE changed"),
        "{why}"
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_capture_from_a_run_records_the_runs_exe_and_addons() {
    let root = temp("capture-run");
    let data = root.join("data");
    let store = Store::new(&data);
    let run = RunDir::create(&data, "r5-1", Some("cafe")).unwrap();
    std::fs::write(run.remote().join(fixture::PROFILE_FILES[0]), [1u8; 10]).unwrap();
    std::fs::write(run.remote().join(fixture::PROFILE_FILES[1]), [2u8; 416]).unwrap();
    std::fs::write(run.root.join("addons.json"), r#"{"swofty.progressive":"1.6.0"}"#).unwrap();
    std::fs::create_dir_all(run.storage().join("swofty.progressive")).unwrap();
    std::fs::write(run.storage().join("swofty.progressive/careers.json"), "{}").unwrap();
    let fixtures = root.join("fixtures");
    let cli = crate::Cli::try_parse_from([
        "swofty",
        "test",
        "fixture",
        "capture",
        "prog-r1",
        "--run",
        "r5-1",
        "--career",
        "driver_career_progressive:2",
        "--store",
        fixtures.to_str().unwrap(),
    ])
    .unwrap();
    let crate::Command::Test(TestCommand::Fixture(FixtureCommand::Capture(a))) = cli.command else {
        panic!("not a capture");
    };
    assert_eq!(capture_command(&store, a).unwrap(), ExitCode::SUCCESS);
    let f = fixture::load(&fixtures.join("prog-r1")).unwrap();
    assert_eq!(f.manifest.exe.sha256, "cafe");
    assert_eq!(f.manifest.addons["swofty.progressive"], "1.6.0");
    assert_eq!(f.manifest.holds.careers[0].slot, Some(2));
    assert!(
        f.manifest
            .files
            .contains_key("storage/swofty.progressive/careers.json")
    );
    assert_eq!(f.manifest.source, "test run r5-1");
    std::fs::remove_dir_all(&root).ok();
}
