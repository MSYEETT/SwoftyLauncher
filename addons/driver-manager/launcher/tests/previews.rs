use std::io::{BufRead, BufReader, Lines, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use driver_manager::check;
use driver_manager::game::{self, List};
use driver_manager::loader;
use driver_manager::model::{Driver, Roster};
use driver_manager::preview::{self, Previews};
use serde_json::{Value, json};

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("F1_25_DIR")?);
    let ok = dir.join(game::DB_REL).is_file() && dir.join("2025_asset_groups/character_package").is_dir();
    ok.then_some(dir)
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::var_os("DM_PREVIEW_OUT")
        .map_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")), PathBuf::from)
        .join(name);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn png_size(path: &Path) -> (u32, u32) {
    let bytes = std::fs::read(path).unwrap();
    let word = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    (word(16), word(20))
}

#[test]
fn previews_from_the_real_game() {
    let Some(dir) = game_dir() else {
        eprintln!("F1_25_DIR is not set or has no character packages; skipping");
        return;
    };
    let data = loader::read(&dir.join(game::DB_REL), Some(&dir), "test-key").unwrap();
    for list in [List::Head, List::Helmet, List::Suit, List::Gloves] {
        let missing = data
            .list(list)
            .iter()
            .filter(|c| !data.installed(list, &c.value))
            .count();
        eprintln!(
            "{}: {} listed, {missing} not installed",
            list.slug(),
            data.list(list).len()
        );
        assert!(
            missing < data.list(list).len() / 2,
            "{} mostly missing",
            list.slug()
        );
    }
    let missing_head = data
        .heads
        .iter()
        .find(|c| !data.installed(List::Head, &c.value))
        .expect("some heads aren't installed");
    let mut d = Driver::blank(Some(&data), 42);
    assert!(data.installed(List::Head, &d.head) && data.installed(List::Helmet, &d.helmet));
    d.head = missing_head.value.clone();
    let issues = check::check(&d, &Roster::default(), Some(&data));
    assert!(
        issues.iter().any(|i| i.text.contains("isn't installed")),
        "{issues:?}"
    );

    let root = scratch("previews");
    let jobs = preview::jobs(&data);
    let total = jobs.len();
    let started = Instant::now();
    let built = preview::build(&dir, &root, &data.key, jobs);
    let elapsed = started.elapsed();
    let ready = built.index.ready.len();
    eprintln!(
        "built {ready} of {total} previews in {elapsed:?} ({} KiB); failed: {:?}",
        built.index.bytes / 1024,
        built.index.failed.iter().take(8).collect::<Vec<_>>()
    );
    assert!(ready * 100 >= total * 99, "only {ready} of {total} previews");
    for (list, value, size) in [
        (List::Head, d.head.as_str(), (96, 96)),
        (List::Head, "max_verstappen", (96, 96)),
        (List::Helmet, "max_verstappen_2025", (128, 64)),
        (List::Suit, "2025-redbull_max_verstappen", (64, 64)),
        (List::Pose, "Pose03", (96, 96)),
        (List::Emote, "Emote03", (96, 96)),
    ] {
        if !data.has(list, value) || !data.installed(list, value) {
            continue;
        }
        if let Some(path) = built.path(list, value) {
            assert_eq!(png_size(&path), size, "{}", path.display());
        }
    }
    let first = data
        .heads
        .iter()
        .find(|c| data.installed(List::Head, &c.value))
        .unwrap();
    assert!(built.path(List::Head, &first.value).is_some());

    let again = Previews::load(&root, &data.key).expect("index reloads");
    assert_eq!(again.index.ready, built.index.ready);
    assert!(Previews::load(&root, "other-build").is_none());

    let started = Instant::now();
    let rebuilt = preview::build(&dir, &root, &data.key, preview::jobs(&data));
    eprintln!("warm rebuild in {:?}", started.elapsed());
    assert_eq!(rebuilt.index.ready.len(), ready);

    let mut large = again;
    let helmet = data
        .helmets
        .iter()
        .find(|c| large.path(List::Helmet, &c.value).is_some())
        .unwrap();
    let path = large.large(&dir, &data, List::Helmet, &helmet.value).unwrap();
    assert_eq!(png_size(&path).0, 256);
}

struct Helper {
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    db: PathBuf,
    dir: PathBuf,
    next: u64,
    invalidated: bool,
}

impl Helper {
    fn send(&mut self, v: Value) {
        writeln!(self.stdin, "{v}").unwrap();
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = self.next;
        self.send(json!({ "id": id, "method": method, "params": params }));
        loop {
            let msg: Value = serde_json::from_str(&self.lines.next().unwrap().unwrap()).unwrap();
            match (msg["id"].as_u64(), msg["method"].as_str()) {
                (Some(host_id), Some(m)) => {
                    let result = match m {
                        "game_build_key" => json!("25251167-previews"),
                        "game_original" => json!(self.db),
                        "game_dir" => json!(self.dir),
                        other => panic!("unexpected host call {other}"),
                    };
                    self.send(json!({ "id": host_id, "result": result }));
                }
                (None, Some("invalidate")) => self.invalidated = true,
                (Some(reply), None) if reply == id => {
                    assert!(msg.get("error").is_none(), "{msg}");
                    return msg["result"].clone();
                }
                _ => {}
            }
        }
    }
}

