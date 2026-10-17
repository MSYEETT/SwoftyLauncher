use swofty_formats::pkg::{Node, Obj, Val};

use super::node::*;
use super::pages::{HOME, STATE};
use crate::Result;

pub const ACTION: &str = "Menu Right Stick Click";
const ACTION_PREFIX: &str = "#input_data.action_sets.menu_actions.actions.";
pub const MOD_MENU: &str = "mod_menu";
pub const MOD_MENU_OFFLINE: &str = "mod_menu_offline";
pub const OFFLINE_TEXT: &str = "Launched with Swofty Launcher";
pub const OFFLINE_PROMPT: &str = "to open settings";
pub const PROMPT_LABEL: &str = "Addon Settings";
const PANEL: &str = "fe_home.contextual_panels.offline_contextual_panel";
const UIDM: &str = "persistent.consume_input_wrapper.persistent_multichoice_message";
const ONCE_ATTR: &str = "swofty_once";
const BUTTON: &str = "lng_continue";
const ONLINE_MESSAGES: &[&str] = &[
    "online_services_generic_exception_online",
    "online_services_generic_exception_online_with_code",
    "online_services_generic_exception_offline",
    "online_services_generic_exception_offline_with_code",
];

pub fn popup_seen() -> String {
    format!("{STATE}.online_popup_seen")
}

pub struct Popup {
    pub title: String,
    pub body: String,
}

impl Popup {
    pub fn for_version(version: &str) -> Self {
        Self {
            title: format!("Swofty Mod v{version}"),
            body: format!(
                "Online services are off while mods are loaded, so multiplayer is unavailable.\n\n\
                 New in v{version}:\n\
                 - ADDON SETTINGS on the home screen (F9 / R3)\n\
                 - Addon settings page listing installed addons\n\
                 - Launcher panel on the home screen"
            ),
        }
    }
}

fn signal(to: &str, pin: &str, from: &str, attrs: &[(&str, &str)]) -> Node {
    let attrs: Vec<(&str, Attr)> = attrs.iter().map(|(k, v)| (*k, Attr::from(*v))).collect();
    sig_with(to, pin, from, &attrs)
}

fn sig_to(s: &Node) -> (&str, &str) {
    let o = s.as_obj();
    let get = |k| {
        o.and_then(|o| o.get(k))
            .and_then(Node::as_val)
            .and_then(Val::as_str)
            .unwrap_or_default()
    };
    (get("to_object_name"), get("to_pin_name"))
}

fn event_mut<'n>(n: &'n mut Node, name: &str) -> Result<&'n mut Vec<Node>> {
    let what = n.name().to_owned();
    n.slot_mut(EVENTS)?
        .iter_mut()
        .find(|e| e.o().ok().and_then(|o| o.str_at("eventName").ok()) == Some(name))
        .ok_or_else(|| err(format!("{what} has no {name} event")))?
        .o_mut()?
        .at_mut("signals")?
        .a_mut()
}

pub fn fe_home(data: &[u8]) -> Result<Vec<u8>> {
    edit(data, |p| {
        let screen = first_screen(p)?;
        if screen.name() != "fe_home" {
            return Err(err("the home screen file does not hold fe_home"));
        }
        let tooltips = screen.child_mut("tooltips")?;
        let mut tip = clone_node(tooltips.child("settings")?, "fe_home.tooltips", MOD_MENU)?;
        set_prop_value(&mut tip, "tooltipsName", Val::str(MOD_MENU))?;
        set_prop_value(tip.child_mut("text")?, "localise", Val::bool(false))?;
        tooltips.slot_mut(CHILDREN)?.push(tip);
        let mut listener = clone_node(screen.child("settings")?, "fe_home", "mod_menu_listener")?;
        set_prop_value(&mut listener, "action_name", Val::str(ACTION))?;
        let ev = listener
            .slot_mut(EVENTS)?
            .first_mut()
            .ok_or_else(|| err("the settings listener has no events"))?
            .o_mut()?;
        if ev.str_at("eventName")? != "pressed" {
            return Err(err("the settings listener does not start with pressed"));
        }
        *ev.at_mut("signals")?.a_mut()? = vec![
            signal("fe_home.Action", "route", "pressed", &[("key", MOD_MENU)]),
            signal("fe_home.audio_theme_trigger", "confirm", "pressed", &[]),
        ];
        fix_ids(&mut listener, Some("fe_home"));
        screen.slot_mut(CHILDREN)?.push(listener);
        Ok(())
    })
}

