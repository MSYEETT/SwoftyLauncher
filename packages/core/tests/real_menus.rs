use std::path::{Path, PathBuf};

use swofty_core::settings::SessionSettings;
use swofty_formats::erp::Erp;

fn dirs() -> Option<(PathBuf, PathBuf)> {
    let game = std::env::var_os("F1_25_DIR").map(PathBuf::from);
    let oracle = std::env::var_os("SWOFTY_MENU_ORACLE").map(PathBuf::from);
    match (game, oracle) {
        (Some(g), Some(o)) => Some((g, o)),
        _ => {
            eprintln!("F1_25_DIR and SWOFTY_MENU_ORACLE are not both set; skipping");
            None
        }
    }
}

fn session() -> SessionSettings {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/menu_session.json");
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn first_difference(a: &[u8], b: &[u8]) -> String {
    let at = a
        .iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .unwrap_or(a.len().min(b.len()));
    format!("sizes {} and {}, first difference at {at:#x}", a.len(), b.len())
}

fn resource_report(ours: &Path, theirs: &Path) -> String {
    let (a, b) = (
        Erp::parse(&std::fs::read(ours).unwrap()).unwrap(),
        Erp::parse(&std::fs::read(theirs).unwrap()).unwrap(),
    );
    let mut out = Vec::new();
    for (x, y) in a.resources.iter().zip(&b.resources) {
        if x.name != y.name {
            out.push(format!("order: {} vs {}", x.name, y.name));
            continue;
        }
        for (i, (fx, fy)) in x.fragments.iter().zip(&y.fragments).enumerate() {
            if fx.raw != fy.raw {
                let (dx, dy) = (fx.data().unwrap(), fy.data().unwrap());
                let what = if dx == dy {
                    "same data, different compression".to_owned()
                } else {
                    first_difference(&dx, &dy)
                };
                out.push(format!("{} fragment {i}: {what}", x.name));
            }
        }
    }
    if a.resources.len() != b.resources.len() {
        out.push(format!(
            "{} vs {} resources",
            a.resources.len(),
            b.resources.len()
        ));
    }
    out.join("\n")
}

#[test]
fn menus_match_the_prototype_byte_for_byte() {
    let Some((game, oracle)) = dirs() else { return };
    let out = std::env::temp_dir().join(format!("swofty-menus-{}", std::process::id()));
    std::fs::remove_dir_all(&out).ok();
    let built = swofty_core::ui::build(&session(), &|rel| game.join(rel), &out).unwrap();
    let mut bad = Vec::new();
    for (rel, path) in &built.erps {
        let theirs = oracle.join(rel);
        if std::fs::read(path).unwrap() != std::fs::read(&theirs).unwrap() {
            bad.push(format!("{rel}:\n{}", resource_report(path, &theirs)));
        }
    }
    for (rel, path) in &built.screens {
        let ours = std::fs::read(path).unwrap();
        let theirs = std::fs::read(oracle.join("native_ui").join(rel)).unwrap();
        if ours != theirs {
            bad.push(format!("{rel}: {}", first_difference(&ours, &theirs)));
        }
    }
    let ui: serde_json::Value =
        serde_json::from_slice(&std::fs::read(oracle.join("native_ui/swofty/ui.json")).unwrap()).unwrap();
    let want: Vec<(String, bool)> = ui["ui"]["screens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["name"].as_str().unwrap().to_owned(),
                s["view_model"].as_bool().unwrap_or(false),
            )
        })
        .collect();
    let got: Vec<(String, bool)> = built
        .session
        .iter()
        .map(|s| (s.name.clone(), s.view_model))
        .collect();
    assert_eq!(got, want);
    for s in &built.session {
        assert_eq!(
            (s.flow.as_str(), s.anchor.as_deref()),
            ("flow_options", Some("settings_container"))
        );
    }
    std::fs::remove_dir_all(&out).ok();
    assert!(bad.is_empty(), "differs from the prototype:\n{}", bad.join("\n"));
}
