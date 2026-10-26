use swofty_formats::pkg::{Node, Pkg};

use super::idf;
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
/// The inbox page's scene. The build adds a branch to its `mail` branch with a value pill over each list row.
pub const SCENE: &str = "eaid://ui_package/art/components/career/laptop/messages_panel.idf";
pub const SCENE_FRAGMENT: usize = 0;
/// The laptop's selector pill (the season selector on STANDINGS): a centred value between two arrows.
const PILL: &str = "eaid://ui_package/art/components/career/laptop/sr_toggle_pill.idf";
/// The scene reference the pills are cloned from.
const PILL_SOURCE: &str = "messages_summary";
const SCENE_BRANCH: &str = "mail";
/// The added branch, and the widget on it: hidden on the stock inbox, shown on a hub tab.
pub const ROWS: &str = "swofty_hub_rows";
const PILL_SCALE: f64 = 0.66;
/// The list stack's place in `mail`, the pitch of its rows, and a pill's top left inside a row (over the second
/// text line), in front of the row.
const STACK_AT: [f64; 2] = [0.2, -1.06];
const ROW_PITCH: f64 = 0.86;
const PILL_IN_ROW: [f64; 2] = [0.1, -0.45];
const FRONT: f64 = -0.3;
const LABEL_BEHAVIOURS: &[&str] = &["NBEL", "NBMG", "NBOS", "NBXA", "NBMO"];
const PANE_TITLE: &str = "reading_pane.subject";
const PANE_BODY: &str = "reading_pane.message_body";
const HEADER: &str = "num_unread.num_unread";
/// The group on the scene's `mail` branch, which holds the list, the reading pane and the header. Stock shows it
/// through a negated binding (visible unless the inbox is empty), so the page sets it visible.
const MAIL: &str = "emails_display";
const HIDDEN: [&str; 7] = [
    "scrollbar",
    "no_emails_display",
    "emails_display.no_mail_label",
    "divider",
    "reading_pane.from",
    "reading_pane.from_image",
    "tool_tip_interface.tutorial_tooltip",
];
/// The group on the reading pane's attachment branch. The page keeps it empty and hidden, which hides the branch's
/// attachment layouts.
const ATTACHMENTS: &str = "reading_pane.attachments";
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

pub fn pill(i: usize) -> String {
    format!("swofty_hub_pill_{i}")
}

fn place(e: &mut Node, at: [f64; 3], scale: f64) -> Result<()> {
    let o = e.o_mut()?;
    o.set("position", Node::Val(idf::vec3([at[0], at[1], at[2], 0.0])));
    o.set("scale", Node::Val(idf::vec3([scale, scale, 1.0, 0.0])));
    Ok(())
}

fn scene_find_mut<'a>(e: &'a mut Node, name: &str) -> Option<&'a mut Node> {
    if idf::elem_name(e) == name {
        return Some(e);
    }
    idf::children_mut(e)
        .ok()?
        .iter_mut()
        .find_map(|k| scene_find_mut(k, name))
}

/// [`SCENE`] with the [`ROWS`] branch in `mail`: one [`PILL`] reference per list row, over the row's second line.
pub fn build_scene(data: &[u8]) -> Result<Vec<u8>> {
    let mut doc = Pkg::parse(data)?;
    let entry = idf::scenes_mut(&mut doc)?
        .first_mut()
        .ok_or_else(|| err(format!("{SCENE} has no scenes")))?;
    if idf::scene_name(entry) != Some(idf::scene_id(SCENE)) {
        return Err(err(format!("not {SCENE}")));
    }
    let root = entry.o_mut()?.at_mut("scene")?;
    if idf::find(root, ROWS).is_some() {
        return Err(err(format!("{SCENE} has {ROWS} already")));
    }
    let branch = idf::elements(root)
        .into_iter()
        .skip(1)
        .find(|(_, e)| idf::elem_name(e) == SCENE_BRANCH)
        .map(|(path, _)| path)
        .ok_or_else(|| err(format!("{SCENE} has no {SCENE_BRANCH}")))?;
    let source = idf::find(root, PILL_SOURCE).ok_or_else(|| err(format!("{SCENE} has no {PILL_SOURCE}")))?;
    let mut pills = Vec::new();
    for i in 0..ENTRIES {
        let mut e = idf::detached(source);
        e.o_mut()?.set("name", s(&pill(i)));
        idf::retarget(&mut e, PILL)?;
        let y = PILL_IN_ROW[1] - ROW_PITCH * i as f64;
        place(&mut e, [PILL_IN_ROW[0], y, 0.0], PILL_SCALE)?;
        pills.push(e);
    }
    let mail =
        scene_find_mut(root, SCENE_BRANCH).ok_or_else(|| err(format!("{SCENE} has no {SCENE_BRANCH}")))?;
    let mut rows = idf::detached(mail);
    rows.o_mut()?.set("name", s(ROWS));
    place(&mut rows, [STACK_AT[0], STACK_AT[1], FRONT], 1.0)?;
    *idf::children_mut(&mut rows)? = pills;
    idf::recompute_ids(&mut rows, &format!("{branch}.{ROWS}"))?;
    idf::children_mut(mail)?.push(rows);
    idf::relookup(entry)?;
    let out = doc.to_bytes()?;
    let back = Pkg::parse(&out)?;
    if back.to_bytes()? != out {
        return Err(err(format!("the patched {SCENE} does not round-trip")));
    }
    let mut bad = idf::xref_mismatches(&back)?;
    bad.extend(idf::id_problems(&back)?);
    if !bad.is_empty() {
        return Err(err(format!("{SCENE}: {}", bad.join("; "))));
    }
    Ok(out)
}

