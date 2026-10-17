use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::builtin::{CORE_ID, core_manifest};
use crate::manifest::{MANIFEST_FILE, ModManifest};
use crate::map::{self, TrackPackage};
use crate::profile::{self, Profile};
use crate::session::Config;
use crate::{Error, Result, io_err};

pub const DEFAULT_PROFILE: &str = "default";

#[derive(Debug, Clone)]
pub struct Store {
    pub root: PathBuf,
}

impl Store {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn open_default() -> Self {
        Self::new(crate::paths::data_dir())
    }

    pub fn mods_dir(&self) -> PathBuf {
        self.root.join("mods")
    }

    pub fn maps_dir(&self) -> PathBuf {
        self.root.join("maps")
    }

    pub fn profiles_dir(&self) -> PathBuf {
        self.root.join("profiles")
    }

    pub fn overlay_dir(&self, profile: &str, buildhash: &str) -> PathBuf {
        self.root.join("cache/overlay").join(profile).join(buildhash)
    }

    pub fn saves_dir(&self, profile: &str) -> PathBuf {
        self.root.join("saves").join(profile)
    }

    pub fn baseline_dir(&self, build: &str) -> PathBuf {
        self.root.join("baseline").join(build)
    }

    pub fn journal_dir(&self) -> PathBuf {
        self.root.join("state/journal")
    }

