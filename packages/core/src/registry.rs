use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use swofty_addon::launcher::{Addon, CommandSpec, Cx, GameFiles, GameStatus, ProfileInfo, StateStore};

use crate::baseline::{Baseline, FileRecord, Objects};
use crate::builtin::CORE_ID;
use crate::helper::HelperAddon;
use crate::install::Game;
use crate::manifest::AddonManifest;
use crate::profile::Profile;
use crate::store::Store;
use crate::{Error, Result};

pub struct DeclaredAddon {
    manifest: AddonManifest,
}

impl DeclaredAddon {
    pub fn new(manifest: AddonManifest) -> Self {
        Self { manifest }
    }
}

impl Addon for DeclaredAddon {
    fn manifest(&self) -> &AddonManifest {
        &self.manifest
    }
}

#[derive(Clone)]
pub struct Entry {
    pub addon: Arc<dyn Addon>,
    pub declared: bool,
    pub helper: bool,
    pub plugin: Option<Arc<HelperAddon>>,
    pub locked: bool,
    pub default_on: bool,
}

impl Entry {
    pub fn id(&self) -> &str {
        &self.addon.manifest().id
    }
}

#[derive(Clone, Default)]
pub struct Registry {
    entries: Vec<Entry>,
}

impl Registry {
    pub fn load(store: &Store) -> Result<Self> {
        let entries = store
            .addons()?
            .into_values()
            .map(|m| {
                let managed = crate::managed::is_managed(&m.id);
                match store.builtins().iter().find(|b| b.addon.manifest().id == m.id) {
                    Some(b) => Entry {
                        addon: b.addon.clone(),
                        declared: false,
                        helper: false,
                        plugin: None,
                        locked: b.locked,
                        default_on: b.default_on,
                    },
                    None => match store.helpers().get(&m, &store.addons_dir().join(&m.id)) {
                        Some(h) => Entry {
                            addon: h.clone(),
                            declared: false,
                            helper: true,
                            plugin: Some(h),
                            locked: managed,
                            default_on: managed,
                        },
                        None => Entry {
                            locked: m.id == CORE_ID || managed,
                            default_on: m.id == CORE_ID || managed,
                            declared: true,
                            helper: false,
                            plugin: None,
                            addon: Arc::new(DeclaredAddon::new(m)),
                        },
                    },
                }
            })
            .collect();
        Ok(Self { entries })
    }

    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id() == id)
    }

    pub fn set_enabled(&self, p: &mut Profile, id: &str, on: bool) -> Result<()> {
        if let Some(e) = self.get(id).filter(|e| e.locked && !on && !p.is_vanilla()) {
            return Err(Error::Other(format!(
                "{} ({id}) is always on and cannot be disabled",
                e.addon.manifest().name
            )));
        }
        p.set_enabled(id, on)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter()
    }

    pub fn manifests(&self) -> BTreeMap<String, AddonManifest> {
        self.entries
            .iter()
            .map(|e| (e.id().to_owned(), e.addon.manifest().clone()))
            .collect()
    }

    pub fn commands(&self, reserved: &[&str]) -> (Vec<(String, CommandSpec)>, Vec<String>) {
        let mut taken: BTreeSet<String> = reserved.iter().map(|s| (*s).to_owned()).collect();
        let mut out = Vec::new();
        let mut refused = Vec::new();
        for e in &self.entries {
            for spec in e.addon.commands() {
                if taken.insert(spec.name.clone()) {
                    out.push((e.id().to_owned(), spec));
                } else {
                    refused.push(format!("{}: the command {} is taken", e.id(), spec.name));
                }
            }
        }
        (out, refused)
    }
}

pub struct ProfileState {
    store: Store,
    profile: String,
}

impl StateStore for ProfileState {
    fn get(&self, addon: &str) -> swofty_addon::Result<Option<serde_json::Value>> {
        let p = self.store.profile(&self.profile).map_err(other)?;
        Ok(p.state.get(addon).cloned())
    }

    fn set(&self, addon: &str, value: serde_json::Value) -> swofty_addon::Result<()> {
        self.store
            .update_profile(&self.profile, |p| p.set_state(addon, value))
            .map_err(other)
    }
}

fn other(e: Error) -> swofty_addon::Error {
    match e {
        Error::Addon(e) => e,
        e => swofty_addon::Error::Other(e.to_string()),
    }
}

pub struct GameDir {
    data: PathBuf,
    game: Game,
    objects: Objects,
    files: BTreeMap<String, FileRecord>,
}

impl GameDir {
    pub fn new(store: &Store, game: &Game) -> Result<Self> {
        Ok(Self {
            data: store.root.clone(),
            game: game.clone(),
            objects: Objects {
                root: store.root.join("baseline/objects"),
            },
            files: Baseline::load_or_new(&store.root, &game.identity)?.files,
        })
    }
}

impl GameFiles for GameDir {
    fn build_key(&self) -> String {
        self.game.identity.key()
    }

    fn exe_version(&self) -> Option<String> {
        crate::exe_version::read(&self.game.dir.join(crate::GAME_EXE))
    }

    fn is_stock(&self, rel: &str) -> bool {
        self.files.contains_key(rel) || self.game.dir.join(rel).is_file()
    }

    fn original(&self, rel: &str) -> swofty_addon::Result<PathBuf> {
        let path = match self.files.get(rel) {
            Some(rec) => self.objects.path(&rec.sha256),
            None => self.game.dir.join(rel),
        };
        if path.is_file() {
            Ok(path)
        } else {
            Err(swofty_addon::Error::Other(format!("{rel} is not a game file")))
        }
    }

    fn original_lang(&self, code: &str) -> swofty_addon::Result<PathBuf> {
        crate::baseline::original_lang(&self.data, &self.game.dir, &self.game.identity, code).map_err(other)
    }

