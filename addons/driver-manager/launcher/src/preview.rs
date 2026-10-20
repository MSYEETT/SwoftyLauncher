use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use swofty_formats::erp::{Erp, Resource};
use swofty_formats::texture;

use crate::game::{Art, GameData, List};
use crate::pixels::{self, Crop, Image, NEUTRAL, WHOLE};

pub const THUMB: u32 = 96;
pub const LARGE: u32 = 192;
const SCHEMA: u32 = 1;
const INDEX: &str = "index.json";
const HEADS: &str = "2025_asset_groups/animation_package/scenes/cinematics/drivers/heads";
const ICONS: &str = "2025_asset_groups/ui_package/texture_arrays/customisation";
const FACE: Crop = Crop {
    x: 0.15,
    y: 0.10,
    w: 0.70,
    h: 0.70,
};

pub fn asset_group_file(group: &str) -> Option<String> {
    let rel = group
        .strip_prefix("asset_groups/")?
        .strip_suffix(".assetgroup")?
        .replace("[CHARACTER_PACKAGE]", "character_package")
        .replace("[LIVERY_PACKAGE]", "livery_package")
        .replace("[ANIMATION_PACKAGE]", "animation_package");
    let safe = !rel.contains(['[', ']', '\\', ':']) && rel.split('/').all(|p| !p.is_empty() && p != "..");
    safe.then(|| format!("2025_asset_groups/{rel}.erp"))
}

fn plain(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c))
        && !name.starts_with('.')
}

pub fn source(game_dir: &Path, list: List, value: &str, art: Option<&Art>) -> Option<PathBuf> {
    let rel = match list {
        List::Head => plain(value).then(|| format!("{HEADS}/{value}.erp"))?,
        List::Helmet | List::Suit | List::Gloves => asset_group_file(&art?.source)?,
        List::Pose | List::Emote => {
            let name = &art?.source;
            let dir = if list == List::Pose { "poses" } else { "emotes" };
            plain(name).then(|| format!("{ICONS}/{dir}/{name}.erp"))?
        }
        List::Nationality | List::Voice => return None,
    };
    Some(game_dir.join(rel))
}

pub fn mark_missing(data: &mut GameData, game_dir: &Path) {
    let mut missing = BTreeSet::new();
    for list in [List::Head, List::Helmet, List::Suit, List::Gloves] {
        for c in data.list(list) {
            let file = source(game_dir, list, &c.value, data.art(list, &c.value));
            if !file.is_some_and(|f| f.is_file()) {
                missing.insert(list.key(&c.value));
            }
        }
    }
    data.missing = missing;
}

fn surfaces(erp: &Path, want: impl Fn(&str) -> bool) -> Result<Vec<Resource>, String> {
    let mut found = Erp::read_matching(erp, |n| {
        n.ends_with(".image") && !n.contains("charname") && want(n)
    })
    .map_err(|e| format!("{}: {e}", erp.display()))?;
    found.sort_by(|a, b| a.name.len().cmp(&b.name.len()).then(a.name.cmp(&b.name)));
    Ok(found)
}

fn decode(surface: &Resource, game_dir: &Path, want: u32) -> Result<Image, String> {
    texture::decode_near(surface, Some(game_dir), want).map_err(|e| e.to_string())
}

fn kit_suffix(list: List) -> &'static str {
    match list {
        List::Helmet => "_helmet",
        List::Suit => "_livery_v2",
        _ => "_glove",
    }
}

fn kit_texture(game_dir: &Path, list: List, art: &Art, want: u32) -> Result<Image, String> {
    let erp = source(game_dir, list, "", Some(art)).ok_or("no asset group")?;
    let suffix = kit_suffix(list);
    let (d, m) = (format!("{suffix}_d.tif.image"), format!("{suffix}_m.tif.image"));
    let found = surfaces(&erp, |n| n.ends_with(&d) || n.ends_with(&m))?;
    let diffuse = found.iter().find(|r| r.name.ends_with(&d));
    let mask = found.iter().find(|r| r.name.ends_with(&m));
    match (diffuse, mask) {
        (Some(diffuse), Some(mask)) if art.colours > 0 => {
            let image = decode(diffuse, game_dir, want)?;
            let mask = decode(mask, game_dir, image.width.max(image.height))?;
            Ok(pixels::tint(&image, &mask, &NEUTRAL))
        }
        (Some(diffuse), _) => Ok(pixels::opaque(decode(diffuse, game_dir, want)?)),
        (None, Some(mask)) => {
            let mask = decode(mask, game_dir, want)?;
            let white = Image {
                width: mask.width,
                height: mask.height,
                rgba: vec![255; mask.rgba.len()],
            };
            Ok(pixels::tint(&white, &mask, &NEUTRAL))
        }
        (None, None) => Err(format!("{}: no {suffix} texture", erp.display())),
    }
}

