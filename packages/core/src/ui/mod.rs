pub mod career_select;
pub mod catalog;
pub mod check;
pub mod era;
pub mod idf;
pub mod node;
pub mod pages;
pub mod patch;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use swofty_formats::erp::{Entry, ErpFile, Resource};

use crate::builtin::CORE_ID;
use crate::settings::{SessionScreen, SessionSettings, UI_DIR, screen_file};
use crate::{Error, Result, io_err};
use node::err;
use pages::{ADJUST_TOOLTIP, HOME, Sources, TILES};
use patch::Popup;

pub const UI: &str = "2025_asset_groups/ui_package";
pub const FLOW: &str = "flow_options";
pub const ANCHOR: &str = "settings_container";
const OPTIONS_FLOW: &str = "options_flow";
const FE_HOME: &str = "eaid://ui_package/screens/fe_home.ui";
const FE_CAREER: &str = "eaid://ui_package/screens/fe_career.ui";
const CONTAINER: &str = "eaid://ui_package/screens/settings_container.ui";
const ROUTES: &str = "eaid://ui_package/routes/routes_frontend_general.uir";
const GAME_ROUTES: &str = "eaid://ui_package/routes/routes.uir";
const FLOWS: &str = "eaid://ui_package/flows.ui";
const REGIONS: &str = "eaid://ui_package/region_loading.uiconfig";
const TOOLTIPS: &str = "eaid://ui_package/databases/tool_tips.uidb";
const UI_DATA: &str = "eaid://ui_package/databases/ui_data.uidb";
const MESSAGES: &str = "eaid://ui_package/message_data/onlineservices_messages.uimsg";
const HUD_MESSAGES: &str = "eaid://ui_package/message_data/hud_messages.uimsg";
const PERSISTENT_UI: &str = "eaid://ui_package/screens/persistent.ui";

pub const ERPS: &[&str] = &[
    "persistent.erp",
    "databases.erp",
    "flow_persistent.erp",
    "flow_frontend.erp",
    era::ARCHIVES[0],
    era::ARCHIVES[1],
];
pub const READS: &[&str] = &["flow_options.erp"];

pub fn ui_file(rel: &str) -> String {
    format!("{UI}/{rel}")
}

#[derive(Debug, Clone, Default)]
pub struct MenuBuild {
    pub erps: BTreeMap<String, PathBuf>,
    pub screens: BTreeMap<String, PathBuf>,
    pub session: Vec<SessionScreen>,
    pub strings: Vec<(String, String)>,
    pub lines: Vec<String>,
    pub warnings: Vec<String>,
}

/// The addon whose copy of a resource (second) in a game archive (first)
/// the build reads; `None` for the game's own copy.
pub type Writer<'a> = dyn Fn(&str, &str) -> Option<String> + 'a;

type Patch<'a> = Box<dyn FnOnce(&[u8]) -> Result<Vec<u8>> + 'a>;

fn read_frag(path: &Path, name: &str, index: usize) -> Result<Vec<u8>> {
    let file = ErpFile::open(path)?;
    let res = file.load(file.erp.find(name)?)?;
    Ok(res
        .fragments
        .get(index)
        .ok_or_else(|| err(format!("{name} has no fragment {index}")))?
        .data()?)
}

fn write_erp(src: &Path, dst: &Path, patches: Vec<(&str, Patch)>) -> Result<()> {
    let patches = patches.into_iter().map(|(name, f)| (name, 0, f)).collect();
    write_archive(src, dst, patches, Vec::new())
}