pub fn offline_panel(data: &[u8]) -> Result<Vec<u8>> {
    edit(data, |p| {
        let screen = first_screen(p)?;
        let cp = screen.child_mut("contextual_panels")?;
        let panel = cp.child_mut("offline_contextual_panel")?;
        literal_text(panel.child_mut("subtitle")?, OFFLINE_TEXT)?;
        literal_text(panel.child_mut("tooltip_label")?, OFFLINE_PROMPT)?;
        set_prop_value(
            panel.child_mut("tooltip_extra_2")?,
            "tooltipsName",
            Val::str(MOD_MENU_OFFLINE),
        )?;
        let decision = cp.child_mut("contextual_decision")?;
        event_mut(decision, "offline")?
            .retain(|s| sig_to(s).0 != "fe_home.contextual_panels.extra_2_listener");
        event_mut(decision, "default")?.push(signal(PANEL, "show", "default", &[]));
        fix_ids(screen, None);
        Ok(())
    })
}

pub fn hide_panel(data: &[u8]) -> Result<Vec<u8>> {
    edit(data, |p| {
        let screen = first_screen(p)?;
        let dec = screen
            .child_mut("contextual_panels")?
            .child_mut("contextual_decision")?;
        for e in dec.slot_mut(EVENTS)? {
            e.o_mut()?.at_mut("signals")?.a_mut()?.retain(|s| {
                let (to, pin) = sig_to(s);
                !(to.ends_with("offline_contextual_panel") && pin == "show")
            });
        }
        Ok(())
    })
}

fn tooltip_dict(p: &mut swofty_formats::pkg::Pkg) -> Result<&mut Obj> {
    root_obj(p)?
        .at_mut("tool_tips")?
        .o_mut()?
        .at_mut("Contents")?
        .o_mut()?
        .at_mut("tooltips_dictionary")?
        .o_mut()?
        .at_mut("Contents")?
        .o_mut()
}

fn add_tooltip(
    data: &[u8],
    name: &str,
    src: &str,
    label: &str,
    action: Option<&str>,
    priority: Option<&str>,
) -> Result<Vec<u8>> {
    edit(data, |p| {
        let d = tooltip_dict(p)?;
        if d.get(name).is_some() {
            return Err(err(format!("the tooltips already have {name}")));
        }
        let mut e = d.at(src)?.clone();
        let eo = e.o_mut()?;
        set_str(eo, "Name", name)?;
        let c = eo.at_mut("Contents")?.o_mut()?;
        let mut set = |key: &str, v: &str| set_str(c.at_mut(key)?.o_mut()?, "Value", v);
        if let Some(action) = action {
            set("action_path", &format!("{ACTION_PREFIX}{action}"))?;
        }
        set("text", label)?;
        if let Some(priority) = priority {
            set("priority", priority)?;
        }
        d.set(name, e);
        Ok(())
    })
}

pub fn tooltips(data: &[u8], adjust: &str) -> Result<Vec<u8>> {
    let d = add_tooltip(
        data,
        MOD_MENU,
        "settings_fe_home",
        PROMPT_LABEL,
        Some(ACTION),
        Some("5"),
    )?;
    let d = add_tooltip(&d, MOD_MENU_OFFLINE, "delete", OFFLINE_PROMPT, Some(ACTION), None)?;
    add_tooltip(&d, adjust, "change_driver", "Adjust", None, None)
}

