//! What a row's copy under a new name does not bring along: rows of other
//! tables keyed by the row's label or a name derived from it, and values
//! elsewhere that pick the row by its label or `stat_id`.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use swofty_formats::gvdb::Gvdb;

use super::{Catalog, NodeId, NodeKind};
use crate::ops::row_shape;

const WARNINGS_PER_ADDON: usize = 8;
const FIELDS_SHOWN: usize = 6;
const NAMES_SHOWN: usize = 6;

/// An `add-row` an addon applied: the source row and the new row, both plain paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Copy {
    pub addon: String,
    pub from: String,
    pub row: String,
}

/// A name the game can derive from a row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Derived {
    pub value: String,
    /// `label`, `label+_<word>`, a field path, or `<field> folder <n>` for a part of a path value.
    pub from: String,
}

impl Derived {
    /// How the name comes from the row: `the label`, `folder 1 of resource_path`.
    pub fn how(&self) -> String {
        match self.from.as_str() {
            "label" => "the label".into(),
            f => match (f.strip_prefix("label+"), f.split_once(" folder ")) {
                (Some(suffix), _) => format!("the label + {suffix}"),
                (None, Some((field, n))) => format!("folder {n} of {field}"),
                (None, None) => format!("the {f} value"),
            },
        }
    }
}

/// Rows of one table family labelled with a derived name of the row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyed {
    pub name: Derived,
    pub family: String,
    /// The tables of the family holding such a row.
    pub tables: Vec<NodeId>,
}

/// Values of one field pattern that pick the row by its label or `stat_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub by: By,
    pub field: String,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum By {
    Label,
    StatId,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Deps {
    pub names: Vec<Derived>,
    pub keyed: Vec<Keyed>,
    pub references: Vec<Reference>,
    /// Values naming the row's label under keys the catalogue does not name.
    pub unnamed: usize,
}

fn identifier(v: &str) -> bool {
    v.len() >= 3 && label_like(v)
}

fn label_like(v: &str) -> bool {
    let b = v.as_bytes();
    b.len() >= 2
        && (b[0].is_ascii_alphabetic() || b[0] == b'_')
        && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
        && b.iter().any(u8::is_ascii_lowercase)
        && !matches!(v, "True" | "False" | "true" | "false")
}

fn parent(cat: &Catalog, id: NodeId) -> Option<NodeId> {
    cat.node(id).parent
}

fn under(cat: &Catalog, mut id: NodeId, row: NodeId) -> bool {
    loop {
        if id == row {
            return true;
        }
        match parent(cat, id) {
            Some(p) => id = p,
            None => return false,
        }
    }
}

fn unnamed(cat: &Catalog, id: NodeId) -> bool {
    parent(cat, id).is_some_and(|p| cat.node(p).kind == NodeKind::Unnamed)
}

fn table_word(cat: &Catalog, row: NodeId) -> Option<&str> {
    match &cat.node(parent(cat, row)?).kind {
        NodeKind::Table { word, .. } => Some(word.as_str()),
        _ => None,
    }
}

fn family(cat: &Catalog, table: NodeId) -> String {
    cat.family(table)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{}_N", row_shape(&cat.path(table))))
}

fn rel(cat: &Catalog, row: NodeId, id: NodeId) -> String {
    let mut parts = Vec::new();
    let mut at = id;
    while at != row {
        parts.push(cat.node(at).name.as_str());
        match parent(cat, at) {
            Some(p) => at = p,
            None => break,
        }
    }
    parts.reverse();
    parts.join(".")
}

/// The fields of `row` outside its nested tables, as (relative path, value).
fn fields(cat: &Catalog, row: NodeId) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![row];
    while let Some(id) = stack.pop() {
        for &c in cat.children(id) {
            let n = cat.node(c);
            match n.kind {
                NodeKind::Key => {
                    if let Some(r) = n.record {
                        out.push((rel(cat, row, c), cat.record(r).value.to_owned()));
                    }
                }
                NodeKind::Container => stack.push(c),
                _ => {}
            }
        }
    }
    out.sort();
    out
}

