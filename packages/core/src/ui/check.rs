use std::collections::{BTreeMap, BTreeSet};

use swofty_formats::hash::djb2;
use swofty_formats::pkg::{Node, Pkg};

use super::catalog::{Catalog, NodeType, Rule, Rules, Scenes, attr_shape, element_type, shape, tag};
use super::idf;
use super::node::*;

const LIST_CLASSES: &[&str] = &["UITG", "UITB", "UILT", "UIL2", "UIEL"];
const SCREEN_CMDS: &[&str] = &["OpenScreen", "CloseScreen", "TransitionScreen"];

fn str_of<'a>(o: &'a swofty_formats::pkg::Obj, key: &str) -> Option<&'a str> {
    o.get(key)?.as_val()?.as_str()
}

fn walk(n: &Node, path: &str, out: &mut Vec<String>) {
    let name = n.name();
    let here = if path.is_empty() {
        name.to_owned()
    } else {
        format!("{path}.{name}")
    };
    let kind = n.type_code();
    let Ok(props) = n.props() else { return };
    if LIST_CLASSES.contains(&kind.as_str()) {
        let db = props
            .get("data")
            .and_then(Node::as_obj)
            .and_then(|d| str_of(d, "dbentry"));
        match db {
            Some(d) if !d.is_empty() => {}
            _ => out.push(format!(
                "{here} [{kind}]: list controls need data bound to an array (data.dbentry); the game fails to open the screen"
            )),
        }
    }
    if props.get("allowVMBinding").is_some() && !path.is_empty() {
        out.push(format!("{here} [{kind}]: only a screen may set allowVMBinding"));
    }
    if let Ok(lay) = n.slot(LAYOUT) {
        for grp in lay {
            let entries: Vec<&Node> = match grp {
                Node::Arr(a) => a.items.iter().collect(),
                other => vec![other],
            };
            for b in entries.into_iter().filter_map(Node::as_obj) {
                let by_node = b.get("nodeId").is_some();
                let by_comp = b.get("componentId").is_some() && b.get("isComponentId").is_some();
                if !(by_node || by_comp) {
                    out.push(format!(
                        "{here} [{kind}]: a layout entry has neither a nodeId nor a componentId"
                    ));
                }
            }
        }
    }
    if let Ok(kids) = n.slot(CHILDREN) {
        for c in kids.iter().filter(|c| c.is_node()) {
            walk(c, &here, out);
        }
    }
}

fn binds_view_model(scr: &Node) -> bool {
    scr.props()
        .ok()
        .and_then(|p| p.get("allowVMBinding"))
        .and_then(Node::as_obj)
        .and_then(|o| o.get("value"))
        .and_then(Node::as_val)
        .is_some_and(|v| matches!(&v.value, swofty_formats::pkg::Value::Raw(b) if b.first() == Some(&1)))
}

pub fn screens(files: &[(String, Vec<u8>)], view_models: &[String]) -> crate::Result<Vec<String>> {
    let mut out = Vec::new();
    for (name, data) in files {
        let p = Pkg::parse(data)?;
        for scr in p.root.o()?.at("Screens")?.a()? {
            walk(scr, "", &mut out);
            let vm = view_models.contains(name);
            if binds_view_model(scr) && !vm {
                out.push(format!(
                    "{name}: sets allowVMBinding without a view model; the page would stay blank and Back would not work"
                ));
            }
            if vm && !binds_view_model(scr) {
                out.push(format!(
                    "{name}: has a view model but does not set allowVMBinding"
                ));
            }
        }
    }
    Ok(out)
}

struct Flow {
    erp: Option<String>,
    screens: BTreeSet<String>,
}

fn flows(data: &[u8]) -> crate::Result<BTreeMap<String, Flow>> {
    let p = Pkg::parse(data)?;
    let mut out = BTreeMap::new();
    for f in p.root.o()?.at("Flows")?.a()? {
        let erp = f
            .props()?
            .get("asset_group")
            .and_then(Node::as_obj)
            .and_then(|o| str_of(o, "value"))
            .map(|v| {
                let base = v.rsplit('/').next().unwrap_or(v);
                base.rsplit_once('.').map_or(base, |(b, _)| b).to_owned()
            });
        let mut screens = BTreeSet::new();
        for scene in f.slot(CHILDREN)? {
            for e in scene.slot(CHILDREN).map(|v| v.as_slice()).unwrap_or_default() {
                if let Some(s) = e
                    .props()
                    .ok()
                    .and_then(|p| p.get("screen"))
                    .and_then(Node::as_obj)
                    .and_then(|o| str_of(o, "value"))
                {
                    screens.insert(s.to_owned());
                }
            }
        }
        out.insert(f.name().to_owned(), Flow { erp, screens });
    }
    Ok(out)
}

fn attrs(n: &Node) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for a in n
        .as_obj()
        .and_then(|o| o.get("Attributes"))
        .and_then(Node::as_arr)
        .map(|a| a.items.as_slice())
        .unwrap_or_default()
    {
        if let Some(o) = a.as_obj()
            && let (Some(k), Some(v)) = (str_of(o, "Id"), str_of(o, "Value"))
        {
            out.insert(k.to_owned(), v.to_owned());
        }
    }
    out
}

