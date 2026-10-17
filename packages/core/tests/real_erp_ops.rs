use std::path::PathBuf;

use swofty_core::asset::{AssetOp, FileKind, OpFile, Source};
use swofty_core::erpbuild::ErpBuild;
use swofty_formats::erp::{Erp, ErpFile, Resource};

const ARCHIVE: &str = "2025_asset_groups/f1_safetycar_package/safety_car_2/wep/safety_car_2.erp";

fn game() -> Option<PathBuf> {
    match std::env::var_os("F1_25_DIR") {
        Some(d) => Some(PathBuf::from(d)),
        None => {
            eprintln!("F1_25_DIR is not set; skipping");
            None
        }
    }
}

#[test]
fn replaces_adds_and_removes_textures_in_a_game_archive() {
    let Some(game) = game() else {
        return;
    };
    let stock = ErpFile::open(&game.join(ARCHIVE)).unwrap();
    let textures: Vec<_> = stock
        .erp
        .resources
        .iter()
        .filter(|r| r.kind() == "GfxSurfaceRes" && r.fragments.len() == 3)
        .take(2)
        .collect();
    let [kept, gone] = textures.as_slice() else {
        panic!("{ARCHIVE} has fewer than two textures with mip paths");
    };
    let dir = std::env::temp_dir().join(format!("swofty-real-erp-ops-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let loaded = stock.load(kept).unwrap();
    let mut sources = Vec::new();
    for (i, f) in loaded.fragments.iter().enumerate() {
        std::fs::write(dir.join(format!("{i}.bin")), f.data().unwrap()).unwrap();
        sources.push(format!("\"{i}.bin\""));
    }
    let added = format!("{}_copy.image", kept.name.trim_end_matches(".image"));
    let file = OpFile::from_value(
        serde_json::from_str(&format!(
            r#"{{"schema_version":1,"ops":[
            {{"op":"replace-resource","erp":"{ARCHIVE}","resource":"{}","fragment":1,"source":"1.bin"}},
            {{"op":"add-resource","erp":"{ARCHIVE}","resource":"{added}","type":"GfxSurfaceRes","sources":[{}]}},
            {{"op":"remove-resource","erp":"{ARCHIVE}","resource":"{}"}}]}}"#,
            kept.name,
            sources.join(","),
            gone.name
        ))
        .unwrap(),
        FileKind::Erp,
    )
    .unwrap();
    let original = |rel: &str| game.join(rel);
    let mut b = ErpBuild::new(&original);
    for op in &file.ops {
        let AssetOp::Erp(op) = op else { continue };
        b.apply(op, &|s: &Source| Ok(s.resolve(&dir)?)).unwrap();
    }
    let out = dir.join("out");
    assert_eq!(b.write(&out).unwrap(), [ARCHIVE]);
    let written = Erp::parse(&std::fs::read(out.join(ARCHIVE)).unwrap()).unwrap();
    assert_eq!(written.resources.len(), stock.erp.resources.len());
    assert!(written.find(&gone.name).is_err());
    let replaced = written.find(&kept.name).unwrap();
    assert_eq!(replaced.hash, kept.hash);
    let copy = written.find(&added).unwrap();
    assert_eq!(
        (copy.kind, &copy.unk, copy.hash),
        (kept.kind, &kept.unk, kept.hash)
    );
    for (f, s) in copy.fragments.iter().zip(&loaded.fragments) {
        assert_eq!(
            (f.name, f.flags, f.compression, f.size),
            (s.name, s.flags, s.compression, s.size)
        );
        assert_eq!(f.data().unwrap(), s.data().unwrap());
    }
    let raw = |r: &Resource| r.fragments.iter().map(|f| f.raw.clone()).collect::<Vec<_>>();
    let mut copied = 0;
    for r in stock
        .erp
        .resources
        .iter()
        .filter(|r| r.name != gone.name && r.name != kept.name)
    {
        let w = written.find(&r.name).unwrap();
        assert_eq!(w.hash, r.hash);
        assert_eq!(raw(w), raw(&stock.load(r).unwrap()));
        copied += 1;
    }
    assert_eq!(copied, stock.erp.resources.len() - 2);
    std::fs::remove_dir_all(&dir).ok();
}