    pub fn requests_dir(&self) -> PathBuf {
        self.root.join("state/requests")
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.root.join("sessions")
    }

    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.json")
    }

    pub fn bin_dir(&self) -> PathBuf {
        self.root.join("bin")
    }

    pub fn mods(&self) -> Result<BTreeMap<String, ModManifest>> {
        let mut out = BTreeMap::from([(CORE_ID.to_owned(), core_manifest())]);
        for dir in subdirs(&self.mods_dir())? {
            if !dir.join(MANIFEST_FILE).is_file() {
                continue;
            }
            let m = ModManifest::load(&dir)?;
            if m.is_builtin() {
                continue;
            }
            out.insert(m.id.clone(), m);
        }
        Ok(out)
    }

    pub fn install_mod(&self, src: &Path) -> Result<ModManifest> {
        let m = ModManifest::load(src)?;
        if m.is_builtin() {
            return Err(Error::Other(format!("{CORE_ID} is built in")));
        }
        let dest = self.mods_dir().join(&m.id);
        if dest.exists() {
            std::fs::remove_dir_all(&dest).map_err(io_err(&dest))?;
        }
        copy_dir(src, &dest)?;
        Ok(m)
    }

    pub fn maps(&self) -> Result<Vec<TrackPackage>> {
        let mut out = Vec::new();
        for dir in subdirs(&self.maps_dir())? {
            let path = dir.join(map::MANIFEST_FILE);
            if path.is_file() {
                out.push(TrackPackage::parse(
                    &std::fs::read(&path).map_err(io_err(&path))?,
                )?);
            }
        }
        Ok(out)
    }

    pub fn import_map(&self, zip: &Path) -> Result<TrackPackage> {
        let scratch = self.maps_dir().join(".nested");
        let result = map::unwrap_nested(zip, &scratch)
            .and_then(|inner| self.import_package(inner.as_deref().unwrap_or(zip)));
        std::fs::remove_dir_all(&scratch).ok();
        result
    }

    fn import_package(&self, zip: &Path) -> Result<TrackPackage> {
        let pkg = map::read_zip_manifest(zip)?;
        let dest = self.maps_dir().join(&pkg.track_code);
        let tmp = self.maps_dir().join(format!(".{}.importing", pkg.track_code));
        if tmp.exists() {
            std::fs::remove_dir_all(&tmp).map_err(io_err(&tmp))?;
        }
        map::extract_zip(zip, &tmp)?;
        if dest.exists() {
            std::fs::remove_dir_all(&dest).map_err(io_err(&dest))?;
        }
        std::fs::rename(&tmp, &dest).map_err(io_err(&dest))?;
        Ok(pkg)
    }

    pub fn remove_map(&self, code: &str) -> Result<()> {
        let dir = self.maps_dir().join(code);
        if !dir.join(map::MANIFEST_FILE).is_file() {
            return Err(Error::Other(format!("no map {code}")));
        }
        std::fs::remove_dir_all(&dir).map_err(io_err(&dir))
    }

    pub fn profile(&self, name: &str) -> Result<Profile> {
        if name == crate::profile::VANILLA {
            return Ok(Profile::vanilla());
        }
        let path = self.profiles_dir().join(format!("{name}.json"));
        let mut p = if path.is_file() {
            crate::read_json(&path)?
        } else {
            Profile::new(name)
        };
        p.normalise();
        Ok(p)
    }

    pub fn profile_names(&self) -> Result<Vec<String>> {
        let mut names = vec![profile::VANILLA.to_owned()];
        let rd = match std::fs::read_dir(self.profiles_dir()) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                names.push(DEFAULT_PROFILE.into());
                return Ok(names);
            }
            Err(e) => return Err(io_err(self.profiles_dir())(e)),
        };
        let mut found: Vec<String> = rd
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_str()?.strip_suffix(".json")?.to_owned();
                profile::valid_name(&name).then_some(name)
            })
            .collect();
        if found.is_empty() {
            found.push(DEFAULT_PROFILE.into());
        }
        found.sort();
        names.extend(found);
        Ok(names)
    }

    pub fn create_profile(&self, name: &str, from: Option<&str>) -> Result<Profile> {
        if !profile::valid_name(name) {
            return Err(Error::Other(format!(
                "{name:?} is not a valid profile name (letters, digits, - and _)"
            )));
        }
        if self.profile_names()?.iter().any(|n| n == name) {
            return Err(Error::Other(format!("a profile named {name} already exists")));
        }
        let mut p = match from {
            Some(src) => self.profile(src)?,
            None => Profile::new(name),
        };
        p.name = name.into();
        p.normalise();
        self.save_profile(&p)?;
        Ok(p)
    }

    pub fn rename_profile(&self, from: &str, to: &str) -> Result<Profile> {
        if from == profile::VANILLA {
            return Err(Error::Other("the vanilla profile cannot be renamed".into()));
        }
        let p = self.create_profile(to, Some(from))?;
        let old = self.profiles_dir().join(format!("{from}.json"));
        if old.is_file() {
            std::fs::remove_file(&old).map_err(io_err(&old))?;
        }
        let saves = self.saves_dir(from);
        if saves.is_dir() {
            std::fs::rename(&saves, self.saves_dir(to)).map_err(io_err(&saves))?;
        }
        if self.config().active_profile.as_deref().unwrap_or(DEFAULT_PROFILE) == from {
            self.set_active_profile(to)?;
        }
        Ok(p)
    }

    pub fn config(&self) -> Config {
        crate::read_json(&self.config_path()).unwrap_or_default()
    }

    pub fn save_config(&self, config: &Config) -> Result<()> {
        crate::write_json(&self.config_path(), config)
    }

    pub fn active_profile(&self) -> String {
        let names = self.profile_names().unwrap_or_default();
        self.config()
            .active_profile
            .filter(|p| names.contains(p))
            .or_else(|| names.get(1).cloned())
            .unwrap_or_else(|| DEFAULT_PROFILE.into())
    }

    pub fn set_active_profile(&self, name: &str) -> Result<()> {
        let mut config = self.config();
        config.active_profile = Some(name.into());
        self.save_config(&config)
    }

    pub fn latest_session(&self) -> Option<PathBuf> {
        let dir = std::fs::read_to_string(self.sessions_dir().join("latest.txt")).ok()?;
        Some(PathBuf::from(dir.trim()))
    }

    pub fn save_profile(&self, p: &Profile) -> Result<()> {
        if p.is_vanilla() {
            return Err(Error::Other(
                "the vanilla profile is built in and cannot be changed".into(),
            ));
        }
        crate::write_json(&self.profiles_dir().join(format!("{}.json", p.name)), p)
    }
}

