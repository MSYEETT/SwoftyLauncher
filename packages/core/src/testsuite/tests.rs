use std::path::{Path, PathBuf};
use std::time::Duration;

use super::checks::{self, Evidence};
use super::lock::{self, GameLock};
use super::plan;
use super::report::{BatchReport, Report, Status, TestReport, exit};
use super::suite::{self, Check, Count, Kind, Selector, Wait};

const DOC_EXAMPLE: &str = r#"{
  "schema": 1,
  "suite": "swofty.rnd-tyres-thermal",
  "tests": [
    {
      "id": "tyre_temps_rise_on_track",
      "tags": ["smoke"],
      "env": { "kind": "benchmark", "track": "bahrain", "laps": 3, "cars": 10 },
      "steps": [
        { "wait": { "event": "race.start" }, "timeout_s": 90 },
        { "sample": { "read": "car[0].tyres[fl].temperature", "every_ms": 500, "for_s": 60, "as": "fl" } }
      ],
      "assert": [
        { "series": "fl", "rises_by": 8 },
        { "log": "\\[rnd-tyres\\] thermal model on" },
        { "no_crash": true }
      ]
    },
    {
      "id": "rnd_tab_hidden_in_team_hq",
      "env": { "kind": "career", "fixture": "fixtures/my-team-r3", "at": "team_hq" },
      "fn": "rnd_tab_hidden"
    }
  ]
}"#;

fn parse(text: &str) -> Result<suite::Suite, String> {
    suite::parse(text, Path::new("tests/x.test.json"), Some("my.addon")).map_err(|e| e.0)
}