fn route_cmd(kind: &str, id: u32, attrs: &[(&str, &str)], deps: Option<&str>) -> Node {
    let mut a = vec![obj(vec![("Id", s("Id")), ("Value", s(&id.to_string()))])];
    a.extend(
        attrs
            .iter()
            .map(|(k, v)| obj(vec![("Id", s(k)), ("Value", s(v))])),
    );
    let mut items = vec![("Attributes", arr(a)), ("Type", s(kind))];
    if let Some(d) = deps {
        items.push(("Dependencies", s(d)));
    }
    obj(items)
}

fn route(id: &str, cmds: Vec<Node>, deps: &str, kind: i32, destination: Option<&str>) -> Node {
    let mut items = vec![("Id", s(id)), ("Commands", arr(cmds))];
    if let Some(dst) = destination {
        items.push((
            "Attributes",
            arr(vec![obj(vec![("Id", s("Destination")), ("Value", s(dst))])]),
        ));
    }
    items.push(("Type", i32v(kind)));
    items.push(("Dependencies", s(deps)));
    obj(items)
}

fn screen_routes(name: &str) -> Vec<Node> {
    let open = |n| route_cmd("OpenScreen", n, &[("Target", name)], None);
    vec![
        route(
            "open",
            vec![
                open(1),
                route_cmd("InputState", 2, &[], Some("1")),
                route_cmd(
                    "TransitionScreen",
                    3,
                    &[("Direction", "in"), ("Target", name)],
                    Some("1"),
                ),
            ],
            "2,3",
            0,
            None,
        ),
        route(
            "open_no_input",
            vec![
                open(1),
                route_cmd(
                    "TransitionScreen",
                    2,
                    &[("Direction", "in"), ("Target", name)],
                    Some("1"),
                ),
            ],
            "2",
            0,
            None,
        ),
        route(
            "close",
            vec![
                route_cmd(
                    "TransitionScreen",
                    1,
                    &[("Direction", "out"), ("Target", name)],
                    None,
                ),
                route_cmd("CloseScreen", 2, &[("Target", name)], Some("1")),
            ],
            "2",
            0,
            None,
        ),
    ]
}

fn subpage_route(id: &str, src: &str, dst: &str, kind: i32) -> Node {
    route(
        id,
        vec![
            route_cmd(
                "TriggerSubRoute",
                1,
                &[("SubRoute", "close"), ("Target", src), ("To", dst)],
                None,
            ),
            route_cmd(
                "TriggerSubRoute",
                2,
                &[
                    ("SubRoute", "open"),
                    ("Target", dst),
                    ("From", src),
                    ("Message", ""),
                ],
                Some("1"),
            ),
        ],
        "2",
        kind,
        Some(dst),
    )
}

fn migrate_route(id: &str, src: &str, flow: &str, dst: &str) -> Node {
    route(
        id,
        vec![
            route_cmd(
                "TriggerSubRoute",
                1,
                &[("SubRoute", "close"), ("Target", src), ("To", dst)],
                None,
            ),
            route_cmd(
                "Migrate",
                2,
                &[("Flow", flow), ("SkipActivator", "true")],
                Some("1"),
            ),
            route_cmd(
                "TriggerSubRoute",
                3,
                &[
                    ("SubRoute", "open_no_input"),
                    ("Target", dst),
                    ("From", src),
                    ("Message", ""),
                ],
                Some("2"),
            ),
        ],
        "3",
        2,
        Some(dst),
    )
}

fn route_group(name: &str, routes: Vec<Node>) -> Node {
    obj(vec![
        ("Id", s(name)),
        ("Routes", arr(routes)),
        (
            "Attributes",
            arr(vec![obj(vec![("Id", s("Source")), ("Value", s(name))])]),
        ),
    ])
}

