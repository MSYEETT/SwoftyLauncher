pub use swofty_addon::{GAME_ID, asset, launcher, manifest, ops, view};

pub mod activation;
pub mod baseline;
pub mod build;
pub mod builtin;
pub mod bundled;
pub mod cache;
pub mod crash;
pub mod erpbuild;
pub mod exe_version;
pub mod helper;
pub mod import;
pub mod install;
pub mod journal;
pub mod managed;
pub mod map;
pub mod offline;
pub mod owned;
pub mod paths;
pub mod pkgedit;
pub mod placed;
pub mod planner;
pub mod process;
pub mod profile;
pub mod recovery;
pub mod registry;
pub mod resolver;
pub mod runtime;
pub mod session;
pub mod settings;
pub mod steam;
pub mod store;
pub mod ui;
pub mod update;
pub mod updater;
pub mod vdf;

pub const STEAM_APP_ID: u32 = 3_059_520;
pub const GAME_EXE: &str = "F1_25.exe";
pub const IMAGE_BASE: u64 = 0x1_4000_0000;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Json {
        path: std::path::PathBuf,
        source: serde_json::Error,
    },
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Formats(#[from] swofty_formats::Error),
    #[error(transparent)]
    Addon(#[from] swofty_addon::Error),
    #[error("invalid manifest: {0}")]
    Manifest(String),
    #[error("invalid track package: {0}")]
    Map(String),
    #[error("{0}")]
    Resolve(String),
    #[error("{}", leftovers(.0))]
    Leftovers(Vec<install::Conflict>),
    #[error("{0}")]
    Other(String),
}

fn leftovers(found: &[install::Conflict]) -> String {
    let unknown: Vec<_> = found.iter().filter(|c| !c.owned).collect();
    let (first, rest) = match unknown.first() {
        Some(c) => (c.to_string(), unknown.len() - 1),
        None => (found[0].to_string(), found.len() - 1),
    };
    match rest {
        0 => first,
        n => format!("{first} (and {n} more)"),
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn io_err(path: impl Into<std::path::PathBuf>) -> impl FnOnce(std::io::Error) -> Error {
    let path = path.into();
    move |source| Error::Io { path, source }
}

/// A filesystem error worth retrying: a freshly unloaded DLL stays briefly
/// locked while Windows Defender (or another antivirus) scans it or the loader
/// releases its handle, so a delete or rename right after the game exits fails
/// with Access Denied (os error 5) or a sharing violation (os error 32).
pub(crate) fn is_locked_file_error(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(32)
}

/// Run a filesystem mutation, retrying a locked-file error with growing backoff
/// before giving up. On a lasting lock the returned error names the likely
/// locker (an antivirus holding the game folder) and what to do about it, so a
/// half-modded folder can be cleaned rather than leaving a bare "os error 5".
pub(crate) fn with_retry<T>(path: &std::path::Path, mut op: impl FnMut() -> std::io::Result<T>) -> Result<T> {
    const DELAYS_MS: [u64; 5] = [50, 150, 300, 600, 1000];
    let mut attempt = 0usize;
    loop {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) if is_locked_file_error(&e) && attempt < DELAYS_MS.len() => {
                std::thread::sleep(std::time::Duration::from_millis(DELAYS_MS[attempt]));
                attempt += 1;
            }
            Err(e) if is_locked_file_error(&e) => {
                return Err(Error::Other(format!(
                    "{}: {e} -- still locked after {} retries. Close F1 25 and the EA app if \
                     they are open; if Windows Defender or another antivirus is scanning the game \
                     folder it can hold the file. Add an exclusion for the game folder, then run \
                     `swofty restore` again.",
                    path.display(),
                    DELAYS_MS.len()
                )));
            }
            Err(e) => return Err(io_err(path.to_path_buf())(e)),
        }
    }
}

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T> {
    let data = std::fs::read(path).map_err(io_err(path))?;
    serde_json::from_slice(&data).map_err(|source| Error::Json {
        path: path.into(),
        source,
    })
}

pub(crate) fn move_file(from: &std::path::Path, to: &std::path::Path) -> Result<()> {
    match std::fs::rename(from, to) {
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            std::fs::copy(from, to).map_err(io_err(to))?;
            std::fs::remove_file(from).map_err(io_err(from))
        }
        r => r.map_err(io_err(from)),
    }
}

pub(crate) fn write_json<T: serde::Serialize>(path: &std::path::Path, value: &T) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io_err(dir))?;
    }
    let mut data = serde_json::to_vec_pretty(value).expect("serialisable");
    data.push(b'\n');
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, data).map_err(io_err(&tmp))?;
    std::fs::rename(&tmp, path).map_err(io_err(path))
}
