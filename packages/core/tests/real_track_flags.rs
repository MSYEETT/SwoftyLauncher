use std::path::{Path, PathBuf};

use swofty_core::activation::place::track_group;
use swofty_core::activation::ui::{MapUi, UiBuild, build_ui};
use swofty_core::map::TrackPackage;
use swofty_formats::erp::{Entry, Erp, ErpFile, Resource};
use swofty_formats::pkg::{Node, Pkg, Val};

const DB: &str = "2025_asset_groups/ui_package/databases.erp";
const FRONT: &str = "2025_asset_groups/ui_package/common_front_end.erp";
const OVERRIDES: &str = "eaid://ui_package/databases/overrides.uidb";
const ARRAY: &str =
    "eaid://ui_package/art/texture_arrays/overrides.uidb/overrides.tracks.flags_circle.texarray";
const FLAG: &str = "eaid://ui_package/art/textures/track_installer/flagtest/flag_circle.tif";

fn flags(path: &Path) -> (u32, Vec<u8>) {
    let surf = Erp::read_matching(path, |n| n == format!("{ARRAY}.image"))
        .unwrap()
        .pop()
        .unwrap();
    let head = surf.fragments[0].data().unwrap();
    (
        u32::from_le_bytes(head[28..32].try_into().unwrap()),
        surf.fragments[1].data().unwrap(),
    )
}

fn track(dir: &Path, front: &ErpFile, pixels: &[u8]) -> TrackPackage {
    let find = |name: &str| front.load(front.erp.find(name).unwrap()).unwrap();
    let mut view = find(ARRAY);
    let mut surf = find(&format!("{ARRAY}.image"));
    let mut head = surf.fragments[0].data().unwrap();
    head[28..32].copy_from_slice(&1u32.to_le_bytes());
    surf.set_fragment(0, &head).unwrap();
    surf.set_fragment(1, pixels).unwrap();
    view.name = FLAG.into();
    surf.name = format!("{FLAG}.image");
    std::fs::create_dir_all(dir).unwrap();
    front
        .write(
            vec![Entry::Owned(view), Entry::Owned(surf)],
            &dir.join("flagtest.erp"),
        )
        .unwrap();
    let images = serde_json::json!({ "resources": { "flag_circle": FLAG } });
    std::fs::write(dir.join("images.json"), images.to_string()).unwrap();
    let manifest = serde_json::json!({
        "track_code": "flagtest",
        "erp": "flagtest.erp",
        "activation": { "copy_from_track": "bahrain" },
        "ui": { "images_file": "images.json" },
    });
    TrackPackage::parse(manifest.to_string().as_bytes()).unwrap()
}

fn longer_list(stock: &Path, dst: &Path, extra: usize) {
    let file = ErpFile::open(stock).unwrap();
    let mut res: Resource = file.load(file.erp.find(OVERRIDES).unwrap()).unwrap();
    let mut pkg = Pkg::parse(&res.fragments[0].data().unwrap()).unwrap();
    let mut cur = &mut pkg.root;
    for p in [
        "overrides",
        "Contents",
        "tracks",
        "Contents",
        "flags_circle",
        "Contents",
    ] {
        cur = cur.as_obj_mut().unwrap().get_mut(p).unwrap();
    }
    let items = &mut cur.as_arr_mut().unwrap().items;
    for n in 0..extra {
        let mut e = items[0].clone();
        e.as_obj_mut()
            .unwrap()
            .set("Name", Node::Val(Val::str(format!("old_track_{n}"))));
        items.push(e);
    }
    res.set_fragment(0, &pkg.to_bytes().unwrap()).unwrap();
    let entries = file
        .erp
        .resources
        .iter()
        .map(|r| match r.name == OVERRIDES {
            true => Entry::Owned(res.clone()),
            false => Entry::Copy(r),
        })
        .collect();
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    file.write(entries, dst).unwrap();
}

fn build(game: &Path, pkg: &TrackPackage, dir: &Path, db: Option<&Path>, cache: &Path) -> UiBuild {
    let resolve = |rel: &str| match (rel == DB, db) {
        (true, Some(db)) => db.to_path_buf(),
        _ => game.join(rel),
    };
    let maps = [MapUi {
        pkg,
        dir,
        asset_group: track_group("flagtest", "flagtest"),
    }];
    build_ui(&maps, &resolve, cache).unwrap()
}

#[test]
fn a_flag_list_longer_than_its_array_pads_the_array() {
    let Some(game) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let out = std::env::temp_dir().join(format!("swofty-track-flags-{}", std::process::id()));
    std::fs::remove_dir_all(&out).ok();
    let (slices, stock) = flags(&game.join(FRONT));
    let size = stock.len() / slices as usize;
    let pixels = vec![0x5a; size];
    let front = ErpFile::open(&game.join(FRONT)).unwrap();
    let pkg = track(&out.join("track"), &front, &pixels);
    let slice = |d: &[u8], i: usize| d[i * size..(i + 1) * size].to_vec();

    let plain = build(&game, &pkg, &out.join("track"), None, &out.join("plain"));
    assert!(plain.notes.is_empty(), "{:?}", plain.notes);
    let (n, data) = flags(&plain.replaced[FRONT]);
    assert_eq!(n, slices + 1);
    assert_eq!(data[..stock.len()], stock[..]);
    assert_eq!(slice(&data, slices as usize), pixels);

    let entries = slices as usize + 3;
    let db = out.join("originals").join(DB);
    longer_list(&game.join(DB), &db, 3);
    for _ in 0..2 {
        let padded = build(&game, &pkg, &out.join("track"), Some(&db), &out.join("padded"));
        assert_eq!(
            padded.notes,
            [format!(
                "tracks.flags_circle: the game's list has {entries} entries but only {slices} pictures; \
                 3 blank picture(s) fill the gap. Repair the game files if a menu shows the wrong picture"
            )]
        );
        let (n, data) = flags(&padded.replaced[FRONT]);
        assert_eq!(n as usize, entries + 1);
        assert_eq!(data[..stock.len()], stock[..]);
        for i in slices as usize..entries {
            assert_eq!(slice(&data, i), slice(&stock, 2), "slice {i} is the blank flag");
        }
        assert_eq!(slice(&data, entries), pixels);
    }
    std::fs::remove_dir_all(&out).ok();
}