type Added = (String, &'static str, Vec<Vec<u8>>);

fn write_archive(
    src: &Path,
    dst: &Path,
    patches: Vec<(&str, usize, Patch)>,
    added: Vec<Added>,
) -> Result<()> {
    let file = ErpFile::open(src)?;
    let mut owned: HashMap<String, Resource> = HashMap::new();
    for (name, index, f) in patches {
        let mut res = match owned.remove(name) {
            Some(r) => r,
            None => file.load(file.erp.find(name)?)?,
        };
        let frag = res
            .fragments
            .get(index)
            .ok_or_else(|| err(format!("{name} has no fragment {index}")))?;
        let data = f(&frag.data()?)?;
        res.set_fragment(index, &data)?;
        owned.insert(name.to_owned(), res);
    }
    let mut extra = Vec::new();
    for (name, kind, data) in added {
        let want = swofty_formats::erp::kind(kind)?;
        let like = file
            .erp
            .resources
            .iter()
            .find(|r| r.kind == want && r.fragments.len() == data.len())
            .ok_or_else(|| {
                err(format!(
                    "{}: no {kind} resource to take {name}'s layout from",
                    src.display()
                ))
            })?;
        let res = Resource::like(&name, like, &data)?;
        match file.erp.resources.iter().any(|r| r.name == name) {
            true => {
                owned.insert(name, res);
            }
            false => extra.push(Entry::Owned(res)),
        }
    }
    let mut entries: Vec<Entry> = file
        .erp
        .resources
        .iter()
        .map(|r| match owned.remove(&r.name) {
            Some(new) => Entry::Owned(new),
            None => Entry::Copy(r),
        })
        .collect();
    entries.extend(extra);
    let dir = dst.parent().expect("has parent");
    std::fs::create_dir_all(dir).map_err(io_err(dir))?;
    file.write(entries, dst)?;
    Ok(())
}

fn panel_shown(settings: &SessionSettings) -> bool {
    settings
        .addons
        .iter()
        .filter(|m| m.id == CORE_ID)
        .flat_map(|m| &m.options)
        .find(|o| o.id == "show_launcher_panel")
        .and_then(|o| o.states.get(o.current))
        .is_none_or(|s| s.value != serde_json::Value::Bool(false))
}

fn core_version(settings: &SessionSettings) -> String {
    settings
        .addons
        .iter()
        .find(|m| m.id == CORE_ID)
        .map(|m| m.version.clone())
        .unwrap_or_else(|| crate::builtin::core_manifest().version.to_string())
}

pub fn build(
    settings: &SessionSettings,
    input: &dyn Fn(&str) -> PathBuf,
    writer: &Writer,
    out: &Path,
    lang: bool,
) -> Result<MenuBuild> {
    let flow_options = input(&ui_file("flow_options.erp"));
    let flow_frontend = input(&ui_file("flow_frontend.erp"));
    let src = Sources::parse(
        &read_frag(&flow_options, CONTAINER, 1)?,
        &read_frag(&flow_frontend, FE_CAREER, 0)?,
        &read_frag(&flow_frontend, FE_HOME, 0)?,
        &read_frag(&flow_options, CONTAINER, 0)?,
    )?;
    let flow = pages::entries(&settings.addons, &settings.maps, &settings.mods);
    let mut built = pages::build_all(&src, &TILES, &flow)?;
    // Addon screens with page: options are their addon's options page under the name
    // swofty_addon_<name>_<addon index>; the view model reads the index after the last '_'.
    let mut generated: Vec<(String, String, String)> = Vec::new();
    for s in settings.ui.iter().flat_map(|u| &u.screens) {
        if s.page.as_deref() != Some("options") {
            continue;
        }
        let e = flow
            .iter()
            .find(|e| e.kind == pages::Kind::Addon && Some(e.id) == s.addon.as_deref())
            .ok_or_else(|| err(format!("screen {}: its addon is not on", s.name)))?;
        let full = e.screen.trim_start_matches("swofty_addon_");
        let screen = format!("swofty_addon_{}_{full}", s.name);
        let anchor = s.anchor.clone().unwrap_or_else(|| ANCHOR.to_owned());
        built
            .order
            .push((screen.clone(), pages::build_options_page(&src, e, &screen)?));
        if !e.options.is_empty() {
            built.view_models.push(screen.clone());
        }
        generated.push((s.name.clone(), anchor, screen));
    }
    let names: Vec<String> = built.order.iter().map(|(n, _)| n.clone()).collect();
    // Addon screens in the options flow get the flow entry, region and route group an ADDON
    // SETTINGS page gets, so routes and signals to them open them.
    let addon_routes: Vec<(String, String)> = settings
        .ui
        .iter()
        .flat_map(|u| &u.screens)
        .filter(|s| s.flow == FLOW && s.page.is_none() && !names.contains(&s.name))
        .map(|s| {
            (
                s.name.clone(),
                s.anchor.clone().unwrap_or_else(|| ANCHOR.to_owned()),
            )
        })
        .collect();
    let addon_screens: Vec<String> = addon_routes.iter().map(|(n, _)| n.clone()).collect();
    let mut opened: Vec<(String, String, String)> = addon_routes
        .iter()
        .map(|(n, a)| (n.clone(), a.clone(), n.clone()))
        .collect();
    opened.extend(generated.iter().cloned());
    let mut addon_groups = addon_routes.clone();
    addon_groups.extend(generated.iter().map(|(_, a, s)| (s.clone(), a.clone())));
    let listed: Vec<String> = names.iter().chain(&addon_screens).cloned().collect();
    let mut problems = check::screens(&built.order, &built.view_models)?;
    let no_scenes = catalog::Scenes::default();
    for (name, data) in &built.order {
        let input = check::UiInput {
            resource: name,
            frag: "temp",
            data,
            stock: None,
        };
        let report = check::ui_edit(&input, &no_scenes, catalog::stock(), catalog::engine())?;
        problems.extend(report.hard.iter().map(|h| h.to_string()));
    }

    let pages: Vec<(String, String)> = flow
        .iter()
        .map(|e| (format!("open_{}_{}", e.kind.key(), e.index), e.screen.clone()))
        .collect();
    let mut groups: Vec<&str> = vec![HOME, "fe_home.mod_menu"];
    groups.extend(pages.iter().map(|(_, s)| s.as_str()));
    let state = pages::state_entries(&flow)?;
    let popup = Popup::for_version(&core_version(settings));
    let show_panel = panel_shown(settings);

    let erp = |rel: &str| (input(&ui_file(rel)), out.join(ui_file(rel)));
    let (src_p, dst_p) = erp("persistent.erp");
    let routes_out = std::cell::RefCell::new(Vec::new());
    let flows_out = std::cell::RefCell::new(Vec::new());
    write_erp(
        &src_p,
        &dst_p,
        vec![
            (
                ROUTES,
                Box::new(|d| {
                    let b = patch::routes(
                        d,
                        patch::route_groups(&pages),
                        patch::screen_route_groups(&addon_groups),
                    )?;
                    *routes_out.borrow_mut() = b.clone();
                    Ok(b)
                }),
            ),
            (
                FLOWS,
                Box::new(|d| {
                    let b = patch::flow_entries(d, OPTIONS_FLOW, &listed)?;
                    *flows_out.borrow_mut() = b.clone();
                    Ok(b)
                }),
            ),
            (REGIONS, Box::new(|d| patch::regions(d, &listed))),
            (GAME_ROUTES, Box::new(|d| patch::anchor_routes(d, &opened))),
        ],
    )?;
    let registered: BTreeMap<String, String> = listed.iter().map(|n| (n.clone(), FLOW.to_owned())).collect();
    problems.extend(check::routes(
        &routes_out.borrow(),
        &flows_out.borrow(),
        &groups,
        &registered,
    )?);
    if !problems.is_empty() {
        return Err(Error::Other(format!(
            "ADDON SETTINGS failed its checks:\n  {}",
            problems.join("\n  ")
        )));
    }

    let (src_d, dst_d) = erp("databases.erp");
    write_erp(
        &src_d,
        &dst_d,
        vec![
            (TOOLTIPS, Box::new(|d| patch::tooltips(d, ADJUST_TOOLTIP))),
            (UI_DATA, Box::new(move |d| patch::ui_data(d, state))),
        ],
    )?;
    let (src_f, dst_f) = erp("flow_persistent.erp");
    write_erp(
        &src_f,
        &dst_f,
        vec![
            (MESSAGES, Box::new(|d| patch::messages(d, lang))),
            (HUD_MESSAGES, Box::new(patch::hud_messages)),
            (
                PERSISTENT_UI,
                Box::new(|d| {
                    let out = patch::persistent_gate(d)?;
                    let changed = check::stock_kept(d, &out, "swofty_")?;
                    if !changed.is_empty() {
                        return Err(Error::Other(format!(
                            "the popup gate changes the game's own popups:\n  {}",
                            changed.join("\n  ")
                        )));
                    }
                    Ok(out)
                }),
            ),
        ],
    )?;
    let (src_h, dst_h) = erp("flow_frontend.erp");
    let (mut career_lines, mut career_warnings) = (Vec::new(), Vec::new());
    let career = career_row(&src_h, writer, &mut career_lines, &mut career_warnings)?;
    let mut frontend: Vec<(&str, usize, Patch)> = vec![(
        FE_HOME,
        0,
        Box::new(move |d| {
            let d = patch::offline_panel(&patch::fe_home(d)?)?;
            if show_panel { Ok(d) } else { patch::hide_panel(&d) }
        }),
    )];
    if let Some((ui, type_ui, scene, templates)) = career {
        frontend.push((career_select::UI, 0, Box::new(move |_| Ok(ui))));
        frontend.push((career_select::TYPE_UI, 0, Box::new(move |_| Ok(type_ui))));
        frontend.push((career_select::SCENE, 0, Box::new(move |_| Ok(scene))));
        frontend.push((
            career_select::TEMPLATES,
            career_select::TEMPLATES_FRAGMENT,
            Box::new(move |_| Ok(templates)),
        ));
    }
    write_archive(&src_h, &dst_h, frontend, Vec::new())?;

    let mut build = MenuBuild::default();
    build.lines.extend(career_lines);
    build.warnings.extend(career_warnings);
    let eras = era_rows(input, writer, out, &mut build.lines, &mut build.warnings)?;
    if lang {
        build.strings = popup.strings();
    }
    for rel in ERPS
        .iter()
        .filter(|r| !era::ARCHIVES.contains(r) || eras.contains(r))
    {
        build.erps.insert(ui_file(rel), out.join(ui_file(rel)));
    }
    for (name, data) in &built.order {
        let rel = format!("{UI_DIR}/{name}.ui");
        let path = out.join(&rel);
        let dir = path.parent().expect("has parent");
        std::fs::create_dir_all(dir).map_err(io_err(dir))?;
        std::fs::write(&path, data).map_err(io_err(&path))?;
        build.screens.insert(rel, path);
        build.session.push(SessionScreen {
            name: name.clone(),
            page: None,
            addon: None,
            file: screen_file(name),
            flow: FLOW.into(),
            anchor: Some(ANCHOR.into()),
            view_model: built.view_models.contains(name),
        });
    }
    Ok(build)
}

type CareerRow = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);

