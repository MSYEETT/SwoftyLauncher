use swofty_formats::pkg::Node;

use super::node::*;
use crate::Result;
use crate::manifest::HubTab;

pub const ARCHIVES: [&str; 2] = ["flow_career_hub.erp", "flow_team_hq.erp"];
/// The laptop's DRIVER DETAILS page file; the hub tab templates join its templates.
pub const UI: &str = "eaid://ui_package/screens/laptop_my_driver.ui";
pub const UI_FRAGMENT: usize = 0;
/// The stock page each hub tab is built from: the driver statistics sub-tab.
pub const BASE: &str = "tab_content_driver_details_panel";
const ROWS: &str = "left_stats";
/// The grid cells of `block_l` in the order the rows fill them: down the first column, then the next.
const SLOTS: [&str; 6] = ["1", "3", "2", "4", "5", "6"];
const DETAIL_TITLE: &str = "details.driver_number.title";
const DETAIL_BODY: &str = "details.driver_number.label";

pub struct Row<'a> {
    pub id: &'a str,
    pub label: &'a str,
    pub description: &'a str,
}

pub struct Page<'a> {
    pub addon: &'a str,
    pub tab: &'a HubTab,
    pub rows: Vec<Row<'a>>,
}

fn templates(p: &mut swofty_formats::pkg::Pkg) -> Result<&mut Vec<Node>> {
    root_obj(p)?.at_mut("Templates")?.a_mut()
}

fn bind_text(n: &mut Node, path: &str) -> Result<()> {
    let p = n.props_mut()?;
    p.set("text", obj(vec![("value", s("")), ("dbentry", s(path))]));
    p.set("localise", prop(boolv(false), None));
    Ok(())
}

fn literal(n: &mut Node, text: &str) -> Result<()> {
    let p = n.props_mut()?;
    p.set("text", prop(s(text), None));
    p.set("localise", prop(boolv(false), None));
    Ok(())
}

fn drop_contexts(n: &mut Node) -> Result<()> {
    n.props_mut()?.drop_key("dataContext");
    for c in n.slot_mut(CHILDREN)? {
        if c.is_node() {
            drop_contexts(c)?;
        }
    }
    Ok(())
}

fn hide(n: &mut Node) -> Result<()> {
    set_bool(n, "visible", false)
}

fn slot_of(n: &Node) -> Option<String> {
    let lay = n.slot(LAYOUT).ok()?;
    let grp = lay.first()?.as_arr()?;
    let step = grp.items.first()?.as_obj()?;
    step.get("debugName")?.as_val()?.as_str().map(str::to_owned)
}

fn page(base: &Node, p: &Page) -> Result<Node> {
    let name = p.tab.template();
    let vm = format!("#{}", p.tab.view_model());
    let mut t = base.clone();
    rename_refs(&mut t, BASE, &name);
    rename(&mut t, &name)?;
    strip_data(&mut t)?;
    drop_contexts(&mut t)?;
    set_prop(&mut t, "allowVMBinding", prop(boolv(true), None))?;

    let details = t.child_mut("details")?;
    set_bool(details, "visible", true)?;
    for c in details.slot_mut(CHILDREN)? {
        if c.is_node() && !matches!(c.name(), "driver_name" | "driver_number") {
            hide(c)?;
        }
    }
    let first = p
        .rows
        .first()
        .ok_or_else(|| err(format!("hub tab {}: no rows", p.tab.name)))?;
    literal(t.find_mut("details.driver_name.title")?, "")?;
    bind_text(t.find_mut("details.driver_name.label")?, &format!("{vm}.title"))?;
    literal(t.find_mut(DETAIL_TITLE)?, first.label)?;
    literal(t.find_mut(DETAIL_BODY)?, first.description)?;
    hide(t.child_mut("right_stats")?)?;

    let rows = t.child_mut(ROWS)?;
    set_bool(rows, "visible", true)?;
    set_bool(rows, "enabled", true)?;
    let cells: Vec<(String, String)> = rows
        .slot(CHILDREN)?
        .iter()
        .filter(|c| c.is_node())
        .filter_map(|c| Some((slot_of(c)?, c.name().to_owned())))
        .collect();
    let mut order = Vec::new();
    for slot in SLOTS {
        let cell = cells
            .iter()
            .find(|(s, _)| s == slot)
            .ok_or_else(|| err(format!("{BASE}.{ROWS} has no cell in slot {slot}")))?;
        order.push(cell.1.clone());
    }
    let at = |i: usize| format!("{name}.{ROWS}.{}", order[i]);
    for c in rows.slot_mut(CHILDREN)?.iter_mut().filter(|c| c.is_node()) {
        let Some(i) = order.iter().position(|o| o == c.name()) else {
            continue;
        };
        let Some(row) = p.rows.get(i) else {
            hide(c)?;
            continue;
        };
        set_bool(c, "enabled", true)?;
        bind_text(c.child_mut("title")?, &format!("{vm}.o{i}.label"))?;
        bind_text(c.child_mut("value")?, &format!("{vm}.o{i}.value"))?;
        for icon in ["left_icon", "right_icon"] {
            hide(c.child_mut(icon)?)?;
        }
        let command = |verb: &str| format!("swofty_{verb}:{}:{}", p.addon, row.id);
        let (next, prev) = (command("next"), command("prev"));
        let mut evs = vec![
            event(
                "focused",
                vec![
                    sig_with(
                        &format!("{name}.{DETAIL_TITLE}"),
                        "setText",
                        "focused",
                        &[("text", row.label.into())],
                    ),
                    sig_with(
                        &format!("{name}.{DETAIL_BODY}"),
                        "setText",
                        "focused",
                        &[("text", row.description.into())],
                    ),
                ],
            ),
            event("navRight", vec![raise_cmd(&name, "navRight", &next, &next)]),
            event("navLeft", vec![raise_cmd(&name, "navLeft", &prev, &prev)]),
        ];
        if i > 0 {
            evs.push(event("navUp", vec![sig(&at(i - 1), "focus", "navUp")]));
        }
        if i + 1 < p.rows.len() {
            evs.push(event("navDown", vec![sig(&at(i + 1), "focus", "navDown")]));
        }
        add_events(c, evs)?;
    }
    add_events(
        &mut t,
        vec![
            event("shown", vec![sig(&at(0), "focus", "shown")]),
            event("enabled", vec![sig(&at(0), "focus", "enabled")]),
        ],
    )?;
    fix_ids(&mut t, None);
    Ok(t)
}

/// Adds each page's template to the laptop page file `data`, built from [`BASE`]: the details panel shows the
/// page title and the focused row's label and description, and the grid cells are the rows.
pub fn build_ui(data: &[u8], pages: &[Page]) -> Result<Vec<u8>> {
    edit(data, |pkg| {
        let list = templates(pkg)?;
        let base = list
            .iter()
            .find(|t| t.is_node() && t.name() == BASE)
            .cloned()
            .ok_or_else(|| err(format!("the laptop page file has no {BASE}")))?;
        for p in pages {
            let name = p.tab.template();
            if list.iter().any(|t| t.is_node() && t.name() == name) {
                return Err(err(format!("the laptop page file has {name} already")));
            }
            list.push(page(&base, p)?);
        }
        Ok(())
    })
}

pub fn has_base(data: &[u8]) -> Result<bool> {
    let p = swofty_formats::pkg::Pkg::parse(data)?;
    Ok(p.root
        .o()?
        .at("Templates")?
        .a()?
        .iter()
        .any(|t| t.is_node() && t.name() == BASE))
}