/// The names the game can derive from `row`: its label, `<label>_<row word>`,
/// identifier values of its fields and the folders of its path values that
/// hold the label or a digit.
pub fn derived(cat: &Catalog, row: NodeId) -> Vec<Derived> {
    derived_with(cat, row, false)
}

fn derived_with(cat: &Catalog, row: NodeId, every_folder: bool) -> Vec<Derived> {
    let mut out = Vec::new();
    let label = cat.node(row).label.clone().unwrap_or_default();
    if label_like(&label) {
        out.push(Derived {
            value: label.clone(),
            from: "label".into(),
        });
        if let Some(word) = table_word(cat, row) {
            out.push(Derived {
                value: format!("{label}_{word}"),
                from: format!("label+_{word}"),
            });
        }
    }
    for (field, value) in fields(cat, row) {
        if field == "_metadata_" || field.ends_with("stat_id") {
            continue;
        }
        if identifier(&value) {
            out.push(Derived { value, from: field });
            continue;
        }
        let path = value.strip_prefix("eaid://").unwrap_or(&value);
        if !path.contains('/') {
            continue;
        }
        for (i, part) in path.split('/').enumerate() {
            let names_row = every_folder
                || (!label.is_empty() && part.contains(label.as_str()))
                || part.bytes().any(|b| b.is_ascii_digit());
            if identifier(part) && names_row {
                out.push(Derived {
                    value: part.to_owned(),
                    from: format!("{field} folder {}", i + 1),
                });
            }
        }
    }
    out
}

fn stat_id(cat: &Catalog, row: NodeId) -> Option<String> {
    let id = cat.find_in(row, "stat_id")?;
    let v = cat.record(cat.node(id).record?).value.trim();
    v.parse::<i64>().is_ok().then(|| v.to_owned())
}

/// Whether a field named `leaf` can hold a `stat_id` of a row of the table `word`.
fn id_field(leaf: &str, word: &str) -> bool {
    let leaf = leaf.to_ascii_lowercase();
    let word = word.to_ascii_lowercase();
    let stem = word.rsplit('_').next().unwrap_or(&word);
    leaf.ends_with("id") && leaf != "stat_id" && (leaf.contains(&word) || leaf.contains(stem))
}

/// Rows labelled and records holding each needle, found in one pass.
struct Index {
    rows: HashMap<String, Vec<NodeId>>,
    values: HashMap<String, Vec<u32>>,
}

impl Index {
    fn new(cat: &Catalog, labels: &HashSet<String>, values: &HashSet<String>) -> Self {
        let mut rows: HashMap<String, Vec<NodeId>> = HashMap::new();
        for id in 0..cat.len() as NodeId {
            let n = cat.node(id);
            if let (NodeKind::Row { .. }, Some(l)) = (&n.kind, &n.label)
                && labels.contains(l)
            {
                rows.entry(l.clone()).or_default().push(id);
            }
        }
        let mut found: HashMap<String, Vec<u32>> = HashMap::new();
        for i in 0..cat.records() as u32 {
            let v = cat.record(i).value;
            if values.contains(v) {
                found.entry(v.to_owned()).or_default().push(i);
            } else if v.contains(',') {
                for item in v.split(',').map(str::trim) {
                    if values.contains(item) {
                        found.entry(item.to_owned()).or_default().push(i);
                    }
                }
            }
        }
        Self { rows, values: found }
    }

    fn rows(&self, label: &str) -> &[NodeId] {
        self.rows.get(label).map_or(&[], Vec::as_slice)
    }

    fn values(&self, v: &str) -> &[u32] {
        self.values.get(v).map_or(&[], Vec::as_slice)
    }
}

struct Skip {
    rows: Vec<NodeId>,
    family: Option<String>,
}

