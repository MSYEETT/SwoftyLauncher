use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Result, io_err};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Op {
    ReplaceFile,
    CreateFile,
    CreateDir,
    RegistryOverride,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    Pending,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub seq: u32,
    pub op: Op,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    pub state: State,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Journal {
    pub schema_version: u32,
    pub id: String,
    pub profile: String,
    pub build_id: String,
    pub entries: Vec<Entry>,
    pub committed: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub session: bool,
    #[serde(skip)]
    pub file: PathBuf,
}

impl Journal {
    pub fn dir(data: &Path) -> PathBuf {
        data.join("state/journal")
    }

    pub fn create(data: &Path, profile: &str, build_id: &str, session: bool) -> Result<Self> {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let id = format!("{n:x}-{}", std::process::id());
        let file = Self::dir(data).join(format!("{id}.json"));
        let j = Self {
            schema_version: 1,
            id,
            profile: profile.into(),
            build_id: build_id.into(),
            entries: Vec::new(),
            committed: false,
            session,
            file,
        };
        j.save()?;
        Ok(j)
    }

    pub fn open(data: &Path) -> Result<Vec<Self>> {
        let dir = Self::dir(data);
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir(&dir) else {
            return Ok(out);
        };
        let mut paths: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        paths.sort();
        for p in paths {
            if let crate::Loaded::Found(mut j) = crate::load_json::<Self>(&p)? {
                j.file = p;
                out.push(j);
            }
        }
        Ok(out)
    }

    pub fn save(&self) -> Result<()> {
        crate::write_json(&self.file, self)
    }

    pub fn begin(
        &mut self,
        op: Op,
        path: &str,
        before: Option<String>,
        after: Option<String>,
    ) -> Result<usize> {
        let seq = self.entries.len() as u32;
        self.entries.push(Entry {
            seq,
            op,
            path: path.into(),
            before,
            after,
            state: State::Pending,
        });
        self.save()?;
        Ok(self.entries.len() - 1)
    }

    /// Mark entry `i` finished. It reaches the disk with the next
    /// [`begin`](Self::begin) or [`commit`](Self::commit), which saves one
    /// flushed write per step; a crash before then leaves it pending, which a
    /// rollback treats exactly as done while the file holds what was written.
    pub fn done(&mut self, i: usize) -> Result<()> {
        self.entries[i].state = State::Done;
        Ok(())
    }

    pub fn commit(&mut self) -> Result<()> {
        self.committed = true;
        self.save()
    }

    pub fn close(self) -> Result<()> {
        std::fs::remove_file(&self.file).map_err(io_err(&self.file))
    }
}
