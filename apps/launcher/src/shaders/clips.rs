use std::path::Path;

use swofty_preview::{CachedClip, ClipStore};

pub(super) type ClipItem = CachedClip;

fn store(root: &Path) -> ClipStore {
    ClipStore::new(&root.join("cache").join("preview"))
}

pub(super) fn scan(root: &Path) -> Vec<ClipItem> {
    store(root).cached()
}

/// Downloads every clip in the CDN index that isn't cached yet, reporting `(fraction, label)`.
pub(super) fn download(root: &Path, progress: &dyn Fn(f32, &str)) -> Result<(), String> {
    progress(0., "Looking for clips");
    let clips = store(root);
    let cached: Vec<String> = clips.cached().into_iter().map(|c| c.id).collect();
    let todo: Vec<_> = clips
        .index()?
        .into_iter()
        .filter(|e| !cached.contains(&e.id))
        .collect();
    let count = todo.len().max(1) as f32;
    for (i, entry) in todo.iter().enumerate() {
        let base = i as f32 / count;
        let label = if entry.title.is_empty() {
            &entry.id
        } else {
            &entry.title
        };
        clips.download(entry, |done, total| {
            let f = if total > 0 { done as f32 / total as f32 } else { 0. };
            progress(base + f.min(1.) / count, label);
        })?;
    }
    progress(1., "Done");
    Ok(())
}
