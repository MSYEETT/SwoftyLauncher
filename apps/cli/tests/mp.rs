use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

fn data_dir(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("swofty-cli-mp-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    for dir in ["profiles", "addons/t.a/db", "game"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    std::fs::write(
        root.join("config.json"),
        r#"{"data_version":3,"active_profile":"grid"}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("profiles/grid.json"),
        r#"{"schema_version":2,"name":"grid","addons":[{"id":"t.a"}]}"#,
    )
    .unwrap();
    let manifest = json!({
        "schema_version": 1, "id": "t.a", "name": "Grid", "version": "1.2.0", "author": "a",
        "compat": { "game": "f1_25" }, "kind": ["database"],
        "operations": { "database": ["db/ops.json"] },
        "options": [{ "id": "laps", "type": "slider", "min": 1, "max": 10, "step": 1, "default": 5, "binds": "db" }],
    });
    std::fs::write(root.join("addons/t.a/swofty.json"), manifest.to_string()).unwrap();
    std::fs::write(
        root.join("addons/t.a/db/ops.json"),
        r#"{"schema_version":1,"ops":[]}"#,
    )
    .unwrap();
    std::fs::write(root.join("game/F1_25.exe"), b"exe").unwrap();
    root
}

fn swofty(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_swofty"))
        .args(args)
        .env("SWOFTY_DATA_DIR", root)
        .output()
        .unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn prints_the_fingerprint_and_compares_two() {
    let root = data_dir("compare");
    let game = root.join("game");
    let game = game.to_str().unwrap();
    let out = swofty(&root, &["mp", "fingerprint", "--profile", "grid", "--game", game]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let f: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(f["version"], 1);
    assert_eq!(f["dlc"], Value::Null);
    assert!(f["game_build"]["build_id"].as_str().unwrap().starts_with("exe-"));
    let addons: Vec<&str> = f["addons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap())
        .collect();
    assert_eq!(addons, ["swofty.core", "t.a"]);
    assert_eq!(f["addons"][1]["version"], "1.2.0");
    assert_eq!(f["addons"][1]["source"]["content"].as_str().unwrap().len(), 64);
    assert_eq!(f["options"], json!({ "t.a": { "laps": 5 } }));
    assert_eq!(f["digest"].as_str().unwrap().len(), 64);

    let mut host = f.clone();
    host["options"]["t.a"]["laps"] = json!(9);
    let (a, b) = (root.join("a.json"), root.join("b.json"));
    std::fs::write(&a, f.to_string()).unwrap();
    std::fs::write(&b, host.to_string()).unwrap();
    let (a, b) = (a.to_str().unwrap(), b.to_str().unwrap());

    let same = swofty(&root, &["mp", "compare", a, a]);
    assert!(same.status.success());
    assert_eq!(stdout(&same), "the fingerprints match\n");

    let differ = swofty(&root, &["mp", "compare", a, b]);
    assert_eq!(differ.status.code(), Some(1));
    assert_eq!(stdout(&differ), "t.a.laps is 5; the host has 9 (fixable)\n");

    let listed = swofty(&root, &["mp", "compare", "--json", a, b]);
    assert_eq!(listed.status.code(), Some(1));
    let found: Value = serde_json::from_str(&stdout(&listed)).unwrap();
    assert_eq!(
        found,
        json!([{ "kind": "option_differs", "addon": "t.a", "option": "laps", "local": 5, "host": 9, "fixable": true }])
    );
    std::fs::remove_dir_all(&root).ok();
}
