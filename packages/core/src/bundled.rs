use std::io::Read;
use std::path::{Path, PathBuf};

use semver::Version;

use crate::manifest::{AddonManifest, MANIFEST_FILE};
use crate::store::Store;
use crate::{Error, Result, io_err};

pub const BUNDLED_ENV: &str = "SWOFTY_BUNDLED_DIR";
pub const RELEASE_DIR: &str = "addons";
pub const UPDATES_DIR: &str = "updates";
pub const PACKAGE_EXT: &str = "swoftyaddon";

pub fn bundled_dir() -> Option<PathBuf> {
    let bin = std::env::current_exe()
        .ok()
        .and_then(|e| Some(e.parent()?.to_path_buf()));
    [
        std::env::var_os(BUNDLED_ENV).map(PathBuf::from),
        bin.as_ref().and_then(|b| Some(b.parent()?.join(RELEASE_DIR))),
        bin.map(|b| b.join(RELEASE_DIR)),
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../native/bundled")),
    ]
    .into_iter()
    .flatten()
    .find(|d| !packages(d).is_empty() || !packages(&d.join(UPDATES_DIR)).is_empty())
}

pub fn packages(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == PACKAGE_EXT))
        .collect();
    out.sort();
    out
}

pub fn package_manifest(pkg: &Path) -> Result<AddonManifest> {
    let file = std::fs::File::open(pkg).map_err(io_err(pkg))?;
    let mut zip = zip::ZipArchive::new(file)?;
    let mut entry = zip
        .by_name(MANIFEST_FILE)
        .map_err(|_| Error::Other(format!("{}: no {MANIFEST_FILE} at the root", pkg.display())))?;
    let mut text = String::new();
    entry.read_to_string(&mut text).map_err(io_err(pkg))?;
    Ok(AddonManifest::parse(&text)?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Install,
    Upgrade,
    Keep,
    Removed,
}

pub fn decide(seeded: Option<&Version>, installed: Option<&Version>, bundled: &Version) -> Action {
    match (seeded, installed) {
        (_, Some(v)) if v < bundled => Action::Upgrade,
        (_, Some(_)) => Action::Keep,
        (None, None) => Action::Install,
        (Some(_), None) => Action::Removed,
    }
}

pub fn seed(store: &Store, dir: &Path) -> Result<Vec<String>> {
    let mut notes = Vec::new();
    let mut config = store.config();
    let mut changed = false;
    for pkg in packages(dir) {
        let m = match package_manifest(&pkg) {
            Ok(m) => m,
            Err(e) => {
                notes.push(format!("bundled addon {} skipped: {e}", pkg.display()));
                continue;
            }
        };
        if store.is_builtin(&m.id) {
            notes.push(format!(
                "bundled addon {} skipped: {} is built in",
                pkg.display(),
                m.id
            ));
            continue;
        }
        let seeded = config.bundled.get(&m.id).and_then(|v| Version::parse(v).ok());
        let installed = installed_version(store, &m.id)?;
        let action = decide(seeded.as_ref(), installed.as_ref(), &m.version);
        match action {
            Action::Install | Action::Upgrade => {
                store.install_addon(&pkg)?;
                notes.push(match (action, &installed) {
                    (Action::Upgrade, Some(old)) => {
                        format!("updated the bundled addon {} from {old} to {}", m.id, m.version)
                    }
                    _ => format!(
                        "installed the bundled addon {} {} (off until you enable it)",
                        m.id, m.version
                    ),
                });
            }
            Action::Keep | Action::Removed => {}
        }
        let record = m.version.to_string();
        if action != Action::Removed && config.bundled.get(&m.id) != Some(&record) {
            config.bundled.insert(m.id.clone(), record);
            changed = true;
        }
    }
    if changed {
        store.save_config(&config)?;
    }
    Ok(notes)
}

pub fn outdated(installed: Option<&Version>, bundled: &Version) -> bool {
    installed.is_some_and(|v| v < bundled)
}

/// Replace each addon installed at an older version than its package in
/// `dir`. An addon that isn't installed stays uninstalled.
pub fn update(store: &Store, dir: &Path) -> Vec<String> {
    let mut notes = Vec::new();
    for pkg in packages(dir) {
        let m = match package_manifest(&pkg) {
            Ok(m) => m,
            Err(e) => {
                notes.push(format!("addon update {} skipped: {e}", pkg.display()));
                continue;
            }
        };
        if store.is_builtin(&m.id) {
            continue;
        }
        let installed = match installed_version(store, &m.id) {
            Ok(v) => v,
            Err(e) => {
                notes.push(format!("addon update for {} skipped: {e}", m.id));
                continue;
            }
        };
        let Some(old) = installed.filter(|v| outdated(Some(v), &m.version)) else {
            continue;
        };
        notes.push(match store.install_addon(&pkg) {
            Ok(_) => format!("updated the addon {} from {old} to {}", m.id, m.version),
            Err(e) => format!(
                "could not update the addon {} from {old} to {}: {e}",
                m.id, m.version
            ),
        });
    }
    notes
}

fn installed_version(store: &Store, id: &str) -> Result<Option<Version>> {
    let dir = store.addons_dir().join(id);
    if !dir.join(MANIFEST_FILE).is_file() {
        return Ok(None);
    }
    Ok(Some(AddonManifest::load(&dir)?.version))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::CORE_ID;
    use crate::store::DEFAULT_PROFILE;

    const ID: &str = "t.bundled";

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("swofty-bundled-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn package(dir: &Path, version: &str) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        for old in packages(dir) {
            std::fs::remove_file(old).unwrap();
        }
        let path = dir.join(format!("{ID}.{PACKAGE_EXT}"));
        let manifest = format!(
            r#"{{"schema_version":1,"id":"{ID}","name":"Bundled","version":"{version}","author":"t","compat":{{"game":"f1_25"}},"options":[{{"id":"on","type":"toggle","default":true,"binds":"native"}}]}}"#
        );
        let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        z.start_file(MANIFEST_FILE, opts).unwrap();
        std::io::Write::write_all(&mut z, manifest.as_bytes()).unwrap();
        z.start_file("native/plugin.dll", opts).unwrap();
        std::io::Write::write_all(&mut z, version.as_bytes()).unwrap();
        z.finish().unwrap();
        path
    }

    fn installed_manifest(store: &Store) -> AddonManifest {
        AddonManifest::load(&store.addons_dir().join(ID)).unwrap()
    }

    fn installed(store: &Store) -> Option<Version> {
        installed_version(store, ID).unwrap()
    }

    #[test]
    fn decides_install_upgrade_keep_and_removed() {
        let b = v("1.1.0");
        assert_eq!(decide(None, None, &b), Action::Install);
        assert_eq!(decide(None, Some(&v("1.0.0")), &b), Action::Upgrade);
        assert_eq!(decide(None, Some(&v("1.1.0")), &b), Action::Keep);
        assert_eq!(decide(Some(&v("1.0.0")), Some(&v("1.0.0")), &b), Action::Upgrade);
        assert_eq!(decide(Some(&v("1.1.0")), Some(&v("1.1.0")), &b), Action::Keep);
        assert_eq!(decide(Some(&v("1.1.0")), Some(&v("2.0.0")), &b), Action::Keep);
        assert_eq!(decide(Some(&v("1.0.0")), None, &b), Action::Removed);
    }

    #[test]
    fn seeds_an_addon_that_is_off_in_new_and_existing_profiles() {
        let root = tmp("off");
        let store = Store::new(root.join("data"));
        std::fs::create_dir_all(store.profiles_dir()).unwrap();
        std::fs::write(
            store.profiles_dir().join("existing.json"),
            r#"{"schema_version":2,"name":"existing","addons":[]}"#,
        )
        .unwrap();
        let dir = root.join("bundled");
        package(&dir, "1.0.0");
        let notes = seed(&store, &dir).unwrap();
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].contains("off until you enable it"));
        assert_eq!(installed(&store), Some(v("1.0.0")));
        assert!(store.addons_dir().join(ID).join("native/plugin.dll").is_file());
        assert_eq!(store.config().bundled[ID], "1.0.0");

        for name in ["existing", DEFAULT_PROFILE] {
            let p = store.profile(name).unwrap();
            assert_eq!(p.enabled().collect::<Vec<_>>(), [CORE_ID], "{name}");
        }
        store.create_profile("fresh", None).unwrap();
        assert!(!store.profile("fresh").unwrap().enabled().any(|id| id == ID));

        let reg = store.registry().unwrap();
        let e = reg.get(ID).unwrap();
        assert!(!e.locked && !e.default_on && e.declared);
        assert!(!store.is_builtin(ID));

        let mut p = store.profile("existing").unwrap();
        reg.set_enabled(&mut p, ID, true).unwrap();
        store.save_profile(&p).unwrap();
        assert!(store.profile("existing").unwrap().enabled().any(|id| id == ID));
        let mut p = store.profile("existing").unwrap();
        reg.set_enabled(&mut p, ID, false).unwrap();
        store.save_profile(&p).unwrap();
        assert!(!store.profile("existing").unwrap().enabled().any(|id| id == ID));

        assert!(seed(&store, &dir).unwrap().is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_removed_addon_stays_removed() {
        let root = tmp("removed");
        let store = Store::new(root.join("data"));
        let dir = root.join("bundled");
        package(&dir, "1.0.0");
        seed(&store, &dir).unwrap();
        let mut p = store.profile(DEFAULT_PROFILE).unwrap();
        p.set_enabled(ID, true).unwrap();
        store.save_profile(&p).unwrap();
        store.remove_addon(ID).unwrap();
        assert_eq!(installed(&store), None);
        assert!(
            !store
                .profile(DEFAULT_PROFILE)
                .unwrap()
                .enabled()
                .any(|id| id == ID)
        );
        assert!(seed(&store, &dir).unwrap().is_empty());
        assert_eq!(installed(&store), None);
        package(&dir, "2.0.0");
        assert!(seed(&store, &dir).unwrap().is_empty());
        assert_eq!(installed(&store), None);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn upgrades_only_to_a_newer_bundled_version() {
        let root = tmp("upgrade");
        let store = Store::new(root.join("data"));
        let dir = root.join("bundled");
        package(&dir, "1.0.0");
        seed(&store, &dir).unwrap();
        let mut p = store.profile(DEFAULT_PROFILE).unwrap();
        p.set_enabled(ID, true).unwrap();
        let m = installed_manifest(&store);
        assert!(p.set_option(&m, "on", serde_json::json!(false)).unwrap());
        store.save_profile(&p).unwrap();
        let before = store.profile(DEFAULT_PROFILE).unwrap();

        package(&dir, "1.1.0");
        let notes = seed(&store, &dir).unwrap();
        assert_eq!(notes, ["updated the bundled addon t.bundled from 1.0.0 to 1.1.0"]);
        assert_eq!(installed(&store), Some(v("1.1.0")));
        let dll = store.addons_dir().join(ID).join("native/plugin.dll");
        assert_eq!(std::fs::read_to_string(&dll).unwrap(), "1.1.0");
        assert_eq!(store.config().bundled[ID], "1.1.0");
        assert_eq!(store.profile(DEFAULT_PROFILE).unwrap(), before);

        package(&dir, "1.1.0");
        assert!(seed(&store, &dir).unwrap().is_empty());
        package(&dir, "1.0.5");
        assert!(seed(&store, &dir).unwrap().is_empty());
        assert_eq!(installed(&store), Some(v("1.1.0")));
        assert_eq!(std::fs::read_to_string(&dll).unwrap(), "1.1.0");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn keeps_a_newer_copy_the_player_installed() {
        let root = tmp("newer");
        let store = Store::new(root.join("data"));
        let own = package(&root.join("own"), "3.0.0");
        store.install_addon(&own).unwrap();
        let dir = root.join("bundled");
        package(&dir, "1.0.0");
        assert!(seed(&store, &dir).unwrap().is_empty());
        assert_eq!(installed(&store), Some(v("3.0.0")));
        assert_eq!(store.config().bundled[ID], "1.0.0");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn decides_which_installed_addons_are_outdated() {
        let b = v("1.1.0");
        assert!(!outdated(None, &b));
        assert!(outdated(Some(&v("1.0.0")), &b));
        assert!(!outdated(Some(&v("1.1.0")), &b));
        assert!(!outdated(Some(&v("2.0.0")), &b));
    }

    #[test]
    fn updates_only_addons_installed_at_an_older_version() {
        let root = tmp("updates");
        let store = Store::new(root.join("data"));
        let bundled = root.join("bundled");
        let updates = bundled.join(UPDATES_DIR);
        package(&updates, "1.1.0");
        assert!(seed(&store, &bundled).unwrap().is_empty());
        assert!(update(&store, &updates).is_empty());
        assert_eq!(installed(&store), None);

        let own = package(&root.join("own"), "1.0.0");
        store.install_addon(&own).unwrap();
        let mut p = store.profile(DEFAULT_PROFILE).unwrap();
        p.set_enabled(ID, true).unwrap();
        let m = installed_manifest(&store);
        assert!(p.set_option(&m, "on", serde_json::json!(false)).unwrap());
        store.save_profile(&p).unwrap();
        store.create_profile("other", None).unwrap();
        let profiles = || {
            (
                store.profile(DEFAULT_PROFILE).unwrap(),
                store.profile("other").unwrap(),
            )
        };
        let before = profiles();
        assert!(before.0.enabled().any(|id| id == ID));
        assert!(!before.1.enabled().any(|id| id == ID));

        let notes = update(&store, &updates);
        assert_eq!(notes, ["updated the addon t.bundled from 1.0.0 to 1.1.0"]);
        assert_eq!(installed(&store), Some(v("1.1.0")));
        let dll = store.addons_dir().join(ID).join("native/plugin.dll");
        assert_eq!(std::fs::read_to_string(&dll).unwrap(), "1.1.0");
        assert_eq!(profiles(), before);
        assert!(store.config().bundled.is_empty());

        assert!(update(&store, &updates).is_empty());
        package(&updates, "1.0.5");
        assert!(update(&store, &updates).is_empty());
        assert_eq!(installed(&store), Some(v("1.1.0")));
        assert_eq!(std::fs::read_to_string(&dll).unwrap(), "1.1.0");

        store.remove_addon(ID).unwrap();
        package(&updates, "2.0.0");
        assert!(update(&store, &updates).is_empty());
        assert_eq!(installed(&store), None);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn skips_packages_without_a_root_manifest() {
        let root = tmp("bad");
        let store = Store::new(root.join("data"));
        let dir = root.join("bundled");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("broken.{PACKAGE_EXT}"));
        let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        z.start_file("readme.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        z.finish().unwrap();
        let notes = seed(&store, &dir).unwrap();
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("no swofty.json at the root"), "{notes:?}");
        assert!(store.config().bundled.is_empty());
        std::fs::remove_dir_all(&root).ok();
    }
}
