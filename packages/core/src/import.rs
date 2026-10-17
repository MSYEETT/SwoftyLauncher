use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use swofty_addon::launcher::Claim;

use crate::manifest::MANIFEST_FILE;
use crate::map;
use crate::registry::Registry;

pub const ADDON_EXTENSION: &str = "swoftymod";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Addon,
    Map,
    Importer(String),
}

pub fn route(registry: &Registry, path: &Path) -> Option<Route> {
    let mut claims = vec![(Route::Addon, addon_claim(path)), (Route::Map, map_claim(path))];
    claims.extend(registry.iter().filter_map(|e| {
        let claim = e.addon.importer()?.claims(path);
        Some((Route::Importer(e.id().to_owned()), claim))
    }));
    if let Some((route, _)) = claims.iter().find(|(_, c)| *c == Claim::Yes) {
        return Some(route.clone());
    }
    let mut maybe = claims.into_iter().filter(|(_, c)| *c == Claim::Maybe);
    match (maybe.next(), maybe.next()) {
        (Some((route, _)), None) => Some(route),
        _ => None,
    }
}

pub fn route_all(registry: &Registry, paths: &[PathBuf]) -> (Vec<(Route, Vec<PathBuf>)>, Vec<PathBuf>) {
    let mut batches: Vec<(Route, Vec<PathBuf>)> = Vec::new();
    let mut unclaimed = Vec::new();
    for path in paths {
        match route(registry, path) {
            Some(r) => match batches.iter_mut().find(|(b, _)| *b == r) {
                Some((_, batch)) => batch.push(path.clone()),
                None => batches.push((r, vec![path.clone()])),
            },
            None => unclaimed.push(path.clone()),
        }
    }
    (batches, unclaimed)
}

pub fn addon_claim(path: &Path) -> Claim {
    if path.is_dir() {
        return claim(addon_root(path).is_some());
    }
    if has_extension(path, ADDON_EXTENSION) {
        return Claim::Yes;
    }
    claim(has_extension(path, "zip") && zip_names(path).is_some_and(|n| holds_addon(&n)))
}

pub fn map_claim(path: &Path) -> Claim {
    if !has_extension(path, "zip") {
        return Claim::No;
    }
    let Some(names) = zip_names(path) else {
        return Claim::No;
    };
    let nested = names
        .iter()
        .filter(|n| n.to_ascii_lowercase().ends_with(".zip"))
        .count();
    claim(names.iter().any(|n| n == map::MANIFEST_FILE) || (nested == 1 && !holds_addon(&names)))
}

pub fn addon_root(dir: &Path) -> Option<PathBuf> {
    if dir.join(MANIFEST_FILE).is_file() {
        return Some(dir.to_path_buf());
    }
    let entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| Some(e.ok()?.path()))
        .collect();
    match entries.as_slice() {
        [only] if only.join(MANIFEST_FILE).is_file() => Some(only.clone()),
        _ => None,
    }
}

fn holds_addon(names: &[String]) -> bool {
    if names.iter().any(|n| n == MANIFEST_FILE) {
        return true;
    }
    let tops: BTreeSet<&str> = names.iter().filter_map(|n| n.split('/').next()).collect();
    match tops.iter().collect::<Vec<_>>().as_slice() {
        [top] => names.iter().any(|n| *n == format!("{top}/{MANIFEST_FILE}")),
        _ => false,
    }
}

fn zip_names(path: &Path) -> Option<Vec<String>> {
    let zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    zip.file_names().map(|n| n.ok().map(|n| n.into_owned())).collect()
}