fn subdirs(dir: &Path) -> Result<Vec<PathBuf>> {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_err(dir)(e)),
    };
    let mut out: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && !p
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        })
        .collect();
    out.sort();
    Ok(out)
}

fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest).map_err(io_err(dest))?;
    for entry in std::fs::read_dir(src).map_err(io_err(src))? {
        let entry = entry.map_err(io_err(src))?;
        let to = dest.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), &to).map_err(io_err(&to))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::tests::{OKAYAMA_LIKE, write_zip};

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("swofty-store-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn imports_and_lists_maps() {
        let root = tmp("maps");
        let store = Store::new(root.join("data"));
        let zip = root.join("pkg.zip");
        write_zip(&zip, OKAYAMA_LIKE, true);
        let pkg = store.import_map(&zip).unwrap();
        assert_eq!(pkg.track_code, "testring");
        assert!(store.maps_dir().join("testring/testring.erp").is_file());
        assert_eq!(store.maps().unwrap().len(), 1);
        store.remove_map("testring").unwrap();
        assert!(store.maps().unwrap().is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn imports_a_package_wrapped_in_another_zip() {
        let root = tmp("nested");
        let store = Store::new(root.join("data"));
        let inner = root.join("testring_testring.zip");
        write_zip(&inner, OKAYAMA_LIKE, true);
        let outer = root.join("download.zip");
        let mut z = zip::ZipWriter::new(std::fs::File::create(&outer).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        z.start_file("readme.txt", opts).unwrap();
        std::io::Write::write_all(&mut z, b"install with the launcher").unwrap();
        z.start_file("testring_testring.zip", opts).unwrap();
        std::io::Write::write_all(&mut z, &std::fs::read(&inner).unwrap()).unwrap();
        z.finish().unwrap();
        assert_eq!(store.import_map(&outer).unwrap().track_code, "testring");
        assert!(store.maps_dir().join("testring/testring.erp").is_file());
        assert!(!store.maps_dir().join(".nested").exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn installs_template_mod_and_keeps_core() {
        let root = tmp("mods");
        let store = Store::new(&root);
        let template = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/mod");
        let m = store.install_mod(&template).unwrap();
        let mods = store.mods().unwrap();
        assert!(mods.contains_key(CORE_ID));
        assert!(mods.contains_key(&m.id));
        let mut p = store.profile(DEFAULT_PROFILE).unwrap();
        p.set_enabled(&m.id, true).unwrap();
        store.save_profile(&p).unwrap();
        let plan = crate::resolver::resolve(&store.profile(DEFAULT_PROFILE).unwrap(), &mods).unwrap();
        assert_eq!(plan.order, [CORE_ID, m.id.as_str()]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn manages_profiles() {
        let root = tmp("profiles");
        let store = Store::new(&root);
        assert_eq!(store.profile_names().unwrap(), ["vanilla", "default"]);
        assert_eq!(store.active_profile(), "default");
        let mut p = store.profile(DEFAULT_PROFILE).unwrap();
        p.set_enabled("a.b", true).unwrap();
        store.save_profile(&p).unwrap();
        store.set_active_profile(DEFAULT_PROFILE).unwrap();
        store.create_profile("grid", Some(DEFAULT_PROFILE)).unwrap();
        assert!(store.create_profile("grid", None).is_err());
        assert!(store.create_profile("vanilla", None).is_err());
        store.rename_profile(DEFAULT_PROFILE, "main").unwrap();
        assert_eq!(store.profile_names().unwrap(), ["vanilla", "grid", "main"]);
        assert_eq!(store.active_profile(), "main");
        let grid = store.profile("grid").unwrap();
        assert_eq!(grid.enabled().collect::<Vec<_>>(), [CORE_ID, "a.b"]);
        std::fs::remove_dir_all(&root).ok();
    }
}