pub fn route_groups(pages: &[(String, String)]) -> Vec<Node> {
    let mut home = screen_routes(HOME);
    home.push(migrate_route("back", HOME, "frontend_flow", "fe_home"));
    home.extend(pages.iter().map(|(rid, s)| subpage_route(rid, HOME, s, 1)));
    let mut groups = vec![route_group(HOME, home)];
    for (_, s) in pages {
        let mut r = screen_routes(s);
        r.push(subpage_route("back", s, HOME, 2));
        groups.push(route_group(s, r));
    }
    groups
}

fn id_of(n: &Node) -> Option<&str> {
    n.as_obj()?.get("Id")?.as_val()?.as_str()
}

fn mod_menu_route(groupings: &[Node]) -> Result<Node> {
    let home = groupings
        .iter()
        .find(|g| id_of(g) == Some("fe_home"))
        .ok_or_else(|| err("the routes have no fe_home grouping"))?;
    let mut r = home
        .o()?
        .at("Routes")?
        .a()?
        .iter()
        .find(|r| id_of(r) == Some("options"))
        .cloned()
        .ok_or_else(|| err("fe_home has no options route"))?;
    set_str(r.o_mut()?, "Id", MOD_MENU)?;
    replace_strings(&mut r, &|v| match v {
        "settings_container" => Some(HOME.into()),
        "open_no_input" => Some("open".into()),
        _ => None,
    });
    Ok(r)
}

pub fn routes(data: &[u8], groups: Vec<Node>) -> Result<Vec<u8>> {
    edit(data, |p| {
        let gs = root_obj(p)?.at_mut("RouteGroupings")?.a_mut()?;
        let replace = mod_menu_route(gs)?;
        for g in groups {
            if gs.iter().any(|x| id_of(x) == id_of(&g)) {
                return Err(err(format!(
                    "route group {} exists already",
                    id_of(&g).unwrap_or_default()
                )));
            }
            gs.push(g);
        }
        let rid = id_of(&replace).unwrap_or_default().to_owned();
        let grp = gs
            .iter_mut()
            .find(|x| id_of(x) == Some("fe_home"))
            .ok_or_else(|| err("the routes have no fe_home grouping"))?;
        let routes = grp.o_mut()?.at_mut("Routes")?.a_mut()?;
        routes.retain(|r| id_of(r) != Some(&rid));
        routes.push(replace);
        Ok(())
    })
}

pub fn flow_entries(data: &[u8], flow: &str, screens: &[String]) -> Result<Vec<u8>> {
    edit(data, |p| {
        let flows = root_obj(p)?.at_mut("Flows")?.a_mut()?;
        let f = flows
            .iter_mut()
            .find(|f| f.name() == flow)
            .ok_or_else(|| err(format!("flows.ui has no {flow}")))?;
        let ents = f
            .slot_mut(CHILDREN)?
            .first_mut()
            .ok_or_else(|| err(format!("{flow} has no scenes")))?
            .slot_mut(CHILDREN)?;
        let tmpl = ents
            .first()
            .cloned()
            .ok_or_else(|| err(format!("{flow} has no entries")))?;
        for screen in screens {
            let entry = format!("enter_{screen}");
            if ents.iter().any(|e| e.name() == entry) {
                return Err(err(format!("{flow} has {entry} already")));
            }
            let mut e = tmpl.clone();
            let p0 = e.props_mut()?;
            set_str(p0, "name", &entry)?;
            set_str(p0.at_mut("screen")?.o_mut()?, "value", screen)?;
            p0.drop_key("command");
            ents.push(e);
        }
        fix_ids(f, None);
        Ok(())
    })
}

