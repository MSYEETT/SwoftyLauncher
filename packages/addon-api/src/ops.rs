use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use swofty_formats::gvdb::{Gvdb, Record, instance_count_path};
use swofty_formats::hash::{fnv1a64, fnv1a64_from};

use crate::asset::Source;
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DbOps {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Vec<String>>,
    pub ops: Vec<DbOp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DbOp {
    Set {
        select: String,
        field: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value_from: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        datasets: Vec<String>,
        #[serde(default)]
        create: bool,
        #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
        tag: Option<u8>,
    },
    AddRow {
        table: String,
        from: String,
        #[serde(default)]
        set: BTreeMap<String, String>,
    },
    RemoveRow {
        select: String,
    },
    ListAppend {
        select: String,
        field: String,
        value: String,
    },
    ListRemove {
        select: String,
        field: String,
        value: String,
    },
    PutRecord {
        key: String,
        #[serde(rename = "type")]
        tag: u8,
        value: String,
    },
    RemoveRecord {
        key: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ErpOp {
    ReplaceResource {
        erp: String,
        resource: String,
        #[serde(default)]
        fragment: usize,
        source: Source,
    },
    AddResource {
        erp: String,
        resource: String,
        #[serde(rename = "type")]
        kind: String,
        sources: Vec<Source>,
    },
    CopyResource {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from_erp: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<Source>,
        erp: String,
        resource: String,
        #[serde(default, rename = "as", skip_serializing_if = "Option::is_none")]
        rename: Option<String>,
    },
    PutResource {
        erp: String,
        resource: String,
        source: Source,
        #[serde(default, rename = "as", skip_serializing_if = "Option::is_none")]
        rename: Option<String>,
    },
    RemoveResource {
        erp: String,
        resource: String,
    },
    AddMipmaps {
        source: Source,
        target: String,
    },
    PatchPkg {
        erp: String,
        resource: String,
        #[serde(default)]
        fragment: usize,
        edits: Vec<PkgEdit>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PkgPatch {
    pub schema_version: u32,
    pub targets: Vec<PkgTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    pub edits: Vec<PkgEdit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PkgTarget {
    pub erp: String,
    pub resource: String,
    #[serde(default)]
    pub fragment: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Signal {
    pub to: String,
    pub pin: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attrs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EventReplacement {
    pub event: String,
    pub signals: Vec<Signal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
pub enum PkgEdit {
    CloneNode {
        src: String,
        #[serde(rename = "as")]
        name: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        set: BTreeMap<String, serde_json::Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        replace_event: Option<EventReplacement>,
    },
    SetText {
        node: String,
        value: String,
        #[serde(default)]
        localise: bool,
    },
    SetProps {
        node: String,
        set: BTreeMap<String, serde_json::Value>,
    },
    AddSignal {
        node: String,
        event: String,
        signal: Signal,
    },
    RemoveSignal {
        node: String,
        event: String,
        to: String,
    },
    CloneEntry {
        at: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<PkgTarget>,
        src: String,
        #[serde(rename = "as")]
        name: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        set: BTreeMap<String, serde_json::Value>,
    },
    SetValue {
        path: String,
        value: serde_json::Value,
    },
}

pub trait FieldNames {
    fn fields(&self, table: &str, row: &str) -> Option<Vec<String>>;
}

impl FieldNames for BTreeMap<String, Vec<String>> {
    fn fields(&self, table: &str, row: &str) -> Option<Vec<String>> {
        self.get(&format!("{table}.{row}")).cloned()
    }
}

struct Schema<'a> {
    file: &'a BTreeMap<String, Vec<String>>,
    names: &'a dyn FieldNames,
}

impl FieldNames for Schema<'_> {
    fn fields(&self, table: &str, row: &str) -> Option<Vec<String>> {
        self.file
            .fields(table, row)
            .or_else(|| self.names.fields(table, row))
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Base {
    Path(String),
    Hash(u64),
}

impl Base {
    fn key(&self, suffix: &str) -> u64 {
        match self {
            Base::Path(p) => fnv1a64(format!("{p}{suffix}")),
            Base::Hash(h) => fnv1a64_from(*h, suffix),
        }
    }

    fn child(&self, name: &str) -> Base {
        match self {
            Base::Path(p) => Base::Path(format!("{p}.{name}")),
            Base::Hash(h) => Base::Hash(fnv1a64_from(*h, format!(".{name}"))),
        }
    }
}

impl std::fmt::Display for Base {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Base::Path(p) => f.write_str(p),
            Base::Hash(h) => write!(f, "{{{h:016x}}}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Row {
    base: Base,
}

impl Row {
    fn field(&self, field: &str) -> u64 {
        self.base.key(&format!(".{field}"))
    }
}

struct Db<'a> {
    gvdb: &'a mut Gvdb,
    index: HashMap<u64, usize>,
    removed: Vec<usize>,
}

impl<'a> Db<'a> {
    fn new(gvdb: &'a mut Gvdb) -> Self {
        let mut index = HashMap::with_capacity(gvdb.records.len());
        for (i, r) in gvdb.records.iter().enumerate() {
            index.entry(r.key).or_insert(i);
        }
        Self {
            gvdb,
            index,
            removed: Vec::new(),
        }
    }

    fn remove(&mut self, key: u64) -> bool {
        match self.index.remove(&key) {
            Some(i) => {
                self.removed.push(i);
                true
            }
            None => false,
        }
    }

    fn finish(self) {
        if self.removed.is_empty() {
            return;
        }
        let removed: HashSet<usize> = self.removed.into_iter().collect();
        let mut i = 0;
        self.gvdb.records.retain(|_| {
            i += 1;
            !removed.contains(&(i - 1))
        });
    }

    fn get(&self, key: u64) -> Option<&Record> {
        self.index.get(&key).map(|&i| &self.gvdb.records[i])
    }

    fn get_mut(&mut self, key: u64) -> Option<&mut Record> {
        self.index.get(&key).map(|&i| &mut self.gvdb.records[i])
    }

    fn text(&self, key: u64) -> Option<String> {
        self.get(key).map(|r| r.text().into_owned())
    }

    fn push(&mut self, record: Record) {
        self.index.insert(record.key, self.gvdb.records.len());
        self.gvdb.records.push(record);
    }

    fn instance_count(&self, table: &str, row: &str) -> Option<usize> {
        self.text(fnv1a64(instance_count_path(table, row)))?
            .trim()
            .parse()
            .ok()
    }
}

fn parse_base(s: &str) -> Result<Base> {
    if let Some(hex) = s.strip_prefix('{') {
        let (hex, rest) = hex
            .split_once('}')
            .ok_or_else(|| Error::Resolve(format!("bad placeholder {s}")))?;
        let h = u64::from_str_radix(hex, 16).map_err(|_| Error::Resolve(format!("bad placeholder {s}")))?;
        return Ok(if rest.is_empty() {
            Base::Hash(h)
        } else {
            Base::Hash(fnv1a64_from(h, rest))
        });
    }
    Ok(Base::Path(s.to_owned()))
}

fn row_name(base: &Base, idx: usize, row: &str) -> Base {
    match base {
        Base::Path(p) if row.is_empty() => Base::Path(format!("{p}{idx}")),
        Base::Path(p) => Base::Path(format!("{p}.{row}_{idx}")),
        Base::Hash(h) if row.is_empty() => Base::Hash(fnv1a64_from(*h, idx.to_string())),
        Base::Hash(h) => Base::Hash(fnv1a64_from(*h, format!(".{row}_{idx}"))),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment<'a> {
    pub name: &'a str,
    pub select: Option<(&'a str, &'a str)>,
}

pub fn first_segment(path: &str) -> Result<(Segment<'_>, Option<&str>)> {
    let end = path.find(['.', '[']).unwrap_or(path.len());
    let (name, rest) = path.split_at(end);
    let Some(inner) = rest.strip_prefix('[') else {
        return Ok((Segment { name, select: None }, rest.strip_prefix('.')));
    };
    let close = inner
        .match_indices(']')
        .map(|(i, _)| i)
        .find(|&i| matches!(inner.as_bytes().get(i + 1), None | Some(b'.')))
        .ok_or_else(|| Error::Resolve(format!("{path}: [ needs a ] at the end of its segment")))?;
    let sel = &inner[..close];
    let (field, want) = sel
        .split_once('=')
        .ok_or_else(|| Error::Resolve(format!("{path}: bad selector [{sel}]")))?;
    let segment = Segment {
        name,
        select: Some((field.trim(), want)),
    };
    Ok((segment, inner[close + 1..].strip_prefix('.')))
}

fn select_row(db: &Db, table: &Base, row: &str, field: &str, want: &str) -> std::result::Result<Base, usize> {
    let count = match table {
        Base::Path(t) if !row.is_empty() => db.instance_count(t, row),
        _ => None,
    };
    let mut hits = Vec::new();
    for idx in 0..count.unwrap_or(4096) {
        let candidate = Row {
            base: row_name(table, idx, row),
        };
        match db.text(candidate.field("_metadata_")) {
            Some(_) if db.text(candidate.field(field)).as_deref() == Some(want) => hits.push(candidate.base),
            Some(_) => {}
            None if count.is_none() => break,
            None => {}
        }
    }
    match hits.len() {
        1 => Ok(hits.remove(0)),
        n => Err(n),
    }
}

fn resolve_path(db: &Db, path: &str) -> Result<Base> {
    if !path.contains('[') {
        return parse_base(path);
    }
    let mut base: Option<Base> = None;
    let mut nested = false;
    let mut rest = Some(path);
    while let Some(p) = rest {
        let (seg, tail) = first_segment(p)?;
        if seg.name.is_empty() {
            return Err(Error::Resolve(format!("{path}: empty segment")));
        }
        base = Some(match (base, seg.select) {
            (None, None) => parse_base(seg.name)?,
            (Some(b), None) => b.child(seg.name),
            (prefix, Some((field, want))) => {
                let (table, row) = match prefix {
                    Some(b) => (b, seg.name),
                    None => (parse_base(seg.name)?, ""),
                };
                match select_row(db, &table, row, field, want) {
                    Ok(b) => {
                        nested = true;
                        b
                    }
                    Err(n) => {
                        let upto = &path[..path.len() - tail.map_or(0, |t| t.len() + 1)];
                        let what = match n {
                            0 => "no row matches".to_owned(),
                            n => format!("{n} rows match"),
                        };
                        let scope = match (nested, row) {
                            (false, _) => String::new(),
                            (true, "") => format!(" in {table}"),
                            (true, row) => format!(" in {table}.{row}"),
                        };
                        return Err(Error::Resolve(format!("{upto}: {what}{scope}")));
                    }
                }
            }
        });
        rest = tail;
    }
    base.ok_or_else(|| Error::Resolve(format!("bad path {path}")))
}

fn resolve_row(db: &Db, select: &str) -> Result<Row> {
    Ok(Row {
        base: resolve_path(db, select)?,
    })
}

fn resolve_value(db: &Db, path: &str) -> Result<String> {
    if !path.contains(['.', ']']) {
        return Err(Error::Resolve(format!("bad path {path}")));
    }
    let key = resolve_path(db, path)?.key("");
    db.text(key)
        .ok_or_else(|| Error::Resolve(format!("{path}: no value")))
}

fn resolve_key(db: &Db, key: &str) -> Result<u64> {
    match key.starts_with("0x") || !key.contains('[') {
        true => record_key(key),
        false => Ok(resolve_path(db, key)?.key("")),
    }
}

pub fn apply_db_ops(db: &mut Gvdb, ops: &DbOps, names: &dyn FieldNames) -> Result<Vec<u64>> {
    let schema = Schema {
        file: &ops.fields,
        names,
    };
    let mut db = Db::new(db);
    let mut replaced = Vec::new();
    for op in &ops.ops {
        replaced.extend(apply(&mut db, op, &schema)?);
    }
    db.finish();
    Ok(replaced)
}

pub fn apply_db_op(db: &mut Gvdb, op: &DbOp, names: &dyn FieldNames) -> Result<Option<u64>> {
    let mut db = Db::new(db);
    let replaced = apply(&mut db, op, names)?;
    db.finish();
    Ok(replaced)
}

pub fn record_key(key: &str) -> Result<u64> {
    match key.strip_prefix("0x") {
        Some(hex) if hex.len() == 16 => {
            u64::from_str_radix(hex, 16).map_err(|_| Error::Resolve(format!("bad record key {key}")))
        }
        Some(_) => Err(Error::Resolve(format!("bad record key {key}"))),
        None => Ok(fnv1a64(key)),
    }
}

fn hex_bytes(key: &str, value: &str) -> Result<Vec<u8>> {
    let bad = || Error::Resolve(format!("{key}: value must be hex bytes"));
    if !value.is_ascii() || !value.len().is_multiple_of(2) {
        return Err(bad());
    }
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).map_err(|_| bad()))
        .collect()
}

pub fn add_row_name<'a>(table: &str, from: &'a str) -> Option<&'a str> {
    from.strip_prefix(table)
        .and_then(|r| r.strip_prefix('.'))
        .map(|r| r.split(['[', '.']).next().unwrap_or(r))
        .map(|r| {
            r.rsplit_once('_')
                .filter(|(_, n)| n.parse::<u32>().is_ok())
                .map_or(r, |(n, _)| n)
        })
}

fn apply(db: &mut Db, op: &DbOp, names: &dyn FieldNames) -> Result<Option<u64>> {
    match op {
        DbOp::Set {
            select,
            field,
            value,
            value_from,
            datasets,
            create,
            tag,
        } => {
            if !datasets.is_empty() {
                return Err(Error::Resolve("dataset-scoped set is not implemented yet".into()));
            }
            let value = match (value, value_from) {
                (Some(v), None) => v.clone(),
                (None, Some(from)) => resolve_value(db, from)?,
                _ => {
                    return Err(Error::Resolve(format!(
                        "{select}.{field}: set needs value or value_from"
                    )));
                }
            };
            let key = match field.contains('[') {
                true => resolve_path(db, &format!("{select}.{field}"))?.key(""),
                false => resolve_row(db, select)?.field(field),
            };
            match db.get_mut(key) {
                Some(r) => r.value = value.into_bytes(),
                None if *create => {
                    let tag =
                        tag.ok_or_else(|| Error::Resolve(format!("{select}.{field}: create needs type")))?;
                    db.push(Record {
                        key,
                        tag,
                        value: value.into_bytes(),
                    });
                }
                None => return Err(Error::Resolve(format!("{select}.{field}: no such key"))),
            }
            Ok(Some(key))
        }
        DbOp::AddRow { table, from, set } => {
            let src = resolve_row(db, from)?;
            let Base::Path(path) = resolve_path(db, table)? else {
                return Err(Error::Resolve(format!(
                    "{table}: add-row needs the table's real path, not a {{hash}}"
                )));
            };
            let row = match &src.base {
                Base::Path(f) => add_row_name(&path, f),
                Base::Hash(_) => None,
            }
            .ok_or_else(|| Error::Resolve(format!("{from} is not a row of {table}")))?;
            let count_path = instance_count_path(&path, row);
            let n = db.instance_count(&path, row).ok_or_else(|| {
                Error::Resolve(format!(
                    "{count_path}: missing; add-row needs the table's real path"
                ))
            })?;
            let fields = names
                .fields(table, row)
                .or_else(|| names.fields(&path, row))
                .ok_or_else(|| Error::Resolve(format!("{table}.{row}: field names unknown")))?;
            if let Some(f) = set.keys().find(|f| !fields.contains(f)) {
                return Err(Error::Resolve(format!("{table}.{row}: unknown field {f}")));
            }
            let table = &path;
            let dst = Row {
                base: Base::Path(format!("{table}.{row}_{n}")),
            };
            let mut rows = Vec::with_capacity(fields.len());
            for field in &fields {
                if db.get(dst.field(field)).is_some() {
                    return Err(Error::Resolve(format!(
                        "{table}.{row}_{n}.{field} already exists"
                    )));
                }
                let s = db
                    .get(src.field(field))
                    .ok_or_else(|| Error::Resolve(format!("{from}: has no {field}")))?;
                let value = set
                    .get(field)
                    .map(|v| v.clone().into_bytes())
                    .unwrap_or_else(|| s.value.clone());
                rows.push(Record {
                    key: dst.field(field),
                    tag: s.tag,
                    value,
                });
            }
            for r in rows {
                db.push(r);
            }
            let count = db
                .get_mut(fnv1a64(&count_path))
                .expect("instance_count read it above");
            count.value = (n + 1).to_string().into_bytes();
            Ok(None)
        }
        DbOp::RemoveRow { .. } | DbOp::ListAppend { .. } | DbOp::ListRemove { .. } => Err(Error::Resolve(
            "remove-row and list ops are not implemented yet".into(),
        )),
        DbOp::PutRecord { key, tag, value } => {
            let value = hex_bytes(key, value)?;
            let k = resolve_key(db, key)?;
            match db.get_mut(k) {
                Some(r) => {
                    r.tag = *tag;
                    r.value = value;
                }
                None => db.push(Record {
                    key: k,
                    tag: *tag,
                    value,
                }),
            }
            Ok(Some(k))
        }
        DbOp::RemoveRecord { key } => {
            let k = resolve_key(db, key)?;
            if !db.remove(k) {
                return Err(Error::Resolve(format!("{key}: no such key")));
            }
            Ok(Some(k))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::{FileKind, OpFile};

    fn db() -> Gvdb {
        let mut records = vec![Record::new("entrants.teams._team_instancecount_", 0, "2")];
        for (i, label) in ["mclaren_26", "gm_cadillac_26"].iter().enumerate() {
            records.push(Record::new(
                &format!("entrants.teams.team_{i}._metadata_"),
                4,
                *label,
            ));
            records.push(Record::new(
                &format!("entrants.teams.team_{i}.stat_id"),
                0,
                (100 + i).to_string(),
            ));
            records.push(Record::new(
                &format!("entrants.teams.team_{i}.display_name_code"),
                2,
                "XXX",
            ));
        }
        let world = fnv1a64("hidden.world_table_");
        for (i, (label, eri)) in [("f1_26", "_f1_26.wep"), ("myteam_26", "_f1_26_myteam.wep")]
            .iter()
            .enumerate()
        {
            for (f, v) in [("_metadata_", *label), ("worldEri", *eri)] {
                let key = fnv1a64_from(world, format!("{i}.{f}"));
                records.push(Record {
                    key,
                    tag: 2,
                    value: v.as_bytes().to_vec(),
                });
            }
        }
        Gvdb { records }
    }

    fn names() -> BTreeMap<String, Vec<String>> {
        let fields = ["_metadata_", "stat_id", "display_name_code"]
            .map(String::from)
            .to_vec();
        BTreeMap::from([("entrants.teams.team".to_owned(), fields)])
    }

    fn ops(json: &str) -> DbOps {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn add_row_appends_and_bumps_count() {
        let mut db = db();
        let ops = ops(
            r#"{"schema_version":1,"ops":[{"op":"add-row","table":"entrants.teams",
            "from":"entrants.teams.team[_metadata_=gm_cadillac_26]",
            "set":{"_metadata_":"team12_26","display_name_code":"T12"}}]}"#,
        );
        apply_db_ops(&mut db, &ops, &names()).unwrap();
        assert_eq!(db.instance_count("entrants.teams", "team"), Some(3));
        assert_eq!(
            db.get_text("entrants.teams.team_2._metadata_").as_deref(),
            Some("team12_26")
        );
        assert_eq!(
            db.get_text("entrants.teams.team_2.stat_id").as_deref(),
            Some("101")
        );
        assert_eq!(
            db.get_text("entrants.teams.team_2.display_name_code").as_deref(),
            Some("T12")
        );
        let bytes = db.to_bytes();
        assert_eq!(Gvdb::parse(&bytes).unwrap().to_bytes(), bytes);
    }

    #[test]
    fn add_row_reads_field_names_from_the_file() {
        let mut db = db();
        let ops = ops(
            r#"{"schema_version":1,"fields":{"entrants.teams.team":["_metadata_","stat_id"]},"ops":[
            {"op":"add-row","table":"entrants.teams","from":"entrants.teams.team[_metadata_=mclaren_26]",
            "set":{"_metadata_":"team12_26"}}]}"#,
        );
        apply_db_ops(&mut db, &ops, &BTreeMap::new()).unwrap();
        assert_eq!(db.instance_count("entrants.teams", "team"), Some(3));
        assert_eq!(
            db.get_text("entrants.teams.team_2.stat_id").as_deref(),
            Some("100")
        );
        assert!(db.get("entrants.teams.team_2.display_name_code").is_none());
    }

    #[test]
    fn add_row_needs_every_field_on_the_source_and_a_free_row() {
        let mut db = db();
        let missing = ops(
            r#"{"schema_version":1,"fields":{"entrants.teams.team":["_metadata_","nope"]},"ops":[
            {"op":"add-row","table":"entrants.teams","from":"entrants.teams.team_0"}]}"#,
        );
        let err = apply_db_ops(&mut db, &missing, &BTreeMap::new()).unwrap_err();
        assert!(err.to_string().contains("has no nope"), "{err}");
        assert_eq!(db.instance_count("entrants.teams", "team"), Some(2));
        db.records
            .push(Record::new("entrants.teams.team_2._metadata_", 4, "stale"));
        let taken = ops(
            r#"{"schema_version":1,"ops":[{"op":"add-row","table":"entrants.teams","from":"entrants.teams.team_0"}]}"#,
        );
        let err = apply_db_ops(&mut db, &taken, &names()).unwrap_err();
        assert!(err.to_string().contains("already exists"), "{err}");
    }

    #[test]
    fn erp_ops_parse_patch_pkg_and_warn() {
        let file = |json: &str| OpFile::from_value(serde_json::from_str(json).unwrap(), FileKind::Erp);
        let ops = file(
            r#"{"schema_version":1,"ops":[
            {"op":"patch-pkg","erp":"a.erp","resource":"eaid://x.layer","edits":[
              {"op":"clone-entry","at":"instances","from":{"erp":"b.erp","resource":"eaid://y.layer"},
               "src":"[name=y$A]","as":"x$B","set":{"overrides.VehPlacement.Id":25}}]},
            {"op":"warn","message":"Imola has room for 26 cars"}]}"#,
        )
        .unwrap();
        assert_eq!(ops.ops.len(), 2);
        assert!(file(r#"{"schema_version":1,"ops":[{"op":"warn"}]}"#).is_err());
    }

    #[test]
    fn set_with_selector_and_value_from_hash_placeholder() {
        let mut db = db();
        let h = format!("{{{:016x}}}", fnv1a64("hidden.world_table_"));
        let json = format!(
            r#"{{"schema_version":1,"ops":[{{"op":"set","select":"{h}[_metadata_=f1_26]","field":"worldEri",
            "value_from":"{h}[_metadata_=myteam_26].worldEri"}}]}}"#
        );
        apply_db_ops(&mut db, &ops(&json), &names()).unwrap();
        let key = fnv1a64_from(fnv1a64("hidden.world_table_"), "0.worldEri");
        assert_eq!(db.get_key(key).unwrap().text(), "_f1_26_myteam.wep");
    }

    #[test]
    fn put_and_remove_records_by_path_and_hash() {
        let mut db = db();
        let count = db.records.len();
        let code = fnv1a64("entrants.teams.team_0.display_name_code");
        let ops = ops(&format!(
            r#"{{"schema_version":1,"ops":[
            {{"op":"put-record","key":"{code:#018x}","type":4,"value":"4d434c"}},
            {{"op":"put-record","key":"entrants.teams.team_0.new_field","type":0,"value":"3132"}},
            {{"op":"remove-record","key":"entrants.teams.team_1.stat_id"}}]}}"#
        ));
        let replaced = apply_db_ops(&mut db, &ops, &names()).unwrap();
        assert_eq!(replaced[0], code);
        assert_eq!(replaced.len(), 3);
        let r = db.get("entrants.teams.team_0.display_name_code").unwrap();
        assert_eq!((r.tag, r.text().as_ref()), (4, "MCL"));
        assert_eq!(
            db.get_text("entrants.teams.team_0.new_field").as_deref(),
            Some("12")
        );
        assert!(db.get("entrants.teams.team_1.stat_id").is_none());
        assert_eq!(db.records.len(), count);
        for bad in [
            r#"{"op":"remove-record","key":"entrants.teams.team_1.nope"}"#,
            r#"{"op":"put-record","key":"0x12","type":0,"value":"00"}"#,
            r#"{"op":"put-record","key":"a.b","type":0,"value":"0"}"#,
        ] {
            let op: DbOp = serde_json::from_str(bad).unwrap();
            assert!(apply_db_op(&mut db, &op, &names()).is_err(), "{bad}");
        }
    }

    const SETS: &str = "track_data.game_setup";

    fn track_db() -> Gvdb {
        let mut records = vec![Record::new(&instance_count_path(SETS, "TrackDataSet"), 0, "2")];
        for (i, (label, tracks)) in [
            ("f1", &["melbourne", "okayama", "okayama"][..]),
            ("f1_2026", &["melbourne", "suzuka"][..]),
        ]
        .into_iter()
        .enumerate()
        {
            let set = format!("{SETS}.TrackDataSet_{i}");
            records.push(Record::new(&format!("{set}._metadata_"), 4, label));
            records.push(Record::new(
                &instance_count_path(&set, "TrackData"),
                0,
                tracks.len().to_string(),
            ));
            for (j, track) in tracks.iter().enumerate() {
                let row = format!("{set}.TrackData_{j}");
                records.push(Record::new(&format!("{row}._metadata_"), 4, *track));
                records.push(Record::new(&format!("{row}.track"), 2, *track));
                records.push(Record::new(
                    &format!("{row}._TrackNameData_instancecount_"),
                    0,
                    "1",
                ));
                records.push(Record::new(
                    &format!("{row}.TrackNameData_0._metadata_"),
                    4,
                    "names",
                ));
                records.push(Record::new(
                    &format!("{row}.TrackNameData_0.title"),
                    4,
                    format!("lng_{label}_{track}"),
                ));
            }
        }
        Gvdb { records }
    }

    #[test]
    fn parses_segments_like_pkg_paths() {
        let (seg, rest) = first_segment("TrackData[name=a.b$c].TrackNameData_0.title").unwrap();
        assert_eq!(
            (seg.name, seg.select, rest),
            (
                "TrackData",
                Some(("name", "a.b$c")),
                Some("TrackNameData_0.title")
            )
        );
        let (seg, rest) = first_segment("t[ k =x[1]").unwrap();
        assert_eq!((seg.select, rest), (Some(("k", "x[1")), None));
        let (seg, _) = first_segment("t[k=x]y]").unwrap();
        assert_eq!(seg.select, Some(("k", "x]y")));
        let (seg, rest) = first_segment("{446668bc25ba571c}[k=v].a").unwrap();
        assert_eq!((seg.name, rest), ("{446668bc25ba571c}", Some("a")));
        for bad in ["t[k=v", "t[kv].a", "t[k=v]a"] {
            assert!(first_segment(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn selects_rows_on_every_segment() {
        let mut db = track_db();
        let ops = ops(r#"{"schema_version":1,"ops":[
            {"op":"set","select":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=suzuka]",
             "field":"TrackNameData_0.title","value":"lng_a"},
            {"op":"set","select":"track_data.game_setup.TrackDataSet_1.TrackData[_metadata_=melbourne]",
             "field":"TrackNameData_0.title","value":"lng_b"},
            {"op":"set","select":"track_data.game_setup.TrackDataSet[_metadata_=f1].TrackData_0",
             "field":"TrackNameData[_metadata_=names].title","value":"lng_c"}]}"#);
        let replaced = apply_db_ops(&mut db, &ops, &names()).unwrap();
        let key = |p: &str| fnv1a64(format!("{SETS}.{p}.TrackNameData_0.title"));
        assert_eq!(
            replaced,
            [
                key("TrackDataSet_1.TrackData_1"),
                key("TrackDataSet_1.TrackData_0"),
                key("TrackDataSet_0.TrackData_0")
            ]
        );
        for (row, want) in [
            ("TrackDataSet_1.TrackData_1", "lng_a"),
            ("TrackDataSet_1.TrackData_0", "lng_b"),
            ("TrackDataSet_0.TrackData_0", "lng_c"),
        ] {
            let got = db.get_text(&format!("{SETS}.{row}.TrackNameData_0.title"));
            assert_eq!(got.as_deref(), Some(want), "{row}");
        }
    }

    #[test]
    fn nested_selector_errors_name_the_segment() {
        let set = |select: &str| {
            let op = DbOp::Set {
                select: select.to_owned(),
                field: "track".into(),
                value: Some("x".into()),
                value_from: None,
                datasets: Vec::new(),
                create: false,
                tag: None,
            };
            apply_db_op(&mut track_db(), &op, &names())
                .unwrap_err()
                .to_string()
        };
        assert_eq!(
            set("track_data.game_setup.TrackDataSet[_metadata_=f9].TrackData[_metadata_=suzuka]"),
            "track_data.game_setup.TrackDataSet[_metadata_=f9]: no row matches"
        );
        assert_eq!(
            set("track_data.game_setup.TrackDataSet[_metadata_=f1].TrackData[_metadata_=okayama]"),
            "track_data.game_setup.TrackDataSet[_metadata_=f1].TrackData[_metadata_=okayama]: \
             2 rows match in track_data.game_setup.TrackDataSet_0.TrackData"
        );
        assert_eq!(
            set("track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=okayama]"),
            "track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=okayama]: \
             no row matches in track_data.game_setup.TrackDataSet_1.TrackData"
        );
        assert!(
            set("track_data.game_setup.TrackDataSet[_metadata_=f1].TrackData[track]")
                .contains("bad selector")
        );
        assert!(set("track_data.game_setup..TrackDataSet[_metadata_=f1]").contains("empty segment"));
    }

    #[test]
    fn add_row_into_a_nested_table() {
        let mut db = track_db();
        let first = ops(r#"{"schema_version":1,
            "fields":{"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData":
              ["_metadata_","track","_TrackNameData_instancecount_","TrackNameData_0._metadata_","TrackNameData_0.title"]},
            "ops":[
            {"op":"add-row","table":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026]",
             "from":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=melbourne]",
             "set":{"_metadata_":"okayama","track":"okayama"}},
            {"op":"set","select":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=okayama]",
             "field":"TrackNameData_0.title","value":"lng_okayama"}]}"#);
        apply_db_ops(&mut db, &first, &BTreeMap::new()).unwrap();
        let set = format!("{SETS}.TrackDataSet_1");
        assert_eq!(db.instance_count(&set, "TrackData"), Some(3));
        assert_eq!(
            db.instance_count(&format!("{SETS}.TrackDataSet_0"), "TrackData"),
            Some(3)
        );
        for (field, want) in [
            ("_metadata_", "okayama"),
            ("track", "okayama"),
            ("TrackNameData_0.title", "lng_okayama"),
        ] {
            let got = db.get_text(&format!("{set}.TrackData_2.{field}"));
            assert_eq!(got.as_deref(), Some(want), "{field}");
        }
        let resolved_fields = first.fields.values().next().unwrap().clone();
        let again: DbOps = serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "fields": { format!("{set}.TrackData"): resolved_fields },
            "ops": [{ "op": "add-row", "table": set,
                "from": "track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=okayama]",
                "set": { "_metadata_": "sepang", "track": "sepang" } }]
        }))
        .unwrap();
        apply_db_ops(&mut db, &again, &BTreeMap::new()).unwrap();
        assert_eq!(db.instance_count(&set, "TrackData"), Some(4));
        assert_eq!(
            db.get_text(&format!("{set}.TrackData_3.TrackNameData_0.title"))
                .as_deref(),
            Some("lng_okayama")
        );
        let wrong_table = ops(
            r#"{"schema_version":1,"ops":[{"op":"add-row","table":"track_data.game_setup.TrackDataSet[_metadata_=f1]",
            "from":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=melbourne]"}]}"#,
        );
        let err = apply_db_ops(&mut db, &wrong_table, &BTreeMap::new()).unwrap_err();
        assert!(err.to_string().contains("is not a row of"), "{err}");
    }

    #[test]
    fn value_from_and_record_keys_take_nested_selectors() {
        let mut db = track_db();
        let ops = ops(r#"{"schema_version":1,"ops":[
            {"op":"set","select":"track_data.game_setup.TrackDataSet[_metadata_=f1].TrackData[_metadata_=melbourne]",
             "field":"TrackNameData_0.title",
             "value_from":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=suzuka].TrackNameData_0.title"},
            {"op":"put-record","key":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=suzuka].laps","type":0,"value":"3533"},
            {"op":"remove-record","key":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=melbourne].track"}]}"#);
        apply_db_ops(&mut db, &ops, &names()).unwrap();
        let get = |p: &str| db.get_text(&format!("{SETS}.{p}"));
        assert_eq!(
            get("TrackDataSet_0.TrackData_0.TrackNameData_0.title").as_deref(),
            Some("lng_f1_2026_suzuka")
        );
        assert_eq!(get("TrackDataSet_1.TrackData_1.laps").as_deref(), Some("53"));
        assert_eq!(get("TrackDataSet_1.TrackData_0.track"), None);
        assert_eq!(
            get("TrackDataSet_0.TrackData_0.track").as_deref(),
            Some("melbourne")
        );
    }

    #[test]
    fn hash_placeholder_takes_nested_selectors() {
        let mut db = db();
        let world = fnv1a64("hidden.world_table_");
        for (i, (label, n)) in [("a", "6"), ("b", "7")].iter().enumerate() {
            for (f, v) in [("_metadata_", *label), ("n", *n)] {
                db.records.push(Record {
                    key: fnv1a64_from(world, format!("1.slot_{i}.{f}")),
                    tag: 0,
                    value: v.as_bytes().to_vec(),
                });
            }
        }
        let h = format!("{{{world:016x}}}");
        let json = format!(
            r#"{{"schema_version":1,"ops":[{{"op":"set","select":"{h}[_metadata_=f1_26]","field":"worldEri",
            "value_from":"{h}[_metadata_=myteam_26].slot[_metadata_=b].n"}}]}}"#
        );
        apply_db_ops(&mut db, &ops(&json), &names()).unwrap();
        assert_eq!(db.get_key(fnv1a64_from(world, "0.worldEri")).unwrap().text(), "7");
        let json = format!(
            r#"{{"schema_version":1,"ops":[{{"op":"set","select":"{h}[_metadata_=myteam_26].slot[_metadata_=c]","field":"n","value":"1"}}]}}"#
        );
        let err = apply_db_ops(&mut db, &ops(&json), &names())
            .unwrap_err()
            .to_string();
        let row = format!("{{{:016x}}}", fnv1a64_from(world, "1"));
        assert!(
            err.ends_with(&format!("[_metadata_=c]: no row matches in {row}.slot")),
            "{err}"
        );
    }

    #[test]
    fn set_missing_key_fails_without_create() {
        let mut db = db();
        let ops = ops(
            r#"{"schema_version":1,"ops":[{"op":"set","select":"entrants.teams.team_0","field":"nope","value":"1"}]}"#,
        );
        assert!(apply_db_ops(&mut db, &ops, &names()).is_err());
    }

    #[test]
    fn pkg_patch_parses_design_example() {
        let p: PkgPatch = serde_json::from_str(
            r#"{"schema_version":1,"targets":[{"erp":"2025_asset_groups/ui_package/flow_frontend.erp",
            "resource":"eaid://ui_package/screens/fe_home.ui"}],"root":"Screens[0]:fe_home","edits":[
            {"op":"clone-node","src":"settings","as":"mod_menu_listener","set":{"action_name":"Menu Right Stick Click"},
             "replace_event":{"event":"pressed","signals":[{"to":"fe_home.Action","pin":"route","attrs":{"key":"mod_menu"}}]}},
            {"op":"set-text","node":"contextual_panels.offline_contextual_panel.subtitle","value":"x","localise":false}]}"#,
        )
        .unwrap();
        assert_eq!(p.edits.len(), 2);
    }
}