impl Skip {
    fn row(&self, cat: &Catalog, id: NodeId) -> bool {
        self.rows.iter().any(|&r| under(cat, id, r))
    }

    fn table(&self, cat: &Catalog, table: NodeId) -> bool {
        self.family.as_deref() == Some(family(cat, table).as_str())
    }
}

fn keyed(cat: &Catalog, index: &Index, names: &[Derived], skip: &Skip) -> Vec<Keyed> {
    let mut by: BTreeMap<(String, Derived), BTreeSet<NodeId>> = BTreeMap::new();
    for name in names {
        for &r in index.rows(&name.value) {
            let Some(t) = parent(cat, r) else { continue };
            if skip.row(cat, r) || skip.table(cat, t) {
                continue;
            }
            by.entry((family(cat, t), name.clone())).or_default().insert(t);
        }
    }
    by.into_iter()
        .map(|((family, name), tables)| Keyed {
            name,
            family,
            tables: tables.into_iter().collect(),
        })
        .collect()
}

/// The field pattern of a record, or `None` for `_metadata_`, unnamed and skipped keys.
fn field_of(cat: &Catalog, record: u32, skip: &Skip) -> Option<String> {
    let id = cat.record_node(record);
    let n = cat.node(id);
    if n.name == "_metadata_" || unnamed(cat, id) || skip.row(cat, id) {
        return None;
    }
    Some(row_shape(&cat.path(id)))
}

/// The row a value can pick: its label and `stat_id`, and whether rows of
/// other tables share the label.
struct Target<'a> {
    label: &'a str,
    stat_id: Option<&'a str>,
    word: &'a str,
    container: String,
    shared: bool,
}

impl<'a> Target<'a> {
    fn new(cat: &'a Catalog, index: &Index, row: NodeId, label: &'a str, stat_id: Option<&'a str>) -> Self {
        let table = parent(cat, row);
        let own = table.map(|t| family(cat, t));
        let shared = index
            .rows(label)
            .iter()
            .any(|&r| parent(cat, r).map(|t| family(cat, t)) != own);
        Self {
            label,
            stat_id,
            word: table_word(cat, row).unwrap_or_default(),
            container: table
                .and_then(|t| parent(cat, t))
                .map(|c| cat.path(c))
                .unwrap_or_default(),
            shared,
        }
    }

    /// Whether the field of `record` names rows of this table: by its name, or
    /// by its catalogue meaning.
    fn named_by(&self, cat: &Catalog, record: u32, field: &str) -> bool {
        let leaf = field.rsplit('.').next().unwrap_or(field);
        (!self.word.is_empty() && leaf.contains(self.word))
            || (!self.container.is_empty()
                && cat
                    .meta(cat.record(record).key)
                    .is_some_and(|m| m.meaning.contains(&self.container)))
    }
}

fn references(cat: &Catalog, index: &Index, target: &Target, skip: &Skip) -> (Vec<Reference>, usize) {
    let mut by: BTreeMap<(By, String), usize> = BTreeMap::new();
    let mut hidden = 0;
    for &r in index.values(target.label) {
        match field_of(cat, r, skip) {
            Some(f) if target.shared && !target.named_by(cat, r, &f) => {}
            Some(f) => *by.entry((By::Label, f)).or_default() += 1,
            None if unnamed(cat, cat.record_node(r)) => hidden += 1,
            None => {}
        }
    }
    if let Some(id) = target.stat_id {
        for &r in index.values(id) {
            if let Some(f) = field_of(cat, r, skip)
                && id_field(f.rsplit('.').next().unwrap_or(&f), target.word)
            {
                *by.entry((By::StatId, f)).or_default() += 1;
            }
        }
    }
    let refs = by
        .into_iter()
        .map(|((by, field), count)| Reference { by, field, count })
        .collect();
    (refs, hidden)
}

