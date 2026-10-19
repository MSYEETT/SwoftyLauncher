use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::baseline::{Baseline, DepotHashes, Identity, Objects, sha256_bytes, sha256_file, sha256_opt};
use crate::journal::{Journal, Op, State};
use crate::profile::VANILLA;
use crate::{Error, Result, io_err, with_retry};

pub const PROXY: &str = "version.dll";
pub const LOADER_DIR: &str = "swofty";
pub const OVERRIDE_HEADER: &str = "[Software\\\\Wine\\\\AppDefaults\\\\F1_25.exe\\\\DllOverrides]";
const OVERRIDE_VALUE: &str = "\"version\"=\"native,builtin\"";

#[derive(Debug, Clone)]
pub struct Game {
    pub dir: PathBuf,
    pub user_reg: Option<PathBuf>,
    pub identity: Identity,
}

#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub profile: String,
    pub data: BTreeMap<String, PathBuf>,
    pub added: BTreeMap<String, PathBuf>,
    pub loader: BTreeMap<String, PathBuf>,
    pub registry_override: bool,
}

impl Plan {
    pub fn vanilla() -> Self {
        Self {
            profile: VANILLA.into(),
            ..Self::default()
        }
    }

    pub fn is_vanilla(&self) -> bool {
        self.data.is_empty() && self.added.is_empty() && self.loader.is_empty() && !self.registry_override
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Drift {
    Changed(String),
    Missing(String),
    Addition(String),
    RegistryOverride,
    OpenJournal(String),
    NewBuild { baseline: String, current: String },
    LeftInPlace(String),
}

impl std::fmt::Display for Drift {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Drift::Changed(p) => write!(f, "{p} differs from the baseline"),
            Drift::Missing(p) => write!(f, "{p} is missing"),
            Drift::Addition(p) => write!(f, "{p} is not part of the game"),
            Drift::RegistryOverride => write!(f, "the prefix has the version.dll override"),
            Drift::OpenJournal(id) => write!(f, "journal {id} is open"),
            Drift::NewBuild { baseline, current } => {
                write!(f, "game build {current} differs from baseline {baseline}")
            }
            Drift::LeftInPlace(p) => write!(
                f,
                "{p} changed since it was written (game update?); left in place"
            ),
        }
    }
}

pub struct Installer<'a> {
    pub data: &'a Path,
    pub game: &'a Game,
    pub depot: &'a dyn DepotHashes,
    pub stop_after: Option<usize>,
}

fn rel_parts(rel: &str) -> Vec<String> {
    let parts: Vec<&str> = rel.split('/').collect();
    (1..parts.len()).map(|i| parts[..i].join("/")).collect()
}

fn remove_section(text: &str, header: &str) -> Option<String> {
    let start = text.find(header)?;
    let from = if start > 0 && text.as_bytes()[start - 1] == b'\n' {
        start - 1
    } else {
        start
    };
    let end = text[start + header.len()..]
        .find("\n[")
        .map_or(text.len(), |i| start + header.len() + i);
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..from]);
    out.push_str(&text[end..]);
    if !text[end..].is_empty() && from == start {
        out = format!("{}{}", &text[..from], &text[end + 1..]);
    }
    if text.ends_with('\n') && !out.ends_with('\n') && !out.is_empty() {
        out.push('\n');
    }
    Some(out)
}

impl<'a> Installer<'a> {
    pub fn new(data: &'a Path, game: &'a Game) -> Self {
        Self {
            data,
            game,
            depot: &(),
            stop_after: None,
        }
    }

    fn objects(&self) -> Objects {
        Objects {
            root: self.data.join("baseline/objects"),
        }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.game.dir.join(rel)
    }

    pub fn baseline(&self) -> Result<Baseline> {
        Baseline::load_or_new(self.data, &self.game.identity)
    }

    pub fn recover(&self) -> Result<Vec<Drift>> {
        let mut report = Vec::new();
        for j in Journal::open(self.data)? {
            report.extend(self.rollback(j)?);
        }
        Ok(report)
    }

