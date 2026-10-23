use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use swofty_formats::erp::{Erp, Resource};
use swofty_formats::texture;

use crate::assets::Files;
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

pub fn source(list: List, value: &str, art: Option<&Art>) -> Option<String> {
    match list {
        List::Head => plain(value).then(|| format!("{HEADS}/{value}.erp")),
        List::Helmet | List::Suit | List::Gloves => asset_group_file(&art?.source),
        List::Pose | List::Emote => {
            let name = &art?.source;
            let dir = if list == List::Pose { "poses" } else { "emotes" };
            plain(name).then(|| format!("{ICONS}/{dir}/{name}.erp"))
        }
        List::Nationality | List::Voice => None,
    }
}

pub fn mark_missing(data: &mut GameData, files: &Files) {
    let mut missing = BTreeSet::new();
    let mut supplied = BTreeMap::new();
    for list in [List::Head, List::Helmet, List::Suit, List::Gloves] {
        for c in data.list(list) {
            let Some(rel) = source(list, &c.value, data.art(list, &c.value)) else {
                missing.insert(list.key(&c.value));
                continue;
            };
            if let Some(s) = files.addon(&rel) {
                supplied.insert(list.key(&c.value), s.addon.clone());
            } else if files.game_dir.is_some() && files.find(&rel).is_none() {
                missing.insert(list.key(&c.value));
            }
        }
    }
    data.missing = missing;
    data.supplied = supplied;
}

fn surfaces(erp: &Path, want: impl Fn(&str) -> bool) -> Result<Vec<Resource>, String> {
    let mut found = Erp::read_matching(erp, |n| {
        n.ends_with(".image") && !n.contains("charname") && want(n)
    })
    .map_err(|e| format!("{}: {e}", erp.display()))?;
    found.sort_by(|a, b| a.name.len().cmp(&b.name.len()).then(a.name.cmp(&b.name)));
    Ok(found)
}

fn decode(surface: &Resource, files: &Files, want: u32) -> Result<Image, String> {
    texture::decode_near(surface, files.game_dir.as_deref(), want).map_err(|e| e.to_string())
}

fn located(files: &Files, rel: Option<String>, what: &str) -> Result<PathBuf, String> {
    let rel = rel.ok_or_else(|| format!("no {what}"))?;
    files.find(&rel).ok_or_else(|| format!("{rel}: not installed"))
}

fn kit_suffix(list: List) -> &'static str {
    match list {
        List::Helmet => "_helmet",
        List::Suit => "_livery_v2",
        _ => "_glove",
    }
}