fn career_row(
    src: &Path,
    writer: &Writer,
    lines: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Result<Option<CareerRow>> {
    let ui = read_frag(src, career_select::UI, 0)?;
    let type_ui = read_frag(src, career_select::TYPE_UI, 0)?;
    let scene = read_frag(src, career_select::SCENE, 0)?;
    let templates = read_frag(src, career_select::TEMPLATES, career_select::TEMPLATES_FRAGMENT)?;
    let edit = career_select::taken(&ui, &scene, &templates).and_then(|taken| match taken {
        true => Ok(None),
        false => Ok(Some((
            career_select::build_ui(&ui)?,
            career_select::build_type_ui(&type_ui)?,
            career_select::build_scene(&scene)?,
            career_select::build_templates(&templates)?,
        ))),
    });
    let edit = match edit {
        Ok(edit) => edit,
        Err(e) => {
            let rel = ui_file(career_select::ARCHIVE);
            let by = [
                career_select::UI,
                career_select::TYPE_UI,
                career_select::SCENE,
                career_select::TEMPLATES,
            ]
            .into_iter()
            .find_map(|res| writer(&rel, res));
            let Some(addon) = by else { return Err(e) };
            warnings.push(format!(
                "ui: {addon} has its own copy of the career content select, which Swofty can't edit; addon careers \
                 follow the stock ones there while it's on ({e})"
            ));
            return Ok(None);
        }
    };
    match edit {
        Some(edit) => {
            lines.push("ui: the career content select gets Built In and Addons rows".to_owned());
            Ok(Some(edit))
        }
        None => {
            lines.push(
                "ui: the career content select already has an Addons row from another addon; left as it is"
                    .to_owned(),
            );
            Ok(None)
        }
    }
}

fn era_rows(
    input: &dyn Fn(&str) -> PathBuf,
    writer: &Writer,
    out: &Path,
    lines: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Result<Vec<&'static str>> {
    let world = input(&ui_file(era::ARCHIVES[0]));
    let title = read_frag(&world, era::TITLE_SCENE, 0)?;
    let mut added = Vec::new();
    if era::ROW_FADE {
        let image = format!("{}.image", era::FADE_SOURCE);
        let fade = era::fade(
            &read_frag(&world, era::FADE_SOURCE, 0)?,
            &read_frag(&world, &image, 0)?,
            era::FADE,
        )?;
        added.push((era::FADE.to_owned(), era::SRV_KIND, vec![fade[0].clone()]));
        added.push((
            format!("{}.image", era::FADE),
            era::SURFACE_KIND,
            fade[1..].to_vec(),
        ));
    }
    let mut written = Vec::new();
    for rel in era::ARCHIVES {
        let (src, dst) = (input(&ui_file(rel)), out.join(ui_file(rel)));
        let ui = read_frag(&src, era::UI, era::UI_FRAGMENT)?;
        let scene = read_frag(&src, era::SCENE, 0)?;
        let edit = era::taken(&ui, &scene).and_then(|taken| match taken {
            true => Ok(None),
            false => Ok(Some((era::build_ui(&ui)?, era::build_scene(&scene, &title)?))),
        });
        let edit = match edit {
            Ok(edit) => edit,
            Err(e) => {
                let by = [
                    (rel, era::UI),
                    (rel, era::SCENE),
                    (era::ARCHIVES[0], era::TITLE_SCENE),
                ]
                .into_iter()
                .find_map(|(erp, res)| writer(&ui_file(erp), res));
                let Some(addon) = by else { return Err(e) };
                warnings.push(format!(
                    "ui: {addon} has its own copy of the era picker in {rel}, which Swofty can't edit; the era picker boxes there are off while it's on ({e})"
                ));
                continue;
            }
        };
        let Some((ui, scene)) = edit else {
            lines.push(format!(
                "ui: the era picker in {rel} already has an Addons row from another addon; left as it is"
            ));
            continue;
        };
        write_archive(
            &src,
            &dst,
            vec![
                (era::UI, era::UI_FRAGMENT, Box::new(move |_| Ok(ui))),
                (era::SCENE, 0, Box::new(move |_| Ok(scene))),
            ],
            added.clone(),
        )?;
        written.push(rel);
    }
    if !written.is_empty() {
        let files: Vec<(String, PathBuf, PathBuf)> = written
            .iter()
            .map(|rel| (ui_file(rel), input(&ui_file(rel)), out.join(ui_file(rel))))
            .collect();
        let pairs: Vec<(&str, &Path, &Path)> = files
            .iter()
            .map(|(name, src, dst)| (name.as_str(), src.as_path(), dst.as_path()))
            .collect();
        let report = check::archive_files(&pairs)?;
        if !report.hard.is_empty() {
            let found: Vec<String> = report.hard.iter().map(ToString::to_string).collect();
            return Err(Error::Other(format!(
                "the era picker rows break the game's UI loader rules: {}",
                found.join("; ")
            )));
        }
        lines.push(format!(
            "ui: the Grand Prix and Time Trial era picker gets Built In and Addons rows in {} archive(s)",
            written.len()
        ));
    }
    Ok(written)
}

#[cfg(test)]
mod tests {

    #[test]
    #[ignore = "needs SWOFTY_STOCK_FLOW_FRONTEND, a stock flow_frontend.erp"]
    fn the_career_content_select_gets_its_rows_within_the_loader_rules() {
        let src = std::path::PathBuf::from(std::env::var_os("SWOFTY_STOCK_FLOW_FRONTEND").expect("set"));
        let mut lines = Vec::new();
        let mut warnings = Vec::new();
        let (ui, type_ui, scene, templates) = career_row(&src, &|_, _| None, &mut lines, &mut warnings)
            .unwrap()
            .expect("a stock archive takes the rows");
        let out = std::env::temp_dir().join(format!("swofty-career-row-{}", std::process::id()));
        let dst = out.join("flow_frontend.erp");
        write_archive(
            &src,
            &dst,
            vec![
                (career_select::UI, 0, Box::new(move |_| Ok(ui))),
                (career_select::TYPE_UI, 0, Box::new(move |_| Ok(type_ui))),
                (career_select::SCENE, 0, Box::new(move |_| Ok(scene))),
                (
                    career_select::TEMPLATES,
                    career_select::TEMPLATES_FRAGMENT,
                    Box::new(move |_| Ok(templates)),
                ),
            ],
            Vec::new(),
        )
        .unwrap();
        let name = ui_file(career_select::ARCHIVE);
        let report = check::archive_files(&[(name.as_str(), src.as_path(), dst.as_path())]).unwrap();
        let hard: Vec<String> = report.hard.iter().map(ToString::to_string).collect();
        assert!(hard.is_empty(), "{hard:?}");
        assert_eq!(
            lines,
            ["ui: the career content select gets Built In and Addons rows"]
        );
        std::fs::remove_dir_all(&out).ok();
    }
    use super::*;
    use node::{arr, node, obj, s};
    use swofty_formats::erp::{Erp, Fragment, HEADER_LEN, MAGIC, Resource, STORED, kind};
    use swofty_formats::pkg::{Node, Pkg};

    const MOD: &str = "F1 2012 Season";

    fn pkg(root: Node) -> Vec<u8> {
        Pkg {
            name: *b"TEST",
            root,
            vca: Vec::new(),
            layout: None,
        }
        .to_bytes()
        .unwrap()
    }

    fn ui() -> Vec<u8> {
        pkg(obj(vec![(
            "Templates",
            arr(vec![node("dynamic_era_select", "UIGR", vec![], vec![], &[], &[])]),
        )]))
    }

    fn scene() -> Vec<u8> {
        pkg(obj(vec![(
            "scenelist",
            arr(vec![obj(vec![(
                "scene",
                obj(vec![("name", s("root")), ("children", arr(vec![]))]),
            )])]),
        )]))
    }

    fn resource(name: &str, ty: &str, frags: &[&[u8]]) -> Resource {
        let mut res = Resource {
            name: name.into(),
            kind: kind(ty).unwrap(),
            unk: vec![0; 6],
            fragments: frags
                .iter()
                .map(|d| Fragment {
                    name: *b"temp",
                    offset: 0,
                    size: d.len() as u64,
                    flags: 0,
                    compression: STORED,
                    packed: d.len() as u64,
                    raw: d.to_vec(),
                })
                .collect(),
            hash: [0; 16],
        };
        res.hash = res.content_hash().unwrap();
        res
    }

    fn archive(root: &Path, rel: &str, ui: &[u8]) -> PathBuf {
        let resources = vec![
            resource(era::UI, "UIDF", &[b"head", ui]),
            resource(era::SCENE, "UILayout", &[&scene()]),
            resource(era::TITLE_SCENE, "UILayout", &[&scene()]),
        ];
        write(root, rel, resources)
    }

    fn write(root: &Path, rel: &str, resources: Vec<Resource>) -> PathBuf {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(MAGIC);
        header[4..8].copy_from_slice(&4u32.to_le_bytes());
        let mut erp = Erp {
            header,
            version: 4,
            resources,
        };
        let path = root.join(ui_file(rel));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, erp.to_bytes()).unwrap();
        path
    }

    #[test]
    fn an_addon_era_picker_that_cannot_be_read_is_left_as_it_is() {
        let root = std::env::temp_dir().join(format!("swofty-era-skip-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let full = ui();
        let odd = full[..full.len() / 2].to_vec();
        let why = Pkg::parse(&odd).unwrap_err().to_string();
        let game = root.join("game");
        for rel in era::ARCHIVES {
            archive(&game, rel, &odd);
        }
        let input = |rel: &str| game.join(rel);
        let out = root.join("out");
        let from_mod = |_: &str, res: &str| (res == era::UI).then(|| MOD.to_owned());
        let (mut lines, mut warnings) = (Vec::new(), Vec::new());
        let written = era_rows(&input, &from_mod, &out, &mut lines, &mut warnings).unwrap();
        assert!(written.is_empty());
        assert!(!out.exists(), "nothing is written");
        assert!(lines.is_empty(), "{lines:?}");
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        for (w, rel) in warnings.iter().zip(era::ARCHIVES) {
            assert!(
                w.starts_with(&format!("ui: {MOD} has its own copy of the era picker in {rel},")),
                "{w}"
            );
            assert!(
                w.contains("the era picker boxes there are off while it's on"),
                "{w}"
            );
            assert!(w.contains(&why), "{w}");
        }

        let stock = |_: &str, _: &str| None;
        let e = era_rows(&input, &stock, &out, &mut Vec::new(), &mut Vec::new()).unwrap_err();
        assert!(e.to_string().contains(&why), "{e}");

        let world_only = |erp: &str, res: &str| {
            (erp.ends_with(era::ARCHIVES[0]) && res == era::UI).then(|| MOD.to_owned())
        };
        let mut warnings = Vec::new();
        let e = era_rows(&input, &world_only, &out, &mut Vec::new(), &mut warnings).unwrap_err();
        assert!(e.to_string().contains(&why), "{e}");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains(era::ARCHIVES[0]), "{warnings:?}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_addon_era_picker_without_the_stock_templates_is_left_as_it_is() {
        let root = std::env::temp_dir().join(format!("swofty-era-templates-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let game = root.join("game");
        for rel in era::ARCHIVES {
            archive(&game, rel, &ui());
        }
        let input = |rel: &str| game.join(rel);
        let out = root.join("out");
        let from_mod = |_: &str, _: &str| Some(MOD.to_owned());
        let mut warnings = Vec::new();
        let written = era_rows(&input, &from_mod, &out, &mut Vec::new(), &mut warnings).unwrap();
        assert!(written.is_empty());
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(
            warnings.iter().all(|w| w.contains("time_trial_select not found")),
            "{warnings:?}"
        );
        let stock = |_: &str, _: &str| None;
        let e = era_rows(&input, &stock, &out, &mut Vec::new(), &mut Vec::new()).unwrap_err();
        assert!(e.to_string().contains("time_trial_select not found"), "{e}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_missing_era_picker_resource_stays_an_error() {
        let root = std::env::temp_dir().join(format!("swofty-era-missing-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let game = root.join("game");
        let mut odd = ui();
        odd.extend_from_slice(&[0; 4]);
        archive(&game, era::ARCHIVES[0], &odd);
        write(
            &game,
            era::ARCHIVES[1],
            vec![resource(era::UI, "UIDF", &[b"head", &odd])],
        );
        let input = |rel: &str| game.join(rel);
        let from_mod = |_: &str, _: &str| Some(MOD.to_owned());
        let mut warnings = Vec::new();
        let e = era_rows(
            &input,
            &from_mod,
            &root.join("out"),
            &mut Vec::new(),
            &mut warnings,
        )
        .unwrap_err();
        assert!(e.to_string().contains(era::SCENE), "{e}");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        std::fs::remove_dir_all(&root).ok();
    }
}