    fn rollback(&self, j: Journal) -> Result<Vec<Drift>> {
        let mut report = Vec::new();
        let same_build = j.build_id == self.game.identity.build_id;
        for e in j.entries.iter().rev() {
            match e.op {
                Op::ReplaceFile => {
                    let path = self.path(&e.path);
                    let now = sha256_opt(&path)?;
                    let before = e.before.as_deref().expect("replace-file has before");
                    if now.as_deref() == Some(before) {
                        continue;
                    }
                    let ours = now == e.after || e.state == State::Pending;
                    if ours && same_build {
                        self.objects().restore_to(before, &path)?;
                    } else {
                        report.push(Drift::LeftInPlace(e.path.clone()));
                    }
                }
                Op::CreateFile => {
                    let path = self.path(&e.path);
                    match sha256_opt(&path)? {
                        None => {}
                        Some(now) if e.state == State::Pending || Some(&now) == e.after.as_ref() => {
                            std::fs::remove_file(&path).map_err(io_err(&path))?;
                        }
                        Some(_) => report.push(Drift::LeftInPlace(e.path.clone())),
                    }
                }
                Op::CreateDir => {
                    let path = self.path(&e.path);
                    if path.is_dir() && std::fs::read_dir(&path).map_err(io_err(&path))?.next().is_none() {
                        std::fs::remove_dir(&path).map_err(io_err(&path))?;
                    }
                }
                Op::RegistryOverride => {
                    let path = PathBuf::from(&e.path);
                    let text = std::fs::read_to_string(&path).map_err(io_err(&path))?;
                    if let Some(out) = remove_section(&text, OVERRIDE_HEADER) {
                        std::fs::write(&path, out).map_err(io_err(&path))?;
                    }
                }
            }
        }
        j.close()?;
        Ok(report)
    }

    fn step(&self, n: &mut usize) -> Result<()> {
        *n += 1;
        match self.stop_after {
            Some(limit) if *n > limit => Err(Error::Other(format!("stopped after {limit} step(s)"))),
            _ => Ok(()),
        }
    }