pub fn render(
    game_dir: &Path,
    list: List,
    value: &str,
    art: Option<&Art>,
    size: u32,
) -> Result<Image, String> {
    match list {
        List::Head => {
            let erp = source(game_dir, list, value, None).ok_or("bad head name")?;
            let found = surfaces(&erp, |n| n.ends_with("_head_d.tif.image"))?;
            let surface = found
                .iter()
                .find(|r| r.name.ends_with(&format!("/{value}_head_d.tif.image")))
                .or_else(|| found.first())
                .ok_or_else(|| format!("{}: no head texture", erp.display()))?;
            let face = decode(surface, game_dir, size * 10 / 7)?;
            Ok(pixels::opaque(pixels::resize(&face, &FACE, size, size)))
        }
        List::Helmet => {
            let art = art.ok_or("no helmet data")?;
            let width = size * 4 / 3;
            let image = kit_texture(game_dir, list, art, width)?;
            let height = (width * image.height / image.width.max(1)).max(1);
            Ok(pixels::resize(&image, &WHOLE, width, height))
        }
        List::Suit | List::Gloves => {
            let art = art.ok_or("no kit data")?;
            let image = kit_texture(game_dir, list, art, 128)?;
            Ok(pixels::chips(
                &pixels::dominant(&image, 3),
                size * 2 / 3,
                size * 2 / 3,
            ))
        }
        List::Pose | List::Emote => {
            let erp = source(game_dir, list, value, art).ok_or("no icon name")?;
            let found = surfaces(&erp, |_| true)?;
            let surface = found
                .first()
                .ok_or_else(|| format!("{}: no icon", erp.display()))?;
            let icon = decode(surface, game_dir, size)?;
            Ok(pixels::resize(&icon, &WHOLE, size, size))
        }
        List::Nationality | List::Voice => Err("no preview for this list".into()),
    }
}

fn file_part(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Index {
    pub schema: u32,
    pub key: String,
    pub ready: BTreeSet<String>,
    pub failed: BTreeMap<String, String>,
    pub millis: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone)]
pub struct Previews {
    pub root: PathBuf,
    pub index: Index,
    large_failed: BTreeSet<String>,
}

impl Previews {
    pub fn file(root: &Path, key: &str, list: List, value: &str, large: bool) -> PathBuf {
        let dir = root.join(list.slug());
        let dir = if large { dir.join("large") } else { dir };
        dir.join(format!("{}.{}.png", file_part(value), file_part(key)))
    }

    pub fn load(root: &Path, key: &str) -> Option<Previews> {
        let index: Index = serde_json::from_slice(&std::fs::read(root.join(INDEX)).ok()?).ok()?;
        (index.schema == SCHEMA && index.key == key).then(|| Previews {
            root: root.to_owned(),
            index,
            large_failed: BTreeSet::new(),
        })
    }

    pub fn path(&self, list: List, value: &str) -> Option<PathBuf> {
        self.index
            .ready
            .contains(&list.key(value))
            .then(|| Self::file(&self.root, &self.index.key, list, value, false))
    }

    pub fn large(&mut self, game_dir: &Path, game: &GameData, list: List, value: &str) -> Option<PathBuf> {
        let key = list.key(value);
        if !self.index.ready.contains(&key) || self.large_failed.contains(&key) {
            return None;
        }
        let path = Self::file(&self.root, &self.index.key, list, value, true);
        if path.is_file() {
            return Some(path);
        }
        let made = render(game_dir, list, value, game.art(list, value), LARGE)
            .and_then(|image| pixels::write_png(&path, &image));
        match made {
            Ok(()) => Some(path),
            Err(e) => {
                eprintln!("preview {key}: {e}");
                self.large_failed.insert(key);
                None
            }
        }
    }
}

pub struct Job {
    pub list: List,
    pub value: String,
    pub art: Option<Art>,
}

pub fn jobs(game: &GameData) -> Vec<Job> {
    List::LOOKS
        .iter()
        .flat_map(|&list| {
            game.choices(list).map(move |c| Job {
                list,
                value: c.value.clone(),
                art: game.art(list, &c.value).cloned(),
            })
        })
        .collect()
}

