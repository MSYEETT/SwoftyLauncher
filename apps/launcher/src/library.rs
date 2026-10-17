use swofty_core::manifest::{AddonManifest, Kind};
use swofty_core::map::{MapArt, TrackPackage};
use swofty_core::profile::{Profile, VANILLA};
use swofty_core::store::Store;

pub struct AddonItem {
    pub manifest: AddonManifest,
    pub enabled: bool,
}

pub struct MapItem {
    pub package: TrackPackage,
    pub art: MapArt,
    pub enabled: bool,
}

pub struct ProfileItem {
    pub name: String,
    pub addons: Vec<String>,
    pub maps: Vec<String>,
}

impl ProfileItem {
    pub fn vanilla(&self) -> bool {
        self.name == VANILLA
    }

    pub fn summary(&self) -> String {
        if self.vanilla() {
            return "No addons".into();
        }
        format!(
            "{} · {}",
            plural(self.addons.len(), "addon", "addons"),
            plural(self.maps.len(), "map", "maps")
        )
    }
}

pub struct Library {
    pub profile: Profile,
    pub rebuild: Vec<String>,
    pub addons: Vec<AddonItem>,
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
        let manifests = store.addons().unwrap_or_else(|e| {
            note(e);
            Default::default()
        });
        let enabled: Vec<&str> = profile.enabled().collect();
        let mut addons: Vec<AddonItem> = manifests
            .values()
            .map(|m| AddonItem {
                enabled: enabled.contains(&m.id.as_str()),
                manifest: m.clone(),
            })
            .collect();
        addons.sort_by_key(|m| (!m.manifest.is_builtin(), m.manifest.name.to_lowercase()));
        let packages = store.maps().unwrap_or_else(|e| {
            note(e);
            Vec::new()
        });
        let profiles = store
            .profile_names()
            .unwrap_or_default()
            .into_iter()
            .map(|name| {
                let p = store.profile(&name).unwrap_or_else(|_| Profile::new(&name));
                ProfileItem {
                    addons: p
                        .enabled()
                        .filter_map(|id| manifests.get(id))
                        .map(|m| m.name.clone())
                        .collect(),
                    maps: p
                        .maps
                        .iter()
                        .map(|code| {
                            packages
                                .iter()
                                .find(|m| &m.track_code == code)
                                .map_or(code.clone(), |m| m.display_name().to_owned())
                        })
                        .collect(),
                    name,
                }
            })
            .collect();
        let maps = packages
            .into_iter()
            .map(|package| MapItem {
                art: store.map_art(&package),
                enabled: profile.maps.contains(&package.track_code),
                package,
            })
            .collect();
        Self {
            rebuild: profile.rebuild_pending(&manifests),
            profile,
            addons,
            maps,
            profiles,
            error,
        }
    }

    pub fn enabled_addons(&self) -> usize {
        self.addons.iter().filter(|m| m.enabled).count()
    }

    pub fn active(&self) -> Option<&ProfileItem> {
        self.profiles.iter().find(|p| p.name == self.profile.name)
    }
}

pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
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