fn one_test(test: &str) -> Result<suite::Test, String> {
    parse(&format!(r#"{{"schema":1,"suite":"s","tests":[{test}]}}"#)).map(|s| s.tests[0].clone())
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("swofty-testsuite-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_documented_test_file_parses() {
    let s = parse(DOC_EXAMPLE).unwrap();
    assert_eq!(s.id, "swofty.rnd-tyres-thermal");
    assert_eq!(s.tests.len(), 2);
    let t = &s.tests[0];
    assert_eq!(t.env.kind, Kind::Benchmark);
    assert_eq!(t.env.str("track"), Some("bahrain"));
    assert_eq!(t.steps[0].host, Some(Wait::Event("race.start".into())));
    assert_eq!(t.steps[0].timeout_s, Some(90));
    assert_eq!(t.steps[1].host, None, "sample needs the agent");
    assert!(matches!(&t.checks[1], Check::Log { count, .. } if *count == Count { min: Some(1), max: None }));
    assert_eq!(t.checks[2], Check::NoCrash);
    assert_eq!(t.addons, ["my.addon"], "the addon whose folder holds the tests");
    assert_eq!(t.skip_reason(), Some(suite::NEEDS_AGENT));
    assert_eq!(s.tests[1].skip_reason(), Some("needs the save sandbox"));
}

#[test]
fn every_documented_step_and_check_parses() {
    let steps = [
        r#"{ "wait": { "event": "career.season.start" }, "timeout_s": 120 }"#,
        r#"{ "route": "contracts" }"#,
        r#"{ "skip": "session" }"#,
        r#"{ "advance": { "days": 7 } }"#,
        r#"{ "ai_drives_player": true }"#,
        r#"{ "pad": { "press": "mfd_fuel", "then": "right" } }"#,
        r#"{ "set": { "car[player].f1_aero.lift_scale": 1.05 } }"#,
        r#"{ "read": "career.money", "as": "money_before" }"#,
        r#"{ "capture": "frame", "name": "team_hq" }"#,
        r#"{ "save": "now" }"#,
    ];
    let checks = [
        r#"{ "log": "\\[career\\] season 2019 starts", "min": 1 }"#,
        r#"{ "value": "car[0].chassis.mass", "between": [790, 830] }"#,
        r#"{ "value": "money_after", "greater_than": "money_before" }"#,
        r#"{ "event": "car.reset", "count": 2 }"#,
        r#"{ "no_crash": true }"#,
        r#"{ "perf": { "p99_ms": { "max": 40 } } }"#,
    ];
    let t = one_test(&format!(
        r#"{{"id":"all","env":{{"kind":"boot"}},"steps":[{}],"assert":[{}]}}"#,
        steps.join(","),
        checks.join(",")
    ))
    .unwrap();
    assert_eq!(t.steps.len(), steps.len());
    assert_eq!(t.checks.len(), checks.len());
    assert_eq!(t.steps.iter().filter(|s| s.host.is_some()).count(), 1);
    assert!(matches!(
        t.checks[3],
        Check::Event {
            count: Count {
                min: Some(2),
                max: Some(2)
            },
            ..
        }
    ));
    assert_eq!(
        t.checks[5],
        Check::Perf {
            p99_max: Some(40.0),
            p99_min: None
        }
    );
    for env in [
        r#"{ "kind": "benchmark", "track": "monaco", "laps": 2, "cars": 20, "weather": "rain", "box": "swofty.classic-seasons-2017/f1_2017" }"#,
        r#"{ "kind": "route", "routes": ["career", "team_hq", "contracts"] }"#,
        r#"{ "kind": "career", "fixture": "fixtures/progressive-2018-r10", "at": "weekend_hub" }"#,
        r#"{ "kind": "time_trial", "box": "mod.season-2008/f1_2008", "track": "venue/albert_park", "car": "mod.season-2008/ferrari" }"#,
        r#"{ "kind": "none" }"#,
    ] {
        one_test(&format!(
            r#"{{"id":"e","env":{env},"assert":[{{"no_crash":true}}]}}"#
        ))
        .unwrap();
    }
}

#[test]
fn unknown_keys_are_refused_with_where_they_are() {
    let cases = [
        (
            r#"{"schema":1,"suite":"s","tests":[],"test":[]}"#,
            "tests/x.test.json: unknown key \"test\" (expected one of: schema, suite, defaults, tests)",
        ),
        (
            r#"{"schema":1,"suite":"s","tests":[{"id":"a","env":{"kind":"boot"},"asert":[]}]}"#,
            "tests[0] (a): unknown key \"asert\"",
        ),
        (
            r#"{"schema":1,"suite":"s","tests":[{"id":"a","env":{"kind":"boot","track":"x"},"assert":[{"no_crash":true}]}]}"#,
            "tests[0] (a).env: unknown key \"track\" (a boot environment takes only \"kind\")",
        ),
        (
            r#"{"schema":1,"suite":"s","tests":[{"id":"a","env":{"kind":"benchmark","track":"x","lap":2},"assert":[{"no_crash":true}]}]}"#,
            "env: unknown key \"lap\" (expected one of: kind, track, laps, cars, weather, box, grid_pos, camera, driver)",
        ),
        (
            r#"{"schema":1,"suite":"s","tests":[{"id":"a","env":{"kind":"boot"},"steps":[{"wait":{"event":"x"},"timeot_s":3}]}]}"#,
            "tests[0] (a).steps[0]: unknown key \"timeot_s\" (expected one of: wait, timeout_s)",
        ),
        (
            r#"{"schema":1,"suite":"s","tests":[{"id":"a","env":{"kind":"boot"},"steps":[{"sample":{"read":"x","as":"y","evry_ms":5}}]}]}"#,
            "steps[0].sample: unknown key \"evry_ms\"",
        ),
        (
            r#"{"schema":1,"suite":"s","tests":[{"id":"a","env":{"kind":"boot"},"assert":[{"log":"x","mni":1}]}]}"#,
            "assert[0]: unknown key \"mni\" (expected one of: log, min, max, count)",
        ),
        (
            r#"{"schema":1,"suite":"s","tests":[{"id":"a","env":{"kind":"boot"},"assert":[{"perf":{"p99_ms":{"maximum":4}}}]}]}"#,
            "assert[0].perf.p99_ms: unknown key \"maximum\" (expected one of: max, min)",
        ),
    ];
    for (text, want) in cases {
        let err = parse(text).unwrap_err();
        assert!(err.contains(want), "{err}\n  wanted: {want}");
    }
}

#[test]
fn bad_values_say_what_was_expected() {
    let cases = [
        (
            r#"{"schema":2,"suite":"s","tests":[]}"#,
            "schema 2 is not supported",
        ),
        (r#"{"suite":"s","tests":[]}"#, "missing \"schema\": 1"),
        (
            r#"{"schema":1,"suite":"s","tests":[{"id":"a"}]}"#,
            "missing \"env\"",
        ),
        (
            r#"{"schema":1,"suite":"s","tests":[{"id":"a","env":{"kind":"race"},"assert":[]}]}"#,
            "unknown kind \"race\" (expected one of: none, boot, benchmark",
        ),
        (
            r#"{"schema":1,"suite":"s","tests":[{"id":"a","env":{"kind":"benchmark"},"assert":[]}]}"#,
            "a benchmark environment needs \"track\"",
        ),
        (
            r#"{"schema":1,"suite":"s","tests":[{"id":"a","env":{"kind":"benchmark","track":"x","cars":40},"assert":[]}]}"#,
            "env.cars: expected 1 to 26, got 40",
        ),
        (
            r#"{"schema":1,"suite":"s","tests":[{"id":"a","env":{"kind":"boot"}}]}"#,
            "a test needs \"steps\", \"assert\" or \"fn\"",
        ),
        (
            r#"{"schema":1,"suite":"s","tests":[{"id":"a","env":{"kind":"boot"},"assert":[{"log":"(unclosed"}]}]}"#,
            "assert[0].log: not a valid regular expression",
        ),
        (
            r#"{"schema":1,"suite":"s","tests":[{"id":"a","env":{"kind":"boot"},"assert":[{"log":"x","count":1,"min":1}]}]}"#,
            "\"count\" is an exact count",
        ),
        (
            r#"{"schema":1,"suite":"s","tests":[{"id":"a","env":{"kind":"boot"},"steps":[{"route":"a","skip":"session"}]}]}"#,
            "a step has one action, this has route and skip",
        ),
        (
            r#"{"schema":1,"suite":"s","tests":[{"id":"a","env":{"kind":"boot"},"assert":[{"no_crash":true}]},{"id":"a","env":{"kind":"boot"},"assert":[{"no_crash":true}]}]}"#,
            "test id \"a\" is used twice",
        ),
        (
            r#"{"schema":1,"suite":"s","tests":[{"id":"a b"}]}"#,
            "\"a b\" is not a valid id",
        ),
        ("{", "not valid JSON"),
    ];
    for (text, want) in cases {
        let err = parse(text).unwrap_err();
        assert!(err.contains(want), "{err}\n  wanted: {want}");
    }
}

#[test]
fn defaults_fill_what_a_test_leaves_out() {
    let s = parse(
        r#"{"schema":1,"suite":"s","defaults":{"timeout_s":60,"addons":["a.one"],"settings":{"a.one":{"x":1}}},
            "tests":[
              {"id":"t1","env":{"kind":"boot"},"assert":[{"no_crash":true}]},
              {"id":"t2","env":{"kind":"boot"},"timeout_s":5,"addons":["a.two"],"settings":{"a.one":{"y":2}},"assert":[{"no_crash":true}]}
            ]}"#,
    )
    .unwrap();
    assert_eq!(s.tests[0].timeout_s, Some(60));
    assert_eq!(s.tests[0].addons, ["a.one"]);
    assert_eq!(s.tests[1].timeout_s, Some(5));
    assert_eq!(s.tests[1].addons, ["a.two"]);
    assert_eq!(s.tests[1].settings["a.one"].len(), 2);
}

#[test]
fn loading_finds_an_addons_tests_folder() {
    let root = temp("load");
    let addon = root.join("my-addon");
    std::fs::create_dir_all(addon.join("tests")).unwrap();
    std::fs::write(addon.join("swofty.json"), r#"{"id":"me.addon"}"#).unwrap();
    std::fs::write(addon.join("tests/b.test.json"), DOC_EXAMPLE).unwrap();
    std::fs::write(
        addon.join("tests/a.test.json"),
        r#"{"schema":1,"suite":"me.smoke","tests":[{"id":"boots","tags":["smoke"],"env":{"kind":"boot"},"assert":[{"no_crash":true}]}]}"#,
    )
    .unwrap();
    std::fs::write(addon.join("tests/notes.json"), "not a test file").unwrap();
    let suites = suite::load(&addon).unwrap();
    assert_eq!(
        suites.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["me.smoke", "swofty.rnd-tyres-thermal"]
    );
    assert_eq!(suites[0].tests[0].addons, ["me.addon"]);
    let one = suite::load(&addon.join("tests/a.test.json")).unwrap();
    assert_eq!(
        one[0].tests[0].addons,
        ["me.addon"],
        "a single file finds its addon too"
    );
    let empty = root.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(suite::load(&empty).unwrap_err().0.contains("no test files"));
    assert!(
        suite::load(&root.join("nope"))
            .unwrap_err()
            .0
            .contains("no such file")
    );
    std::fs::write(addon.join("tests/c.test.json"), DOC_EXAMPLE).unwrap();
    assert!(
        suite::load(&addon)
            .unwrap_err()
            .0
            .contains("each suite id belongs to one file")
    );
    std::fs::remove_dir_all(&root).ok();
}

fn suite_with(tests: &[&str]) -> suite::Suite {
    parse(&format!(
        r#"{{"schema":1,"suite":"s","tests":[{}]}}"#,
        tests.join(",")
    ))
    .unwrap()
}

#[test]
fn tests_with_the_same_environment_share_a_game_start() {
    let s = suite_with(&[
        r#"{"id":"b1","env":{"kind":"benchmark","track":"bahrain","laps":3},"assert":[{"no_crash":true}]}"#,
        r#"{"id":"m1","tags":["smoke"],"env":{"kind":"boot"},"assert":[{"no_crash":true}]}"#,
        r#"{"id":"b2","env":{"laps":3,"track":"bahrain","kind":"benchmark"},"assert":[{"log":"x"}]}"#,
        r#"{"id":"m2","env":{"kind":"boot"},"assert":[{"log":"y"}]}"#,
        r#"{"id":"m3","env":{"kind":"boot"},"fresh":true,"assert":[{"log":"y"}]}"#,
        r#"{"id":"m4","env":{"kind":"boot"},"addons":["other"],"assert":[{"log":"y"}]}"#,
        r#"{"id":"r1","env":{"kind":"route","routes":["career"]},"assert":[{"no_crash":true}]}"#,
        r#"{"id":"c1","env":{"kind":"career","fixture":"f"},"assert":[{"no_crash":true}]}"#,
        r#"{"id":"a1","env":{"kind":"boot"},"steps":[{"route":"contracts"}],"assert":[{"log":"y"}]}"#,
    ]);
    let p = plan::plan(std::slice::from_ref(&s), &Selector::default());
    let shape: Vec<(String, Vec<&str>)> = p
        .batches
        .iter()
        .map(|b| (b.label(), b.tests.iter().map(|t| t.id.as_str()).collect()))
        .collect();
    assert_eq!(
        shape,
        [
            ("boot".to_owned(), vec!["m1", "m2", "a1"]),
            ("boot".to_owned(), vec!["m3"]),
            ("boot".to_owned(), vec!["m4"]),
            ("benchmark bahrain 3 laps".to_owned(), vec!["b1", "b2"]),
            ("route career".to_owned(), vec!["r1"]),
            ("career f".to_owned(), vec!["c1"]),
        ]
    );
    assert_eq!(p.tests(), 9);
    assert_eq!(
        p.game_starts(),
        4,
        "route and career batches only hold skipped tests"
    );
    assert_eq!(p.headline(), "9 tests in 4 game starts");
    let text = p.text();
    assert!(
        text.starts_with("swofty test plan: 9 tests in 4 game starts\n"),
        "{text}"
    );
    assert!(
        text.contains("  boot ............................. m1\n"),
        "{text}"
    );
    assert!(
        text.contains("a1                           skipped: needs the test agent"),
        "{text}"
    );
    assert!(
        text.contains("c1                           skipped: needs the save sandbox"),
        "{text}"
    );

    let smoke = Selector {
        tags: vec!["smoke".into()],
        ids: vec![],
    };
    let p = plan::plan(std::slice::from_ref(&s), &smoke);
    assert_eq!(p.tests(), 1);
    let by_id = Selector {
        tags: vec![],
        ids: vec!["s/b2".into(), "m3".into()],
    };
    let p = plan::plan(std::slice::from_ref(&s), &by_id);
    assert_eq!(p.tests(), 2);
    assert_eq!(p.game_starts(), 2);
    let json = serde_json::to_value(p.json()).unwrap();
    assert_eq!(json["game_starts"], 2);
}

#[test]
fn canonical_keys_ignore_key_order() {
    let a: serde_json::Value = serde_json::from_str(r#"{"b":1,"a":{"y":2,"x":[1,{"q":1,"p":2}]}}"#).unwrap();
    let b: serde_json::Value = serde_json::from_str(r#"{"a":{"x":[1,{"p":2,"q":1}],"y":2},"b":1}"#).unwrap();
    assert_eq!(plan::canonical(&a), plan::canonical(&b));
    assert_eq!(
        plan::canonical(&a),
        r#"{"a":{"x":[1,{"p":2,"q":1}],"y":2},"b":1}"#
    );
}

#[test]
fn the_benchmark_xml_has_the_race() {
    let t = one_test(
        r#"{"id":"b","env":{"kind":"benchmark","track":"monaco","laps":2,"cars":12,"weather":"rain","box":"a&b/c","grid_pos":4},"assert":[{"no_crash":true}]}"#,
    )
    .unwrap();
    let xml = super::bench::xml(&t.env);
    assert_eq!(
        xml,
        "<?xml version=\"1.0\" standalone=\"yes\" ?>\n<config infinite_loop=\"false\" show_fps=\"false\" >\n  \
         <track name=\"monaco\" laps=\"2\" weather=\"wet\" num_cars=\"12\" camera_mode=\"cycle\" grid_pos=\"4\" \
         setup_mode=\"a&amp;b/c\" />\n</config>\n"
    );
    let plain =
        one_test(r#"{"id":"b","env":{"kind":"benchmark","track":"bahrain"},"assert":[{"no_crash":true}]}"#)
            .unwrap();
    assert!(super::bench::xml(&plain.env).contains("laps=\"3\" weather=\"clear\" num_cars=\"20\""));
    assert_eq!(super::bench::args(), ["-benchmark", "swofty_test.xml"]);
}

const LOG: &str = "\
[14:13:47.100] swofty loader (version.dll proxy) attached, pid 25512
[14:13:50.000] [events] subscribe race.start by my.addon -> ok
[14:13:51.000] [rnd-tyres] thermal model on
[14:14:10.000] [career] flow \"#957880c6\" Entering -> Active
[14:14:12.000] [events] race.start (bahrain, 3 laps)
[14:14:13.000] [events] race.started_late
[14:14:14.000] [events] career.loaded has no replace phase (notify event); handler from x not attached
[14:15:00.000] [events] race.start
[14:15:01.000] [events] lap: 2
[14:15:02.000] [events] race.start hooked at base+0x7a7880
[14:15:02.000] [events] race.start signature matched 0 time(s); hook not installed
";

#[test]
fn log_and_event_checks_count_lines() {
    assert_eq!(checks::count_log(LOG, r"\[rnd-tyres\] thermal model on"), 1);
    assert_eq!(checks::count_log(LOG, r"\[events\]"), 8);
    assert_eq!(
        checks::count_event(LOG, "race.start"),
        2,
        "not the subscribe line or race.started_late"
    );
    assert_eq!(
        checks::count_event(LOG, "career.loaded"),
        0,
        "a refusal is not the event"
    );
    assert_eq!(checks::count_event(LOG, "lap"), 1);
    assert!(LOG.contains(checks::ready_line(Kind::Boot)));
    assert!(LOG.contains(checks::ready_line(Kind::None)));
    assert!(checks::waited(LOG, &Wait::Event("race.start".into())));
    assert!(!checks::waited(LOG, &Wait::Log("never".into())));

    let ev = Evidence {
        log: LOG.into(),
        log_path: Some("batches/1/loader.log".into()),
        ..Evidence::default()
    };
    let log = |pattern: &str, min, max| Check::Log {
        pattern: pattern.into(),
        count: Count { min, max },
    };
    assert!(checks::evaluate(&log(r"thermal model on", Some(1), None), &ev).is_ok());
    let f = checks::evaluate(&log(r"thermal model off", Some(1), None), &ev).unwrap_err();
    assert_eq!(
        f.text(),
        "expected log /thermal model off/ at least 1 time(s), got 0 times"
    );
    assert_eq!(f.evidence.as_deref(), Some("batches/1/loader.log"));
    let f = checks::evaluate(&log(r"\[events\]", None, Some(3)), &ev).unwrap_err();
    assert_eq!(f.actual, "8 times");
    let event = Check::Event {
        id: "race.start".into(),
        count: Count {
            min: Some(2),
            max: Some(2),
        },
    };
    assert!(checks::evaluate(&event, &ev).is_ok());
    assert!(checks::evaluate(&Check::NoCrash, &ev).is_ok());
    let crashed = Evidence {
        crash: Some("crash: exception 0xc0000005".into()),
        ..ev.clone()
    };
    let f = checks::evaluate(&Check::NoCrash, &crashed).unwrap_err();
    assert_eq!(f.text(), "expected no crash, got crash: exception 0xc0000005");
}

#[test]
fn perf_reads_the_p99_of_the_last_report() {
    let perf = "\
Swofty Launcher performance report (format 1)
== Summary
frame times: p50 8.10 ms, p95 12.00 ms, p99 31.50 ms, max 210 ms
== Summary
frame times: p50 8.10 ms, p95 12.00 ms, p99 22.25 ms, max 90 ms
";
    assert_eq!(checks::p99(perf), Some(22.25));
    assert_eq!(checks::p99("nothing"), None);
    let check = |max| Check::Perf {
        p99_max: Some(max),
        p99_min: None,
    };
    let ev = Evidence {
        perf: Some(perf.into()),
        ..Evidence::default()
    };
    assert!(checks::evaluate(&check(40.0), &ev).is_ok());
    let f = checks::evaluate(&check(20.0), &ev).unwrap_err();
    assert_eq!(f.text(), "expected p99 frame time at most 20 ms, got 22.25 ms");
    let f = checks::evaluate(&check(20.0), &Evidence::default()).unwrap_err();
    assert!(f.actual.contains("no perf.log report"), "{}", f.actual);
}

#[test]
fn the_lock_is_a_folder_with_an_owner_line() {
    let root = temp("lock");
    let dir = root.join("game.lock.d");
    let owner = lock::owner_line(
        "20261028-140200",
        &["swofty.a".into(), "swofty.b".into()],
        "14:02",
    );
    assert_eq!(owner, "swofty-test 20261028-140200 swofty.a,swofty.b 14:02");
    let held = GameLock::try_acquire(&dir, &owner).unwrap().unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.join("owner")).unwrap(),
        format!("{owner}\n")
    );
    let busy = GameLock::try_acquire(&dir, "someone else").unwrap().unwrap_err();
    assert_eq!(busy.owner.as_deref(), Some(owner.as_str()));
    assert!(busy.text(false).contains(&format!("is held by \"{owner}\"")));
    let mut heard = 0;
    let waited = GameLock::acquire(
        &dir,
        "x",
        Duration::from_millis(60),
        Duration::from_millis(10),
        |_| heard += 1,
    )
    .unwrap();
    assert!(waited.is_err(), "still held after the wait");
    assert_eq!(heard, 1, "told once while waiting");
    assert!(dir.is_dir(), "a busy lock is never cleared");
    held.release().unwrap();
    assert!(!dir.exists());

    let first = GameLock::try_acquire(&dir, "first").unwrap().unwrap();
    let releaser = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        drop(first);
    });
    let second = GameLock::acquire(
        &dir,
        "second",
        Duration::from_secs(10),
        Duration::from_millis(10),
        |_| {},
    )
    .unwrap()
    .unwrap();
    releaser.join().unwrap();
    assert_eq!(std::fs::read_to_string(dir.join("owner")).unwrap(), "second\n");
    drop(second);
    assert!(!dir.exists(), "dropping the lock releases it");

    std::fs::create_dir(&dir).unwrap();
    let anon = GameLock::try_acquire(&dir, "x").unwrap().unwrap_err();
    assert_eq!(anon.owner, None);
    let stale = lock::Busy {
        age: Some(lock::STALE_AFTER + Duration::from_secs(60)),
        ..anon.clone()
    };
    assert!(stale.text(false).contains("only its holder clears it"));
    assert!(!stale.text(true).contains("left over"));
    assert!(dir.is_dir());
    let missing_parent = root.join("no/such/dir.lock.d");
    assert!(GameLock::try_acquire(&missing_parent, "x").is_err());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn the_default_lock_follows_the_environment() {
    let data = Path::new("/home/me/.local/share/swofty");
    let path = lock::default_path(data);
    match std::env::var_os(lock::ENV).filter(|v| !v.is_empty()) {
        Some(v) => assert_eq!(path, PathBuf::from(v)),
        None if cfg!(target_os = "linux") && Path::new("/mnt/work").is_dir() => {
            assert_eq!(path, Path::new("/mnt/work/.f1-game.lock.d"))
        }
        None => assert_eq!(path, Path::new("/home/me/.local/share/swofty-game.lock.d")),
    }
}

fn report() -> Report {
    let mut r = Report::new("20261028-1402", "swofty-test/20261028-1402");
    r.game_starts = 2;
    r.ms = 125_000;
    let batch = |index: usize, env: &str| BatchReport {
        index,
        env: env.into(),
        key: env.into(),
        addons: vec![],
        started_game: true,
        ready_ms: None,
        ms: None,
        session: None,
        end: None,
        error: None,
        clean: Some(true),
        notes: vec![],
    };
    r.batches = vec![
        batch(1, "benchmark bahrain 3 laps"),
        batch(2, "career fixtures/my-team-r3"),
    ];
    let test = |id: &str, batch: usize, status: Status| TestReport {
        suite: "swofty.rnd-tyres-thermal".into(),
        id: id.into(),
        env: String::new(),
        batch,
        status,
        ms: None,
        reason: None,
        failures: vec![],
        steps: vec![],
    };
    let mut pass = test("tyre_temps_rise_on_track", 1, Status::Pass);
    pass.ms = Some(112_000);
    let mut fail = test("rnd_upgrade_applies", 1, Status::Fail);
    fail.ms = Some(3_000);
    fail.failures.push(checks::Failure {
        check: "log /x/".into(),
        expected: "log /\\[x\\] <on>/ at least 1 time(s)".into(),
        actual: "0 times".into(),
        evidence: Some("batches/1/loader.log".into()),
    });
    let mut skipped = test("rnd_tab_hidden_in_team_hq", 2, Status::Skipped);
    skipped.reason = Some("needs the save sandbox".into());
    r.tests = vec![pass, fail, skipped];
    r
}

#[test]
fn the_summary_reads_like_the_docs() {
    let r = report();
    assert_eq!(
        r.summary(),
        "swofty test: 3 tests in 2 game starts\n\
         \x20 benchmark bahrain 3 laps ......... tyre_temps_rise_on_track     pass   (1 m 52 s)\n\
         \x20                                    rnd_upgrade_applies          FAIL   expected log /\\[x\\] <on>/ at least 1 time(s), got 0 times\n\
         \x20 career fixtures/my-team-r3 ....... rnd_tab_hidden_in_team_hq    skipped: needs the save sandbox\n\
         1 passed, 1 failed, 1 skipped  ·  report: swofty-test/20261028-1402/report.json, junit.xml\n"
    );
}

#[test]
fn exit_codes_rank_infra_over_failures() {
    let mut r = report();
    assert_eq!(r.compute_exit(), exit::FAIL);
    r.tests.retain(|t| t.status != Status::Fail);
    assert_eq!(r.compute_exit(), exit::PASS, "skipped tests don't fail a run");
    r.batches[0].clean = Some(false);
    assert_eq!(r.compute_exit(), exit::INFRA, "a game folder left unclean");
    r.batches[0].clean = Some(true);
    r.tests[0].status = Status::Error;
    assert_eq!(r.compute_exit(), exit::INFRA);
    r.tests[0].status = Status::NotRun;
    assert_eq!(r.compute_exit(), exit::INFRA);
    r.tests[0].status = Status::Pass;
    r.errors.push("Steam is not running".into());
    assert_eq!(r.compute_exit(), exit::INFRA);
    assert_eq!((exit::PASS, exit::FAIL, exit::INFRA, exit::BUSY), (0, 1, 2, 3));
}

#[test]
fn junit_escapes_and_counts() {
    let r = report();
    let x = r.junit();
    assert!(x.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
    assert!(x.contains(
        "<testsuites name=\"swofty test\" tests=\"3\" failures=\"1\" errors=\"0\" skipped=\"1\" time=\"125.000\">"
    ));
    assert!(x.contains(
        "<testsuite name=\"swofty.rnd-tyres-thermal\" tests=\"3\" failures=\"1\" errors=\"0\" skipped=\"1\" time=\"115.000\">"
    ));
    assert!(x.contains(
        "<testcase classname=\"swofty.rnd-tyres-thermal\" name=\"tyre_temps_rise_on_track\" time=\"112.000\"/>"
    ));
    assert!(
        x.contains("<failure message=\"expected log /\\[x\\] &lt;on&gt;/ at least 1 time(s), got 0 times\">")
    );
    assert!(x.contains("(batches/1/loader.log)</failure>"));
    assert!(x.contains("<skipped message=\"needs the save sandbox\"/>"));
    assert_eq!(x.matches("<testcase ").count(), 3);
    assert_eq!(x.matches("</testcase>").count(), 2);
}

#[test]
fn the_json_report_round_trips() {
    let r = report();
    let back: Report = serde_json::from_str(&r.json()).unwrap();
    assert_eq!(back, r);
    let v: serde_json::Value = serde_json::from_str(&r.json()).unwrap();
    assert_eq!(v["tests"][1]["status"], "fail");
    assert_eq!(v["tests"][2]["status"], "skipped");
    assert_eq!(v["schema"], 1);
}

#[test]
fn durations_read_as_minutes_and_seconds() {
    use super::report::duration;
    assert_eq!(duration(112_000), "1 m 52 s");
    assert_eq!(duration(40_400), "40 s");
    assert_eq!(duration(59_600), "1 m 0 s");
}