    pub fn apply(&self, plan: &Plan) -> Result<()> {
        self.recover()?;
        if plan.is_vanilla() {
            return Ok(());
        }
        let objects = self.objects();
        let mut baseline = self.baseline()?;
        for rel in plan.data.keys() {
            let rec = baseline
                .record(&objects, &self.game.dir, rel, self.depot)?
                .clone();
            if sha256_file(&self.path(rel))? != rec.sha256 {
                return Err(Error::Other(format!(
                    "{rel} is not the original; run swofty restore or verify the game files"
                )));
            }
        }
        baseline.save(self.data)?;
        for rel in plan.added.keys().chain(plan.loader.keys()) {
            if self.path(rel).exists() {
                return Err(Error::Other(format!(
                    "{rel} already exists in the game folder; run swofty restore"
                )));
            }
        }
        let mut j = Journal::create(self.data, &plan.profile, &self.game.identity.build_id)?;
        let mut n = 0;
        for (rel, src) in &plan.data {
            let path = self.path(rel);
            let after = sha256_file(src)?;
            let i = j.begin(
                Op::ReplaceFile,
                rel,
                Some(baseline.files[rel].sha256.clone()),
                Some(after),
            )?;
            self.step(&mut n)?;
            std::fs::copy(src, &path).map_err(io_err(&path))?;
            j.done(i)?;
        }
        for (rel, src) in plan.added.iter().chain(&plan.loader) {
            for dir in rel_parts(rel) {
                let path = self.path(&dir);
                if !path.is_dir() {
                    let i = j.begin(Op::CreateDir, &dir, None, None)?;
                    self.step(&mut n)?;
                    std::fs::create_dir(&path).map_err(io_err(&path))?;
                    j.done(i)?;
                }
            }
            let path = self.path(rel);
            let i = j.begin(Op::CreateFile, rel, None, Some(sha256_file(src)?))?;
            self.step(&mut n)?;
            std::fs::copy(src, &path).map_err(io_err(&path))?;
            j.done(i)?;
        }
        if plan.registry_override
            && let Some(reg) = &self.game.user_reg
        {
            let text = std::fs::read_to_string(reg).map_err(io_err(reg))?;
            if !text.contains(OVERRIDE_HEADER) {
                let before = objects.put_file(reg)?;
                let mut out = text.clone();
                if !out.ends_with('\n') && !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&format!("\n{OVERRIDE_HEADER}\n{OVERRIDE_VALUE}\n"));
                let i = j.begin(
                    Op::RegistryOverride,
                    &reg.display().to_string(),
                    Some(before),
                    Some(sha256_bytes(out.as_bytes())),
                )?;
                self.step(&mut n)?;
                std::fs::write(reg, out).map_err(io_err(reg))?;
                j.done(i)?;
            }
        }
        j.commit()
    }

    pub fn restore(&self) -> Result<Vec<Drift>> {
        let mut report = self.recover()?;
        let proxy = self.path(PROXY);
        let baseline = self.baseline()?;
        if proxy.is_file() && !baseline.files.contains_key(PROXY) {
            with_retry(&proxy, || std::fs::remove_file(&proxy))?;
        }
        let loader = self.path(LOADER_DIR);
        if loader.is_dir() {
            let logs = self.data.join("sessions/orphaned-logs");
            for e in std::fs::read_dir(&loader).map_err(io_err(&loader))?.flatten() {
                if e.path().extension().is_some_and(|x| x == "log") {
                    std::fs::create_dir_all(&logs).map_err(io_err(&logs))?;
                    crate::move_file(&e.path(), &logs.join(e.file_name()))?;
                }
            }
            with_retry(&loader, || std::fs::remove_dir_all(&loader))?;
        }
        if let Some(reg) = &self.game.user_reg
            && let Ok(text) = std::fs::read_to_string(reg)
            && let Some(out) = remove_section(&text, OVERRIDE_HEADER)
        {
            self.objects().put_file(reg)?;
            std::fs::write(reg, out).map_err(io_err(reg))?;
        }
        for (rel, rec) in &baseline.files {
            let path = self.path(rel);
            if sha256_opt(&path)?.as_deref() != Some(rec.sha256.as_str()) {
                self.objects().restore_to(&rec.sha256, &path)?;
            }
        }
        report.extend(self.verify()?);
        Ok(report)
    }

    pub fn verify(&self) -> Result<Vec<Drift>> {
        let mut out: Vec<Drift> = Journal::open(self.data)?
            .into_iter()
            .map(|j| Drift::OpenJournal(j.id))
            .collect();
        let baseline = self.baseline()?;
        for (rel, rec) in &baseline.files {
            match sha256_opt(&self.path(rel))? {
                None => out.push(Drift::Missing(rel.clone())),
                Some(s) if s != rec.sha256 => out.push(Drift::Changed(rel.clone())),
                Some(_) => {}
            }
        }
        for rel in [PROXY, LOADER_DIR] {
            if self.path(rel).exists() && !baseline.files.contains_key(rel) {
                out.push(Drift::Addition(rel.into()));
            }
        }
        if let Some(reg) = &self.game.user_reg
            && std::fs::read_to_string(reg).is_ok_and(|t| t.contains(OVERRIDE_HEADER))
        {
            out.push(Drift::RegistryOverride);
        }
        let latest = std::fs::read_dir(self.data.join("baseline"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| crate::read_json::<Baseline>(&e.path().join("baseline.json")).ok())
            .find(|b| {
                b.build_id != self.game.identity.build_id && b.files.keys().any(|k| self.path(k).exists())
            });
        if let Some(old) = latest
            && baseline.files.is_empty()
        {
            out.push(Drift::NewBuild {
                baseline: old.build_id,
                current: self.game.identity.build_id.clone(),
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_appended_section_exactly() {
        let orig = "WINE REGISTRY Version 2\n\n[Software\\\\Wine] 1\n\"a\"=\"b\"\n";
        let with = format!("{orig}\n{OVERRIDE_HEADER}\n{OVERRIDE_VALUE}\n");
        assert_eq!(remove_section(&with, OVERRIDE_HEADER).unwrap(), orig);
        let middle = format!("{orig}\n{OVERRIDE_HEADER}\n{OVERRIDE_VALUE}\n\n[Other] 2\n\"x\"=\"y\"\n");
        assert_eq!(
            remove_section(&middle, OVERRIDE_HEADER).unwrap(),
            format!("{orig}\n[Other] 2\n\"x\"=\"y\"\n")
        );
    }

    #[test]
    fn dir_prefixes() {
        assert_eq!(
            rel_parts("swofty/addons/core/x.dll"),
            ["swofty", "swofty/addons", "swofty/addons/core"]
        );
        assert!(rel_parts("version.dll").is_empty());
    }
}
