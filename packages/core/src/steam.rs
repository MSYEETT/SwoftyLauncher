use std::path::{Path, PathBuf};

use crate::vdf::{self, Vdf};
use crate::{GAME_EXE, STEAM_APP_ID};

#[derive(Debug, Clone, PartialEq)]
pub struct GameInstall {
    pub steam_root: PathBuf,
    pub library: PathBuf,
    pub dir: PathBuf,
    pub build_id: Option<u64>,
    pub compatdata: Option<PathBuf>,
}

impl GameInstall {
    pub fn exe(&self) -> PathBuf {
        self.dir.join(GAME_EXE)
    }

    pub fn prefix(&self) -> Option<PathBuf> {
        self.compatdata.as_ref().map(|c| c.join("pfx"))
    }
}

pub fn steam_roots() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os("SWOFTY_STEAM_ROOT") {
        out.push(dir.into());
    }
    if cfg!(windows) {
        for var in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Some(p) = std::env::var_os(var) {
                out.push(PathBuf::from(p).join("Steam"));
            }
        }
    } else if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        out.push(home.join(".steam/steam"));
        out.push(home.join(".local/share/Steam"));
        out.push(home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
        out.push(home.join(".var/app/com.valvesoftware.Steam/data/Steam"));
    }
    let mut seen = Vec::new();
    out.retain(|p| {
        let real = p.canonicalize().unwrap_or_else(|_| p.clone());
        let keep = p.join("steamapps").is_dir() && !seen.contains(&real);
        seen.push(real);
        keep
    });
    out
}

fn read_vdf(path: &Path) -> Option<Vdf> {
    vdf::parse(&std::fs::read_to_string(path).ok()?)
}

pub fn libraries(steam_root: &Path) -> Vec<PathBuf> {
    let mut libs = vec![steam_root.to_path_buf()];
    if let Some(v) = read_vdf(&steam_root.join("steamapps/libraryfolders.vdf")) {
        let entries = v.get("libraryfolders").into_iter().flat_map(Vdf::entries);
        for (_, e) in entries {
            if let Some(p) = e.get("path").and_then(Vdf::str) {
                let p = PathBuf::from(p);
                let real = |x: &Path| x.canonicalize().unwrap_or_else(|_| x.to_path_buf());
                if !libs.iter().any(|l| real(l) == real(&p)) {
                    libs.push(p);
                }
            }
        }
    }
    libs
}

pub fn find_game() -> Option<GameInstall> {
    steam_roots().into_iter().find_map(|root| find_game_in(&root))
}

pub fn find_game_in(steam_root: &Path) -> Option<GameInstall> {
    libraries(steam_root).into_iter().find_map(|lib| {
        let steamapps = lib.join("steamapps");
        let acf = read_vdf(&steamapps.join(format!("appmanifest_{STEAM_APP_ID}.acf")))?;
        let state = acf.get("AppState")?;
        let dir = steamapps.join("common").join(state.get("installdir")?.str()?);
        let build_id = state
            .get("buildid")
            .and_then(Vdf::str)
            .and_then(|b| b.parse().ok());
        let compat = steamapps.join("compatdata").join(STEAM_APP_ID.to_string());
        Some(GameInstall {
            steam_root: steam_root.to_path_buf(),
            library: lib.clone(),
            dir,
            build_id,
            compatdata: compat.is_dir().then_some(compat),
        })
    })
}

pub const STEAMVR_APP_ID: u32 = 250_820;

pub fn steamvr_installed(steam_root: &Path) -> bool {
    libraries(steam_root).iter().any(|lib| {
        lib.join(format!("steamapps/appmanifest_{STEAMVR_APP_ID}.acf"))
            .is_file()
    })
}

pub fn launch_options(steam_root: &Path) -> Vec<(String, String)> {
    let Ok(users) = std::fs::read_dir(steam_root.join("userdata")) else {
        return Vec::new();
    };
    users
        .flatten()
        .filter_map(|u| {
            let cfg = read_vdf(&u.path().join("config/localconfig.vdf"))?;
            let apps = cfg.path(&["UserLocalConfigStore", "Software", "Valve", "Steam", "apps"])?;
            let opts = apps.get(&STEAM_APP_ID.to_string())?.get("LaunchOptions")?.str()?;
            Some((u.file_name().to_string_lossy().into_owned(), opts.to_owned()))
        })
        .collect()
}

pub fn compat_tool(steam_root: &Path) -> Option<String> {
    let cfg = read_vdf(&steam_root.join("config/config.vdf"))?;
    let map = cfg.path(&[
        "InstallConfigStore",
        "Software",
        "Valve",
        "Steam",
        "CompatToolMapping",
    ])?;
    map.get(&STEAM_APP_ID.to_string())?
        .get("name")?
        .str()
        .map(str::to_owned)
}

pub fn wrapper_installed(launch_options: &str) -> bool {
    launch_options.contains("swofty")
        && launch_options.contains("run")
        && launch_options.contains("%command%")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vdf_path(p: &Path) -> String {
        p.display().to_string().replace('\\', "\\\\")
    }

    #[test]
    fn finds_game_in_secondary_library() {
        let root = std::env::temp_dir().join(format!("swofty-steam-{}", std::process::id()));
        let lib = root.join("lib2");
        std::fs::create_dir_all(root.join("steamapps")).unwrap();
        std::fs::create_dir_all(lib.join("steamapps/common/F1 25")).unwrap();
        std::fs::create_dir_all(lib.join("steamapps/compatdata/3059520/pfx")).unwrap();
        std::fs::write(
            root.join("steamapps/libraryfolders.vdf"),
            format!(
                r#""libraryfolders" {{ "0" {{ "path" "{}" }} "1" {{ "path" "{}" }} }}"#,
                vdf_path(&root),
                vdf_path(&lib)
            ),
        )
        .unwrap();
        std::fs::write(
            lib.join("steamapps/appmanifest_3059520.acf"),
            r#""AppState" { "appid" "3059520" "installdir" "F1 25" "buildid" "25251167" }"#,
        )
        .unwrap();
        let g = find_game_in(&root).unwrap();
        assert_eq!(g.dir, lib.join("steamapps/common/F1 25"));
        assert_eq!(g.build_id, Some(25_251_167));
        assert!(g.prefix().unwrap().ends_with("compatdata/3059520/pfx"));
        assert!(!steamvr_installed(&root));
        std::fs::write(
            lib.join("steamapps/appmanifest_250820.acf"),
            r#""AppState" { "appid" "250820" "installdir" "SteamVR" }"#,
        )
        .unwrap();
        assert!(steamvr_installed(&root));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn detects_wrapper() {
        assert!(wrapper_installed(
            "~/.local/share/swofty/bin/swofty run -- %command%"
        ));
        assert!(!wrapper_installed("echo %command%"));
    }
}
