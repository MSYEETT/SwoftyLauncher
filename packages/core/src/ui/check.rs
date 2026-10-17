use std::collections::{BTreeMap, BTreeSet};

use swofty_formats::pkg::{Node, Pkg};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::node::{arr, boolv, node, obj, prop, s, ui_bytes};

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
}