    fn dir(&self) -> Option<PathBuf> {
        Some(self.game.dir.clone())
    }

    fn status(&self) -> GameStatus {
        crate::process::game_status(Some(&self.data))
    }

    fn steam_save_dir(&self) -> Option<PathBuf> {
        crate::steam::find_save_dir()
    }
}

pub struct Host {
    pub addon: String,
    pub profile: ProfileInfo,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub game: Option<GameDir>,
    state: ProfileState,
}

impl Host {
    pub fn new(store: &Store, addon: &str, profile: &Profile, game: Option<&Game>) -> Result<Self> {
        let data_dir = store.addon_data_dir(addon);
        let cache_dir = store.addon_cache_dir(addon);
        for dir in [&data_dir, &cache_dir] {
            std::fs::create_dir_all(dir).map_err(crate::io_err(dir))?;
        }
        Ok(Self {
            addon: addon.into(),
            profile: ProfileInfo {
                name: profile.name.clone(),
                vanilla: profile.is_vanilla(),
                enabled: profile.enabled().any(|id| id == addon),
            },
            data_dir,
            cache_dir,
            game: game.map(|g| GameDir::new(store, g)).transpose()?,
            state: ProfileState {
                store: store.clone(),
                profile: profile.name.clone(),
            },
        })
    }

    pub fn cx(&self) -> Cx<'_> {
        Cx::new(
            &self.addon,
            &self.profile,
            &self.data_dir,
            &self.cache_dir,
            self.game.as_ref().map(|g| g as &dyn GameFiles),
            &self.state,
        )
    }
}

#[cfg(test)]
mod tests {
    use swofty_addon::launcher::{Builtin, CommandSpec};

    use super::*;

    struct Fake {
        manifest: AddonManifest,
        command: &'static str,
    }

    impl Addon for Fake {
        fn manifest(&self) -> &AddonManifest {
            &self.manifest
        }

        fn commands(&self) -> Vec<CommandSpec> {
            vec![CommandSpec {
                name: self.command.into(),
                about: String::new(),
                args: Vec::new(),
                subcommands: Vec::new(),
            }]
        }
    }

    fn fake(id: &str, command: &'static str, default_on: bool) -> Builtin {
        let manifest = AddonManifest::parse(&format!(
            r#"{{"schema_version":1,"id":"{id}","name":"{id}","version":"1.0.0","author":"t","compat":{{"game":"f1_25"}}}}"#
        ))
        .unwrap();
        Builtin {
            addon: Arc::new(Fake { manifest, command }),
            locked: false,
            default_on,
        }
    }

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-registry-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        dir
    }

    #[test]
    fn lists_core_builtins_and_refuses_taken_commands() {
        let root = tmp("commands");
        let store =
            Store::new(&root).with_builtins(vec![fake("t.a", "mods", true), fake("t.b", "mods", false)]);
        let reg = Registry::load(&store).unwrap();
        assert_eq!(
            reg.iter().map(Entry::id).collect::<Vec<_>>(),
            [CORE_ID, "t.a", "t.b"]
        );
        assert!(reg.get(CORE_ID).unwrap().declared && reg.get(CORE_ID).unwrap().locked);
        assert!(!reg.get("t.a").unwrap().declared);
        let (commands, refused) = reg.commands(&["build"]);
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].0, "t.a");
        assert_eq!(refused, ["t.b: the command mods is taken"]);
        let (_, refused) = reg.commands(&["mods"]);
        assert_eq!(refused.len(), 2);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn locked_entries_stay_on() {
        let root = tmp("locked");
        let locked = Builtin {
            locked: true,
            ..fake("t.a", "a", true)
        };
        let store = Store::new(&root).with_builtins(vec![locked, fake("t.b", "b", true)]);
        let reg = Registry::load(&store).unwrap();
        let mut p = store.profile("default").unwrap();
        let e = reg.set_enabled(&mut p, "t.a", false).unwrap_err();
        assert_eq!(e.to_string(), "t.a (t.a) is always on and cannot be disabled");
        assert!(reg.set_enabled(&mut p, CORE_ID, false).is_err());
        reg.set_enabled(&mut p, "t.a", true).unwrap();
        reg.set_enabled(&mut p, "t.b", false).unwrap();
        assert_eq!(p.enabled().collect::<Vec<_>>(), [CORE_ID, "t.a"]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn state_lives_in_the_profile_and_follows_copies() {
        let root = tmp("state");
        let store = Store::new(&root).with_builtins(vec![fake("t.a", "a", true)]);
        let p = store.profile("default").unwrap();
        assert_eq!(p.enabled().collect::<Vec<_>>(), [CORE_ID, "t.a"]);
        let host = Host::new(&store, "t.a", &p, None).unwrap();
        assert!(host.data_dir.ends_with("addon-data/t.a") && host.data_dir.is_dir());
        assert!(host.cache_dir.ends_with("cache/addons/t.a") && host.cache_dir.is_dir());
        let cx = host.cx();
        assert!(cx.profile.enabled);
        assert_eq!(cx.state::<Vec<String>>().unwrap(), Vec::<String>::new());
        cx.set_state(&["x"]).unwrap();
        assert_eq!(cx.state::<Vec<String>>().unwrap(), ["x"]);
        store.create_profile("copy", Some("default")).unwrap();
        assert_eq!(
            store.profile("copy").unwrap().state["t.a"],
            serde_json::json!(["x"])
        );
        let vanilla = Host::new(&store, "t.a", &Profile::vanilla(), None).unwrap();
        assert!(vanilla.cx().set_state(&["y"]).is_err());
        std::fs::remove_dir_all(&root).ok();
    }
}