pub fn regions(data: &[u8], screens: &[String]) -> Result<Vec<u8>> {
    edit(data, |p| {
        let regs = root_obj(p)?
            .at_mut("regions")?
            .o_mut()?
            .at_mut("Contents")?
            .a_mut()?;
        let reg = regs
            .iter_mut()
            .find(|r| r.o().ok().and_then(|o| o.str_at("Name").ok()) == Some("region_undefined"))
            .ok_or_else(|| err("region_loading has no region_undefined"))?;
        let c = reg.o_mut()?.at_mut("Contents")?.o_mut()?;
        for screen in screens {
            if c.get(screen).is_some() {
                return Err(err(format!("region_undefined has {screen} already")));
            }
            let mut e = c.at("fe_home")?.clone();
            let eo = e.o_mut()?;
            set_str(eo, "Name", screen)?;
            set_str(
                eo,
                "Value",
                &format!("asset_groups/ui_package/screens/{screen}.assetgroup"),
            )?;
            c.set(screen, e);
        }
        Ok(())
    })
}

pub fn ui_data(data: &[u8], entries: Vec<(String, Node)>) -> Result<Vec<u8>> {
    edit(data, |p| {
        let c = root_obj(p)?
            .at_mut("ui_menu_data")?
            .o_mut()?
            .at_mut("Contents")?
            .o_mut()?;
        if c.get("swofty").is_some() {
            return Err(err("ui_menu_data has swofty already"));
        }
        c.set("swofty", uidb_dict("swofty", entries));
        Ok(())
    })
}

fn token(name: &str, value: &str) -> Node {
    obj(vec![
        ("localise", boolv(false)),
        ("value", s(value)),
        ("name", s(name)),
    ])
}

pub fn messages(data: &[u8], popup: &Popup) -> Result<Vec<u8>> {
    edit(data, |p| {
        let mut done = Vec::new();
        for m in root_obj(p)?.at_mut("messages")?.a_mut()? {
            let m = m.o_mut()?;
            let id = m.str_at("id")?.to_owned();
            if !ONLINE_MESSAGES.contains(&id.as_str()) {
                continue;
            }
            set_str(m, "title", "[swofty_title]")?;
            set_str(m, "messageBody", "[swofty_body]")?;
            m.set(
                "titleTokens",
                obj(vec![("swofty_title", token("swofty_title", &popup.title))]),
            );
            m.set(
                "bodyTokens",
                obj(vec![("swofty_body", token("swofty_body", &popup.body))]),
            );
            *m.at_mut("options")?.a_mut()? = vec![obj(vec![
                ("value", s(BUTTON)),
                ("response", s("cancel")),
                ("defaultSelection", boolv(true)),
            ])];
            m.at_mut("attributes")?.a_mut()?.push(obj(vec![
                ("localise", boolv(false)),
                ("value", s("true")),
                ("name", s(ONCE_ATTR)),
                ("dbEntry", s("")),
            ]));
            done.push(id);
        }
        done.sort();
        let mut want: Vec<&str> = ONLINE_MESSAGES.to_vec();
        want.sort();
        if done != want {
            return Err(err(format!(
                "the online services messages changed: found {done:?}"
            )));
        }
        Ok(())
    })
}

fn multichoice(p: &mut swofty_formats::pkg::Pkg) -> Result<&mut Node> {
    let scr = first_screen(p)?;
    if scr.name() != "persistent" {
        return Err(err("the persistent screen file does not hold persistent"));
    }
    scr.child_mut("consume_input_wrapper")?
        .child_mut("persistent_multichoice_message")
}

fn relabel(sigs: &[Node], from: &str) -> Result<Vec<Node>> {
    let mut out = sigs.to_vec();
    for s in &mut out {
        set_str(s.o_mut()?, "from_pin_name", from)?;
    }
    Ok(out)
}

