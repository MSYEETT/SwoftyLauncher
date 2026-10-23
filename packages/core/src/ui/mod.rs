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
const FLOWS: &str = "eaid://ui_package/flows.ui";
const REGIONS: &str = "eaid://ui_package/region_loading.uiconfig";
const TOOLTIPS: &str = "eaid://ui_package/databases/tool_tips.uidb";
const UI_DATA: &str = "eaid://ui_package/databases/ui_data.uidb";
const MESSAGES: &str = "eaid://ui_package/message_data/onlineservices_messages.uimsg";
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
}

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
    let built = pages::build_all(&src, &TILES, &flow)?;
    let names: Vec<String> = built.order.iter().map(|(n, _)| n.clone()).collect();
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
                    let b = patch::routes(d, patch::route_groups(&pages))?;
                    *routes_out.borrow_mut() = b.clone();
                    Ok(b)
                }),
            ),
            (
                FLOWS,
                Box::new(|d| {
                    let b = patch::flow_entries(d, OPTIONS_FLOW, &names)?;
                    *flows_out.borrow_mut() = b.clone();
                    Ok(b)
                }),
            ),
            (REGIONS, Box::new(|d| patch::regions(d, &names))),
        ],
    )?;
    let registered: BTreeMap<String, String> = names.iter().map(|n| (n.clone(), FLOW.to_owned())).collect();
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
    write_erp(
        &src_h,
        &dst_h,
        vec![(
            FE_HOME,
            Box::new(move |d| {
                let d = patch::offline_panel(&patch::fe_home(d)?)?;
                if show_panel { Ok(d) } else { patch::hide_panel(&d) }
            }),
        )],
    )?;

    let mut build = MenuBuild::default();
    let eras = era_rows(input, out, &mut build.lines)?;
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
            file: screen_file(name),
            flow: FLOW.into(),
            anchor: Some(ANCHOR.into()),
            view_model: built.view_models.contains(name),
        });
    }
    Ok(build)
}

fn era_rows(
    input: &dyn Fn(&str) -> PathBuf,
    out: &Path,
    lines: &mut Vec<String>,
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
        if era::taken(&ui, &read_frag(&src, era::SCENE, 0)?)? {
            lines.push(format!(
                "ui: the era picker in {rel} already has an Addons row from another addon; left as it is"
            ));
            continue;
        }
        let title = &title;
        write_archive(
            &src,
            &dst,
            vec![
                (era::UI, era::UI_FRAGMENT, Box::new(era::build_ui)),
                (era::SCENE, 0, Box::new(move |d| era::build_scene(d, title))),
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
