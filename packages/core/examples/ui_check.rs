use std::collections::BTreeSet;
use std::path::Path;

use swofty_core::ui::catalog::{self, Scenes};
use swofty_core::ui::check;
use swofty_formats::erp::Erp;

fn findings(erp: &Path, resource: &str, stock: Option<&Path>) -> Vec<String> {
    let all = Erp::read_matching(erp, |n| {
        n == resource || (n.ends_with(".idf") && !n.contains('?'))
    })
    .expect("read the archive");
    let mut scenes = Scenes::default();
    for r in all.iter().filter(|r| r.kind() == "UILayout") {
        for f in &r.fragments {
            scenes.add(&f.data().expect("scene fragment")).ok();
        }
    }
    let target = all
        .iter()
        .find(|r| r.name == resource)
        .expect("the resource is in the archive");
    let stock = stock.and_then(|s| Erp::read_matching(s, |n| n == resource).ok()?.into_iter().next());
    let mut out = Vec::new();
    for (i, f) in target.fragments.iter().enumerate() {
        let data = f.data().expect("fragment");
        let report = if target.kind() == "UIDF" {
            let base = stock.as_ref().and_then(|s| s.fragments.get(i)?.data().ok());
            check::ui_edit(
                &f.name(),
                &data,
                base.as_deref(),
                &scenes,
                catalog::stock(),
                catalog::rules(),
            )
        } else {
            check::scene_edit(&data, catalog::stock(), catalog::rules())
        }
        .expect("check the fragment");
        out.extend(report.hard.iter().map(|h| format!("HARD {h}")));
        out.extend(report.warnings.iter().map(|w| format!("warn {w}")));
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (erp, resource) = match args.as_slice() {
        [erp, resource, ..] => (Path::new(erp), resource.as_str()),
        _ => panic!(
            "usage: ui_check <archive.erp> <eaid resource> [--stock <stock.erp>] [--against <other.erp>]"
        ),
    };
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .map(|i| Path::new(&args[i + 1]))
    };
    let stock = flag("--stock");
    let found = findings(erp, resource, stock);
    let base: BTreeSet<String> = match flag("--against") {
        Some(other) => findings(other, resource, stock).into_iter().collect(),
        None => BTreeSet::new(),
    };
    let fresh: Vec<&String> = found.iter().filter(|f| !base.contains(*f)).collect();
    for f in &fresh {
        println!("{f}");
    }
    let hard = found.iter().filter(|f| f.starts_with("HARD")).count();
    eprintln!(
        "{} findings ({hard} hard), {} not in the --against archive",
        found.len(),
        fresh.len()
    );
    std::process::exit(i32::from(hard > 0));
}
