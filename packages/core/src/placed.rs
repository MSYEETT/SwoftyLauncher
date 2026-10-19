//! The files the launcher has written into the game folder, with the bytes it
//! wrote. Journals undo a session; this record outlives them, so a file left
//! behind by a session whose journal is gone can still be proven to be ours
//! (same path, same SHA-256) and cleaned up without touching anything a
//! player copied in by hand.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Result, read_json, write_json};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacedFile {
    pub path: String,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub replaced: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placed {
    pub schema_version: u32,
    #[serde(default)]
    pub files: BTreeMap<String, PlacedFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,
}

/// Game paths compare the way the game folder's file system does: NTFS is
/// case-insensitive, and either slash separates folders on Windows.
pub fn key(rel: &str) -> String {
    let rel = rel.replace('\\', "/");
    if cfg!(windows) {
        rel.to_ascii_lowercase()
    } else {
        rel
    }
}

impl Placed {
    pub fn path(data: &Path) -> PathBuf {
        data.join("state/placed.json")
    }

    pub fn load(data: &Path) -> Result<Self> {
        let path = Self::path(data);
        if !path.is_file() {
            return Ok(Self {
                schema_version: 1,
                ..Self::default()
            });
        }
        read_json(&path)
    }

    pub fn save(&self, data: &Path) -> Result<()> {
        if self.files.is_empty() && self.registry.is_none() {
            let path = Self::path(data);
            return match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(crate::io_err(&path)(e)),
                _ => Ok(()),
            };
        }
        write_json(&Self::path(data), self)
    }

    pub fn get(&self, rel: &str) -> Option<&PlacedFile> {
        self.files.get(&key(rel))
    }

    pub fn wrote(&self, rel: &str, sha256: &str) -> bool {
        self.get(rel).is_some_and(|f| f.sha256 == sha256)
    }

    pub fn insert(&mut self, rel: &str, sha256: String, replaced: bool) {
        self.files.insert(
            key(rel),
            PlacedFile {
                path: rel.to_owned(),
                sha256,
                replaced,
            },
        );
    }

    pub fn remove(&mut self, rel: &str) {
        self.files.remove(&key(rel));
    }
}
