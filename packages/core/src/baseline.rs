use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{Error, Result, io_err};

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_bytes(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path).map_err(io_err(path))?;
    let mut h = Sha256::new();
    let mut buf = [0; 1 << 16];
    loop {
        match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => h.update(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(io_err(path)(e)),
        }
    }
    Ok(hex(&h.finalize()))
}

pub fn sha256_opt(path: &Path) -> Result<Option<String>> {
    if path.is_file() {
        sha256_file(path).map(Some)
    } else {
        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct Objects {
    pub root: PathBuf,
}

impl Objects {
    pub fn path(&self, sha: &str) -> PathBuf {
        self.root.join(&sha[..2]).join(sha)
    }

    pub fn has(&self, sha: &str) -> bool {
        self.path(sha).is_file()
    }

    pub fn put_file(&self, src: &Path) -> Result<String> {
        let sha = sha256_file(src)?;
        let dst = self.path(&sha);
        if !dst.is_file() {
            let dir = dst.parent().expect("has parent");
            std::fs::create_dir_all(dir).map_err(io_err(dir))?;
            let tmp = dst.with_extension("tmp");
            std::fs::copy(src, &tmp).map_err(io_err(&tmp))?;
            if sha256_file(&tmp)? != sha {
                return Err(Error::Other(format!(
                    "{} changed while it was backed up",
                    src.display()
                )));
            }
            std::fs::rename(&tmp, &dst).map_err(io_err(&dst))?;
        }
        Ok(sha)
    }

    pub fn restore_to(&self, sha: &str, dst: &Path) -> Result<()> {
        let src = self.path(sha);
        if !src.is_file() {
            return Err(Error::Other(format!(
                "backup {sha} for {} is missing",
                dst.display()
            )));
        }
        let tmp = dst.with_file_name(format!(
            ".{}.swofty-restore",
            dst.file_name().unwrap_or_default().to_string_lossy()
        ));
        std::fs::copy(&src, &tmp).map_err(io_err(&tmp))?;
        std::fs::rename(&tmp, dst).map_err(io_err(dst))?;
        if sha256_file(dst)? != sha {
            return Err(Error::Other(format!(
                "{} does not match its backup after restore",
                dst.display()
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    Depot,
    FirstSight,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileRecord {
    pub size: u64,
    pub sha256: String,
    pub source: Source,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Identity {
    pub build_id: String,
    pub exe_sha256: String,
}

impl Identity {
    pub fn detect(game_dir: &Path, build_id: Option<u64>) -> Result<Self> {
        let exe = sha256_file(&game_dir.join(crate::GAME_EXE))?;
        Ok(Self {
            build_id: build_id.map_or_else(|| format!("exe-{}", &exe[..12]), |b| b.to_string()),
            exe_sha256: exe,
        })
    }

    pub fn detect_cached(data: &Path, game_dir: &Path, build_id: Option<u64>) -> Result<Self> {
        let exe = game_dir.join(crate::GAME_EXE);
        let meta = std::fs::metadata(&exe).map_err(io_err(&exe))?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        let stamp = format!("{}|{}|{mtime}", exe.display(), meta.len());
        let cache = data.join("state/exe_hash.json");
        let cached: Option<(String, String)> = crate::read_json(&cache).ok();
        let sha = match cached {
            Some((s, sha)) if s == stamp => sha,
            _ => {
                let sha = sha256_file(&exe)?;
                crate::write_json(&cache, &(stamp, sha.clone()))?;
                sha
            }
        };
        Ok(Self {
            build_id: build_id.map_or_else(|| format!("exe-{}", &sha[..12]), |b| b.to_string()),
            exe_sha256: sha,
        })
    }

    pub fn key(&self) -> String {
        format!("{}-{}", self.build_id, &self.exe_sha256[..12])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Baseline {
    pub schema_version: u32,
    pub build_id: String,
    pub exe_sha256: String,
    #[serde(default)]
    pub files: BTreeMap<String, FileRecord>,
}

pub trait DepotHashes {
    fn sha256(&self, rel: &str) -> Option<String>;
}

impl DepotHashes for () {
    fn sha256(&self, _: &str) -> Option<String> {
        None
    }
}

impl Baseline {
    pub fn new(id: &Identity) -> Self {
        Self {
            schema_version: 1,
            build_id: id.build_id.clone(),
            exe_sha256: id.exe_sha256.clone(),
            files: BTreeMap::new(),
        }
    }

    pub fn path(data: &Path, id: &Identity) -> PathBuf {
        data.join("baseline").join(id.key()).join("baseline.json")
    }

    pub fn load_or_new(data: &Path, id: &Identity) -> Result<Self> {
        let p = Self::path(data, id);
        if p.is_file() {
            crate::read_json(&p)
        } else {
            Ok(Self::new(id))
        }
    }

    pub fn save(&self, data: &Path) -> Result<()> {
        let id = Identity {
            build_id: self.build_id.clone(),
            exe_sha256: self.exe_sha256.clone(),
        };
        crate::write_json(&Self::path(data, &id), self)
    }

    pub fn record(
        &mut self,
        objects: &Objects,
        game: &Path,
        rel: &str,
        depot: &dyn DepotHashes,
    ) -> Result<&FileRecord> {
        if !self.files.contains_key(rel) {
            let path = game.join(rel);
            let size = std::fs::metadata(&path).map_err(io_err(&path))?.len();
            let sha = objects.put_file(&path)?;
            let source = match depot.sha256(rel) {
                Some(d) if d == sha => Source::Depot,
                Some(d) => {
                    return Err(Error::Other(format!(
                        "{rel} does not match Steam's depot hash ({d}); verify the game files"
                    )));
                }
                None => Source::FirstSight,
            };
            self.files.insert(
                rel.to_owned(),
                FileRecord {
                    size,
                    sha256: sha,
                    source,
                },
            );
        }
        Ok(&self.files[rel])
    }
}
