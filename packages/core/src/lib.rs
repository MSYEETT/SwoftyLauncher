pub mod activation;
pub mod baseline;
pub mod build;
pub mod builtin;
pub mod crash;
pub mod erpbuild;
pub mod install;
pub mod journal;
pub mod manifest;
pub mod map;
pub mod offline;
pub mod ops;
pub mod paths;
pub mod pkgedit;
pub mod process;
pub mod profile;
pub mod resolver;
pub mod session;
pub mod settings;
pub mod steam;
pub mod store;
pub mod ui;
pub mod vdf;

pub const GAME_ID: &str = "f1_25";
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
    #[error("invalid manifest: {0}")]
    Manifest(String),
    #[error("invalid map package: {0}")]
    Map(String),
    #[error("{0}")]
    Resolve(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn io_err(path: impl Into<std::path::PathBuf>) -> impl FnOnce(std::io::Error) -> Error {
    let path = path.into();
    move |source| Error::Io { path, source }
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
