use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use serde::Deserialize;
use swofty_formats::pkg::{Node, Pkg, Val, Value};

use super::idf;

const STOCK_JSON: &str = include_str!("../../../../tools/uicatalog/catalog.json");
const RULES_JSON: &str = include_str!("../../../../tools/uicatalog/loader_rules.json");

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Rules {
    pub schema_version: u32,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct Rule {
    pub id: String,
    pub code: String,
    pub node_type: Option<String>,
    pub name: Option<String>,
    pub effect: String,
    pub evidence: String,
    pub message: Option<String>,
}

impl Rule {
    pub fn matches(&self, code: &str, node_type: &str, name: &str) -> bool {
        self.code == code
            && self
                .node_type
                .as_deref()
                .is_none_or(|t| t == "*" || t == node_type)
            && self.name.as_deref().is_none_or(|n| n == "*" || n == name)
    }
}

pub fn rules() -> &'static Rules {
    static RULES: OnceLock<Rules> = OnceLock::new();
    RULES.get_or_init(|| serde_json::from_str(RULES_JSON).expect("tools/uicatalog/loader_rules.json parses"))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Catalog {
    pub node_types: BTreeMap<String, NodeType>,
    pub scene_elements: BTreeMap<String, ElementType>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct NodeType {
    pub count: u64,
    pub roles: BTreeMap<String, u64>,
    pub props: BTreeMap<String, Prop>,
    pub commands: BTreeMap<String, Named>,
    pub events: BTreeMap<String, Named>,
    pub pins_in: BTreeMap<String, Pin>,
    pub pins_out: BTreeMap<String, u64>,
    pub links: BTreeMap<String, u64>,
    pub children: BTreeMap<String, u64>,
    pub parents: BTreeMap<String, u64>,
    pub layout: BTreeMap<String, u64>,
    pub layout_elements: BTreeMap<String, u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Prop {
    pub count: u64,
    pub shapes: BTreeMap<String, u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Named {
    pub count: u64,
    pub no_signals: u64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Pin {
    pub count: u64,
    pub attributes: BTreeMap<String, u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ElementType {
    pub count: u64,
    pub params: BTreeMap<String, Param>,
    pub parents: BTreeMap<String, u64>,
    pub children: BTreeMap<String, u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Param {
    pub count: u64,
    pub types: BTreeMap<String, u64>,
}

pub fn stock() -> &'static Catalog {
    static STOCK: OnceLock<Catalog> = OnceLock::new();
    STOCK.get_or_init(|| serde_json::from_str(STOCK_JSON).expect("tools/uicatalog/catalog.json parses"))
}

pub fn tag(ty: [u8; 4]) -> String {
    String::from_utf8_lossy(&ty).into_owned()
}

pub fn shape(n: &Node) -> String {
    match n {
        Node::Val(v) => format!("={}", tag(v.ty)),
        Node::Arr(_) => "arr".into(),
        Node::Iar(_) => "Iar".into(),
        Node::Obj(o) => {
            let mut keys: Vec<&str> = o.keys().collect();
            match o.get("value") {
                Some(inner) => {
                    let base = match inner {
                        Node::Val(v) => tag(v.ty),
                        other => shape(other),
                    };
                    let mut extra: Vec<&str> = keys.into_iter().filter(|k| *k != "value").collect();
                    extra.sort_unstable();
                    if extra.is_empty() {
                        base
                    } else {
                        format!("{base}+{}", extra.join("+"))
                    }
                }
                None => {
                    keys.sort_unstable();
                    format!("obj{{{}}}", keys.join(","))
                }
            }
        }
    }
}

fn attr_paths(n: &Node, pre: &str, out: &mut Vec<String>) {
    match n {
        Node::Val(v) => out.push(format!("{pre}:{}", tag(v.ty))),
        Node::Obj(o) => {
            for (k, v) in &o.items {
                let key = k.as_str().unwrap_or_default();
                let p = if pre.is_empty() {
                    key.to_owned()
                } else {
                    format!("{pre}.{key}")
                };
                attr_paths(v, &p, out);
            }
        }
        Node::Arr(_) => out.push(format!("{pre}[]")),
        Node::Iar(_) => out.push(pre.to_owned()),
    }
}

pub fn attr_shape(n: &Node) -> String {
    let mut out = Vec::new();
    attr_paths(n, "", &mut out);
    out.sort_unstable();
    out.join(" ")
}

pub fn shnm(v: &Val) -> String {
    match &v.value {
        Value::Str(s) => s.clone(),
        Value::Raw(b) => {
            let end = b.iter().rposition(|c| *c != 0).map_or(0, |i| i + 1);
            String::from_utf8_lossy(&b[..end]).into_owned()
        }
    }
}

pub fn element_type(e: &Node) -> String {
    e.as_obj()
        .and_then(|o| o.get("typeName"))
        .and_then(Node::as_val)
        .map(shnm)
        .unwrap_or_else(|| "?".into())
}

#[derive(Debug, Default)]
pub struct Scene {
    pub elements: Vec<(String, String, Option<u64>, String)>,
    pub by_name: HashMap<String, Vec<usize>>,
    pub xrefs: Vec<u64>,
}

#[derive(Debug, Default)]
pub struct Scenes {
    pub scenes: HashMap<u64, Scene>,
}

type Hit<'a> = (&'a str, Option<u64>, &'a str, u64);

impl Scenes {
    pub fn add(&mut self, data: &[u8]) -> crate::Result<()> {
        let p = Pkg::parse(data)?;
        for entry in idf::scenes(&p)? {
            let (Some(sid), Some(root)) = (
                idf::scene_name(entry),
                entry.as_obj().and_then(|o| o.get("scene")),
            ) else {
                continue;
            };
            let mut sc = Scene::default();
            for (i, (path, e)) in idf::elements(root).into_iter().enumerate() {
                let name = idf::elem_name(e).to_owned();
                let xref = idf::xref(e);
                sc.by_name
                    .entry(name.clone())
                    .or_default()
                    .push(sc.elements.len());
                sc.xrefs.extend(xref);
                sc.elements.push((
                    name,
                    element_type(e),
                    xref,
                    if i == 0 { String::new() } else { path },
                ));
            }
            self.scenes.insert(sid, sc);
        }
        Ok(())
    }

    fn hits(&self, sid: u64, name: &str, prefix: &str) -> Vec<Hit<'_>> {
        let Some(sc) = self.scenes.get(&sid) else {
            return Vec::new();
        };
        sc.by_name
            .get(name)
            .into_iter()
            .flatten()
            .map(|i| &sc.elements[*i])
            .filter(|(_, _, _, path)| path.starts_with(prefix))
            .map(|(_, ty, x, path)| (ty.as_str(), *x, path.as_str(), sid))
            .collect()
    }

    fn lookup(&self, name: &str, sid: u64, prefix: &str) -> Vec<Hit<'_>> {
        let hits = self.hits(sid, name, prefix);
        if !hits.is_empty() || !prefix.is_empty() {
            return hits;
        }
        let mut seen = Vec::new();
        let mut queue = std::collections::VecDeque::from([sid]);
        while let Some(s) = queue.pop_front() {
            if seen.contains(&s) {
                continue;
            }
            seen.push(s);
            let Some(sc) = self.scenes.get(&s) else { continue };
            let hits = self.hits(s, name, "");
            if !hits.is_empty() {
                return hits;
            }
            queue.extend(sc.xrefs.iter().copied());
        }
        Vec::new()
    }

    pub fn resolve(&self, names: &[&str], sid: u64) -> Result<(Vec<String>, Option<u64>), String> {
        self.resolve_in(names, sid, "")
    }

    fn resolve_in(
        &self,
        names: &[&str],
        sid: u64,
        prefix: &str,
    ) -> Result<(Vec<String>, Option<u64>), String> {
        let Some((first, rest)) = names.split_first() else {
            return Err("empty layout path".into());
        };
        if !self.scenes.contains_key(&sid) {
            return Err(format!("scene {sid:#018x} is not loaded"));
        }
        let mut hits = self.lookup(first, sid, prefix);
        if hits.is_empty() {
            return Err(format!("no element {first}"));
        }
        if rest.is_empty() {
            return Ok((vec![hits[0].0.to_owned()], hits[0].1));
        }
        hits.sort_by_key(|h| h.1.is_none());
        let mut first_err = None;
        for (ty, xref, path, s) in hits {
            let next = match xref {
                Some(x) => self.resolve_in(rest, x, ""),
                None => self.resolve_in(rest, s, &format!("{path}.")),
            };
            match next {
                Ok((mut chain, last)) => {
                    chain.insert(0, ty.to_owned());
                    return Ok((chain, last));
                }
                Err(e) => {
                    first_err.get_or_insert(e);
                }
            }
        }
        Err(first_err.unwrap_or_default())
    }
}