fn prune(root: &Path, key: &str) {
    let keep = format!(".{}.png", file_part(key));
    for list in List::LOOKS {
        let dir = root.join(list.slug());
        for d in [dir.join("large"), dir] {
            let Ok(entries) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if (name.ends_with(".png") || name.ends_with(".png.tmp")) && !name.ends_with(&keep) {
                    std::fs::remove_file(e.path()).ok();
                }
            }
        }
    }
}

pub fn build(game_dir: &Path, root: &Path, key: &str, jobs: Vec<Job>) -> Previews {
    let started = Instant::now();
    prune(root, key);
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .clamp(1, 8);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                while let Some(job) = jobs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let path = Previews::file(root, key, job.list, &job.value, false);
                    let made = if path.is_file() {
                        Ok(())
                    } else {
                        render(game_dir, job.list, &job.value, job.art.as_ref(), THUMB)
                            .and_then(|image| pixels::write_png(&path, &image))
                    };
                    let size = std::fs::metadata(&path).map_or(0, |m| m.len());
                    results
                        .lock()
                        .expect("results lock")
                        .push((job.list.key(&job.value), made.map(|()| size)));
                }
            });
        }
    });
    let mut index = Index {
        schema: SCHEMA,
        key: key.to_owned(),
        ..Index::default()
    };
    for (k, r) in results.into_inner().expect("results lock") {
        match r {
            Ok(size) => {
                index.bytes += size;
                index.ready.insert(k);
            }
            Err(e) => {
                index.failed.insert(k, e);
            }
        }
    }
    index.millis = started.elapsed().as_millis() as u64;
    if let Ok(bytes) = serde_json::to_vec_pretty(&index) {
        let path = root.join(INDEX);
        let tmp = path.with_extension("json.tmp");
        if std::fs::create_dir_all(root).is_ok() && std::fs::write(&tmp, bytes).is_ok() {
            std::fs::rename(&tmp, &path).ok();
        }
    }
    Previews {
        root: root.to_owned(),
        index,
        large_failed: BTreeSet::new(),
    }
}

pub struct Building(Receiver<Previews>);

impl Building {
    pub fn start(game_dir: PathBuf, root: PathBuf, key: String, jobs: Vec<Job>) -> Building {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            tx.send(build(&game_dir, &root, &key, jobs)).ok();
        });
        Building(rx)
    }

    pub fn poll(&self) -> Option<Option<Previews>> {
        match self.0.try_recv() {
            Ok(p) => Some(Some(p)),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_groups_map_to_archives() {
        assert_eq!(
            asset_group_file("asset_groups/[CHARACTER_PACKAGE]/drivers/male/x/y_helmet.assetgroup")
                .as_deref(),
            Some("2025_asset_groups/character_package/drivers/male/x/y_helmet.erp")
        );
        assert_eq!(
            asset_group_file("asset_groups/[LIVERY_PACKAGE]/tier/t.assetgroup").as_deref(),
            Some("2025_asset_groups/livery_package/tier/t.erp")
        );
        assert!(asset_group_file("asset_groups/[CHARACTER_PACKAGE]/../x.assetgroup").is_none());
        assert!(asset_group_file("asset_groups/[OTHER]/x.assetgroup").is_none());
        assert!(asset_group_file("").is_none());
    }

    #[test]
    fn sources_refuse_odd_names() {
        let root = Path::new("/g");
        assert_eq!(
            source(root, List::Head, "male_ls1", None),
            Some(root.join(format!("{HEADS}/male_ls1.erp")))
        );
        assert!(source(root, List::Head, "../x", None).is_none());
        let art = Art {
            source: "pose_02".into(),
            colours: 0,
        };
        assert_eq!(
            source(root, List::Pose, "Pose03", Some(&art)),
            Some(root.join(format!("{ICONS}/poses/pose_02.erp")))
        );
        assert!(source(root, List::Helmet, "x", None).is_none());
    }

    #[test]
    fn file_names_carry_the_build_key() {
        let p = Previews::file(
            Path::new("/c"),
            "25251167:abc",
            List::Helmet,
            "classic-senna01 x",
            false,
        );
        assert_eq!(p, Path::new("/c/helmet/classic-senna01_x.25251167_abc.png"));
        let l = Previews::file(Path::new("/c"), "k", List::Head, "male_ls1", true);
        assert_eq!(l, Path::new("/c/head/large/male_ls1.k.png"));
    }
}
