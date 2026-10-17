use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::STEAM_APP_ID;

pub const DEFAULT_WINEDEBUG: &str = "+timestamp,+pid,+tid,+loaddll,fixme-all";
pub const LOADER_LOG: &str = "swofty/loader.log";
pub const LOADER_MARK: &str = "loader_mark.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub schema: u32,
    pub ticket: String,
    pub expires: String,
    pub mode: String,
    pub game: GameRef,
    pub profile: String,
    pub overlay: Overlay,
    #[serde(default)]
    pub plugins: Vec<PluginPin>,
    #[serde(default)]
    pub options: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
    #[serde(default)]
    pub ui_bridge: UiBridge,
    pub policy: Policy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameRef {
    pub app_id: u32,
    pub build: String,
    pub exe_sha256: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Overlay {
    pub root: String,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginPin {
    pub id: String,
    pub dll: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UiBridge {
    #[serde(default)]
    pub notify_map: BTreeMap<String, (String, String, serde_json::Value)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Policy {
    pub offline: String,
    pub save_namespace: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub launch: LaunchConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_dir: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub console: bool,
    #[serde(default)]
    pub data_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LaunchConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_affinity: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default = "default_winedebug")]
    pub winedebug: String,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            cpu_affinity: None,
            env: BTreeMap::new(),
            winedebug: default_winedebug(),
        }
    }
}

/// The Windows environment block cannot exceed 32767 characters, and
/// Proton's steam helper packs the game's environment into one. A `nix
/// develop` (or CI) shell more than doubles that with build variables, and
/// the helper then creates a game process that dies during startup before
/// loading a single DLL. Build-shell variables are not part of running a
/// game, so they are dropped, and whatever remains is trimmed to fit.
pub fn game_env(
    parent: impl IntoIterator<Item = (String, String)>,
    overrides: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    const LIMIT: usize = 30_000; // under the 32767 block limit, names included

    fn is_build_shell_var(name: &str) -> bool {
        const EXACT: &[&str] = &[
            "AR",
            "AS",
            "CC",
            "CXX",
            "LD",
            "NM",
            "OBJCOPY",
            "OBJDUMP",
            "RANLIB",
            "READELF",
            "SIZE",
            "STRIP",
            "STRINGS",
            "PKG_CONFIG",
            "LIBCLANG_PATH",
            "CONFIG_SHELL",
            "HOST_PATH",
            "SOURCE_DATE_EPOCH",
            "DETERMINISTIC_BUILD",
            "IN_NIX_SHELL",
            "NIXPKGS_CMAKE_PREFIX_PATH",
            "CMAKE_INCLUDE_PATH",
            "CMAKE_LIBRARY_PATH",
            "PYTHONPATH",
            "PYTHONHASHSEED",
            "PYTHONNOUSERSITE",
            "_PYTHON_HOST_PLATFORM",
            "_PYTHON_SYSCONFIGDATA_NAME",
            "buildInputs",
            "nativeBuildInputs",
            "buildPhase",
            "builder",
            "shell",
            "shellHook",
            "stdenv",
            "out",
            "outputs",
            "name",
            "system",
            "phases",
            "patches",
            "strictDeps",
            "doCheck",
            "doInstallCheck",
            "preferLocalBuild",
            "allowSubstitutes",
            "dontAddDisableDepTrack",
            "__structuredAttrs",
            "cmakeFlags",
            "configureFlags",
            "mesonFlags",
        ];
        const PREFIXES: &[&str] = &["NIX_", "NIXPKGS_", "CMAKE_", "deps", "propagated"];
        const SUFFIX: &str = "FLAGS";
        EXACT.contains(&name) || PREFIXES.iter().any(|p| name.starts_with(p)) || name.ends_with(SUFFIX)
    }

    fn block_size(env: &BTreeMap<String, String>) -> usize {
        env.iter().map(|(k, v)| k.len() + v.len() + 2).sum()
    }

    let mut env: BTreeMap<String, String> = parent
        .into_iter()
        .filter(|(k, _)| !is_build_shell_var(k))
        .collect();
    for (k, v) in overrides {
        env.insert(k.clone(), v.clone());
    }
    // Trim whatever is left to fit: the largest values go first, and the
    // variables a game launch cannot do without are kept.
    const KEEP: &[&str] = &["PATH", "HOME", "USER", "TEMP", "TMP", "TMPDIR", "LANG"];
    while block_size(&env) > LIMIT {
        let Some(largest) = env
            .iter()
            .filter(|(k, _)| !KEEP.contains(&k.as_str()) && !overrides.contains_key(*k))
            .max_by_key(|(k, v)| k.len() + v.len())
            .map(|(k, _)| k.clone())
        else {
            break;
        };
        env.remove(&largest);
    }
    env
}

