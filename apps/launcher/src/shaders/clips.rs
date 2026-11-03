use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

const INDEX_URL: &str = "https://cdn.swofty.net/launcher/preview/clips.json";

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ClipItem {
    pub id: String,
    pub title: String,
    pub dir: PathBuf,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct Entry {
    id: String,
    #[serde(default)]
    title: String,
    url: String,
    #[serde(default)]
    sha256: String,
    #[serde(default)]
    size: u64,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum Index {
    List(Vec<Entry>),
    Wrapped { clips: Vec<Entry> },
}

pub(super) fn dir(root: &Path) -> PathBuf {
    root.join("cache").join("preview")
}

pub(super) fn scan(root: &Path) -> Vec<ClipItem> {
    let Ok(read) = fs::read_dir(dir(root)) else {
        return Vec::new();
    };
    let mut clips: Vec<ClipItem> = read
        .filter_map(Result::ok)
        .filter(|e| e.path().join("clip.json").is_file())
        .map(|e| {
            let dir = e.path();
            let id = e.file_name().to_string_lossy().into_owned();
            let title = fs::read_to_string(dir.join("clip.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .and_then(|v| v.get("title").and_then(|t| t.as_str()).map(str::to_owned))
                .unwrap_or_else(|| id.clone());
            ClipItem { id, title, dir }
        })
        .collect();
    clips.sort_by(|a, b| a.id.cmp(&b.id));
    clips
}

/// Downloads every clip in the CDN index that isn't cached yet, reporting `(fraction, label)`.
pub(super) fn download(root: &Path, progress: &dyn Fn(f32, &str)) -> Result<(), String> {
    progress(0., "Looking for clips");
    let mut response = ureq::get(INDEX_URL)
        .call()
        .map_err(|e| format!("Couldn't reach the clip index: {e}"))?;
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("Couldn't read the clip index: {e}"))?;
    let entries = match serde_json::from_str::<Index>(&text)
        .map_err(|e| format!("The clip index is unreadable: {e}"))?
    {
        Index::List(list) => list,
        Index::Wrapped { clips } => clips,
    };
    let todo: Vec<&Entry> = entries
        .iter()
        .filter(|e| !dir(root).join(&e.id).join("clip.json").is_file())
        .collect();
    let count = todo.len().max(1) as f32;
    for (i, entry) in todo.iter().enumerate() {
        let base = i as f32 / count;
        let label = if entry.title.is_empty() {
            &entry.id
        } else {
            &entry.title
        };
        fetch(root, entry, &|f| progress(base + f / count, label))?;
    }
    progress(1., "Done");
    Ok(())
}

fn fetch(root: &Path, entry: &Entry, progress: &dyn Fn(f32)) -> Result<(), String> {
    if entry.id.is_empty() || entry.id.contains(['/', '\\', '.']) {
        return Err(format!("{:?} isn't a valid clip id", entry.id));
    }
    let mut response = ureq::get(&entry.url)
        .call()
        .map_err(|e| format!("Couldn't download {}: {e}", entry.id))?;
    let total = if entry.size > 0 {
        entry.size
    } else {
        response.body().content_length().unwrap_or(0)
    };
    let mut reader = response.body_mut().as_reader();
    let mut bytes = Vec::with_capacity(total as usize);
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let n = reader
            .read(&mut chunk)
            .map_err(|e| format!("Couldn't download {}: {e}", entry.id))?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..n]);
        if total > 0 {
            progress((bytes.len() as f32 / total as f32).min(1.));
        }
    }
    if !entry.sha256.is_empty() {
        let got: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if !got.eq_ignore_ascii_case(&entry.sha256) {
            return Err(format!("{} didn't download intact", entry.id));
        }
    }
    let target = dir(root).join(&entry.id);
    let partial = dir(root).join(format!("{}.partial", entry.id));
    fs::remove_dir_all(&partial).ok();
    fs::create_dir_all(&partial).map_err(|e| e.to_string())?;
    if let Err(e) = unpack(&bytes, &partial) {
        fs::remove_dir_all(&partial).ok();
        return Err(format!("{} is not a clip: {e}", entry.id));
    }
    fs::remove_dir_all(&target).ok();
    fs::rename(&partial, &target).map_err(|e| e.to_string())
}

fn unpack(bytes: &[u8], into: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
        if file.is_dir() {
            continue;
        }
        let Some(name) = file
            .enclosed_name()
            .and_then(|n| n.file_name().map(PathBuf::from))
        else {
            continue;
        };
        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(|e| e.to_string())?;
        fs::File::create(into.join(name))
            .and_then(|mut out| out.write_all(&data))
            .map_err(|e| e.to_string())?;
    }
    if !into.join("clip.json").is_file() {
        return Err("clip.json is missing".into());
    }
    Ok(())
}
