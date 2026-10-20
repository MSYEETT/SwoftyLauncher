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

    /// Back up `src` under its SHA-256. The copy is flushed to disk before it
    /// takes its name, since it is the only copy of an original once the game
    /// file is replaced; an object a crash left damaged is written again.
    pub fn put_file(&self, src: &Path) -> Result<String> {
        let sha = sha256_file(src)?;
        let dst = self.path(&sha);
        if !dst.is_file() || sha256_file(&dst)? != sha {
            let dir = dst.parent().expect("has parent");
            std::fs::create_dir_all(dir).map_err(io_err(dir))?;
            let tmp = dst.with_extension("tmp");
            std::fs::copy(src, &tmp).map_err(io_err(&tmp))?;
            crate::sync_file(&tmp)?;
            if sha256_file(&tmp)? != sha {
                return Err(Error::Other(format!(
                    "{} changed while it was backed up",
                    src.display()
                )));
            }
            std::fs::rename(&tmp, &dst).map_err(io_err(&dst))?;
            crate::sync_dir(Some(dir));
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
        if let Some(dir) = dst.parent() {
            std::fs::create_dir_all(dir).map_err(io_err(dir))?;
        }
        let tmp = dst.with_file_name(format!(
            ".{}.swofty-restore",
            dst.file_name().unwrap_or_default().to_string_lossy()
        ));
        std::fs::copy(&src, &tmp).map_err(io_err(&tmp))?;
        crate::sync_file(&tmp)?;
        if sha256_file(&tmp)? != sha {
            std::fs::remove_file(&tmp).ok();
            return Err(Error::Other(format!(
                "the backup of {} is damaged; verify the game files in Steam",
                dst.display()
            )));
        }
        std::fs::rename(&tmp, dst).map_err(io_err(dst))?;
        crate::sync_dir(dst.parent());
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rebuild: Option<Rebuild>,
}

/// Set on a baseline started over because the previous one was damaged.
/// Until the player has been told to verify the game files, a game file is
/// only recorded as an original when its bytes are already a backed-up
/// original in the object store.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rebuild {
    pub damaged: String,
    pub at: u64,
    #[serde(default)]
    pub told_to_verify: bool,
}

impl Rebuild {
    pub fn checking(&self) -> bool {
        !self.told_to_verify
    }
}

pub const DAMAGED: &str = "the launcher's record of your original game files is damaged; verify the game files in Steam, then press Play again";

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
            rebuild: None,
        }
    }

    pub fn path(data: &Path, id: &Identity) -> PathBuf {
        data.join("baseline").join(id.key()).join("baseline.json")
    }

    /// The baseline for `id`. A damaged one is moved aside and started over
    /// in rebuild mode (see [`Rebuild`]), which is saved at once so every
    /// later load sees it; a missing one next to a damaged copy is rebuilt
    /// the same way.
    pub fn load_or_new(data: &Path, id: &Identity) -> Result<Self> {
        let p = Self::path(data, id);
        match crate::load_json(&p)? {
            crate::Loaded::Found(b) => Ok(b),
            crate::Loaded::Missing => Ok(match damaged_copy(&p) {
                Some(name) => Self::rebuilding(id, name),
                None => Self::new(id),
            }),
            crate::Loaded::Damaged(moved) => {
                let name = moved
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                eprintln!(
                    "swofty: warning: the record of the original game files for build {} was damaged; \
                     rebuilding it from game files that match backed-up originals",
                    id.build_id
                );
                let b = Self::rebuilding(id, name);
                b.save(data)?;
                Ok(b)
            }
        }
    }

    fn rebuilding(id: &Identity, damaged: String) -> Self {
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Self {
            rebuild: Some(Rebuild {
                damaged,
                at,
                told_to_verify: false,
            }),
            ..Self::new(id)
        }
    }

    pub fn checking(&self) -> bool {
        self.rebuild.as_ref().is_some_and(Rebuild::checking)
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

fn damaged_copy(path: &Path) -> Option<String> {
    let name = format!("{}{}", path.file_name()?.to_string_lossy(), crate::DAMAGED_SUFFIX);
    std::fs::read_dir(path.parent()?)
        .ok()?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(&name))
        .max()
}

pub const LANG_PREFIX: &str = "localisation/2025_release/language_";

pub fn lang_dir(data: &Path, id: &Identity) -> PathBuf {
    data.join("baseline").join(id.key()).join("lang")
}

pub fn original_lang(data: &Path, game: &Path, id: &Identity, code: &str) -> Result<PathBuf> {
    let dir = lang_dir(data, id);
    if !dir.is_dir() {
        extract_lang(game, &dir)?;
    }
    let path = dir.join(format!("language_{code}.lng"));
    if path.is_file() {
        Ok(path)
    } else {
        Err(Error::Other(format!("the game has no {code} language file")))
    }
}

pub fn extract_lang(game: &Path, dir: &Path) -> Result<Vec<String>> {
    let exe_path = game.join(crate::GAME_EXE);
    let mut exe = std::fs::File::open(&exe_path).map_err(io_err(&exe_path))?;
    let archive = swofty_formats::nefs::Archive::open_exe(&mut exe)?;
    let data_path = game.join(&archive.volume);
    let mut data = std::fs::File::open(&data_path).map_err(io_err(&data_path))?;
    let tmp = dir.with_extension(format!("tmp-{}", std::process::id()));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp).map_err(io_err(&tmp))?;
    }
    std::fs::create_dir_all(&tmp).map_err(io_err(&tmp))?;
    let mut written = Vec::new();
    for entry in &archive.entries {
        let Some(name) = entry
            .path
            .strip_prefix(LANG_PREFIX)
            .filter(|n| n.ends_with(".lng") && !n.contains('/'))
        else {
            continue;
        };
        let bytes = archive.read(entry, &mut data)?;
        if !swofty_formats::lng::is_lng(&bytes) {
            std::fs::remove_dir_all(&tmp).ok();
            return Err(Error::Other(format!(
                "{} in {} is not a language file",
                entry.path,
                crate::GAME_EXE
            )));
        }
        let name = format!("language_{name}");
        let path = tmp.join(&name);
        std::fs::write(&path, bytes).map_err(io_err(&path))?;
        written.push(name);
    }
    if written.is_empty() {
        std::fs::remove_dir_all(&tmp).ok();
        return Err(Error::Other(format!(
            "{} holds no language files",
            crate::GAME_EXE
        )));
    }
    written.sort();
    if let Err(e) = std::fs::rename(&tmp, dir) {
        std::fs::remove_dir_all(&tmp).ok();
        if !dir.is_dir() {
            return Err(io_err(dir)(e));
        }
    }
    Ok(written)
}

pub fn exe_version(game: &Path) -> Result<Option<String>> {
    let path = game.join(crate::GAME_EXE);
    let mut exe = std::fs::File::open(&path).map_err(io_err(&path))?;
    Ok(swofty_formats::pe::file_version(&mut exe)?)
}