fn find<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Object(map) => map.get(key).or_else(|| map.values().find_map(|x| find(x, key))),
        Value::Array(items) => items.iter().find_map(|x| find(x, key)),
        _ => None,
    }
}

fn find_select<'a>(v: &'a Value, id: &str) -> Option<&'a Value> {
    match v {
        Value::Object(map) => {
            if let Some(sel) = map.get("select").filter(|s| s["id"] == id) {
                return Some(sel);
            }
            map.values().find_map(|x| find_select(x, id))
        }
        Value::Array(items) => items.iter().find_map(|x| find_select(x, id)),
        _ => None,
    }
}

#[test]
fn the_page_shows_previews() {
    let Some(dir) = game_dir() else {
        eprintln!("F1_25_DIR is not set or has no character packages; skipping");
        return;
    };
    let out = scratch("page");
    let (data_dir, cache_dir) = (out.join("data"), out.join("cache"));
    let mut child = Command::new(env!("CARGO_BIN_EXE_driver-manager-helper"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut h = Helper {
        stdin: child.stdin.take().unwrap(),
        lines: BufReader::new(child.stdout.take().unwrap()).lines(),
        db: dir.join(game::DB_REL),
        dir: dir.clone(),
        next: 0,
        invalidated: false,
    };
    h.call(
        "hello",
        json!({
        "api": 1, "addon": "swofty.driver-manager", "data_dir": data_dir, "cache_dir": cache_dir,
        "profile": { "name": "default", "vanilla": false, "enabled": true }, "launcher": "1.0.12" }),
    );
    let started = Instant::now();
    let mut view = h.call("view", json!({}));
    while view["tick"] == true {
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "previews never finished"
        );
        std::thread::sleep(Duration::from_millis(50));
        h.call("tick", json!({}));
        if h.invalidated {
            h.invalidated = false;
            view = h.call("view", json!({}));
        }
    }
    eprintln!("lists and previews ready after {:?}", started.elapsed());
    assert_eq!(
        h.call("event", json!({ "event": { "click": { "id": "add" } } })),
        "reload"
    );
    let edit = h.call("view", json!({}));
    std::fs::write(
        out.join("edit-view.json"),
        serde_json::to_vec_pretty(&edit).unwrap(),
    )
    .unwrap();
    for id in ["head", "helmet", "suit", "gloves", "pose", "emote"] {
        let sel = find_select(&edit, id).unwrap_or_else(|| panic!("no {id} select"));
        let opts = sel["options"].as_array().unwrap();
        let with = opts
            .iter()
            .filter(|o| o["image"].as_str().is_some_and(|p| Path::new(p).is_file()))
            .count();
        eprintln!("{id}: {with} of {} options have a thumbnail", opts.len());
        assert!(with * 100 >= opts.len() * 90, "{id}: {with} of {}", opts.len());
    }
    assert!(
        find_select(&edit, "voice").unwrap()["options"]
            .as_array()
            .unwrap()
            .iter()
            .all(|o| o.get("image").is_none())
    );
    let looks = edit["body"]["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["card"]["title"] == "Looks")
        .expect("a looks card");
    let images = looks["card"]["blocks"][0]["grid"]["blocks"].as_array().unwrap();
    assert_eq!(images.len(), 2);
    assert!(images.iter().all(|i| {
        i["image"]["path"]
            .as_str()
            .is_some_and(|p| Path::new(p).is_file())
    }));
    assert!(edit["body"]["blocks"][0]["hero"]["art"]["image"].is_string());

    let form_id = find(&edit, "form")
        .and_then(|f| f["id"].as_str())
        .unwrap()
        .to_owned();
    let values = json!({ "first": "Preview", "last": "Tester", "code": "PRV", "number": 77 });
    let saved = h.call(
        "event",
        json!({ "event": { "submit": { "form": form_id, "values": values } } }),
    );
    assert!(
        saved["notice"].as_str().is_some_and(|n| n.starts_with("Added")),
        "{saved}"
    );
    let list = h.call("view", json!({}));
    std::fs::write(
        out.join("list-view.json"),
        serde_json::to_vec_pretty(&list).unwrap(),
    )
    .unwrap();
    let art = &list["body"]["list"]["rows"][0]["art"]["image"];
    assert!(art.as_str().is_some_and(|p| Path::new(p).is_file()), "{art}");
    drop(h);
    child.wait().unwrap();
}
