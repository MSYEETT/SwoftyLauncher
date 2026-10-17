use std::collections::{BTreeMap, HashMap};

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
}

impl<'a> Db<'a> {
    fn new(gvdb: &'a mut Gvdb) -> Self {
        let mut index = HashMap::with_capacity(gvdb.records.len());
        for (i, r) in gvdb.records.iter().enumerate() {
            index.entry(r.key).or_insert(i);
        }
        Self { gvdb, index }
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

fn split_table(path: &str) -> (&str, &str) {
    match path.rsplit_once('.') {
        Some((t, r)) if !t.starts_with('{') || t.contains('}') => (t, r),
        _ => (path, ""),
    }
}

fn resolve_row(db: &Db, select: &str) -> Result<Row> {
    let Some((path, filter)) = select.strip_suffix(']').and_then(|s| s.split_once('[')) else {
        return Ok(Row {
            base: parse_base(select)?,
        });
    };
    let (field, want) = filter
        .split_once('=')
        .ok_or_else(|| Error::Resolve(format!("bad selector {select}")))?;
    let (base, row) = if path.starts_with('{') && !path.contains('.') {
        (parse_base(path)?, String::new())
    } else {
        let (table, row) = split_table(path);
        (parse_base(table)?, row.to_owned())
    };
    let count = match &base {
        Base::Path(t) if !row.is_empty() => db.instance_count(t, &row),
        _ => None,
    };
    let mut hits = Vec::new();
    for idx in 0..count.unwrap_or(4096) {
        let candidate = Row {
            base: row_name(&base, idx, &row),
        };
        match db.text(candidate.field("_metadata_")) {
            Some(_) if db.text(candidate.field(field)).as_deref() == Some(want) => hits.push(candidate),
            Some(_) => {}
            None if count.is_none() => break,
            None => {}
        }
    }
    match hits.len() {
        1 => Ok(hits.remove(0)),
        0 => Err(Error::Resolve(format!("{select}: no row matches"))),
        n => Err(Error::Resolve(format!("{select}: {n} rows match"))),
    }
}

fn resolve_value(db: &Db, path: &str) -> Result<String> {
    let (select, field) = match path.rfind(']') {
        Some(i) => (&path[..=i], path[i + 1..].trim_start_matches('.')),
        None => path
            .rsplit_once('.')
            .ok_or_else(|| Error::Resolve(format!("bad path {path}")))?,
    };
    let row = resolve_row(db, select)?;
    db.text(row.field(field))
        .ok_or_else(|| Error::Resolve(format!("{path}: no value")))
}

pub fn apply_db_ops(db: &mut Gvdb, ops: &DbOps, names: &dyn FieldNames) -> Result<()> {
    let schema = Schema {
        file: &ops.fields,
        names,
    };
    let mut db = Db::new(db);
    for op in &ops.ops {
        apply(&mut db, op, &schema)?;
    }
    Ok(())
}

pub fn apply_db_op(db: &mut Gvdb, op: &DbOp, names: &dyn FieldNames) -> Result<()> {
    apply(&mut Db::new(db), op, names)
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

fn apply(db: &mut Db, op: &DbOp, names: &dyn FieldNames) -> Result<()> {
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
            let key = resolve_row(db, select)?.field(field);
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
            Ok(())
        }
        DbOp::AddRow { table, from, set } => {
            let src = resolve_row(db, from)?;
            let row = add_row_name(table, from)
                .ok_or_else(|| Error::Resolve(format!("{from} is not a row of {table}")))?;
            let count_path = instance_count_path(table, row);
            let n = db.instance_count(table, row).ok_or_else(|| {
                Error::Resolve(format!(
                    "{count_path}: missing; add-row needs the table's real path"
                ))
            })?;
            let fields = names
                .fields(table, row)
                .ok_or_else(|| Error::Resolve(format!("{table}.{row}: field names unknown")))?;
            if let Some(f) = set.keys().find(|f| !fields.contains(f)) {
                return Err(Error::Resolve(format!("{table}.{row}: unknown field {f}")));
            }
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
            Ok(())
        }
        DbOp::RemoveRow { .. } | DbOp::ListAppend { .. } | DbOp::ListRemove { .. } => Err(Error::Resolve(
            "remove-row and list ops are not implemented yet".into(),
        )),
        DbOp::PutRecord { .. } | DbOp::RemoveRecord { .. } => Err(Error::Resolve(
            "put-record and remove-record are not implemented yet".into(),
        )),
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