pub fn routes(
    routes: &[u8],
    flows_ui: &[u8],
    changed: &[&str],
    registered: &BTreeMap<String, String>,
) -> crate::Result<Vec<String>> {
    let fl = flows(flows_ui)?;
    let p = Pkg::parse(routes)?;
    let mut groups: BTreeMap<String, BTreeMap<String, &Node>> = BTreeMap::new();
    for g in p.root.o()?.at("RouteGroupings")?.a()? {
        let go = g.o()?;
        let gid = str_of(go, "Id").unwrap_or_default().to_owned();
        for r in go.at("Routes")?.a()? {
            let rid = r.o().ok().and_then(|o| str_of(o, "Id")).unwrap_or_default();
            groups.entry(gid.clone()).or_default().insert(rid.to_owned(), r);
        }
    }
    let known = |t: &str| registered.contains_key(t) || fl.values().any(|f| f.screens.contains(t));
    let mut out = Vec::new();
    for (name, flow) in registered {
        let ok = fl
            .values()
            .any(|f| f.erp.as_deref() == Some(flow) && f.screens.contains(name));
        if !ok {
            out.push(format!(
                "screen {name}: no flow that loads {flow} lists it, so routes to it cannot open it"
            ));
        }
    }
    for (gid, rs) in &groups {
        for (rid, r) in rs {
            let id = format!("{gid}.{rid}");
            if !changed.iter().any(|c| *c == gid || *c == id) {
                continue;
            }
            let mut targets = BTreeSet::new();
            if let Some(d) = attrs(r).get("Destination").filter(|d| !d.is_empty()) {
                targets.insert(d.clone());
            }
            for c in r.o()?.at("Commands")?.a()? {
                let kind = c.o().ok().and_then(|o| str_of(o, "Type")).unwrap_or_default();
                let at = attrs(c);
                if kind == "Migrate" {
                    let f = at.get("Flow").map(String::as_str).unwrap_or_default();
                    if !fl.contains_key(f) {
                        out.push(format!("{id}: Migrate to unknown flow {f:?}"));
                    }
                }
                if kind == "TriggerSubRoute" || SCREEN_CMDS.contains(&kind) {
                    for k in ["Target", "To"] {
                        if let Some(t) = at.get(k).filter(|t| !t.is_empty()) {
                            targets.insert(t.clone());
                        }
                    }
                }
                if kind == "TriggerSubRoute" {
                    let t = at.get("Target").map(String::as_str).unwrap_or_default();
                    let sub = at.get("SubRoute").map(String::as_str).unwrap_or_default();
                    if !groups.get(t).is_some_and(|g| g.contains_key(sub)) {
                        out.push(format!("{id}: sub-route {t}.{sub} does not exist"));
                    }
                }
            }
            for t in targets {
                if !known(&t) {
                    out.push(format!(
                        "{id}: target screen {t:?} is not defined in any flow (fatal in game)"
                    ));
                }
            }
        }
    }
    Ok(out)
}

type Signal = (String, String, String);

fn index<'a>(n: &'a Node, path: &str, out: &mut BTreeMap<String, &'a Node>) {
    let here = if path.is_empty() {
        n.name().to_owned()
    } else {
        format!("{path}.{}", n.name())
    };
    for c in n.slot(CHILDREN).into_iter().flatten().filter(|c| c.is_node()) {
        index(c, &here, out);
    }
    out.insert(here, n);
}

fn tree(p: &Pkg) -> crate::Result<BTreeMap<String, &Node>> {
    let mut out = BTreeMap::new();
    for scr in p.root.o()?.at("Screens")?.a()? {
        index(scr, "", &mut out);
    }
    Ok(out)
}

fn signals(n: &Node) -> Vec<Signal> {
    let mut out = Vec::new();
    for e in [COMMANDS, EVENTS]
        .into_iter()
        .flat_map(|i| n.slot(i).into_iter().flatten())
    {
        let sigs = e.as_obj().and_then(|o| o.get("signals")).and_then(Node::as_arr);
        for s in sigs.into_iter().flat_map(|a| &a.items).filter_map(Node::as_obj) {
            out.push((
                str_of(s, "to_object_name").unwrap_or_default().to_owned(),
                str_of(s, "to_pin_name").unwrap_or_default().to_owned(),
                s.get("attributes").map(canonical).unwrap_or_default(),
            ));
        }
    }
    out
}

fn kids(n: &Node) -> Vec<&str> {
    n.slot(CHILDREN)
        .map(|k| k.iter().filter(|c| c.is_node()).map(|c| c.name()).collect())
        .unwrap_or_default()
}

pub fn stock_kept(stock: &[u8], patched: &[u8], prefix: &str) -> crate::Result<Vec<String>> {
    let (a, b) = (Pkg::parse(stock)?, Pkg::parse(patched)?);
    let (old, new) = (tree(&a)?, tree(&b)?);
    let sent: BTreeSet<Signal> = old.values().flat_map(|n| signals(n)).collect();
    let mut out = Vec::new();
    for (path, n) in &old {
        let Some(m) = new.get(path) else {
            out.push(format!("{path}: removed"));
            continue;
        };
        let slot = |x: &Node, i: usize| x.a().ok().and_then(|a| a.get(i)).map(canonical);
        if (0..8)
            .filter(|i| ![CHILDREN, COMMANDS, EVENTS].contains(i))
            .any(|i| slot(n, i) != slot(m, i))
        {
            out.push(format!("{path}: its properties, layout or behaviours changed"));
        }
        let (k0, k1) = (kids(n), kids(m));
        if !k1.starts_with(&k0) || k1[k0.len()..].iter().any(|k| !k.starts_with(prefix)) {
            out.push(format!("{path}: its stock children changed"));
        }
    }
    for (path, m) in &new {
        for sig in signals(m) {
            if old.contains_key(&sig.0) && !sent.contains(&sig) {
                out.push(format!(
                    "{path}: sends {}.{} {}, which no stock node sends",
                    sig.0, sig.1, sig.2
                ));
            }
        }
    }
    Ok(out)
}

fn integrity_walk<'a>(n: &'a Node, parent: Option<&str>, out: &mut Vec<(String, &'a Node)>) {
    let path = match parent {
        Some(p) => format!("{p}.{}", n.name()),
        None => n.name().to_owned(),
    };
    out.push((path.clone(), n));
    for c in n.slot(CHILDREN).into_iter().flatten().filter(|c| c.is_node()) {
        integrity_walk(c, Some(&path), out);
    }
}

fn u32_at(o: &swofty_formats::pkg::Obj, key: &str) -> Option<u32> {
    o.get(key)?.as_val()?.as_u32()
}

pub fn integrity(data: &[u8]) -> crate::Result<(usize, usize, Vec<String>)> {
    let p = Pkg::parse(data)?;
    let root = p.root.o()?;
    let mut nodes = Vec::new();
    for key in ["Screens", "Templates"] {
        for top in root
            .get(key)
            .and_then(Node::as_arr)
            .map(|a| a.items.as_slice())
            .unwrap_or_default()
        {
            integrity_walk(top, None, &mut nodes);
        }
    }
    let paths: BTreeSet<&str> = nodes.iter().map(|(p, _)| p.as_str()).collect();
    let (mut bad, mut signals) = (Vec::new(), 0);
    for (path, n) in &nodes {
        let props = n.props()?;
        if props.get("id").is_some() && u32_at(props, "id") != Some(djb2(path)) {
            bad.push(format!("{path}: id"));
        }
        if props.get("nameId").is_some() && u32_at(props, "nameId") != Some(djb2(n.name())) {
            bad.push(format!("{path}: nameId"));
        }
        for grp in n.slot(LAYOUT).into_iter().flatten().filter_map(Node::as_arr) {
            for b in grp.items.iter().filter_map(Node::as_obj) {
                if let (Some(debug), Some(id)) = (str_of(b, "debugName"), u32_at(b, "nodeId"))
                    && id != djb2(debug)
                {
                    bad.push(format!("{path}: layout {debug}"));
                }
            }
        }
        for (slot, key, hash) in [
            (EVENTS, "eventName", "event"),
            (COMMANDS, "commandName", "command"),
        ] {
            for e in n.slot(slot).into_iter().flatten().filter_map(Node::as_obj) {
                let Some(name) = str_of(e, key) else { continue };
                if e.get(hash).is_some() && u32_at(e, hash) != Some(djb2(name)) {
                    bad.push(format!("{path}: {key} {name}"));
                }
                for s in e.at("signals")?.a()?.iter().filter_map(Node::as_obj) {
                    signals += 1;
                    let to = str_of(s, "to_object_name").unwrap_or_default();
                    let pin = str_of(s, "to_pin_name").unwrap_or_default();
                    if u32_at(s, "to_object") != Some(djb2(to)) || u32_at(s, "to_pin") != Some(djb2(pin)) {
                        bad.push(format!("{path}: hash {to}.{pin}"));
                    }
                    if !paths.contains(to) {
                        bad.push(format!("{path}: on {name} -> missing {to}.{pin}"));
                    }
                }
            }
        }
    }
    Ok((nodes.len(), signals, bad))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Hard,
    Warn,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    pub level: Level,
    pub code: String,
    pub node_type: String,
    pub name: String,
    pub at: String,
    pub message: String,
    pub source: String,
}

