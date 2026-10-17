pub mod bxml;
pub mod erp;
pub mod gvdb;
pub mod hash;
pub mod pkg;
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
}

pub type Result<T> = std::result::Result<T, Error>;