/// The label-keyed side data of `row` in the stock database `cat`.
pub fn deps(cat: &Catalog, row: NodeId) -> Deps {
    let names = derived(cat, row);
    let label = cat.node(row).label.clone().unwrap_or_default();
    let id = stat_id(cat, row);
    let mut labels: HashSet<String> = names.iter().map(|n| n.value.clone()).collect();
    labels.insert(label.clone());
    let values: HashSet<String> = [Some(label.clone()), id.clone()].into_iter().flatten().collect();
    let index = Index::new(cat, &labels, &values);
    let skip = Skip {
        rows: vec![row],
        family: parent(cat, row).map(|t| family(cat, t)),
    };
    let keyed = keyed(cat, &index, &names, &skip);
    let target = Target::new(cat, &index, row, &label, id.as_deref());
    let (references, unnamed) = references(cat, &index, &target, &skip);
    Deps {
        names,
        keyed,
        references,
        unnamed,
    }
}

struct Pair {
    src: NodeId,
    dst: NodeId,
    /// (source name, copy name) for each derived name the copy changed.
    names: Vec<(Derived, String)>,
    label: (String, String),
    stat_id: Option<(String, String)>,
}

fn pair(cat: &Catalog, src: NodeId, dst: NodeId) -> Pair {
    let ours: HashMap<String, String> = derived_with(cat, dst, true)
        .into_iter()
        .map(|d| (d.from, d.value))
        .collect();
    let names = derived(cat, src)
        .into_iter()
        .filter_map(|d| {
            let new = ours.get(&d.from)?;
            (*new != d.value).then(|| (d, new.clone()))
        })
        .collect();
    let label = |r: NodeId| cat.node(r).label.clone().unwrap_or_default();
    Pair {
        src,
        dst,
        names,
        label: (label(src), label(dst)),
        stat_id: stat_id(cat, src).zip(stat_id(cat, dst)).filter(|(a, b)| a != b),
    }
}

fn list(names: &BTreeSet<String>, shown: usize) -> String {
    let v: Vec<&str> = names.iter().take(shown).map(String::as_str).collect();
    match names.len() > v.len() {
        true => format!("{} and {} more", v.join(", "), names.len() - v.len()),
        false => v.join(", "),
    }
}

#[derive(Default)]
struct Missing {
    hows: BTreeSet<String>,
    sources: BTreeSet<String>,
    tables: BTreeSet<NodeId>,
    lacking: BTreeMap<String, BTreeSet<NodeId>>,
    example: Option<NodeId>,
    derived: bool,
}

#[derive(Default)]
struct Refs {
    news: BTreeSet<String>,
    fields: Vec<(String, usize)>,
    total: usize,
}

/// Build warnings for the side data `copies` lack in the built database `db`.
/// `name` gives an addon's display name from its id.
pub fn check(db: &Gvdb, copies: &[Copy], name: &dyn Fn(&str) -> String) -> Vec<String> {
    if copies.is_empty() {
        return Vec::new();
    }
    let cat = Catalog::from_gvdb(db.clone(), String::new());
    check_with(&cat, copies, name)
}

