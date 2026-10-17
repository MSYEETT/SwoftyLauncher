use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Deserialize;
use swofty_formats::gvdb::Gvdb;

pub use crate::asset::Kind;
use crate::asset::{AssetOp, FileKind, FileOp, GroupOp, LangOp, Op, OpFile, Origin};
use crate::ops::{DbOps, ErpOp, apply_db_ops};
use crate::owned::{self, Added};
use crate::{Error, Result};

pub const FEATURES_FILE: &str = "swofty/addons/core/features.json";

const BUILD_DELIVERIES: [Kind; 3] = [Kind::Erp, Kind::Db, Kind::File];
const RUNTIME_DELIVERIES: [Kind; 0] = [];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Build,
    Runtime,
    Waiting,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Features {
    pub stable: BTreeSet<Kind>,
    pub experimental: BTreeSet<Kind>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Level {
    Stable,
    Experimental,
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct FeaturesFile {
    #[serde(default)]
    features: BTreeMap<String, Level>,
}

impl Features {
    pub fn load(loader: &Path) -> Result<Self> {
        let path = loader.join(FEATURES_FILE);
        if !path.is_file() {
            return Ok(Self::default());
        }
        let file: FeaturesFile = crate::read_json(&path)?;
        let mut out = Self::default();
        for (name, level) in file.features {
            let Ok(kind) = serde_json::from_value::<Kind>(name.into()) else {
                continue;
            };
            match level {
                Level::Stable => out.stable.insert(kind),
                Level::Experimental => out.experimental.insert(kind),
                Level::Other => false,
            };
        }
        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Target {
    Group(String),
    Resource { erp: String, uri: String },
    Record(u64),
    String { language: String, key: String },
    File(String),
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Group(g) => write!(f, "asset group {g}"),
            Target::Resource { erp, uri } => write!(f, "{uri} in {erp}"),
            Target::Record(key) => write!(f, "database record {key:#018x}"),
            Target::String { language, key } => write!(f, "text {key} ({language})"),
            Target::File(rel) => f.write_str(rel),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub target: Target,
    pub winner: Origin,
    pub overridden: Vec<Origin>,
}

impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lost: Vec<String> = self.overridden.iter().map(Origin::to_string).collect();
        write!(
            f,
            "{}: {} overrides {}",
            self.target,
            self.winner,
            lost.join(", ")
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct Planned {
    pub runtime: Vec<Op>,
    pub waiting: Vec<Op>,
    pub conflicts: Vec<Conflict>,
}

pub struct Planner<'a> {
    features: &'a Features,
    opt_in: &'a BTreeSet<Kind>,
    db: Vec<(Origin, DbOps)>,
    erp: Vec<Op>,
    files: Vec<Op>,
    planned: Planned,
    writers: BTreeMap<Target, Vec<Origin>>,
}

fn erp_target(op: &ErpOp) -> Option<(Target, bool)> {
    let resource = |erp: &str, uri: &str| Target::Resource {
        erp: erp.to_owned(),
        uri: uri.to_owned(),
    };
    match op {
        ErpOp::ReplaceResource { erp, resource: r, .. } | ErpOp::RemoveResource { erp, resource: r } => {
            Some((resource(erp, r), true))
        }
        ErpOp::PutResource {
            erp,
            resource: r,
            rename,
            ..
        } => Some((resource(erp, rename.as_deref().unwrap_or(r)), true)),
        ErpOp::CopyResource {
            erp,
            resource: r,
            rename,
            ..
        } => Some((resource(erp, rename.as_deref().unwrap_or(r)), false)),
        ErpOp::AddResource { erp, resource: r, .. } | ErpOp::PatchPkg { erp, resource: r, .. } => {
            Some((resource(erp, r), false))
        }
        ErpOp::AddMipmaps { .. } => None,
    }
}

fn target(op: &AssetOp) -> Option<(Target, bool)> {
    match op {
        AssetOp::Group(GroupOp::Register { group, .. }) => Some((Target::Group(group.clone()), false)),
        AssetOp::Erp(op) => erp_target(op),
        AssetOp::Lang(LangOp::Set { language, key, .. }) => Some((
            Target::String {
                language: language.clone(),
                key: key.clone(),
            },
            true,
        )),
        AssetOp::File(FileOp::Put { target, .. }) => Some((Target::File(target.clone()), true)),
        AssetOp::Db(_) | AssetOp::Lang(LangOp::Table { .. }) | AssetOp::Warn { .. } => None,
    }
}

impl<'a> Planner<'a> {
    pub fn new(features: &'a Features, opt_in: &'a BTreeSet<Kind>) -> Self {
        Self {
            features,
            opt_in,
            db: Vec::new(),
            erp: Vec::new(),
            files: Vec::new(),
            planned: Planned::default(),
            writers: BTreeMap::new(),
        }
    }

    pub fn route(&self, kind: Kind) -> Route {
        let stable = self.features.stable.contains(&kind);
        let experimental = self.features.experimental.contains(&kind);
        let build = BUILD_DELIVERIES.contains(&kind);
        let runtime = RUNTIME_DELIVERIES.contains(&kind)
            && if build {
                stable || (experimental && self.opt_in.contains(&kind))
            } else {
                stable || experimental
            };
        match (runtime, build) {
            (true, _) => Route::Runtime,
            (false, true) => Route::Build,
            (false, false) => Route::Waiting,
        }
    }

    pub fn push(&mut self, op: Op) {
        if let Some((target, replaces)) = target(&op.op) {
            self.write(target, &op.origin, replaces);
        }
        let Some(kind) = op.op.kind() else {
            self.erp.push(op);
            return;
        };
        match self.route(kind) {
            Route::Runtime => self.planned.runtime.push(op),
            Route::Waiting => self.planned.waiting.push(op),
            Route::Build => match op.op {
                AssetOp::Db(db) => match self.db.last_mut() {
                    Some((origin, ops)) if *origin == op.origin => ops.ops.push(db),
                    _ => self.db.push((
                        op.origin,
                        DbOps {
                            schema_version: 1,
                            fields: BTreeMap::new(),
                            ops: vec![db],
                        },
                    )),
                },
                AssetOp::Erp(_) => self.erp.push(op),
                _ => self.files.push(op),
            },
        }
    }

    pub fn push_file(&mut self, origin: &Origin, kind: FileKind, file: OpFile) {
        let db = file.ops.iter().any(|op| matches!(op, AssetOp::Db(_)));
        if (kind == FileKind::Database || db) && self.route(Kind::Db) == Route::Build {
            let ops = DbOps {
                schema_version: file.schema_version,
                fields: file.fields,
                ops: Vec::new(),
            };
            self.db.push((origin.clone(), ops));
        }
        for op in file.ops {
            self.push(Op {
                op,
                origin: origin.clone(),
            });
        }
    }

    fn write(&mut self, target: Target, origin: &Origin, replaces: bool) {
        let writers = self.writers.entry(target.clone()).or_default();
        if replaces {
            let lost: Vec<Origin> = writers.drain(..).filter(|w| w != origin).collect();
            let conflicts = &mut self.planned.conflicts;
            match conflicts.iter_mut().find(|c| c.target == target) {
                _ if lost.is_empty() => {}
                Some(c) => {
                    let before = std::mem::replace(&mut c.winner, origin.clone());
                    for o in std::iter::once(before).chain(lost) {
                        if o != *origin && !c.overridden.contains(&o) {
                            c.overridden.push(o);
                        }
                    }
                    c.overridden.retain(|o| o != origin);
                }
                None => conflicts.push(Conflict {
                    target,
                    winner: origin.clone(),
                    overridden: lost,
                }),
            }
        }
        if !writers.contains(origin) {
            writers.push(origin.clone());
        }
    }

    pub fn has_db(&self) -> bool {
        !self.db.is_empty()
    }

    pub fn apply_db(&mut self, db: &mut Gvdb) -> Result<Vec<Added>> {
        let names: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut added = Vec::new();
        for (origin, ops) in std::mem::take(&mut self.db) {
            let before = owned::counts(db);
            let replaced = apply_db_ops(db, &ops, &names)
                .map_err(|e| Error::Other(format!("applying {origin}: {e}")))?;
            for key in replaced {
                self.write(Target::Record(key), &origin, true);
            }
            added.extend(owned::added(db, &origin.addon, &before));
        }
        Ok(added)
    }

    pub fn take_erp(&mut self) -> Vec<Op> {
        std::mem::take(&mut self.erp)
    }

    pub fn take_files(&mut self) -> Vec<Op> {
        std::mem::take(&mut self.files)
    }

    pub fn finish(self) -> Planned {
        self.planned
    }
}

pub fn waiting_lines(waiting: &[Op]) -> Vec<String> {
    let mut counts: BTreeMap<(String, Kind), usize> = BTreeMap::new();
    for op in waiting {
        if let Some(kind) = op.op.kind() {
            *counts.entry((op.origin.to_string(), kind)).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .map(|((origin, kind), n)| {
            format!("{origin}: {n} {kind} op(s) wait for {kind} support in swofty_core")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::Source;

    fn origin(addon: &str) -> Origin {
        Origin {
            addon: addon.into(),
            item: None,
        }
    }

    fn op(addon: &str, json: &str) -> Op {
        Op {
            op: serde_json::from_str(json).unwrap(),
            origin: origin(addon),
        }
    }

    #[test]
    fn reads_features_and_routes_each_kind() {
        let dir = std::env::temp_dir().join(format!("swofty-features-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(Features::load(&dir).unwrap(), Features::default());
        let path = dir.join(FEATURES_FILE);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{ "schema": 1, "features": { "group": "stable", "erp": "experimental", "db": "experimental",
                 "lang": "experimental", "file": "experimental", "audio": "stable", "tex": "soon" } }"#,
        )
        .unwrap();
        let features = Features::load(&dir).unwrap();
        assert_eq!(features.stable, BTreeSet::from([Kind::Group]));
        assert_eq!(features.experimental.len(), 4);
        let opt_in = BTreeSet::from([Kind::Erp]);
        let p = Planner::new(&features, &opt_in);
        assert_eq!(p.route(Kind::Erp), Route::Build);
        assert_eq!(p.route(Kind::Db), Route::Build);
        assert_eq!(p.route(Kind::File), Route::Build);
        assert_eq!(p.route(Kind::Lang), Route::Waiting);
        assert_eq!(p.route(Kind::Group), Route::Waiting);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn records_conflicts_for_replacing_writes_only() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let mut p = Planner::new(&features, &opt_in);
        let put = r#"{"op":"erp.put-resource","erp":"a.erp","resource":"r","source":"x.erp"}"#;
        let patch = r#"{"op":"erp.patch-pkg","erp":"a.erp","resource":"r","edits":[]}"#;
        p.push(op("a", put));
        p.push(op("b", patch));
        p.push(op("c", put));
        p.push(op("c", r#"{"op":"file.put","source":"f","target":"x/y.dds"}"#));
        p.push(op("d", r#"{"op":"file.put","source":"f","target":"x/y.dds"}"#));
        p.push(op(
            "a",
            r#"{"op":"lang.set","language":"*","key":"k","value":"v"}"#,
        ));
        p.push(op("a", r#"{"op":"warn","message":"m"}"#));
        assert_eq!(p.take_erp().len(), 4);
        let files = p.take_files();
        assert_eq!(files.len(), 2);
        assert!(
            matches!(&files[0].op, AssetOp::File(FileOp::Put { source, .. }) if *source == Source::addon("f"))
        );
        let planned = p.finish();
        assert_eq!(planned.waiting.len(), 1);
        assert_eq!(
            waiting_lines(&planned.waiting),
            ["a: 1 lang op(s) wait for lang support in swofty_core"]
        );
        let shown: Vec<String> = planned.conflicts.iter().map(Conflict::to_string).collect();
        assert_eq!(shown, ["r in a.erp: c overrides a, b", "x/y.dds: d overrides c"]);
    }

    #[test]
    fn database_files_keep_their_schema_and_report_replaced_records() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let mut p = Planner::new(&features, &opt_in);
        let empty: OpFile = serde_json::from_str(r#"{"schema_version":1,"ops":[]}"#).unwrap();
        p.push_file(&origin("a"), FileKind::Database, empty.clone());
        p.push_file(&origin("a"), FileKind::Assets, empty);
        assert_eq!(p.db.len(), 1);
        let key = "rules.x";
        p.push(op(
            "a",
            &format!(r#"{{"op":"db.put-record","key":"{key}","type":0,"value":"31"}}"#),
        ));
        p.push(op(
            "b",
            &format!(r#"{{"op":"db.put-record","key":"{key}","type":0,"value":"32"}}"#),
        ));
        assert_eq!(p.db.len(), 2);
        let mut db = Gvdb::default();
        p.apply_db(&mut db).unwrap();
        assert_eq!(db.get_text(key).as_deref(), Some("2"));
        let planned = p.finish();
        assert_eq!(planned.conflicts.len(), 1);
        assert_eq!(planned.conflicts[0].winner, origin("b"));
        assert_eq!(planned.conflicts[0].overridden, [origin("a")]);
    }
}