impl Finding {
    pub fn warn(code: &str, node_type: &str, name: &str, at: &str, message: String) -> Self {
        Self {
            level: Level::Warn,
            code: code.to_owned(),
            node_type: node_type.to_owned(),
            name: name.to_owned(),
            at: at.to_owned(),
            message,
            source: "catalogue".into(),
        }
    }

    fn stock_diff(code: &str, node_type: &str, name: &str, at: &str, message: String) -> Self {
        Self {
            source: "stock diff".into(),
            ..Self::warn(code, node_type, name, at, message)
        }
    }

    fn hard(code: &str, at: &str, message: String, source: &str) -> Self {
        Self {
            level: Level::Hard,
            source: source.to_owned(),
            ..Self::warn(code, "", "", at, message)
        }
    }
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {} [{}]", self.at, self.message, self.source)
    }
}

#[derive(Debug, Default)]
pub struct Report {
    pub hard: Vec<Finding>,
    pub warnings: Vec<Finding>,
}

impl Report {
    fn add(&mut self, f: Finding, rules: &Rules) {
        let rule = rules
            .rules
            .iter()
            .find(|r| r.matches(&f.code, &f.node_type, &f.name));
        match rule {
            Some(r) if r.effect != "warn" => self.hard.push(Finding {
                level: Level::Hard,
                source: format!("{} {}: {}", r.id, r.effect, r.evidence),
                message: r.message.clone().unwrap_or(f.message.clone()),
                ..f
            }),
            _ if f.level == Level::Hard => self.hard.push(f),
            _ => self.warnings.push(f),
        }
    }
}

fn integrity_findings(data: &[u8], frag: &str) -> crate::Result<Vec<Finding>> {
    let p = Pkg::parse(data)?;
    let root = p.root.o()?;
    let tops: BTreeSet<&str> = ["Screens", "Templates"]
        .iter()
        .filter_map(|k| root.get(k).and_then(Node::as_arr))
        .flat_map(|a| a.items.iter().filter(|t| t.is_node()).map(|t| t.name()))
        .collect();
    let (_, _, bad) = integrity(data)?;
    Ok(bad
        .into_iter()
        .filter(|b| match b.split_once(" -> missing ") {
            Some((_, to)) => tops.contains(to.split('.').next().unwrap_or_default()),
            None => true,
        })
        .map(|b| {
            let (at, msg) = b.split_once(": ").unwrap_or((&b, ""));
            Finding::hard(
                "integrity",
                &format!("{frag}: {at}"),
                msg.to_owned(),
                "format: the loader resolves nodes, events and signals by these djb2 hashes",
            )
        })
        .collect())
}

fn vm_keys(n: &Node, out: &mut BTreeSet<String>) {
    let Ok(props) = n.props() else { return };
    for (k, v) in &props.items {
        let Some(o) = v.as_obj() else { continue };
        let bound = o.get("dbentry").and_then(Node::as_val).and_then(|v| v.as_str());
        let named = (k.as_str() == Some("dbEntry"))
            .then(|| o.get("value").and_then(Node::as_val).and_then(|v| v.as_str()))
            .flatten();
        for key in bound.into_iter().chain(named) {
            if !key.is_empty()
                && !key.starts_with('#')
                && key.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            {
                out.insert(key.split('.').next().unwrap_or(key).to_owned());
            }
        }
    }
    for c in n.slot(CHILDREN).into_iter().flatten().filter(|c| c.is_node()) {
        vm_keys(c, out);
    }
}

fn roots(data: &[u8]) -> crate::Result<BTreeMap<String, BTreeSet<String>>> {
    let p = Pkg::parse(data)?;
    let root = p.root.o()?;
    let mut out = BTreeMap::new();
    for key in ["Screens", "Templates"] {
        for t in root
            .get(key)
            .and_then(Node::as_arr)
            .map(|a| a.items.as_slice())
            .unwrap_or_default()
            .iter()
            .filter(|t| t.is_node())
        {
            let mut keys = BTreeSet::new();
            vm_keys(t, &mut keys);
            out.insert(t.name().to_owned(), keys);
        }
    }
    Ok(out)
}

pub fn runtime_bindings(stock: &[u8], edited: &[u8], frag: &str) -> crate::Result<Vec<Finding>> {
    let (old, new) = (roots(stock)?, roots(edited)?);
    let mut out = Vec::new();
    for (name, keys) in &new {
        let Some(before) = old.get(name) else { continue };
        for k in keys.difference(before) {
            out.push(Finding::stock_diff(
                "vm-key-new",
                "",
                k,
                &format!("{frag}: {name}"),
                format!(
                    "binds view-model key {k}, which no stock {name} node binds; stock {name} only binds {}. The binding resolves only if native code has published {k} when the template binds, so the screen depends on that order",
                    before.iter().take(8).cloned().collect::<Vec<_>>().join(", ")
                ),
            ));
        }
    }
    Ok(out)
}

