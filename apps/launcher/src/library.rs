use swofty_core::manifest::{Kind, ModManifest};
use swofty_core::map::TrackPackage;
use swofty_core::profile::Profile;
use swofty_core::store::Store;

pub struct ModItem {
    pub manifest: ModManifest,
    pub enabled: bool,
}

pub struct MapItem {
    pub package: TrackPackage,
    pub enabled: bool,
}

pub struct ProfileItem {
    pub name: String,
    pub mods: Vec<String>,
    pub maps: usize,
}

pub struct Library {
    pub profile: Profile,
    pub mods: Vec<ModItem>,
    pub maps: Vec<MapItem>,
    pub profiles: Vec<ProfileItem>,
    pub error: Option<String>,
}

impl Library {
    pub fn load(store: &Store) -> Self {
        let mut error = None;
        let mut note = |e: swofty_core::Error| error = Some(e.to_string());
        let profile = store.profile(&store.active_profile()).unwrap_or_else(|e| {
            note(e);
            Profile::vanilla()
        });
        let manifests = store.mods().unwrap_or_else(|e| {
            note(e);
            Default::default()
        });
        let enabled: Vec<&str> = profile.enabled().collect();
        let mut mods: Vec<ModItem> = manifests
            .values()
            .map(|m| ModItem {
                enabled: enabled.contains(&m.id.as_str()),
                manifest: m.clone(),
            })
            .collect();
        mods.sort_by_key(|m| (!m.manifest.is_builtin(), m.manifest.name.to_lowercase()));
        let maps = store
            .maps()
            .unwrap_or_else(|e| {
                note(e);
                Vec::new()
            })
            .into_iter()
            .map(|package| MapItem {
                enabled: profile.maps.contains(&package.track_code),
                package,
            })
            .collect();
        let profiles = store
            .profile_names()
            .unwrap_or_default()
            .into_iter()
            .map(|name| {
                let p = store.profile(&name).unwrap_or_else(|_| Profile::new(&name));
                ProfileItem {
                    mods: p
                        .enabled()
                        .filter_map(|id| manifests.get(id))
                        .map(|m| m.name.clone())
                        .collect(),
                    maps: p.maps.len(),
                    name,
                }
            })
            .collect();
        Self {
            profile,
            mods,
            maps,
            profiles,
            error,
        }
    }

    pub fn enabled_mods(&self) -> usize {
        self.mods.iter().filter(|m| m.enabled).count()
    }
}

pub fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Database => "Database",
        Kind::Erp => "ERP",
        Kind::Ui => "UI",
        Kind::Files => "Files",
        Kind::Native => "Native",
    }
}
