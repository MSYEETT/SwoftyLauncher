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

#[test]
fn menus_and_a_map_patch_one_databases_erp() {
    use swofty_core::activation::place::track_group;
    use swofty_core::activation::ui::{MapUi, build_ui};
    use swofty_core::build::{Built, build_menus};
    use swofty_core::map::TrackPackage;
    use swofty_formats::pkg::Pkg;

    let Some(game) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let Some(dir) = std::env::var_os("SWOFTY_MAPS_DIR")
        .map(PathBuf::from)
        .map(|d| d.join("bahrain24"))
        .filter(|d| d.join("manifest.json").is_file())
    else {
        eprintln!("SWOFTY_MAPS_DIR has no bahrain24 package; skipping");
        return;
    };
    let pkg = TrackPackage::parse(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!("swofty-menus-map-{}", std::process::id()));
    std::fs::remove_dir_all(&out).ok();
    let resolve = |rel: &str| game.join(rel);
    let maps = [MapUi {
        pkg: &pkg,
        dir: &dir,
        asset_group: track_group("bahrain24", "bahrain24"),
    }];
    let map_ui = build_ui(&maps, &resolve, &out.join("cache")).unwrap();
    let rel = "2025_asset_groups/ui_package/databases.erp";
    assert!(map_ui.replaced.contains_key(rel));
    let mut built = Built {
        overlay: out.join("overlay"),
        ..Built::default()
    };
    built.plan.data.extend(map_ui.replaced.clone());
    let mut session = session();
    build_menus(&mut built, &mut session, &resolve).unwrap();
    assert_eq!(built.plan.data[rel], out.join("overlay").join(rel));
    for (r, src) in &map_ui.replaced {
        if !swofty_core::ui::ERPS.iter().any(|e| r.ends_with(e)) {
            assert_eq!(&built.plan.data[r], src, "{r} keeps the map build");
        }
    }

    let data = |path: &Path, name: &str| {
        let res = Erp::read_matching(path, |n| n == name).unwrap();
        res[0].fragments[0].data().unwrap()
    };
    let (ours, maps_only, stock) = (&built.plan.data[rel], &map_ui.replaced[rel], game.join(rel));
    for name in [
        "eaid://ui_package/databases/overrides.uidb",
        "eaid://ui_package/databases/minimap.uidb",
    ] {
        assert_eq!(
            data(ours, name),
            data(maps_only, name),
            "{name} lost the map edits"
        );
        assert_ne!(data(ours, name), data(&stock, name), "{name} has no map edits");
    }
    for (r, maps_only) in &map_ui.replaced {
        let Some(ours) = built.plan.data.get(r).filter(|o| *o != maps_only) else {
            continue;
        };
        let stock = Erp::parse(&std::fs::read(game.join(r)).unwrap()).unwrap();
        for res in Erp::parse(&std::fs::read(maps_only).unwrap()).unwrap().resources {
            let theirs = res.content_hash().unwrap();
            let edited = stock.find(&res.name).ok().map(|s| s.hash) != Some(theirs);
            if edited {
                assert_eq!(
                    data(ours, &res.name),
                    data(maps_only, &res.name),
                    "{r}: {} lost the map edits",
                    res.name
                );
            }
        }
    }
    let has = |bytes: Vec<u8>, path: &[&str]| {
        let p = Pkg::parse(&bytes).unwrap();
        let mut n = &p.root;
        for k in path {
            match n.as_obj().and_then(|o| o.get(k)) {
                Some(x) => n = x,
                None => return false,
            }
        }
        true
    };
    let ui_data = "eaid://ui_package/databases/ui_data.uidb";
    assert!(has(data(ours, ui_data), &["ui_menu_data", "Contents", "swofty"]));
    assert!(!has(
        data(maps_only, ui_data),
        &["ui_menu_data", "Contents", "swofty"]
    ));
    let tips = "eaid://ui_package/databases/tool_tips.uidb";
    for t in ["mod_menu", "mod_menu_offline", "swofty_adjust"] {
        let path = ["tool_tips", "Contents", "tooltips_dictionary", "Contents", t];
        assert!(has(data(ours, tips), &path));
    }
    std::fs::remove_dir_all(&out).ok();
}
