use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use swofty_core::ui::catalog::{self, Scenes};
use swofty_core::ui::check;
use swofty_formats::erp::Erp;

fn archives(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() && !p.ends_with("texture_arrays") && !p.ends_with("textures") {
            archives(&p, out);
        } else if p.extension().is_some_and(|x| x == "erp") {
            out.push(p);
        }
    }
}

#[test]
fn every_stock_ui_and_scene_passes_the_catalogue() {
    let Some(game) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let mut erps = Vec::new();
    archives(&game.join("2025_asset_groups/ui_package"), &mut erps);
    erps.sort();
    let (mut seen, mut bad, mut checked) = (BTreeSet::new(), Vec::new(), 0);
    for erp in erps {
        let all = Erp::read_matching(&erp, |n| {
            n.ends_with(".ui") || (n.ends_with(".idf") && !n.contains('?'))
        })
        .unwrap();
        let mut scenes = Scenes::default();
        for r in all.iter().filter(|r| r.kind() == "UILayout") {
            for f in &r.fragments {
                scenes.add(&f.data().unwrap()).unwrap();
            }
        }
        for r in all
            .iter()
            .filter(|r| matches!(r.kind().as_str(), "UIDF" | "UILayout"))
        {
            if !seen.insert((r.name.clone(), r.content_hash().unwrap())) {
                continue;
            }
            for f in &r.fragments {
                let data = f.data().unwrap();
                let report = if r.kind() == "UIDF" {
                    check::ui_edit(
                        &f.name(),
                        &data,
                        Some(&data),
                        &scenes,
                        catalog::stock(),
                        catalog::rules(),
                    )
                } else {
                    check::scene_edit(&data, catalog::stock(), catalog::rules())
                }
                .unwrap();
                checked += 1;
                bad.extend(
                    report
                        .hard
                        .iter()
                        .chain(&report.warnings)
                        .map(|m| format!("{} {m}", r.name)),
                );
            }
        }
    }
    assert!(checked > 5000, "only {checked} fragments checked");
    assert!(
        bad.is_empty(),
        "{} findings on stock files:\n{}",
        bad.len(),
        bad[..bad.len().min(40)].join("\n")
    );
}