fn kit_texture(files: &Files, list: List, art: &Art, want: u32) -> Result<Image, String> {
    let erp = located(files, source(list, "", Some(art)), "asset group")?;
    let suffix = kit_suffix(list);
    let (d, m) = (format!("{suffix}_d.tif.image"), format!("{suffix}_m.tif.image"));
    let found = surfaces(&erp, |n| n.ends_with(&d) || n.ends_with(&m))?;
    let diffuse = found.iter().find(|r| r.name.ends_with(&d));
    let mask = found.iter().find(|r| r.name.ends_with(&m));
    match (diffuse, mask) {
        (Some(diffuse), Some(mask)) if art.colours > 0 => {
            let image = decode(diffuse, files, want)?;
            let mask = decode(mask, files, image.width.max(image.height))?;
            Ok(pixels::tint(&image, &mask, &NEUTRAL))
        }
        (Some(diffuse), _) => Ok(pixels::opaque(decode(diffuse, files, want)?)),
        (None, Some(mask)) => {
            let mask = decode(mask, files, want)?;
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

pub fn render(files: &Files, list: List, value: &str, art: Option<&Art>, size: u32) -> Result<Image, String> {
    match list {
        List::Head => {
            let erp = located(files, source(list, value, None), "head name")?;
            let found = surfaces(&erp, |n| n.ends_with("_head_d.tif.image"))?;
            let surface = found
                .iter()
                .find(|r| r.name.ends_with(&format!("/{value}_head_d.tif.image")))
                .or_else(|| found.first())
                .ok_or_else(|| format!("{}: no head texture", erp.display()))?;
            let face = decode(surface, files, size * 10 / 7)?;
            Ok(pixels::opaque(pixels::resize(&face, &FACE, size, size)))
        }
        List::Helmet => {
            let art = art.ok_or("no helmet data")?;
            let width = size * 4 / 3;
            let image = kit_texture(files, list, art, width)?;
            let height = (width * image.height / image.width.max(1)).max(1);
            Ok(pixels::resize(&image, &WHOLE, width, height))
        }
        List::Suit | List::Gloves => {
            let art = art.ok_or("no kit data")?;
            let image = kit_texture(files, list, art, 128)?;
            Ok(pixels::chips(
                &pixels::dominant(&image, 3),
                size * 2 / 3,
                size * 2 / 3,
            ))
        }
        List::Pose | List::Emote => {
            let erp = located(files, source(list, value, art), "icon name")?;
            let found = surfaces(&erp, |_| true)?;
            let surface = found
                .first()
                .ok_or_else(|| format!("{}: no icon", erp.display()))?;
            let icon = decode(surface, files, size)?;
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

    pub fn large(&mut self, files: &Files, game: &GameData, list: List, value: &str) -> Option<PathBuf> {
        let key = list.key(value);
        if !self.index.ready.contains(&key) || self.large_failed.contains(&key) {
            return None;
        }
        let path = Self::file(&self.root, &self.index.key, list, value, true);
        if path.is_file() {
            return Some(path);
        }
        let made = render(files, list, value, game.art(list, value), LARGE)
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

pub fn missing_jobs(game: &GameData, index: &Index) -> Vec<Job> {
    jobs(game)
        .into_iter()
        .filter(|j| {
            let key = j.list.key(&j.value);
            !index.ready.contains(&key) && !index.failed.contains_key(&key)
        })
        .collect()
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

pub fn build(files: &Files, root: &Path, key: &str, jobs: Vec<Job>, before: Option<Index>) -> Previews {
    let started = Instant::now();
    if before.is_none() {
        prune(root, key);
    }
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
                        render(files, job.list, &job.value, job.art.as_ref(), THUMB)
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
    let mut index = before.unwrap_or_else(|| Index {
        schema: SCHEMA,
        key: key.to_owned(),
        ..Index::default()
    });
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
    index.millis += started.elapsed().as_millis() as u64;
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
    pub fn start(
        files: Files,
        root: PathBuf,
        key: String,
        jobs: Vec<Job>,
        before: Option<Index>,
    ) -> Building {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            tx.send(build(&files, &root, &key, jobs, before)).ok();
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
        assert_eq!(
            source(List::Head, "male_ls1", None),
            Some(format!("{HEADS}/male_ls1.erp"))
        );
        assert!(source(List::Head, "../x", None).is_none());
        let art = Art {
            source: "pose_02".into(),
            colours: 0,
        };
        assert_eq!(
            source(List::Pose, "Pose03", Some(&art)),
            Some(format!("{ICONS}/poses/pose_02.erp"))
        );
        assert!(source(List::Helmet, "x", None).is_none());
    }

    #[test]
    fn heads_an_addon_puts_in_the_game_count_as_installed() {
        let dir = std::env::temp_dir().join(format!("dm-missing-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let game = dir.join("game");
        std::fs::create_dir_all(game.join(HEADS)).unwrap();
        std::fs::write(game.join(HEADS).join("charles_leclerc.erp"), "erp").unwrap();
        std::fs::create_dir_all(dir.join("addon/files")).unwrap();
        std::fs::write(dir.join("addon/files/kvyat.erp"), "erp").unwrap();
        let mut data = GameData {
            heads: ["charles_leclerc", "daniil_kvyat", "nikita_mazepin"]
                .iter()
                .map(|v| crate::game::Choice {
                    value: (*v).to_owned(),
                    label: (*v).to_owned(),
                })
                .collect(),
            ..GameData::default()
        };
        let mut files = Files {
            game_dir: Some(game.clone()),
            ..Files::default()
        };
        files.supplied.insert(
            crate::assets::rel_key(&format!("{HEADS}/daniil_kvyat.erp")),
            crate::assets::Supplied {
                file: dir.join("addon/files/kvyat.erp"),
                addon: "More Driver Models".into(),
            },
        );
        mark_missing(&mut data, &files);
        assert!(data.available(List::Head, "charles_leclerc"));
        assert!(data.available(List::Head, "daniil_kvyat"));
        assert!(!data.available(List::Head, "nikita_mazepin"));
        assert_eq!(
            data.supplied.get("head/daniil_kvyat").map(String::as_str),
            Some("More Driver Models")
        );
        std::fs::remove_dir_all(&dir).ok();
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
