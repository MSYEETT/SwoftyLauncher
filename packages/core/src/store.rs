use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use swofty_addon::launcher::Builtin;

use crate::builtin::{CORE_ID, core_manifest};
use crate::manifest::{AddonManifest, MANIFEST_FILE};
use crate::map::{self, TrackPackage};
use crate::profile::{self, Profile};
use crate::session::Config;
use crate::settings::{self, Merge, Requests, SessionSettings};
use crate::{Error, Result, io_err};

pub const DEFAULT_PROFILE: &str = "default";
pub const DATA_VERSION: u32 = 2;
const LEGACY_ADDONS_DIR: &str = "mods";

#[derive(Debug, Clone)]
pub struct Store {
    pub root: PathBuf,
    builtins: Arc<[Builtin]>,
}

impl Store {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            builtins: Arc::new([]),
        }
    }

    pub fn with_builtins(mut self, builtins: Vec<Builtin>) -> Self {
        self.builtins = builtins.into();
        self
    }

    pub fn builtins(&self) -> &[Builtin] {
        &self.builtins
    }

    pub fn registry(&self) -> Result<crate::registry::Registry> {
        crate::registry::Registry::load(self)
    }

    pub fn open_default() -> Self {
        Self::new(crate::paths::data_dir())
    }

    pub fn addons_dir(&self) -> PathBuf {
        self.root.join("addons")
    }

    pub fn addon_data_dir(&self, id: &str) -> PathBuf {
        self.root.join("addon-data").join(id)
    }

    pub fn addon_cache_dir(&self, id: &str) -> PathBuf {
        self.root.join("cache/addons").join(id)
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

    pub fn migrate(&self) -> Result<Vec<String>> {
        let mut notes = Vec::new();
        if !self.root.is_dir() {
            return Ok(notes);
        }
        let path = self.config_path();
        let config: Result<Config> = if path.is_file() {
            crate::read_json(&path)
        } else {
            Ok(Config::default())
        };
        if config.as_ref().is_ok_and(|c| c.data_version >= DATA_VERSION) {
            return Ok(notes);
        }
        let old = self.root.join(LEGACY_ADDONS_DIR);
        let new = self.addons_dir();
        if old.is_dir() && !new.exists() {
            std::fs::rename(&old, &new).map_err(io_err(&old))?;
            notes.push(format!("moved {} to {}", old.display(), new.display()));
        } else if old.is_dir() {
            let taken: BTreeSet<String> = subdirs(&new)?.iter().filter_map(|d| manifest_id(d)).collect();
            for dir in subdirs(&old)? {
                let to = new.join(dir.file_name().expect("has a name"));
                match manifest_id(&dir) {
                    Some(id) if !taken.contains(&id) && !to.exists() => {
                        std::fs::rename(&dir, &to).map_err(io_err(&dir))?;
                        notes.push(format!("moved {} to {}", dir.display(), to.display()));
                    }
                    Some(id) => notes.push(format!(
                        "kept {}: {} already has {id}",
                        dir.display(),
                        new.display()
                    )),
                    None => notes.push(format!(
                        "kept {}: no addon id in its {MANIFEST_FILE}",
                        dir.display()
                    )),
                }
            }
            if std::fs::remove_dir(&old).is_ok() {
                notes.push(format!("removed the empty {}", old.display()));
            }
        }
        match config {
            Ok(mut config) => {
                config.data_version = DATA_VERSION;
                self.save_config(&config)?;
            }
            Err(e) => notes.push(format!("data_version not recorded: {e}")),
        }
        Ok(notes)
    }

    pub fn addons(&self) -> Result<BTreeMap<String, AddonManifest>> {
        let mut out = BTreeMap::from([(CORE_ID.to_owned(), core_manifest())]);
        for b in self.builtins.iter() {
            let m = b.addon.manifest();
            out.entry(m.id.clone()).or_insert_with(|| m.clone());
        }
        for dir in subdirs(&self.addons_dir())? {
            if !dir.join(MANIFEST_FILE).is_file() {
                continue;
            }
            let m = AddonManifest::load(&dir)?;
            if self.is_builtin(&m.id) {
                continue;
            }
            out.insert(m.id.clone(), m);
        }
        Ok(out)
    }

    pub fn is_builtin(&self, id: &str) -> bool {
        id == CORE_ID || self.builtins.iter().any(|b| b.addon.manifest().id == id)
    }

    fn adopt_builtins(&self, p: &mut Profile) {
        if p.is_vanilla() {
            return;
        }
        for b in self.builtins.iter() {
            let id = &b.addon.manifest().id;
            let listed = p.addons.iter().any(|a| &a.id == id);
            if b.locked || (b.default_on && !listed) {
                p.set_enabled(id, true).expect("not vanilla");
            }
        }
    }

    pub fn install_addon(&self, src: &Path) -> Result<AddonManifest> {
        if src.is_file() {
            let scratch = self.addons_dir().join(".importing");
            let result = self.install_addon_zip(src, &scratch);
            std::fs::remove_dir_all(&scratch).ok();
            return result;
        }
        let src = &crate::import::addon_root(src).unwrap_or_else(|| src.to_path_buf());
        let m = AddonManifest::load(src)?;
        if self.is_builtin(&m.id) {
            return Err(Error::Other(format!("{} is built in", m.id)));
        }
        let dest = self.addons_dir().join(&m.id);
        if dest.exists() {
            std::fs::remove_dir_all(&dest).map_err(io_err(&dest))?;
        }
        copy_dir(src, &dest)?;
        Ok(m)
    }

    fn install_addon_zip(&self, zip: &Path, scratch: &Path) -> Result<AddonManifest> {
        if scratch.exists() {
            std::fs::remove_dir_all(scratch).map_err(io_err(scratch))?;
        }
        map::unzip(zip, scratch, Error::Other)?;
        let root = crate::import::addon_root(scratch)
            .ok_or_else(|| Error::Other(format!("{} has no {MANIFEST_FILE}", zip.display())))?;
        self.install_addon(&root)
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
        map::art(&dest, &pkg);
        Ok(pkg)
    }

    pub fn map_art(&self, pkg: &TrackPackage) -> map::MapArt {
        map::art(&self.maps_dir().join(&pkg.track_code), pkg)
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
        self.adopt_builtins(&mut p);
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
        self.adopt_builtins(&mut p);
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

    pub fn requests_path(&self, profile: &str) -> PathBuf {
        self.requests_dir().join(format!("{profile}.json"))
    }

    pub fn session_settings(&self, profile: &str) -> Result<SessionSettings> {
        let requests = crate::paths::game_folder_view(&self.requests_path(profile));
        SessionSettings::build(
            &self.profile(profile)?,
            &self.addons()?,
            &self.maps()?,
            Some(requests),
        )
    }

    pub fn merge_requests(&self, profile: &str, consume: bool) -> Result<Option<Merge>> {
        let path = self.requests_path(profile);
        if !path.is_file() {
            return Ok(None);
        }
        let req: Requests = crate::read_json(&path)?;
        let merged = if req.profile.is_empty() || req.profile == profile {
            let mut p = self.profile(profile)?;
            let out = settings::merge(&mut p, &self.addons()?, &req);
            if !out.changed.is_empty() {
                self.save_profile(&p)?;
            }
            Some(out)
        } else {
            None
        };
        if consume {
            std::fs::remove_file(&path).map_err(io_err(&path))?;
        }
        Ok(merged)
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

fn manifest_id(dir: &Path) -> Option<String> {
    let doc: serde_json::Value = crate::read_json(&dir.join(MANIFEST_FILE)).ok()?;
    Some(doc.get("id")?.as_str()?.to_owned())
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
    fn installs_template_addon_and_keeps_core() {
        let root = tmp("addons");
        let store = Store::new(&root);
        let template = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/addon");
        let m = store.install_addon(&template).unwrap();
        assert!(store.addons_dir().join(&m.id).join(MANIFEST_FILE).is_file());
        let addons = store.addons().unwrap();
        assert!(addons.contains_key(CORE_ID));
        assert!(addons.contains_key(&m.id));
        let mut p = store.profile(DEFAULT_PROFILE).unwrap();
        p.set_enabled(&m.id, true).unwrap();
        store.save_profile(&p).unwrap();
        let plan = crate::resolver::resolve(&store.profile(DEFAULT_PROFILE).unwrap(), &addons).unwrap();
        assert_eq!(plan.order, [CORE_ID, m.id.as_str()]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn installs_addon_zips() {
        let root = tmp("addon-zips");
        let store = Store::new(root.join("data"));
        let manifest = r#"{"schema_version":1,"id":"t.zipped","name":"Zipped","version":"1.0.0","author":"t","compat":{"game":"f1_25"}}"#;
        for (file, prefix) in [("flat.swoftymod", ""), ("wrapped.zip", "zipped/")] {
            let path = root.join(file);
            let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            z.start_file(format!("{prefix}{MANIFEST_FILE}"), opts).unwrap();
            std::io::Write::write_all(&mut z, manifest.as_bytes()).unwrap();
            z.start_file(format!("{prefix}plugins/a.dll"), opts).unwrap();
            z.finish().unwrap();
            assert_eq!(store.install_addon(&path).unwrap().id, "t.zipped");
            assert!(store.addons_dir().join("t.zipped/plugins/a.dll").is_file());
            assert!(!store.addons_dir().join(".importing").exists());
        }
        let bare = root.join("bare.zip");
        zip::ZipWriter::new(std::fs::File::create(&bare).unwrap())
            .finish()
            .unwrap();
        assert!(store.install_addon(&bare).is_err());
        std::fs::remove_dir_all(&root).ok();
    }

    fn write_addon(dir: &Path, id: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join(MANIFEST_FILE),
            format!(
                r#"{{"schema_version":1,"id":"{id}","name":"{id}","version":"1.0.0","author":"t","compat":{{"game":"f1_25"}}}}"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn migrate_renames_the_mods_dir() {
        let root = tmp("migrate-rename");
        let store = Store::new(&root);
        write_addon(&root.join("mods/a.one"), "a.one");
        std::fs::write(store.config_path(), r#"{"active_profile":"grid"}"#).unwrap();
        assert_eq!(store.migrate().unwrap().len(), 1);
        assert!(!root.join("mods").exists());
        assert!(store.addons().unwrap().contains_key("a.one"));
        let config = store.config();
        assert_eq!(config.data_version, DATA_VERSION);
        assert_eq!(config.active_profile.as_deref(), Some("grid"));
        write_addon(&root.join("mods/a.late"), "a.late");
        assert!(store.migrate().unwrap().is_empty());
        assert!(root.join("mods/a.late").is_dir());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn migrate_merges_into_an_existing_addons_dir() {
        let root = tmp("migrate-merge");
        let store = Store::new(&root);
        write_addon(&root.join("addons/a.one"), "a.one");
        write_addon(&root.join("mods/one-copy"), "a.one");
        write_addon(&root.join("mods/a.two"), "a.two");
        std::fs::create_dir_all(root.join("mods/stray")).unwrap();
        std::fs::write(store.config_path(), "{").unwrap();
        let notes = store.migrate().unwrap();
        assert_eq!(notes.len(), 4, "{notes:?}");
        assert!(notes[3].starts_with("data_version not recorded"));
        assert!(root.join("addons/a.two").join(MANIFEST_FILE).is_file());
        assert!(root.join("mods/one-copy").is_dir() && root.join("mods/stray").is_dir());
        assert_eq!(
            store.addons().unwrap().keys().collect::<Vec<_>>(),
            ["a.one", "a.two", CORE_ID]
        );

        std::fs::remove_dir_all(root.join("mods/one-copy")).unwrap();
        std::fs::remove_dir_all(root.join("mods/stray")).unwrap();
        write_addon(&root.join("mods/a.three"), "a.three");
        std::fs::remove_file(store.config_path()).unwrap();
        assert_eq!(store.migrate().unwrap().len(), 2);
        assert!(!root.join("mods").exists());
        assert!(root.join("addons/a.three").is_dir());
        assert_eq!(store.config().data_version, DATA_VERSION);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn migrate_leaves_a_missing_data_dir_alone() {
        let root = tmp("migrate-missing").join("data");
        assert!(Store::new(&root).migrate().unwrap().is_empty());
        assert!(!root.exists());
    }

    #[test]
    fn reads_profiles_that_list_mods_and_writes_addons() {
        let root = tmp("old-profile");
        let store = Store::new(&root);
        let path = store.profiles_dir().join("grid.json");
        std::fs::create_dir_all(store.profiles_dir()).unwrap();
        std::fs::write(
            &path,
            r#"{"schema_version":1,"name":"grid","mods":[{"id":"swofty.core"},{"id":"a.b","enabled":false},{"id":"c.d"}],"options":{"c.d":{"x":1}}}"#,
        )
        .unwrap();
        let p = store.profile("grid").unwrap();
        assert_eq!(p.enabled().collect::<Vec<_>>(), [CORE_ID, "c.d"]);
        store.save_profile(&p).unwrap();
        let saved: serde_json::Value = crate::read_json(&path).unwrap();
        assert_eq!(saved["schema_version"], profile::SCHEMA_VERSION);
        assert_eq!(
            saved["addons"][1],
            serde_json::json!({"id": "a.b", "enabled": false})
        );
        assert!(saved.get("mods").is_none());
        assert_eq!(saved["options"]["c.d"]["x"], 1);
        assert_eq!(store.profile("grid").unwrap(), p);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn merges_in_game_requests_into_the_profile() {
        let root = tmp("requests");
        let store = Store::new(&root);
        let path = store.requests_path(DEFAULT_PROFILE);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{"schema":1,"profile":"default","seq":1,"options":{"swofty.core":{"show_launcher_panel":false}}}"#,
        )
        .unwrap();
        let out = store.merge_requests(DEFAULT_PROFILE, false).unwrap().unwrap();
        assert_eq!(out.changed, ["swofty.core.show_launcher_panel = Off"]);
        assert!(path.is_file());
        let core = crate::builtin::core_manifest();
        let p = store.profile(DEFAULT_PROFILE).unwrap();
        assert_eq!(p.option(&core, &core.options[0]), false);
        let again = store.merge_requests(DEFAULT_PROFILE, true).unwrap().unwrap();
        assert!(again.changed.is_empty());
        assert!(!path.exists());
        assert_eq!(store.merge_requests(DEFAULT_PROFILE, true).unwrap(), None);
        let s = store.session_settings(DEFAULT_PROFILE).unwrap();
        assert_eq!(s.addons[0].options[0].current, 0);
        assert!(s.requests.unwrap().ends_with("default.json"));
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