fn persistent_gate_once(data: &[u8], seen: &str) -> Result<Vec<u8>> {
    edit(data, |p| {
        let m = multichoice(p)?;
        let act = event_mut(m, "activate_message")?;
        let names: Vec<(String, String)> = act
            .iter()
            .map(|s| {
                let (a, b) = sig_to(s);
                (a.to_owned(), b.to_owned())
            })
            .collect();
        let want = [
            (
                "persistent.consume_input_wrapper".to_owned(),
                "consumeStart".to_owned(),
            ),
            (UIDM.to_owned(), "show".to_owned()),
        ];
        if names != want {
            return Err(err(format!("activate_message changed: {names:?}")));
        }
        let show = act.clone();
        *act = vec![sig(
            &format!("{UIDM}.swofty_once_check"),
            "is_valid",
            "activate_message",
        )];
        let hidden_props = || {
            vec![
                ("enabled", prop(boolv(false), None)),
                ("visible", prop(boolv(false), None)),
            ]
        };
        let mut once = hidden_props();
        once.push((
            "dbEntry",
            prop(s(""), Some(&format!("user_attributes.{ONCE_ATTR}"))),
        ));
        let mut check = hidden_props();
        check.push(("value", prop(boolv(true), None)));
        check.push(("dbEntry", prop(s(seen), None)));
        let mut mark = hidden_props();
        mark.push(("source", prop(boolv(true), None)));
        mark.push(("target", prop(boolv(false), Some(seen))));
        let mut no_seen = vec![sig(&format!("{UIDM}.swofty_mark_seen"), "trigger", "no")];
        no_seen.extend(relabel(&show, "no")?);
        let kids = vec![
            node(
                "swofty_once_check",
                "DBVV",
                once,
                vec![
                    event(
                        "yes",
                        vec![sig(&format!("{UIDM}.swofty_seen_check"), "query", "yes")],
                    ),
                    event("no", relabel(&show, "no")?),
                ],
                &[],
                &[],
            ),
            node(
                "swofty_seen_check",
                "SVCD",
                check,
                vec![
                    event("no", no_seen),
                    event(
                        "yes",
                        vec![
                            sig(UIDM, "deactivate_message", "yes"),
                            sig(&format!("{UIDM}.disable_input"), "trigger", "yes"),
                        ],
                    ),
                ],
                &[],
                &[],
            ),
            node("swofty_mark_seen", "UIDS", mark, vec![], &[], &[]),
        ];
        m.slot_mut(CHILDREN)?.extend(kids);
        fix_ids(first_screen(p)?, None);
        Ok(())
    })
}

pub fn persistent_gate(data: &[u8], popup: &Popup) -> Result<Vec<u8>> {
    let seen = popup_seen();
    let data = persistent_gate_once(data, &seen)?;
    edit(&data, |p| {
        let m = multichoice(p)?;
        let local = |on: bool| {
            vec![
                sig_with(
                    &format!("{UIDM}.title"),
                    "setLocalise",
                    "no",
                    &[("localise", Attr::Bool(on))],
                ),
                sig_with(
                    &format!("{UIDM}.body"),
                    "setLocalise",
                    "no",
                    &[("localise", Attr::Bool(on))],
                ),
            ]
        };
        let restore = local(true);
        let literal = vec![
            sig_with(
                &format!("{UIDM}.title"),
                "setLocalise",
                "no",
                &[("localise", Attr::Bool(false))],
            ),
            sig_with(
                &format!("{UIDM}.title"),
                "setText",
                "no",
                &[("text", popup.title.as_str().into())],
            ),
            sig_with(
                &format!("{UIDM}.body"),
                "setLocalise",
                "no",
                &[("localise", Attr::Bool(false))],
            ),
            sig_with(
                &format!("{UIDM}.body"),
                "setText",
                "no",
                &[("text", popup.body.as_str().into())],
            ),
        ];
        let once = event_mut(m.child_mut("swofty_once_check")?, "no")?;
        let mut sigs = restore;
        sigs.append(once);
        *once = sigs;
        event_mut(m.child_mut("swofty_seen_check")?, "no")?.extend(literal);
        fix_ids(first_screen(p)?, None);
        Ok(())
    })
}
