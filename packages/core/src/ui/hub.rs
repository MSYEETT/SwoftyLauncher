use swofty_formats::pkg::Node;

use super::node::*;
use crate::Result;
use crate::manifest::HubTab;

pub const ARCHIVES: [&str; 2] = ["flow_career_hub.erp", "flow_team_hq.erp"];
/// The laptop's OVERVIEW page file; the hub tab templates join its templates.
pub const UI: &str = "eaid://ui_package/screens/laptop_overview.ui";
pub const UI_FRAGMENT: usize = 0;
/// The stock page each hub tab is built from: the laptop's inbox, a list of six entries beside a reading pane.
pub const BASE: &str = "tab_content_emails_panel";
const LIST: &str = "email_list";
const ENTRIES: usize = 6;
const PANE_TITLE: &str = "reading_pane.subject";
const PANE_BODY: &str = "reading_pane.message_body";
const HEADER: &str = "num_unread.num_unread";
const HIDDEN: [&str; 6] = [
    "scrollbar",
    "no_emails_display",
    "emails_display",
    "reading_pane.from",
    "reading_pane.from_image",
    "tool_tip_interface.tutorial_tooltip",
];
/// The inbox's attachment widgets under the reading pane, which the page leaves out.
const ATTACHMENTS: &str = "attachments";
const TOOLTIPS: &str = "tool_tip_interface";
const TOOLTIP_SOURCE: &str = "exit_laptop";

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

fn retype(n: &mut Node, code: &str) -> Result<()> {
    n.props_mut()?.at_mut("type")?.v_mut()?.set_u32(fourcc(code));
    Ok(())
}

fn adjust_tooltip(t: &mut Node, name: &str) -> Result<()> {
    let tips = t.child_mut(TOOLTIPS)?;
    let mut tip = tips.child(TOOLTIP_SOURCE)?.clone();
    rename_refs(
        &mut tip,
        &format!("{name}.{TOOLTIPS}.{TOOLTIP_SOURCE}"),
        &format!("{name}.{TOOLTIPS}.adjust"),
    );
    rename(&mut tip, "adjust")?;
    set_prop_value(
        &mut tip,
        "tooltipsName",
        swofty_formats::pkg::Val::str(super::pages::ADJUST_TOOLTIP),
    )?;
    set_bool(&mut tip, "visible", true)?;
    set_prop_value(
        tip.child_mut("text")?,
        "localise",
        swofty_formats::pkg::Val::bool(false),
    )?;
    tips.slot_mut(CHILDREN)?.push(tip);
    Ok(())
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
    if p.rows.is_empty() || p.rows.len() > ENTRIES {
        return Err(err(format!(
            "hub tab {}: {} rows, not 1 to {ENTRIES}",
            p.tab.name,
            p.rows.len()
        )));
    }
    for path in HIDDEN {
        hide(t.find_mut(path)?)?;
    }
    let header = t.find_mut(HEADER)?;
    bind_text(header, &format!("{vm}.title"))?;
    set_bool(header, "enabled", true)?;
    set_bool(t.child_mut("num_unread")?, "visible", true)?;
    let pane = t.child_mut("reading_pane")?;
    set_bool(pane, "visible", true)?;
    pane.slot_mut(CHILDREN)?
        .retain(|c| !(c.is_node() && c.name() == ATTACHMENTS));
    literal(t.find_mut(PANE_TITLE)?, p.rows[0].label)?;
    literal(t.find_mut(PANE_BODY)?, p.rows[0].description)?;
    adjust_tooltip(&mut t, &name)?;

    let list = t.child_mut(LIST)?;
    super::pages::as_group(list)?;
    list.slot_mut(EVENTS)?.clear();
    set_bool(list, "visible", true)?;
    set_bool(list, "enabled", true)?;
    let at = |i: usize| format!("{name}.{LIST}.entry_{i}");
    for i in 0..ENTRIES {
        let entry = list.child_mut(&format!("entry_{i}"))?;
        retype(entry, "UIGP")?;
        let Some(row) = p.rows.get(i) else {
            hide(entry)?;
            set_bool(entry, "enabled", false)?;
            continue;
        };
        set_bool(entry, "enabled", true)?;
        set_bool(entry, "visible", true)?;
        bind_text(entry.child_mut("from")?, &format!("{vm}.o{i}.label"))?;
        bind_text(entry.child_mut("subject")?, &format!("{vm}.o{i}.value"))?;
        hide(entry.child_mut("unread_icon")?)?;
        let command = |verb: &str| format!("swofty_{verb}:{}:{}", p.addon, row.id);
        let (next, prev) = (command("next"), command("prev"));
        let mut evs = vec![
            event(
                "focused",
                vec![
                    sig_with(
                        &format!("{name}.{PANE_TITLE}"),
                        "setText",
                        "focused",
                        &[("text", row.label.into())],
                    ),
                    sig_with(
                        &format!("{name}.{PANE_BODY}"),
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
        add_events(entry, evs)?;
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

/// Adds each page's template to the laptop page file `data`, built from [`BASE`]: the inbox header shows the page
/// title, its entries are the rows (label over value), and the reading pane shows the focused row's label and
/// description.
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