/// [`check`] on a database already catalogued.
pub fn check_with(cat: &Catalog, copies: &[Copy], name: &dyn Fn(&str) -> String) -> Vec<String> {
    let pairs: Vec<(&Copy, Pair)> = copies
        .iter()
        .filter_map(|c| Some((c, pair(cat, cat.find(&c.from)?, cat.find(&c.row)?))))
        .filter(|(_, p)| p.label.0 != p.label.1)
        .collect();
    let mut labels = HashSet::new();
    let mut values = HashSet::new();
    for (_, p) in &pairs {
        for (d, new) in &p.names {
            labels.insert(d.value.clone());
            labels.insert(new.clone());
        }
        labels.insert(p.label.0.clone());
        values.extend([p.label.0.clone(), p.label.1.clone()]);
        if let Some((a, b)) = &p.stat_id {
            values.extend([a.clone(), b.clone()]);
        }
    }
    let index = Index::new(cat, &labels, &values);
    let mut missing: BTreeMap<(String, String, String), Missing> = BTreeMap::new();
    let mut refs: BTreeMap<(String, NodeId, By, String), Refs> = BTreeMap::new();
    for (copy, p) in &pairs {
        let skip = Skip {
            rows: vec![p.src, p.dst],
            family: parent(cat, p.dst).map(|t| family(cat, t)),
        };
        let olds: Vec<Derived> = p.names.iter().map(|(d, _)| d.clone()).collect();
        for k in keyed(cat, &index, &olds, &skip) {
            let new = &p
                .names
                .iter()
                .find(|(d, _)| *d == k.name)
                .expect("keyed from these names")
                .1;
            let has: HashSet<NodeId> = index.rows(new).iter().filter_map(|&r| parent(cat, r)).collect();
            let lacking: BTreeSet<NodeId> = k.tables.iter().copied().filter(|t| !has.contains(t)).collect();
            if lacking.is_empty() {
                continue;
            }
            let m = missing
                .entry((copy.addon.clone(), k.family.clone(), k.name.value.clone()))
                .or_default();
            m.hows.insert(k.name.how());
            m.sources.insert(cat.select_path(p.src));
            m.tables.extend(k.tables.iter().copied());
            m.lacking.entry(new.clone()).or_default().extend(lacking);
            m.derived |= k.name.from != "label";
            if m.example.is_none() {
                m.example = index
                    .rows(&k.name.value)
                    .iter()
                    .copied()
                    .find(|&r| parent(cat, r).is_some_and(|t| k.tables.contains(&t)));
            }
        }
        let target = Target::new(
            cat,
            &index,
            p.src,
            &p.label.0,
            p.stat_id.as_ref().map(|(a, _)| a.as_str()),
        );
        let (ours, _) = references(cat, &index, &target, &skip);
        let theirs = Target {
            label: &p.label.1,
            stat_id: p.stat_id.as_ref().map(|(_, b)| b.as_str()),
            shared: false,
            ..target
        };
        let (theirs, _) = references(cat, &index, &theirs, &skip);
        let have: HashSet<(By, &str)> = theirs.iter().map(|r| (r.by, r.field.as_str())).collect();
        for by in [By::Label, By::StatId] {
            let fields: Vec<(String, usize)> = ours
                .iter()
                .filter(|r| r.by == by && !have.contains(&(by, r.field.as_str())))
                .map(|r| (r.field.clone(), r.count))
                .collect();
            if fields.is_empty() {
                continue;
            }
            let key: Vec<String> = fields.iter().map(|(f, n)| format!("{f} ({n})")).collect();
            let new = match by {
                By::Label => p.label.1.clone(),
                By::StatId => p.stat_id.as_ref().map(|(_, b)| b.clone()).unwrap_or_default(),
            };
            let r = refs
                .entry((copy.addon.clone(), p.src, by, key.join(", ")))
                .or_default();
            r.news.insert(new);
            r.total = fields.iter().map(|(_, n)| n).sum();
            r.fields = fields;
        }
    }
    let mut per_addon: BTreeMap<String, Vec<(bool, String)>> = BTreeMap::new();
    for ((addon, family, old), m) in missing {
        let lacking: BTreeSet<String> = m.lacking.keys().cloned().collect();
        let worst = m.lacking.values().map(BTreeSet::len).max().unwrap_or(0);
        let example = m.example.map(|r| cat.select_path(r)).unwrap_or_default();
        let table = m
            .example
            .and_then(|r| parent(cat, r))
            .map(|t| cat.select_path(t))
            .unwrap_or_else(|| family.clone());
        let place = match m.tables.len() {
            1 => format!(
                "{table} has a row labelled {old} and none labelled {}",
                list(&lacking, NAMES_SHOWN)
            ),
            n => format!(
                "{family} has rows labelled {old} in {n} tables and none labelled {} in up to {worst} of them",
                list(&lacking, NAMES_SHOWN)
            ),
        };
        let line = format!(
            "{}: {place}; {old} is {} of the add-row source {}. Add a row per copy, such as an add-row into {table} from {example} that sets _metadata_",
            name(&addon),
            m.hows.iter().cloned().collect::<Vec<_>>().join(" and "),
            m.sources.iter().cloned().collect::<Vec<_>>().join(", "),
        );
        per_addon.entry(addon).or_default().push((!m.derived, line));
    }
    let mut out = Vec::new();
    for (addon, mut lines) in per_addon {
        lines.sort();
        let extra = lines.len().saturating_sub(WARNINGS_PER_ADDON);
        out.extend(lines.into_iter().take(WARNINGS_PER_ADDON).map(|(_, l)| l));
        if extra > 0 {
            out.push(format!(
                "{}: {extra} more table(s) have rows labelled with names of add-row sources and none for the copies; swofty db deps <source row> lists them",
                name(&addon)
            ));
        }
    }
    for ((addon, src, by, _), r) in refs {
        let shown: Vec<String> = r
            .fields
            .iter()
            .take(FIELDS_SHOWN)
            .map(|(f, n)| format!("{f} ({n})"))
            .collect();
        let more = match r.fields.len() > shown.len() {
            true => format!(" and {} more field(s)", r.fields.len() - shown.len()),
            false => String::new(),
        };
        let what = match by {
            By::Label => "label",
            By::StatId => "stat_id",
        };
        out.push(format!(
            "{}: {} value(s) pick the add-row source {} by its {what} and none pick the copy {}: {}{more}",
            name(&addon),
            r.total,
            cat.select_path(src),
            list(&r.news, NAMES_SHOWN),
            shown.join(", "),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use swofty_formats::gvdb::Record;

    use super::*;
    use crate::ops::{DbOps, DbReport, apply_db_ops_report};
    use crate::rowkeys::{Names, RowKeys};

    const PACKAGES: &str = "entrants.vehicles.vehicle_packages";
    const PRESETS: &str = "car_setup.presets";

    fn stock() -> Gvdb {
        let package = format!("{PACKAGES}.vehicle_package_0");
        let records = vec![
            Record::new(&format!("{PACKAGES}._vehicle_package_instancecount_"), 0, "1"),
            Record::new(&format!("{package}._metadata_"), 4, "f1_2025"),
            Record::new(&format!("{package}.stat_id"), 0, "24"),
            Record::new(
                &format!("{package}.ai_package_folder"),
                2,
                "f1_2025_vehicle_package",
            ),
            Record::new(
                &format!("{package}.resource_path"),
                2,
                "f1_2025_vehicle_package/teams/[FILENAME]/wep/[FILENAME]",
            ),
            Record::new(
                &format!("{PRESETS}._vehicle_package_setups_instancecount_"),
                0,
                "1",
            ),
            Record::new(
                &format!("{PRESETS}.vehicle_package_setups_0._metadata_"),
                4,
                "f1_2025_vehicle_package",
            ),
            Record::new("entrants.vehicles._vehicle_instancecount_", 0, "1"),
            Record::new("entrants.vehicles.vehicle_0._metadata_", 4, "mercedes_2025"),
            Record::new("entrants.vehicles.vehicle_0.vehicle_package", 4, "f1_2025"),
        ];
        Gvdb { records }
    }

    fn build(ops: &str) -> Vec<String> {
        let mut db = stock();
        let ops: DbOps = serde_json::from_str(ops).unwrap();
        let none = BTreeMap::new();
        let names = Names {
            fields: &none,
            keys: RowKeys::builtin(),
            stock: None,
        };
        let mut report = DbReport::default();
        apply_db_ops_report(&mut db, &ops, &names, &mut report).unwrap();
        let copies: Vec<Copy> = report
            .added_rows
            .into_iter()
            .map(|r| Copy {
                addon: "seasons".into(),
                from: r.from,
                row: r.row,
            })
            .collect();
        check(&db, &copies, &|id| format!("[{id}]"))
    }

    const PACKAGE_COPY: &str = r#"{ "op": "add-row", "table": "entrants.vehicles.vehicle_packages",
        "from": "entrants.vehicles.vehicle_packages.vehicle_package[_metadata_=f1_2025]",
        "set": { "_metadata_": "cs_2019", "resource_path": "cs_2019_vehicle_package/teams/[FILENAME]/wep/[FILENAME]" } }"#;

    #[test]
    fn a_relabelled_package_copy_lacks_its_setup_presets() {
        let warnings = build(&format!(r#"{{"schema_version":1,"ops":[{PACKAGE_COPY}]}}"#));
        let presets = warnings
            .iter()
            .find(|w| w.contains("car_setup.presets.vehicle_package_setups has a row"))
            .unwrap_or_else(|| panic!("{warnings:#?}"));
        assert!(presets.starts_with("[seasons]: "), "{presets}");
        assert!(
            presets.contains("labelled f1_2025_vehicle_package and none labelled cs_2019_vehicle_package"),
            "{presets}"
        );
        assert!(presets.contains("folder 1 of resource_path"), "{presets}");
        assert!(
            presets.contains(
                "from car_setup.presets.vehicle_package_setups[_metadata_=f1_2025_vehicle_package]"
            ),
            "{presets}"
        );
        let refs = warnings
            .iter()
            .find(|w| w.contains("pick the add-row source"))
            .unwrap_or_else(|| panic!("{warnings:#?}"));
        assert!(
            refs.contains(
                "by its label and none pick the copy cs_2019: entrants.vehicles.vehicle_N.vehicle_package (1)"
            ),
            "{refs}"
        );
    }

    #[test]
    fn a_preset_row_for_the_copy_clears_the_warning() {
        let preset = r#"{ "op": "add-row", "table": "car_setup.presets",
            "from": "car_setup.presets.vehicle_package_setups[_metadata_=f1_2025_vehicle_package]",
            "set": { "_metadata_": "cs_2019_vehicle_package" } }"#;
        let warnings = build(&format!(
            r#"{{"schema_version":1,"ops":[{PACKAGE_COPY},{preset}]}}"#
        ));
        assert!(
            !warnings
                .iter()
                .any(|w| w.contains("car_setup.presets.vehicle_package_setups has")),
            "{warnings:#?}"
        );
    }

    #[test]
    fn a_copy_under_its_source_label_brings_everything() {
        let same = r#"{ "op": "add-row", "table": "entrants.vehicles.vehicle_packages",
            "from": "entrants.vehicles.vehicle_packages.vehicle_package_0", "set": {} }"#;
        assert_eq!(
            build(&format!(r#"{{"schema_version":1,"ops":[{same}]}}"#)),
            Vec::<String>::new()
        );
    }

    #[test]
    fn derived_names_skip_generic_folders() {
        let cat = Catalog::from_gvdb(stock(), String::new());
        let row = cat
            .find("entrants.vehicles.vehicle_packages.vehicle_package_0")
            .unwrap();
        let names: Vec<(String, String)> = derived(&cat, row)
            .into_iter()
            .map(|d| (d.how(), d))
            .map(|(h, d)| (d.value, h))
            .collect();
        assert!(names.contains(&(
            "f1_2025_vehicle_package".into(),
            "folder 1 of resource_path".into()
        )));
        assert!(names.contains(&(
            "f1_2025_vehicle_package".into(),
            "the label + _vehicle_package".into()
        )));
        assert!(names.contains(&(
            "f1_2025_vehicle_package".into(),
            "the ai_package_folder value".into()
        )));
        assert!(
            !names.iter().any(|(v, _)| v == "teams" || v == "wep"),
            "{names:?}"
        );
    }
}