fn has_extension(path: &Path, ext: &str) -> bool {
    path.is_file() && path.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

fn claim(yes: bool) -> Claim {
    if yes { Claim::Yes } else { Claim::No }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use swofty_addon::launcher::{Addon, Builtin, Cx, Importer, Progress};

    use super::*;
    use crate::manifest::AddonManifest;
    use crate::map::tests::{OKAYAMA_LIKE, write_zip};
    use crate::store::Store;

    struct Fake {
        manifest: AddonManifest,
        yes: &'static str,
        maybe: &'static str,
    }

    impl Addon for Fake {
        fn manifest(&self) -> &AddonManifest {
            &self.manifest
        }

        fn importer(&self) -> Option<&dyn Importer> {
            Some(self)
        }
    }

    impl Importer for Fake {
        fn claims(&self, path: &Path) -> Claim {
            match path.extension().and_then(|e| e.to_str()) {
                Some(e) if e == self.yes => Claim::Yes,
                Some(e) if e == self.maybe => Claim::Maybe,
                _ => Claim::No,
            }
        }

        fn import(&self, _: &Cx, paths: &[PathBuf], _: &dyn Progress) -> swofty_addon::Result<Vec<String>> {
            Ok(paths.iter().map(|p| p.display().to_string()).collect())
        }
    }

    fn fake(id: &str, yes: &'static str, maybe: &'static str) -> Builtin {
        let manifest = AddonManifest::parse(&format!(
            r#"{{"schema_version":1,"id":"{id}","name":"{id}","version":"1.0.0","author":"t","compat":{{"game":"f1_25"}}}}"#
        ))
        .unwrap();
        Builtin {
            addon: Arc::new(Fake { manifest, yes, maybe }),
            locked: false,
            default_on: true,
        }
    }

    fn zip_of(path: &Path, files: &[&str]) {
        let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for name in files {
            z.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut z, b"{}").unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn routes_drops_to_core_then_the_claiming_addon() {
        let root = std::env::temp_dir().join(format!("swofty-import-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(root.join("my-addon")).unwrap();
        std::fs::write(root.join("my-addon").join(MANIFEST_FILE), "{}").unwrap();
        std::fs::create_dir_all(root.join("loose")).unwrap();
        zip_of(&root.join("pack.swoftymod"), &[MANIFEST_FILE]);
        zip_of(
            &root.join("wrapped.zip"),
            &["my-addon/swofty.json", "my-addon/plugin.dll"],
        );
        write_zip(&root.join("track.zip"), OKAYAMA_LIKE, true);
        zip_of(&root.join("download.zip"), &["readme.txt", "track.zip"]);
        zip_of(&root.join("other.zip"), &["readme.txt"]);
        std::fs::write(root.join("livery.serps"), "x").unwrap();
        std::fs::write(root.join("notes.txt"), "x").unwrap();
        let store = Store::new(root.join("data")).with_builtins(vec![fake("t.mods", "serps", "zip")]);
        let reg = Registry::load(&store).unwrap();
        let at = |name: &str| route(&reg, &root.join(name));
        assert_eq!(at("my-addon"), Some(Route::Addon));
        assert_eq!(at("pack.swoftymod"), Some(Route::Addon));
        assert_eq!(at("wrapped.zip"), Some(Route::Addon));
        assert_eq!(at("track.zip"), Some(Route::Map));
        assert_eq!(at("download.zip"), Some(Route::Map));
        assert_eq!(at("livery.serps"), Some(Route::Importer("t.mods".into())));
        assert_eq!(at("other.zip"), Some(Route::Importer("t.mods".into())));
        assert_eq!(at("loose"), None);
        assert_eq!(at("notes.txt"), None);
        let paths: Vec<PathBuf> = ["livery.serps", "notes.txt", "pack.swoftymod", "other.zip"]
            .iter()
            .map(|n| root.join(n))
            .collect();
        let (batches, unclaimed) = route_all(&reg, &paths);
        assert_eq!(
            batches,
            [
                (
                    Route::Importer("t.mods".into()),
                    vec![paths[0].clone(), paths[3].clone()]
                ),
                (Route::Addon, vec![paths[2].clone()]),
            ]
        );
        assert_eq!(unclaimed, [paths[1].clone()]);
        let two = Store::new(root.join("data"))
            .with_builtins(vec![fake("t.mods", "serps", "zip"), fake("t.more", "lng", "zip")]);
        let reg = Registry::load(&two).unwrap();
        assert_eq!(route(&reg, &root.join("other.zip")), None);
        assert_eq!(
            route(&reg, &root.join("livery.serps")),
            Some(Route::Importer("t.mods".into()))
        );
        std::fs::remove_dir_all(&root).ok();
    }
}