fn default_winedebug() -> String {
    DEFAULT_WINEDEBUG.into()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LaunchRequest {
    #[serde(default)]
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub winedebug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub apply: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionRecord {
    pub dir: PathBuf,
    pub request_id: Option<String>,
    pub started: Option<f64>,
    pub ended: Option<f64>,
    pub exit_code: Option<i32>,
}

impl SessionRecord {
    pub fn read(dir: &Path) -> Self {
        let read = |f: &str| {
            std::fs::read_to_string(dir.join(f))
                .ok()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
        };
        Self {
            dir: dir.into(),
            request_id: read("request_id"),
            started: read("game_start").and_then(|s| s.parse().ok()),
            ended: read("game_end").and_then(|s| s.parse().ok()),
            exit_code: read("exit_code").and_then(|s| s.parse().ok()),
        }
    }

    pub fn crashed(&self) -> bool {
        self.exit_code.is_some_and(|c| c != 0)
    }

    pub fn wine_log(&self) -> PathBuf {
        self.dir.join(format!("steam-{STEAM_APP_ID}.log"))
    }

    pub fn owns_loader_log(&self, log: &Path) -> bool {
        let Some(before) = std::fs::read(self.dir.join(LOADER_MARK))
            .ok()
            .and_then(|d| serde_json::from_slice::<Option<LogMark>>(&d).ok())
        else {
            return false;
        };
        LogMark::replaced(log, before.as_ref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogMark {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inode: Option<u64>,
    pub head: String,
}

impl LogMark {
    pub fn read(path: &Path) -> Option<Self> {
        let file = std::fs::File::open(path).ok()?;
        #[cfg(unix)]
        let inode = Some(std::os::unix::fs::MetadataExt::ino(&file.metadata().ok()?));
        #[cfg(not(unix))]
        let inode = None;
        let mut head = Vec::new();
        file.take(512).read_to_end(&mut head).ok()?;
        let end = head.iter().position(|&b| b == b'\n').unwrap_or(head.len());
        Some(Self {
            inode,
            head: String::from_utf8_lossy(&head[..end]).trim_end().to_owned(),
        })
    }

    pub fn replaced(path: &Path, before: Option<&Self>) -> bool {
        Self::read(path).is_some_and(|now| before != Some(&now))
    }
}

pub fn parse_cpu_list(spec: &str) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part.split_once('-') {
            Some((a, b)) => out.extend(a.parse::<usize>().ok()?..=b.parse::<usize>().ok()?),
            None => out.push(part.parse().ok()?),
        }
    }
    (!out.is_empty()).then_some(out)
}

#[derive(Debug, Clone, PartialEq)]
pub struct SteamCommand {
    pub proton: Option<String>,
    pub verb: Option<String>,
    pub target: Option<usize>,
    pub argv: Vec<String>,
}

pub fn parse_steam_command(argv: &[String]) -> SteamCommand {
    let proton = argv.iter().position(|a| a.ends_with("/proton"));
    let verb = proton.and_then(|i| argv.get(i + 1)).cloned();
    let target = proton.map(|i| i + 2).filter(|&i| i < argv.len()).or_else(|| {
        argv.iter()
            .rposition(|a| a.to_ascii_lowercase().ends_with(".exe"))
    });
    SteamCommand {
        proton: proton.map(|i| argv[i].clone()),
        verb,
        target,
        argv: argv.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_keeps_console_flag() {
        let mut config = Config::default();
        assert!(!serde_json::to_string(&config).unwrap().contains("console"));
        config.console = true;
        let json = serde_json::to_string(&config).unwrap();
        assert!(serde_json::from_str::<Config>(&json).unwrap().console);
    }

    #[test]
    fn game_env_drops_build_shell_junk() {
        let parent = [
            ("PATH".to_string(), "/usr/bin".to_string()),
            ("HOME".to_string(), "/home/u".to_string()),
            ("SteamAppId".to_string(), "3059520".to_string()),
            ("NIX_LDFLAGS".to_string(), "-L/nix/store/aaaa ".repeat(200)),
            ("CFLAGS".to_string(), "-O2".to_string()),
            ("buildInputs".to_string(), "x".to_string()),
            ("depsBuildBuild".to_string(), String::new()),
        ];
        let env = game_env(parent, &BTreeMap::new());
        assert!(env.contains_key("PATH") && env.contains_key("SteamAppId"));
        for dropped in ["NIX_LDFLAGS", "CFLAGS", "buildInputs", "depsBuildBuild"] {
            assert!(!env.contains_key(dropped), "{dropped} kept");
        }
    }

    #[test]
    fn game_env_fits_the_windows_block() {
        let parent = (0..400).map(|i| (format!("VAR{i:03}"), "x".repeat(200)));
        let mut overrides = BTreeMap::new();
        overrides.insert("WINEDEBUG".to_string(), "+all".to_string());
        let env = game_env(parent, &overrides);
        let size: usize = env.iter().map(|(k, v)| k.len() + v.len() + 2).sum();
        assert!(size <= 30_000, "{size}");
        assert_eq!(env.get("WINEDEBUG").map(String::as_str), Some("+all"));
    }

    #[test]
    fn cpu_lists() {
        assert_eq!(parse_cpu_list("0-2,5"), Some(vec![0, 1, 2, 5]));
        assert_eq!(parse_cpu_list("x"), None);
    }

    #[test]
    fn reads_session_records() {
        let dir = std::env::temp_dir().join(format!("swofty-session-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("request_id"), "abc\n").unwrap();
        std::fs::write(dir.join("game_start"), "100.5\n").unwrap();
        let live = SessionRecord::read(&dir);
        assert_eq!(live.request_id.as_deref(), Some("abc"));
        assert_eq!((live.started, live.exit_code), (Some(100.5), None));
        assert!(!live.crashed());
        std::fs::write(dir.join("exit_code"), "3\n").unwrap();
        assert!(SessionRecord::read(&dir).crashed());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tells_a_rotated_loader_log_from_a_stale_one() {
        let dir = std::env::temp_dir().join(format!("swofty-loader-mark-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("loader.log");
        assert!(!LogMark::replaced(&log, None));
        std::fs::write(
            &log,
            "2026-10-17 21:44:08.228 [  416] swofty loader (version.dll proxy) attached, pid 412\nold\n",
        )
        .unwrap();
        assert!(LogMark::replaced(&log, None));
        let before = LogMark::read(&log);
        let session = SessionRecord::read(&dir);
        assert!(!session.owns_loader_log(&log));
        std::fs::write(dir.join(LOADER_MARK), serde_json::to_vec(&before).unwrap()).unwrap();
        assert!(!LogMark::replaced(&log, before.as_ref()));
        assert!(!session.owns_loader_log(&log));
        std::fs::rename(&log, dir.join("loader.prev.log")).unwrap();
        std::fs::write(&log, "").unwrap();
        assert!(session.owns_loader_log(&log));
        std::fs::write(
            &log,
            "2026-10-17 22:12:08.004 [  420] swofty loader (version.dll proxy) attached, pid 416\n",
        )
        .unwrap();
        assert!(session.owns_loader_log(&log));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn steam_command_shape() {
        let argv: Vec<String> = [
            "/steam/ubuntu12_32/reaper",
            "SteamLaunch",
            "AppId=3059520",
            "--",
            "/steam/SteamLinuxRuntime_sniper/_v2-entry-point",
            "--verb=waitforexitandrun",
            "--",
            "/steam/compatibilitytools.d/Proton/proton",
            "waitforexitandrun",
            "/games/F1 25/EAAntiCheat.GameServiceLauncher.exe",
        ]
        .map(String::from)
        .to_vec();
        let c = parse_steam_command(&argv);
        assert_eq!(c.verb.as_deref(), Some("waitforexitandrun"));
        assert_eq!(c.target, Some(9));
        assert!(c.proton.unwrap().ends_with("/proton"));
    }
}
