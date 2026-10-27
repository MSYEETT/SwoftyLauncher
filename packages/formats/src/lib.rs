pub mod bxml;
pub mod cube;
pub mod erp;
pub mod gvdb;
pub mod hash;
pub mod lng;
pub mod lz4;
pub mod material;
pub mod nefs;
pub mod pe;
pub mod pkg;
pub mod texarray;
pub mod texture;

mod bytes;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unexpected end of data at {0:#x}")]
    Eof(usize),
    #[error("bad magic at {offset:#x}: expected {expected}")]
    Magic { offset: usize, expected: &'static str },
    #[error("{0}")]
    Format(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{}: {source}", path.display())]
    IoAt {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

/// An I/O error on `path`, named in the message.
pub(crate) fn io_at(path: &std::path::Path) -> impl FnOnce(std::io::Error) -> Error + '_ {
    move |source| Error::IoAt {
        path: path.to_path_buf(),
        source,
    }
}