pub fn required_props(data: &[u8], rules: &Rules, frag: &str) -> crate::Result<Vec<Finding>> {
    let wanted: Vec<&Rule> = rules
        .rules
        .iter()
        .filter(|r| matches!(r.code.as_str(), "required-prop" | "forbidden-prop"))
        .collect();
    let mut out = Vec::new();
    if wanted.is_empty() {
        return Ok(out);
    }
    let p = Pkg::parse(data)?;
    let mut nodes = Vec::new();
    for key in ["Screens", "Templates"] {
        for top in p
            .root
            .o()?
            .get(key)
            .and_then(Node::as_arr)
            .map(|a| a.items.as_slice())
            .unwrap_or_default()
        {
            integrity_walk(top, None, &mut nodes);
        }
    }
    for (path, n) in nodes {
        let t = n.type_code();
        let props = n.props()?;
        for r in &wanted {
            let Some(name) = r.name.as_deref() else { continue };
            if !r.node_type.as_deref().is_none_or(|x| x == "*" || x == t) {
                continue;
            }
            let has = props.get(name).is_some();
            if (r.code == "required-prop") != has {
                let what = if has { "sets" } else { "does not set" };
                out.push(Finding::warn(
                    &r.code,
                    &t,
                    name,
                    &format!("{frag}: {path} [{t}]"),
                    format!("{what} {name}"),
                ));
            }
        }
    }
    Ok(out)
}

pub fn ui_edit(
    frag: &str,
    data: &[u8],
    stock: Option<&[u8]>,
    scenes: &Scenes,
    cat: &Catalog,
    rules: &Rules,
) -> crate::Result<Report> {
    let mut report = Report::default();
    let mut found = integrity_findings(data, frag)?;
    found.extend(against_stock(data, cat, scenes)?.into_iter().map(|mut f| {
        f.at = format!("{frag}: {}", f.at);
        f
    }));
    if let Some(s) = stock {
        found.extend(runtime_bindings(s, data, frag)?);
    }
    found.extend(required_props(data, rules, frag)?);
    for f in found {
        report.add(f, rules);
    }
    Ok(report)
}

pub fn scene_edit(data: &[u8], cat: &Catalog, rules: &Rules) -> crate::Result<Report> {
    let mut report = Report::default();
    let p = Pkg::parse(data)?;
    for m in idf::xref_mismatches(&p)? {
        let (at, msg) = m.split_once(": ").unwrap_or((&m, ""));
        report.add(
            Finding::hard(
                "xref-mismatch",
                at,
                msg.to_owned(),
                "format: the game loads the scene an eXRef's xref id names",
            ),
            rules,
        );
    }
    for f in scene_against_stock(data, cat)? {
        report.add(f, rules);
    }
    Ok(report)
}

const SKIP_PROPS: &[&str] = &["id", "nameId", "version", "name", "type"];
const COMPONENTS: usize = 5;

struct Stock<'a> {
    cat: &'a Catalog,
    scenes: &'a Scenes,
    types: BTreeMap<String, String>,
    out: Vec<Finding>,
}

fn node_scene(n: &Node) -> Option<u64> {
    let comp = |o: &swofty_formats::pkg::Obj| match &o.get("componentId")?.as_val()?.value {
        swofty_formats::pkg::Value::Raw(b) if b.len() >= 8 => {
            Some(u64::from_le_bytes(b[..8].try_into().ok()?))
        }
        _ => None,
    };
    if let Some(id) = n
        .slot(COMPONENTS)
        .into_iter()
        .flatten()
        .filter_map(Node::as_obj)
        .find_map(comp)
    {
        return Some(id);
    }
    layout_groups(n)
        .into_iter()
        .flatten()
        .filter(|b| b.get("isComponentId").is_some())
        .find_map(comp)
}

fn layout_groups(n: &Node) -> Vec<Vec<&swofty_formats::pkg::Obj>> {
    n.slot(LAYOUT)
        .into_iter()
        .flatten()
        .map(|grp| match grp {
            Node::Arr(a) => a.items.iter().filter_map(Node::as_obj).collect(),
            other => other.as_obj().into_iter().collect(),
        })
        .collect()
}

fn seen<V>(m: &BTreeMap<String, V>) -> String {
    let keys: Vec<&str> = m.keys().map(String::as_str).take(8).collect();
    let more = if m.len() > 8 { ", …" } else { "" };
    format!("{}{more}", keys.join(", "))
}