fn pills(vm: &str, rows: usize) -> Result<Node> {
    let mut group = node(
        ROWS,
        "UIGP",
        vec![("visible", prop(boolv(true), None))],
        vec![],
        &[ROWS],
        GROUP_BEHAVIOURS,
    );
    for i in 0..ENTRIES {
        let name = pill(i);
        let mut p = node(
            &name,
            "UIGP",
            vec![("visible", prop(boolv(i < rows), None))],
            vec![],
            &[&name],
            GROUP_BEHAVIOURS,
        );
        if i < rows {
            let mut value = node("value", "UILB", vec![], vec![], &["label"], LABEL_BEHAVIOURS);
            bind_text(&mut value, &format!("{vm}.o{i}.value"))?;
            let kids = p.slot_mut(CHILDREN)?;
            kids.push(value);
            for (arrow, at) in [("left_arrow", "toggle_left"), ("right_arrow", "toggle_right")] {
                kids.push(node(arrow, "UIGP", vec![], vec![], &[at], GROUP_BEHAVIOURS));
            }
        }
        group.slot_mut(CHILDREN)?.push(p);
    }
    Ok(group)
}

fn hide(n: &mut Node) -> Result<()> {
    set_bool(n, "visible", false)
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
    set_bool(t.child_mut(MAIL)?, "visible", true)?;
    for path in HIDDEN {
        hide(t.find_mut(path)?)?;
    }
    let header = t.find_mut(HEADER)?;
    bind_text(header, &format!("{vm}.title"))?;
    set_bool(header, "enabled", true)?;
    set_bool(t.child_mut("num_unread")?, "visible", true)?;
    set_bool(t.child_mut("reading_pane")?, "visible", true)?;
    let attachments = t.find_mut(ATTACHMENTS)?;
    attachments.slot_mut(CHILDREN)?.clear();
    attachments.slot_mut(EVENTS)?.clear();
    hide(attachments)?;
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
        super::pages::as_group(entry)?;
        let Some(row) = p.rows.get(i) else {
            hide(entry)?;
            set_bool(entry, "enabled", false)?;
            continue;
        };
        set_bool(entry, "enabled", true)?;
        set_bool(entry, "visible", true)?;
        bind_text(entry.child_mut("from")?, &format!("{vm}.o{i}.label"))?;
        hide(entry.child_mut("subject")?)?;
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
    t.slot_mut(CHILDREN)?.push(pills(&vm, p.rows.len())?);
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
/// title, its entries are the rows (the label, with the value in a pill between arrows under it), and the reading
/// pane shows the focused row's label and description. [`BASE`] gets a hidden widget on the [`ROWS`] branch that
/// [`build_scene`] adds.
pub fn build_ui(data: &[u8], pages: &[Page]) -> Result<Vec<u8>> {
    edit(data, |pkg| {
        let list = templates(pkg)?;
        let stock = list
            .iter_mut()
            .find(|t| t.is_node() && t.name() == BASE)
            .ok_or_else(|| err(format!("the laptop page file has no {BASE}")))?;
        let base = stock.clone();
        stock.slot_mut(CHILDREN)?.push(hidden(ROWS, &[ROWS]));
        fix_ids(stock, None);
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