impl Stock<'_> {
    fn visit(
        &mut self,
        n: &Node,
        path: &str,
        parent: Option<&str>,
        role: &str,
        ctx: Option<u64>,
        scopes: &[u64],
    ) {
        let t = n.type_code();
        let here = format!("{path} [{t}]");
        let own = node_scene(n);
        let kid_ctx = own.or(ctx);
        let mut kid_scopes = scopes.to_vec();
        if let Some(nt) = self.cat.node_types.get(&t) {
            self.node(n, &here, nt, &t, parent, role, ctx, scopes, &mut kid_scopes);
        } else {
            self.out.push(Finding::warn(
                "node-type",
                &t,
                &t,
                &here,
                format!("node type {t} is not in any stock .ui"),
            ));
        }
        for c in n.slot(CHILDREN).into_iter().flatten().filter(|c| c.is_node()) {
            let p = format!("{path}.{}", c.name());
            self.visit(c, &p, Some(&t), "child", kid_ctx, &kid_scopes);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn node(
        &mut self,
        n: &Node,
        here: &str,
        nt: &NodeType,
        t: &str,
        parent: Option<&str>,
        role: &str,
        ctx: Option<u64>,
        scopes: &[u64],
        kid_scopes: &mut Vec<u64>,
    ) {
        let out = &mut self.out;
        if role != "child" && !nt.roles.contains_key(role) {
            out.push(Finding::warn(
                "root-type",
                t,
                role,
                here,
                format!("stock never uses a {t} as a {role}; it is a {}", seen(&nt.roles)),
            ));
        }
        if let Some(p) = parent
            && self.cat.node_types.contains_key(p)
            && !nt.parents.contains_key(p)
        {
            out.push(Finding::warn(
                "parent-type",
                t,
                p,
                here,
                format!(
                    "stock never puts a {t} under a {p}; its parents are {}",
                    seen(&nt.parents)
                ),
            ));
        }
        if let Ok(props) = n.props() {
            for (k, v) in &props.items {
                let key = k.as_str().unwrap_or_default();
                if SKIP_PROPS.contains(&key) {
                    continue;
                }
                match nt.props.get(key) {
                    None => out.push(Finding::warn(
                        "prop",
                        t,
                        key,
                        here,
                        format!("property {key} is never set on a stock {t}"),
                    )),
                    Some(p) => {
                        let sh = shape(v);
                        if !p.shapes.contains_key(&sh) {
                            out.push(Finding::warn(
                                "prop-shape",
                                t,
                                key,
                                here,
                                format!("property {key} is {sh}; stock {t} uses {}", seen(&p.shapes)),
                            ));
                        }
                    }
                }
            }
        }
        for (slot, key, kind, stock) in [
            (COMMANDS, "commandName", "command", &nt.commands),
            (EVENTS, "eventName", "event", &nt.events),
        ] {
            for e in n.slot(slot).into_iter().flatten().filter_map(Node::as_obj) {
                let name = str_of(e, key).unwrap_or("?");
                let sigs: Vec<&swofty_formats::pkg::Obj> = e
                    .get("signals")
                    .and_then(Node::as_arr)
                    .map(|a| a.items.iter().filter_map(Node::as_obj).collect())
                    .unwrap_or_default();
                match stock.get(name) {
                    None => out.push(Finding::warn(
                        kind,
                        t,
                        name,
                        here,
                        format!("{kind} {name} is never declared on a stock {t}"),
                    )),
                    Some(s) if sigs.is_empty() && s.no_signals == 0 => out.push(Finding::warn(
                        if kind == "event" {
                            "event-no-signals"
                        } else {
                            "command-no-signals"
                        },
                        t,
                        name,
                        here,
                        format!(
                            "{kind} {name} has no signals; all {} stock {t} {name} {kind}s have signals",
                            s.count
                        ),
                    )),
                    Some(s) if !sigs.is_empty() && s.no_signals == s.count => out.push(Finding::warn(
                        if kind == "event" {
                            "event-signals"
                        } else {
                            "command-signals"
                        },
                        t,
                        name,
                        here,
                        format!("{kind} {name} has signals; no stock {t} {name} {kind} has any"),
                    )),
                    _ => {}
                }
                for s in sigs {
                    let to = str_of(s, "to_object_name").unwrap_or_default();
                    let pin = str_of(s, "to_pin_name").unwrap_or_default();
                    let from = str_of(s, "from_pin_name").unwrap_or("null");
                    if !nt.pins_out.contains_key(from) {
                        out.push(Finding::warn(
                            "pin-out",
                            t,
                            from,
                            here,
                            format!("on {name} sends from pin {from}, which no stock {t} sends from"),
                        ));
                    }
                    let Some(tt) = self.types.get(to) else { continue };
                    let Some(target) = self.cat.node_types.get(tt) else {
                        continue;
                    };
                    let Some(p) = target.pins_in.get(pin) else {
                        out.push(Finding::warn(
                            "pin-in",
                            tt,
                            pin,
                            here,
                            format!("on {name} -> {to}.{pin}: no stock signal reaches pin {pin} on a {tt}"),
                        ));
                        continue;
                    };
                    if !nt.links.contains_key(&format!("{tt}.{pin}")) {
                        out.push(Finding::warn(
                            "link",
                            t,
                            &format!("{tt}.{pin}"),
                            here,
                            format!("on {name} -> {to}.{pin}: no stock {t} signals a {tt}.{pin}"),
                        ));
                    }
                    let attrs = s.get("attributes").map(attr_shape);
                    let with: u64 = p.attributes.values().sum();
                    match attrs {
                        Some(a) if with == 0 => out.push(Finding::warn("pin-attributes", tt, pin, here, format!(
                            "on {name} -> {to}.{pin}: sends attributes {a}; stock signals to {tt}.{pin} carry none"
                        ))),
                        Some(a) if !p.attributes.contains_key(&a) => out.push(Finding::warn("pin-attributes", tt, pin, here, format!(
                            "on {name} -> {to}.{pin}: attributes {a}; stock signals to {tt}.{pin} carry {}",
                            seen(&p.attributes)
                        ))),
                        None if with == p.count => out.push(Finding::warn("pin-attributes", tt, pin, here, format!(
                            "on {name} -> {to}.{pin}: no attributes; every stock signal to {tt}.{pin} carries {}",
                            seen(&p.attributes)
                        ))),
                        _ => {}
                    }
                }
            }
        }
        for grp in layout_groups(n) {
            let kinds: Vec<&str> = grp
                .iter()
                .map(|b| {
                    if b.get("isComponentId").is_some() {
                        "comp"
                    } else if b.get("nodeId").is_some() {
                        "node"
                    } else {
                        "?"
                    }
                })
                .collect();
            let key = format!("{role}:{}", kinds.join(">"));
            if !nt.layout.contains_key(&key) {
                out.push(Finding::warn(
                    "layout-shape",
                    t,
                    &kinds.join(">"),
                    here,
                    format!(
                        "layout shape {} as a {role} is never used by a stock {t}; it uses {}",
                        kinds.join(">"),
                        seen(&nt.layout)
                    ),
                ));
            }
            let names: Vec<&str> = grp
                .iter()
                .filter(|b| b.get("nodeId").is_some())
                .filter_map(|b| str_of(b, "debugName"))
                .collect();
            if role != "child" || names.is_empty() || names.len() != grp.len() {
                continue;
            }
            let hit = ctx
                .into_iter()
                .chain(scopes.iter().rev().copied())
                .filter(|sid| self.scenes.scenes.contains_key(sid))
                .find_map(|sid| self.scenes.resolve(&names, sid).ok());
            let Some((chain, xref)) = hit else { continue };
            let ch = chain.join(">");
            if chain[..chain.len() - 1].iter().any(|c| c != "eXRef") {
                out.push(Finding::warn("layout-through", t, &ch, here, format!(
                    "layout {} walks {ch}; in stock every element before the last in a layout path is an eXRef",
                    names.join(">")
                )));
            } else if !nt.layout_elements.contains_key(&ch) {
                out.push(Finding::warn(
                    "layout-element",
                    t,
                    &ch,
                    here,
                    format!(
                        "layout {} binds {ch}; stock {t} binds {}",
                        names.join(">"),
                        seen(&nt.layout_elements)
                    ),
                ));
            }
            if let Some(x) = xref {
                kid_scopes.push(x);
            }
        }
    }
}

fn index_types(n: &Node, path: &str, out: &mut BTreeMap<String, String>) {
    out.insert(path.to_owned(), n.type_code());
    for c in n.slot(CHILDREN).into_iter().flatten().filter(|c| c.is_node()) {
        index_types(c, &format!("{path}.{}", c.name()), out);
    }
}

pub fn against_stock(data: &[u8], cat: &Catalog, scenes: &Scenes) -> crate::Result<Vec<Finding>> {
    let p = Pkg::parse(data)?;
    let root = p.root.o()?;
    let mut out = Vec::new();
    for (key, role) in [("Screens", "screen_root"), ("Templates", "template_root")] {
        let tops: Vec<&Node> = root
            .get(key)
            .and_then(Node::as_arr)
            .map(|a| a.items.iter().filter(|t| t.is_node()).collect())
            .unwrap_or_default();
        let mut types = BTreeMap::new();
        for t in &tops {
            index_types(t, t.name(), &mut types);
        }
        let mut st = Stock {
            cat,
            scenes,
            types,
            out: Vec::new(),
        };
        for t in tops {
            st.visit(t, t.name(), None, role, node_scene(t), &[]);
        }
        out.extend(st.out);
    }
    Ok(out)
}

pub fn scene_against_stock(data: &[u8], cat: &Catalog) -> crate::Result<Vec<Finding>> {
    let p = Pkg::parse(data)?;
    let mut out = Vec::new();
    for entry in idf::scenes(&p)? {
        let Some(root) = entry.as_obj().and_then(|o| o.get("scene")) else {
            continue;
        };
        scene_walk(root, None, "", cat, &mut out);
    }
    Ok(out)
}

fn scene_walk(e: &Node, parent: Option<&str>, path: &str, cat: &Catalog, out: &mut Vec<Finding>) {
    let t = element_type(e);
    let here = format!("{} [{t}]", if path.is_empty() { idf::elem_name(e) } else { path });
    match cat.scene_elements.get(&t) {
        None => out.push(Finding::warn(
            "element-type",
            &t,
            &t,
            &here,
            format!("element type {t} is not in any stock scene"),
        )),
        Some(et) => {
            if let Some(p) = parent
                && !et.parents.contains_key(p)
            {
                out.push(Finding::warn(
                    "element-parent",
                    &t,
                    p,
                    &here,
                    format!(
                        "stock never puts a {t} under a {p}; its parents are {}",
                        seen(&et.parents)
                    ),
                ));
            }
            for prm in e
                .as_obj()
                .and_then(|o| o.get("params"))
                .and_then(Node::as_arr)
                .map(|a| a.items.as_slice())
                .unwrap_or_default()
                .iter()
                .filter_map(Node::as_obj)
            {
                let name = str_of(prm, "name").unwrap_or_default();
                let ty = match prm.get("value") {
                    Some(Node::Val(v)) => tag(v.ty),
                    Some(other) => shape(other),
                    None => "none".into(),
                };
                match et.params.get(name) {
                    None => out.push(Finding::warn(
                        "param",
                        &t,
                        name,
                        &here,
                        format!("param {name} is never set on a stock {t}"),
                    )),
                    Some(sp) if !sp.types.contains_key(&ty) => out.push(Finding::warn(
                        "param-type",
                        &t,
                        name,
                        &here,
                        format!("param {name} is {ty}; stock {t} uses {}", seen(&sp.types)),
                    )),
                    _ => {}
                }
            }
        }
    }
    for c in e
        .as_obj()
        .and_then(|o| o.get("children"))
        .and_then(Node::as_arr)
        .map(|a| a.items.as_slice())
        .unwrap_or_default()
        .iter()
        .filter(|c| c.as_obj().is_some())
    {
        let name = idf::elem_name(c);
        let p = match (parent, path) {
            (None, _) => name.to_owned(),
            (_, "") => name.to_owned(),
            _ => format!("{path}.{name}"),
        };
        scene_walk(c, Some(&t), &p, cat, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::node::{arr, boolv, command, event, fix_ids, node, obj, prop, s, sig, u32v, ui_bytes};

    fn screen_with(kids: Vec<Node>, vm: bool) -> Vec<u8> {
        let mut props = vec![];
        if vm {
            props.push(("allowVMBinding", prop(boolv(true), None)));
        }
        let mut scr = node("page", "UISC", props, vec![], &[], &[]);
        scr.a_mut().unwrap()[CHILDREN] = arr(kids);
        ui_bytes(vec![scr]).unwrap()
    }

    #[test]
    fn integrity_finds_stale_hashes_and_missing_targets() {
        use crate::ui::node::{event, sig};
        let kid = node(
            "kid",
            "UIGP",
            vec![],
            vec![event("pressed", vec![sig("page.kid", "focus", "pressed")])],
            &["slot"],
            &[],
        );
        let mut scr = node("page", "UISC", vec![], vec![], &[], &[]);
        scr.a_mut().unwrap()[CHILDREN] = arr(vec![kid]);
        let mut bad = scr.clone();
        fix_ids(&mut scr, None);
        let (nodes, signals, problems) = integrity(&ui_bytes(vec![scr]).unwrap()).unwrap();
        assert_eq!((nodes, signals), (2, 1));
        assert!(problems.is_empty(), "{problems:?}");
        let kid = &mut bad.slot_mut(CHILDREN).unwrap()[0];
        let e = &mut kid.slot_mut(EVENTS).unwrap()[0];
        let s = &mut e.o_mut().unwrap().at_mut("signals").unwrap().a_mut().unwrap()[0];
        set_str(s.o_mut().unwrap(), "to_object_name", "page.gone").unwrap();
        let (_, _, problems) = integrity(&ui_bytes(vec![bad]).unwrap()).unwrap();
        assert_eq!(
            problems,
            [
                "page: id",
                "page.kid: id",
                "page.kid: hash page.gone.focus",
                "page.kid: on pressed -> missing page.gone.focus"
            ]
        );
    }

    #[test]
    fn list_controls_need_bound_data() {
        let bound = node(
            "ok",
            "UITG",
            vec![("data", obj(vec![("value", s("")), ("dbentry", s("#a.b"))]))],
            vec![],
            &[],
            &[],
        );
        let literal = node(
            "bad",
            "UITG",
            vec![("data", prop(s("x"), None))],
            vec![],
            &[],
            &[],
        );
        let files = vec![("page".to_owned(), screen_with(vec![bound, literal], false))];
        let out = screens(&files, &[]).unwrap();
        assert_eq!(out.len(), 1, "{out:?}");
        assert!(out[0].starts_with("page.bad [UITG]"));
    }

    #[test]
    fn view_model_binding_needs_a_view_model() {
        let files = vec![("page".to_owned(), screen_with(vec![], true))];
        assert_eq!(screens(&files, &[]).unwrap().len(), 1);
        assert!(screens(&files, &["page".to_owned()]).unwrap().is_empty());
        let plain = vec![("page".to_owned(), screen_with(vec![], false))];
        assert_eq!(screens(&plain, &["page".to_owned()]).unwrap().len(), 1);
    }

    fn cmd(kind: &str, attrs: &[(&str, &str)]) -> Node {
        let a = attrs
            .iter()
            .map(|(k, v)| obj(vec![("Id", s(k)), ("Value", s(v))]))
            .collect();
        obj(vec![("Attributes", arr(a)), ("Type", s(kind))])
    }

    fn route(id: &str, cmds: Vec<Node>) -> Node {
        obj(vec![("Id", s(id)), ("Commands", arr(cmds))])
    }

    fn pkg(root: Node) -> Vec<u8> {
        Pkg {
            name: [0; 4],
            root,
            vca: Vec::new(),
        }
        .to_bytes()
        .unwrap()
    }

    #[test]
    fn routes_must_reach_screens_and_sub_routes() {
        let entry = node(
            "enter_page",
            "FLEN",
            vec![("screen", prop(s("page"), None))],
            vec![],
            &[],
            &[],
        );
        let mut scene = node("scene", "FLSC", vec![], vec![], &[], &[]);
        scene.a_mut().unwrap()[CHILDREN] = arr(vec![entry]);
        let mut flow = node(
            "options_flow",
            "FLOW",
            vec![(
                "asset_group",
                prop(s("asset_groups/ui_package/flow_options.assetgroup"), None),
            )],
            vec![],
            &[],
            &[],
        );
        flow.a_mut().unwrap()[CHILDREN] = arr(vec![scene]);
        let flows = pkg(obj(vec![("Flows", arr(vec![flow]))]));
        let group = |id: &str, routes: Vec<Node>| obj(vec![("Id", s(id)), ("Routes", arr(routes))]);
        let routes = pkg(obj(vec![(
            "RouteGroupings",
            arr(vec![
                group(
                    "page",
                    vec![route("open", vec![cmd("OpenScreen", &[("Target", "page")])])],
                ),
                group(
                    "home",
                    vec![
                        route(
                            "go",
                            vec![cmd(
                                "TriggerSubRoute",
                                &[("SubRoute", "open"), ("Target", "page")],
                            )],
                        ),
                        route(
                            "bad",
                            vec![
                                cmd("TriggerSubRoute", &[("SubRoute", "shut"), ("Target", "page")]),
                                cmd("OpenScreen", &[("Target", "nowhere")]),
                                cmd("Migrate", &[("Flow", "no_flow")]),
                            ],
                        ),
                    ],
                ),
            ]),
        )]));
        let registered = BTreeMap::from([("page".to_owned(), "flow_options".to_owned())]);
        let out = super::routes(&routes, &flows, &["page", "home.go"], &registered).unwrap();
        assert!(out.is_empty(), "{out:?}");
        let out = super::routes(&routes, &flows, &["home"], &registered).unwrap();
        assert_eq!(out.len(), 3, "{out:?}");
        let elsewhere = BTreeMap::from([("page".to_owned(), "flow_frontend".to_owned())]);
        let out = super::routes(&routes, &flows, &[], &elsewhere).unwrap();
        assert_eq!(out.len(), 1, "{out:?}");
    }

    fn cat() -> Catalog {
        serde_json::from_value(serde_json::json!({
            "node_types": {
                "UIGP": {
                    "roles": { "template_root": 1, "child": 1 },
                    "parents": { "UIGP": 1 },
                    "props": { "visible": { "count": 1, "shapes": { "bool": 1 } } },
                    "events": { "navUp": { "count": 3, "no_signals": 0 }, "anim_complete": { "count": 2, "no_signals": 2 } },
                    "commands": { "focus": { "count": 1, "no_signals": 0 } },
                    "pins_in": { "focus": { "count": 1 }, "raise_command": { "count": 1, "attributes": { "command.name.value:stri": 1 } } },
                    "pins_out": { "navUp": 1, "anim_complete": 1, "focus": 1 },
                    "links": { "UIGP.focus": 1, "UIGP.raise_command": 1 },
                    "layout": { "child:node": 1, "child:node>node": 1 },
                    "layout_elements": { "eXRef": 1, "eXRef>eNull": 1 }
                }
            },
            "scene_elements": {
                "eScene": {},
                "eXRef": { "parents": { "eScene": 1, "eMaskTexture": 1 }, "params": { "fileRef": { "count": 1, "types": { "stri": 1 } } } },
                "eNull": { "parents": { "eScene": 1 } },
                "eMaskTexture": { "parents": { "eScene": 1 } },
                "eMask": { "parents": { "eNull": 1 }, "params": {} }
            }
        }))
        .unwrap()
    }

    fn u64v(v: u64) -> Node {
        Node::Val(swofty_formats::pkg::Val::raw(*b"ui64", v.to_le_bytes()))
    }

    fn el(name: &str, ty: &str, kids: Vec<Node>, xref: Option<u64>) -> Node {
        let mut t = ty.as_bytes().to_vec();
        t.resize(16, 0);
        let mut pairs = vec![
            ("name", s(name)),
            ("typeName", Node::Val(swofty_formats::pkg::Val::raw(*b"shnm", t))),
            ("params", arr(vec![])),
            ("children", arr(kids)),
        ];
        if let Some(x) = xref {
            pairs.push(("xref", u64v(x)));
        }
        obj(pairs)
    }

    fn scene(id: u64, kids: Vec<Node>) -> Vec<u8> {
        let entry = obj(vec![
            ("scene", el("Scene Root", "eScene", kids, None)),
            ("name", u64v(id)),
            ("nodeidlookup", arr(vec![])),
        ]);
        pkg(obj(vec![("scenelist", arr(vec![entry]))]))
    }

    fn template_node(kids: Vec<Node>, scene_id: u64) -> Node {
        let mut root = node("tpl", "UIGP", vec![], vec![], &[], &[]);
        root.a_mut().unwrap()[CHILDREN] = arr(kids);
        root.a_mut().unwrap()[COMPONENTS] = arr(vec![obj(vec![
            ("componentId", u64v(scene_id)),
            ("debugName", s("eaid://ui_package/art/x")),
            ("name", s("x")),
        ])]);
        fix_ids(&mut root, None);
        root
    }

    fn template(kids: Vec<Node>, scene_id: u64) -> Vec<u8> {
        pkg(obj(vec![("Templates", arr(vec![template_node(kids, scene_id)]))]))
    }

    #[test]
    fn catalogue_flags_what_stock_never_does() {
        let mut row = node(
            "row",
            "UIGP",
            vec![
                ("visible", prop(s("yes"), None)),
                ("invented", prop(boolv(true), None)),
            ],
            vec![
                event("navUp", vec![]),
                event("anim_complete", vec![sig("tpl.row", "focus", "anim_complete")]),
                event("mystery", vec![sig("tpl.row", "focus", "mystery")]),
            ],
            &[],
            &[],
        );
        row.a_mut().unwrap()[COMMANDS] =
            arr(vec![command("focus", vec![sig("tpl.row", "explode", "focus")])]);
        let other = node("other", "UICX", vec![], vec![], &[], &[]);
        let out = against_stock(&template(vec![row, other], 1), &cat(), &Scenes::default()).unwrap();
        let has = |s: &str| out.iter().any(|f| f.to_string().contains(s));
        assert!(has("property visible is stri; stock UIGP uses bool"), "{out:#?}");
        assert!(has("property invented is never set"), "{out:#?}");
        assert!(has("event navUp has no signals"), "{out:#?}");
        assert!(has("event anim_complete has signals; no stock"), "{out:#?}");
        assert!(has("event mystery is never declared"), "{out:#?}");
        assert!(has("no stock signal reaches pin explode on a UIGP"), "{out:#?}");
        assert!(has("node type UICX is not in any stock .ui"), "{out:#?}");
        assert_eq!(out.len(), 8, "{out:#?}");
    }

    #[test]
    fn layout_paths_through_a_mask_are_flagged() {
        let mut scenes = Scenes::default();
        let row = el("row", "eXRef", vec![], Some(2));
        scenes
            .add(&scene(1, vec![el("row_mask", "eMaskTexture", vec![row], None)]))
            .unwrap();
        scenes
            .add(&scene(2, vec![el("item_spawner", "eNull", vec![], None)]))
            .unwrap();
        let direct = node("a", "UIGP", vec![], vec![], &["row"], &[]);
        let nested = node("b", "UIGP", vec![], vec![], &["row", "item_spawner"], &[]);
        let masked = node("c", "UIGP", vec![], vec![], &["row_mask", "row"], &[]);
        let out = against_stock(&template(vec![direct, nested, masked], 1), &cat(), &scenes).unwrap();
        assert_eq!(out.len(), 1, "{out:#?}");
        assert!(
            out[0]
                .to_string()
                .starts_with("tpl.c [UIGP]: layout row_mask>row walks eMaskTexture>eXRef"),
            "{out:#?}"
        );
    }

    #[test]
    fn scenes_are_checked_for_parenting_and_params() {
        let mut xref = el("row", "eXRef", vec![], Some(2));
        xref.as_obj_mut().unwrap().set(
            "params",
            arr(vec![obj(vec![("name", s("fileRef")), ("value", u32v(3))])]),
        );
        let data = scene(
            1,
            vec![
                el("m", "eMask", vec![xref], None),
                el("w", "eWiggle", vec![], None),
            ],
        );
        let out = scene_against_stock(&data, &cat()).unwrap();
        let has = |s: &str| out.iter().any(|f| f.to_string().contains(s));
        assert!(
            has("m [eMask]: stock never puts a eMask under a eScene"),
            "{out:#?}"
        );
        assert!(
            has("m.row [eXRef]: stock never puts a eXRef under a eMask"),
            "{out:#?}"
        );
        assert!(has("param fileRef is ui32; stock eXRef uses stri"), "{out:#?}");
        assert!(has("element type eWiggle is not in any stock scene"), "{out:#?}");
    }

    #[test]
    fn the_stock_catalogue_knows_the_row_navigation_pins() {
        let c = super::super::catalog::stock();
        let f1ts = &c.node_types["F1TS"];
        for pin in ["focus", "inputAllow", "inputDisallow", "focus_and_reset"] {
            assert!(f1ts.pins_in.contains_key(pin), "{pin}");
        }
        assert!(c.node_types["UIGP"].commands.contains_key("block_input"));
        assert!(c.scene_elements["eMaskTexture"].children.contains_key("eXRef"));
    }

    #[test]
    fn loader_rules_turn_matching_findings_into_hard_ones() {
        let rules: Rules = serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "rules": [
                { "id": "nav-needs-signals", "code": "event-no-signals", "node_type": "UIGP", "name": "navUp", "effect": "reject", "evidence": "FUN_140000000 L2" },
                { "id": "row-needs-visible", "code": "required-prop", "node_type": "UIGP", "name": "visible", "effect": "drop", "evidence": "FUN_140000010 L1" }
            ]
        }))
        .unwrap();
        let row = node("row", "UIGP", vec![], vec![event("navUp", vec![])], &[], &[]);
        let report = ui_edit(
            "syst",
            &template(vec![row], 1),
            None,
            &Scenes::default(),
            &cat(),
            &rules,
        )
        .unwrap();
        assert_eq!(report.hard.len(), 3, "{report:#?}");
        assert!(
            report
                .hard
                .iter()
                .any(|h| h.source.starts_with("nav-needs-signals reject"))
        );
        assert!(
            report
                .hard
                .iter()
                .any(|h| h.code == "required-prop" && h.source.starts_with("row-needs-visible drop"))
        );
        let none = ui_edit(
            "syst",
            &template(vec![], 1),
            None,
            &Scenes::default(),
            &cat(),
            &Rules::default(),
        )
        .unwrap();
        assert!(none.hard.is_empty() && none.warnings.is_empty(), "{none:#?}");
    }

    #[test]
    fn bindings_to_view_model_keys_stock_never_binds_are_flagged() {
        let spawner = |name: &str, key: &str| {
            node(
                name,
                "F1TS",
                vec![("dataContext", prop(s(""), Some(key)))],
                vec![],
                &[],
                &[],
            )
        };
        let stock = template(vec![spawner("a", "tiles")], 1);
        let query = node(
            "has",
            "SVCD",
            vec![("dbEntry", prop(s("addon_tiles_visible"), None))],
            vec![],
            &[],
            &[],
        );
        let global = node(
            "g",
            "UILB",
            vec![("text", prop(s(""), Some("#hud.title")))],
            vec![],
            &[],
            &[],
        );
        let edited = template(
            vec![
                spawner("a", "tiles"),
                spawner("b", "addon_tiles.list"),
                query,
                global,
            ],
            1,
        );
        let found = runtime_bindings(&stock, &edited, "syst").unwrap();
        let keys: Vec<&str> = found.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(keys, ["addon_tiles", "addon_tiles_visible"], "{found:#?}");
        assert!(
            found
                .iter()
                .all(|f| f.level == Level::Warn && f.code == "vm-key-new")
        );
    }

    #[test]
    fn broken_hashes_are_hard_and_signals_leaving_the_file_are_not() {
        let row = node(
            "row",
            "UIGP",
            vec![],
            vec![event("navUp", vec![sig("elsewhere.node", "focus", "navUp")])],
            &[],
            &[],
        );
        let mut root = template_node(vec![row], 1);
        root.find_mut("row")
            .unwrap()
            .props_mut()
            .unwrap()
            .set("nameId", u32v(1));
        let report = ui_edit(
            "syst",
            &pkg(obj(vec![("Templates", arr(vec![root]))])),
            None,
            &Scenes::default(),
            &cat(),
            &Rules::default(),
        )
        .unwrap();
        assert_eq!(report.hard.len(), 1, "{report:#?}");
        assert!(
            report.hard[0].to_string().contains("tpl.row: nameId"),
            "{report:#?}"
        );
    }
}
