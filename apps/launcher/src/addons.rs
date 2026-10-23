//! The Addons page: install cards, filters, the list in load order and the
//! selected addon's details and conflicts.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Disableable, Icon, Sizable, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::conflicts::{self, Pair, Report};
use swofty_core::import::{self, Route};
use swofty_core::manifest::{Category, Kind, MANIFEST_FILE};
use swofty_core::profile::VANILLA;

use crate::library::{AddonItem, Changes, Source, plural};
use crate::pages::{GUTTER, addon_icon, badge, chevron, fact, label, logo, tag, track_badge};
use crate::shell::{Page, Shell};
use crate::theme::{Palette, palette};

const FADE: Duration = Duration::from_secs(4);
const FILTERS_W: f32 = 226.;
const DETAIL_W: f32 = 380.;
const SPLIT_AT: f32 = 1000.;
const ITEMS_SHOWN: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Type {
    Database,
    Archives,
    Text,
    Simplified,
    Files,
    Code,
}

impl Type {
    const ALL: [Type; 6] = [
        Self::Database,
        Self::Archives,
        Self::Text,
        Self::Simplified,
        Self::Files,
        Self::Code,
    ];

    fn of(kind: Kind) -> Self {
        match kind {
            Kind::Database => Self::Database,
            Kind::Erp => Self::Archives,
            Kind::Text | Kind::Ui => Self::Text,
            Kind::Simplified => Self::Simplified,
            Kind::Files => Self::Files,
            Kind::Native => Self::Code,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Database => "Database",
            Self::Archives => "Archives",
            Self::Text => "Text & UI",
            Self::Simplified => "Simplified",
            Self::Files => "Files",
            Self::Code => "Code",
        }
    }

    /// An addon's types in [`Type::ALL`] order, as "Database, Archives".
    fn list(m: &AddonItem) -> String {
        let types: BTreeSet<Type> = m.manifest.kind.iter().map(|k| Self::of(*k)).collect();
        types.into_iter().map(Self::label).collect::<Vec<_>>().join(", ")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum State {
    On,
    Off,
    Conflicts,
    Update,
}

impl State {
    const ALL: [State; 4] = [Self::On, Self::Off, Self::Conflicts, Self::Update];

    fn label(self) -> &'static str {
        match self {
            Self::On => "On",
            Self::Off => "Off",
            Self::Conflicts => "Has conflicts",
            Self::Update => "Update available",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    Category,
    Type,
    Source,
    State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Facet {
    Category(Option<Category>),
    Type(Type),
    Source(Source),
    State(State),
}

impl Facet {
    fn group(self) -> Group {
        match self {
            Self::Category(_) => Group::Category,
            Self::Type(_) => Group::Type,
            Self::Source(_) => Group::Source,
            Self::State(_) => Group::State,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Category(Some(c)) => c.label(),
            Self::Category(None) => "Uncategorised",
            Self::Type(t) => t.label(),
            Self::Source(s) => s.label(),
            Self::State(s) => s.label(),
        }
    }
}

/// Addons that load first, in this order: SwoftyMod, then Simplified for Swofty.
/// Nothing is placed above them.
const PINNED: [&str; 2] = [swofty_core::builtin::CORE_ID, swofty_core::managed::ID];

/// How far `id`, at `from` among `addons`, moves to land on `to`, kept below
/// the pinned addons.
fn pinned_delta(addons: &[swofty_core::profile::ProfileAddon], id: &str, from: usize, to: usize) -> isize {
    let floor = addons
        .iter()
        .filter(|a| a.id != id)
        .enumerate()
        .filter(|(_, a)| PINNED.contains(&a.id.as_str()))
        .last()
        .map_or(0, |(i, _)| i + 1);
    to.max(floor) as isize - from as isize
}

/// The experimental features an addon turns on, as "Overlay".
fn experimental_labels(m: &AddonItem) -> String {
    m.manifest
        .experimental_features
        .iter()
        .map(|f| f.label())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The icon an addon without its own shows: a race track for tracks, else a box.
fn fallback_icon(m: &AddonItem, size: f32, p: Palette) -> Div {
    match m.category() {
        Some(Category::Tracks) => track_badge(size, p),
        _ => badge(IconName::Package, size, p),
    }
}

#[derive(Clone)]
struct DraggedAddon {
    id: String,
    name: SharedString,
}

impl Render for DraggedAddon {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        h_flex()
            .px(px(10.))
            .py(px(6.))
            .gap(px(6.))
            .rounded(px(6.))
            .bg(p.elevated)
            .border_1()
            .border_color(p.accent)
            .text_size(px(13.))
            .text_color(p.text)
            .child(
                Icon::new(IconName::GripVertical)
                    .size(px(13.))
                    .text_color(p.text_muted),
            )
            .child(self.name.clone())
    }
}

pub(crate) struct AddonsPage {
    search: Entity<InputState>,
    window: AnyWindowHandle,
    facets: BTreeSet<Facet>,
    scroll: ScrollHandle,
    seen: Option<Instant>,
    scroll_pending: bool,
    animating: bool,
    report: Option<Report>,
    read_at: Option<Instant>,
    pub(crate) checking: bool,
    open_pairs: HashSet<(String, String)>,
    details: HashSet<(String, String)>,
    all_items: HashSet<(String, String)>,
    advanced: bool,
    changes: HashMap<String, Changes>,
    _search: Subscription,
}

impl AddonsPage {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Shell>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search addons"));
        let changed = cx.subscribe(&search, |_, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            search,
            window: window.window_handle(),
            facets: BTreeSet::new(),
            scroll: ScrollHandle::new(),
            seen: None,
            scroll_pending: false,
            animating: false,
            report: None,
            read_at: None,
            checking: false,
            open_pairs: HashSet::new(),
            details: HashSet::new(),
            all_items: HashSet::new(),
            advanced: false,
            changes: HashMap::new(),
            _search: changed,
        }
    }

    fn clear_search(&self, cx: &mut App) {
        let (input, window) = (self.search.clone(), self.window);
        cx.defer(move |cx| {
            window
                .update(cx, |_, window, cx| {
                    input.update(cx, |s, cx| s.set_value("", window, cx))
                })
                .ok();
        });
    }
}

fn matches_query(m: &AddonItem, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let man = &m.manifest;
    [
        Some(&man.name),
        Some(&man.id),
        Some(&man.author),
        man.description.as_ref(),
    ]
    .into_iter()
    .flatten()
    .any(|s| s.to_lowercase().contains(query))
}

fn pair_counts(pair: &Pair) -> String {
    let strings = pair.strings;
    let parts: Vec<String> = [
        (pair.database, "database value", "database values"),
        (pair.resources, "game asset", "game assets"),
        (pair.files, "file", "files"),
        (strings, "text string", "text strings"),
    ]
    .into_iter()
    .filter(|(n, ..)| *n > 0)
    .map(|(n, one, many)| plural(n, one, many))
    .collect();
    match parts.as_slice() {
        [] => plural(pair.items.len(), "item", "items"),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn title_case(word: &str) -> String {
    word.split(['_', '-'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn unbracket(path: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in path.chars() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// A conflict item's group (an archive, a database table, text or a folder)
/// and its name within the group.
fn item_group(item: &str) -> (String, String) {
    if let Some(key) = item.strip_prefix("text ") {
        return ("Text".to_owned(), key.to_owned());
    }
    if let Some(group) = item.strip_prefix("asset group ") {
        return ("Asset groups".to_owned(), group.to_owned());
    }
    if let Some((erp, uri)) = item.split_once(".erp:") {
        let archive = erp.rsplit('/').next().unwrap_or(erp);
        let uri = uri.trim_start_matches("eaid://");
        return (format!("{archive}.erp archive"), uri.to_owned());
    }
    if let Some((select, field)) = item.rsplit_once("].") {
        let row = select.rsplit_once('=').map_or(select, |(_, l)| l);
        let rows = unbracket(select);
        let module = swofty_core::dbedit::modules().iter().find(|m| {
            let shaped = unbracket(&m.rows);
            shaped == rows || shaped.starts_with(&format!("{rows}."))
        });
        if let Some(m) = module {
            let label = m
                .fields
                .iter()
                .find(|f| f.path == field)
                .map_or_else(|| field.to_owned(), |f| f.label.clone());
            return (m.title.clone(), format!("{} · {label}", title_case(row)));
        }
        let table = rows.split('.').next().unwrap_or(&rows);
        return (
            format!("{} table", title_case(table)),
            format!("{} · {field}", title_case(row)),
        );
    }
    if item.starts_with("0x") {
        return ("Database".to_owned(), item.to_owned());
    }
    if !item.contains('/') {
        let table = item.split('.').next().unwrap_or(item);
        return (format!("{} table", title_case(table)), item.to_owned());
    }
    let (dir, file) = item.rsplit_once('/').unwrap_or(("", item));
    (format!("Files in {dir}"), file.to_owned())
}

fn ago(seconds: f64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0., |d| d.as_secs_f64());
    let mins = ((now - seconds).max(0.) / 60.) as u64;
    match mins {
        0 => "just now".to_owned(),
        1..60 => format!("{} ago", plural(mins as usize, "minute", "minutes")),
        60..1440 => format!("{} ago", plural((mins / 60) as usize, "hour", "hours")),
        _ => format!("{} ago", plural((mins / 1440) as usize, "day", "days")),
    }
}

fn checkbox(on: bool, p: Palette) -> Div {
    div()
        .size(px(14.))
        .flex_shrink_0()
        .rounded(px(3.))
        .border_1()
        .flex()
        .items_center()
        .justify_center()
        .map(|d| {
            if on {
                d.bg(p.accent)
                    .border_color(p.accent)
                    .child(Icon::new(IconName::Check).size(px(11.)).text_color(p.on_accent))
            } else {
                d.border_color(p.border)
            }
        })
}

fn stock_removals_text(n: usize) -> String {
    format!(
        "{} of stock database records skipped. The game crashes when a record it reads is missing.",
        plural(n, "removal", "removals")
    )
}

fn pill(icon: IconName, text: impl Into<SharedString>, color: Hsla) -> Div {
    h_flex()
        .h(px(20.))
        .px(px(6.))
        .gap(px(4.))
        .flex_shrink_0()
        .rounded(px(4.))
        .bg(color.opacity(0.14))
        .text_color(color)
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .child(Icon::new(icon).size(px(11.)))
        .child(text.into())
}

impl Shell {
    fn addon_on(&self, m: &AddonItem) -> bool {
        !self.vanilla() && (m.enabled || m.locked)
    }

    fn update_ready(&self, m: &AddonItem) -> bool {
        m.manifest.id == swofty_core::managed::ID && self.updates.plan().addon.is_some()
    }

    fn pairs_of<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a Pair> + 'a {
        self.addons
            .report
            .iter()
            .flat_map(|r| r.pairs.iter())
            .filter(move |p| p.winner == id || p.loser == id)
    }

    /// The conflicts the list shows a badge for: an addon that is on, from a
    /// report of the current load order.
    fn live_pairs<'a>(&'a self, m: &'a AddonItem) -> Vec<&'a Pair> {
        let current = self
            .addons
            .report
            .as_ref()
            .is_some_and(|r| r.is_current(&self.library.profile));
        if !current || !self.addon_on(m) {
            return Vec::new();
        }
        self.pairs_of(&m.manifest.id).collect()
    }

    /// Removals of stock database records the last build of the current load order skipped.
    fn stock_removals(&self, m: &AddonItem) -> usize {
        match &self.addons.report {
            Some(r) if r.is_current(&self.library.profile) && self.addon_on(m) => {
                r.stock_removals.get(&m.manifest.id).copied().unwrap_or(0)
            }
            _ => 0,
        }
    }

    fn changed_since(&self, id: &str, built_at: f64) -> bool {
        std::fs::metadata(self.store.addons_dir().join(id).join(MANIFEST_FILE))
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .is_some_and(|t| t.as_secs_f64() > built_at)
    }

    fn addon_name(&self, id: &str) -> String {
        self.library
            .addons
            .iter()
            .find(|a| a.manifest.id == id)
            .map_or_else(|| id.to_owned(), |a| a.manifest.name.clone())
    }

    fn matches_facet(&self, m: &AddonItem, f: Facet) -> bool {
        match f {
            Facet::Category(c) => m.category() == c,
            Facet::Type(t) => m.manifest.kind.iter().any(|k| Type::of(*k) == t),
            Facet::Source(s) => m.source == s,
            Facet::State(State::On) => self.addon_on(m),
            Facet::State(State::Off) => !self.addon_on(m),
            Facet::State(State::Conflicts) => !self.live_pairs(m).is_empty(),
            Facet::State(State::Update) => self.update_ready(m),
        }
    }

    fn passes(&self, m: &AddonItem, query: &str, except: Option<Group>) -> bool {
        let groups = [Group::Category, Group::Type, Group::Source, Group::State];
        matches_query(m, query)
            && groups.into_iter().filter(|g| Some(*g) != except).all(|g| {
                let mut chosen = self.addons.facets.iter().filter(|f| f.group() == g).peekable();
                chosen.peek().is_none() || chosen.any(|f| self.matches_facet(m, *f))
            })
    }

    /// The profile's enabled addons in load order, then the rest; and how many
    /// are in the load order.
    fn ordered(&self) -> (Vec<&AddonItem>, usize) {
        let find = |id: &str| self.library.listed().find(|a| a.manifest.id == id);
        if self.vanilla() {
            return (self.library.listed().collect(), 0);
        }
        let mut out: Vec<&AddonItem> = self.library.profile.enabled().filter_map(find).collect();
        let loaded = out.len();
        for m in self.library.listed() {
            if !out.iter().any(|o| o.manifest.id == m.manifest.id) {
                out.push(m);
            }
        }
        (out, loaded)
    }

    fn sync_addons(&mut self) {
        let at = self.library.loaded;
        if self.addons.read_at == Some(at) {
            return;
        }
        self.addons.read_at = Some(at);
        self.addons.changes.clear();
        let profile = self.library.profile.name.clone();
        let last = (!self.vanilla())
            .then(|| conflicts::last(&self.store, &profile))
            .flatten();
        let checked = self.addons.report.take().filter(|r| r.profile == profile);
        self.addons.report = match (last, checked) {
            (Some(l), Some(c)) => Some(if c.built_at > l.built_at { c } else { l }),
            (l, c) => l.or(c),
        };
    }

    fn sync_highlight(&mut self, query: &mut String, cx: &mut Context<Self>) -> Option<(String, f32)> {
        let (id, at) = self.highlight.clone()?;
        let t = at.elapsed();
        if t >= FADE {
            self.highlight = None;
            return None;
        }
        if self.addons.seen != Some(at) {
            let item = self.library.listed().find(|a| a.manifest.id == id);
            if let Some(hidden) = item.map(|m| !self.passes(m, query, None)) {
                self.addons.seen = Some(at);
                self.addons.scroll_pending = true;
                self.open_addon = Some(id.clone());
                if hidden {
                    self.addons.facets.clear();
                    if !query.is_empty() {
                        query.clear();
                        self.addons.clear_search(cx);
                    }
                }
            }
            self.animate_highlight(cx);
        }
        Some((id, 1. - t.as_secs_f32() / FADE.as_secs_f32()))
    }

    fn animate_highlight(&mut self, cx: &mut Context<Self>) {
        if self.addons.animating {
            return;
        }
        self.addons.animating = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(33)).await;
                let done = this
                    .update(cx, |s, cx| {
                        let done = s.highlight.as_ref().is_none_or(|(_, at)| at.elapsed() >= FADE);
                        if done {
                            s.highlight = None;
                            s.addons.animating = false;
                        }
                        cx.notify();
                        done
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
            }
        })
        .detach();
    }

    fn pick_addons(&mut self, folder: bool, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: !folder,
            directories: folder,
            multiple: true,
            prompt: Some("Install".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && !paths.is_empty()
            {
                this.update_in(cx, |s, window, cx| s.install_picked(paths, window, cx))
                    .ok();
            }
        })
        .detach();
    }

    fn install_picked(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        let registry = self.library.registry.clone();
        let (addons, other): (Vec<PathBuf>, Vec<PathBuf>) = paths
            .into_iter()
            .partition(|p| import::route(&registry, p) == Some(Route::Addon));
        if !other.is_empty() {
            self.drop_paths(other, window, cx);
        }
        if addons.is_empty() {
            return;
        }
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let results = cx
                .background_executor()
                .spawn(async move {
                    addons
                        .iter()
                        .map(|path| {
                            store.install_addon(path).map_err(|e| {
                                let name = path.file_name().map_or_else(
                                    || path.display().to_string(),
                                    |n| n.to_string_lossy().into_owned(),
                                );
                                format!("{name}: {e}")
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            this.update_in(cx, |s, window, cx| {
                let mut last = None;
                for r in results {
                    match r {
                        Ok(m) => {
                            let text = format!("Installed {} {}", m.name, m.version);
                            window.push_notification(Notification::success(text), cx);
                            last = Some(m.id);
                        }
                        Err(e) => window.push_notification(Notification::error(e), cx),
                    }
                }
                s.reload(cx);
                if let Some(id) = last {
                    s.show_addon(&id, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn move_onto(&mut self, id: &str, target: &str, window: &mut Window, cx: &mut Context<Self>) {
        let addons = &self.library.profile.addons;
        let at = |x: &str| addons.iter().position(|a| a.id == x);
        let (Some(a), Some(b)) = (at(id), at(target)) else {
            return;
        };
        let delta = pinned_delta(addons, id, a, b);
        if delta != 0 {
            let id = id.to_owned();
            self.update_profile(window, cx, |p| p.move_addon(&id, delta));
        }
    }

    /// Whether the list shows `id` among the addons that are on.
    fn listed_on(&self, id: &str) -> bool {
        let (order, loaded) = self.ordered();
        order[..loaded].iter().any(|m| m.manifest.id == id)
    }

    /// Drops `id` on `target`: onto an addon that is on, it turns on and takes that
    /// place in the load order; onto one that is off, it turns off.
    fn drop_onto(&mut self, id: &str, target: &str, window: &mut Window, cx: &mut Context<Self>) {
        match (self.listed_on(id), self.listed_on(target)) {
            (true, true) => {
                if self
                    .library
                    .listed()
                    .any(|m| m.manifest.id == id && self.movable(m))
                {
                    self.move_onto(id, target, window, cx);
                }
            }
            (false, true) => {
                let registry = self.library.registry.clone();
                let (id, target) = (id.to_owned(), target.to_owned());
                self.update_profile(window, cx, |p| {
                    registry.set_enabled(p, &id, true)?;
                    let at = |x: &str| p.addons.iter().position(|a| a.id == x);
                    match (at(&id), at(&target)) {
                        (Some(a), Some(b)) => match pinned_delta(&p.addons, &id, a, b) {
                            0 => Ok(()),
                            delta => p.move_addon(&id, delta),
                        },
                        _ => Ok(()),
                    }
                });
            }
            (true, false) => self.set_addon_enabled(id, false, window, cx),
            (false, false) => {}
        }
    }

    /// Drops `id` on a group's heading: ON turns it on, OFF turns it off.
    fn drop_into(&mut self, id: &str, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.listed_on(id) != on {
            self.set_addon_enabled(id, on, window, cx);
        }
    }

    /// Addons that can be dragged between ON and OFF, and turned on or off.
    fn switchable(&self, m: &AddonItem) -> bool {
        !self.vanilla() && !m.locked
    }

    fn move_step(&mut self, id: &str, up: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (order, loaded) = self.ordered();
        let order: Vec<String> = order[..loaded].iter().map(|m| m.manifest.id.clone()).collect();
        let Some(i) = order.iter().position(|x| x == id) else {
            return;
        };
        let j = if up { i.checked_sub(1) } else { Some(i + 1) };
        if let Some(target) = j.and_then(|j| order.get(j)) {
            self.move_onto(id, target, window, cx);
        }
    }

    fn movable(&self, m: &AddonItem) -> bool {
        !self.vanilla()
            && m.enabled
            && !m.locked
            && !m.manifest.is_builtin()
            && !PINNED.contains(&m.manifest.id.as_str())
    }

    /// Turns off every addon except those that are always on and those whose
    /// turning off warns about saves, which keep their own switch.
    fn turn_off_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let registry = self.library.registry.clone();
        let profile = &self.library.profile;
        let (mut off, mut kept) = (Vec::new(), Vec::new());
        for m in self.library.listed().filter(|m| m.enabled && !m.locked) {
            match registry.turn_off_warning(profile, &m.manifest.id) {
                Some(_) => kept.push(m.manifest.name.clone()),
                None => off.push(m.manifest.id.clone()),
            }
        }
        if off.is_empty() {
            return;
        }
        let before = profile.addons.clone();
        self.update_profile(window, cx, |p| {
            off.iter().try_for_each(|id| registry.set_enabled(p, id, false))
        });
        let shell = cx.entity().downgrade();
        let text = format!("Turned off {}", plural(off.len(), "addon", "addons"));
        let note = Notification::success(text)
            .action(move |_, _, cx| {
                let (shell, before) = (shell.clone(), before.clone());
                let note = cx.entity().downgrade();
                Button::new("undo-turn-off-all")
                    .outline()
                    .small()
                    .label("Undo")
                    .on_click(move |_, window, cx| {
                        let before = before.clone();
                        shell
                            .update(cx, |s, cx| {
                                s.update_profile(window, cx, |p| {
                                    let rest: Vec<_> = p
                                        .addons
                                        .iter()
                                        .filter(|a| !before.iter().any(|b| b.id == a.id))
                                        .cloned()
                                        .collect();
                                    p.addons = before.into_iter().chain(rest).collect();
                                    Ok(())
                                })
                            })
                            .ok();
                        note.update(cx, |n, cx| n.dismiss(window, cx)).ok();
                    })
            })
            .autohide(true);
        window.push_notification(note, cx);
        if !kept.is_empty() {
            window.push_notification(
                Notification::info(format!(
                    "Left on: {}. Turning it off warns about your saves, so use its own switch.",
                    kept.join(", ")
                )),
                cx,
            );
        }
    }

    fn launching(&self) -> bool {
        use crate::shell::PlayState::{Launching, Restoring, Running};
        matches!(self.play_state(), Launching | Restoring | Running)
    }

    fn check_conflicts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.addons.checking || self.vanilla() || self.launching() {
            return;
        }
        self.addons.checking = true;
        let (store, install) = (self.store.clone(), self.install.clone());
        let profile = self.library.profile.name.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let game = crate::sections::game(&store, install.as_ref()).ok_or_else(|| {
                        "Locate F1 25 first. Checking conflicts reads its files.".to_owned()
                    })?;
                    conflicts::check(&store, &profile, &game).map_err(|e| e.to_string())
                })
                .await;
            this.update_in(cx, |s, window, cx| {
                s.addons.checking = false;
                match result {
                    Ok(report) => {
                        let text = match report.pairs.len() {
                            0 => "No conflicts between addons".to_owned(),
                            n => format!("Found {}", plural(n, "conflict", "conflicts")),
                        };
                        window.push_notification(Notification::success(text), cx);
                        if report.profile == s.library.profile.name {
                            s.addons.report = Some(report);
                        }
                    }
                    Err(e) => window
                        .push_notification(Notification::error(e).title("Could not check conflicts"), cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn addons_page(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        self.sync_addons();
        let mut query = self.addons.search.read(cx).value().trim().to_lowercase();
        let highlight = self.sync_highlight(&mut query, cx);
        let installed = plural(
            self.library.listed().count(),
            "addon installed",
            "addons installed",
        );
        let subtitle = if self.vanilla() {
            format!("{installed} · {VANILLA} plays the game without them")
        } else {
            let on = self.library.listed().filter(|m| self.addon_on(m)).count();
            format!("{installed} · {on} on in {}", self.library.profile.name)
        };
        let actions = (!self.vanilla()).then(|| {
            h_flex()
                .gap(px(8.))
                .child(self.check_button("check-conflicts-page", false, cx))
                .child(
                    Button::new("turn-off-all")
                        .outline()
                        .icon(IconName::PowerOff)
                        .label("Turn off all")
                        .on_click(cx.listener(|this, _, window, cx| this.turn_off_all(window, cx))),
                )
        });
        let pending = (!self.vanilla() && !self.library.rebuild.is_empty()).then(|| {
            h_flex()
                .px(px(GUTTER))
                .pb(px(12.))
                .gap(px(8.))
                .flex_shrink_0()
                .text_size(px(13.))
                .text_color(p.text_muted)
                .child(
                    Icon::new(IconName::CircleAlert)
                        .size(px(14.))
                        .text_color(p.warning),
                )
                .child(format!(
                    "{} changed since the last build · the next launch rebuilds {}",
                    plural(
                        self.library.rebuild.len(),
                        "database setting",
                        "database settings"
                    ),
                    self.library.profile.name
                ))
        });
        let hidden = self.library.addons.len() - self.library.listed().count();
        let tracks = self.library.tracks_page.filter(|_| hidden > 0).map(|i| {
            h_flex()
                .px(px(GUTTER))
                .pb(px(12.))
                .gap(px(8.))
                .flex_shrink_0()
                .items_center()
                .text_size(px(13.))
                .text_color(p.text_muted)
                .child(Icon::new(IconName::Map).size(px(14.)))
                .child(format!(
                    "{} {} on the Tracks page",
                    plural(hidden, "track addon", "track addons"),
                    if hidden == 1 { "is" } else { "are" }
                ))
                .child(
                    Button::new("open-tracks")
                        .ghost()
                        .small()
                        .label("Open Tracks")
                        .on_click(cx.listener(move |this, _, _, cx| this.open_page(Page::Section(i), cx))),
                )
        });
        let split = self.content_width >= SPLIT_AT;
        let selected = self.open_addon.clone().filter(|id| {
            self.library
                .listed()
                .any(|a| &a.manifest.id == id && self.passes(a, &query, None))
        });
        if selected.is_none() {
            self.open_addon = None;
        }
        let list = self.addon_list(&query, highlight.as_ref(), split, p, cx);
        let detail = selected
            .filter(|_| split)
            .map(|id| self.addon_detail(&id, false, p, cx));
        v_flex()
            .size_full()
            .child(self.header(Page::Addons.label(), subtitle, div().children(actions), p))
            .children(pending)
            .children(tracks)
            .child(self.install_cards(p, cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .px(px(GUTTER))
                    .pb(px(20.))
                    .gap(px(20.))
                    .child(self.filters(&query, p, cx))
                    .child(list)
                    .children(detail),
            )
            .into_any_element()
    }

    fn install_cards(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let roomy = self.content_width >= 900. && !self.short;
        let short = self.short;
        let wide = self.content_width >= 900.;
        let card =
            |id: &'static str, icon: IconName, color: Hsla, title: &'static str, body: &'static str| {
                let title = div()
                    .flex_shrink_0()
                    .text_size(px(if short { 13. } else { 14. }))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.text)
                    .child(title);
                let body = div()
                    .min_w_0()
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .text_color(p.text_muted)
                    .when(short, |d| d.truncate())
                    .child(body);
                let text = if short {
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(8.))
                        .child(Icon::new(icon).size(px(15.)).text_color(color))
                        .child(title)
                        .when(wide, |d| d.child(body))
                } else {
                    v_flex().flex_1().min_w_0().gap(px(2.)).child(title).child(body)
                };
                h_flex()
                    .id(id)
                    .flex_1()
                    .min_w_0()
                    .min_h(px(if short { 40. } else { 78. }))
                    .px(px(if short { 12. } else { 16. }))
                    .py(px(if short { 6. } else { 12. }))
                    .gap(px(if short { 10. } else { 14. }))
                    .rounded(px(8.))
                    .border_1()
                    .bg(p.surface)
                    .when(roomy, |d| {
                        d.child(
                            div()
                                .size(px(40.))
                                .flex_shrink_0()
                                .rounded(px(8.))
                                .bg(color.opacity(0.14))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new(icon).size(px(19.)).text_color(color)),
                        )
                    })
                    .child(text)
            };
        let folder = div()
            .id("install-folder")
            .flex_shrink_0()
            .text_size(px(12.))
            .text_color(p.text_muted)
            .cursor_pointer()
            .hover(move |s| s.text_color(p.text))
            .child("Choose a folder…")
            .on_click(cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.pick_addons(true, window, cx)
            }));
        let addon = card(
            "install-addon",
            IconName::Upload,
            p.accent,
            "Installing a Swofty Addon?",
            if short {
                "Drop it here or click to choose"
            } else {
                "Drag and drop it here or click to choose a file"
            },
        )
        .border_dashed()
        .border_color(p.border)
        .cursor_pointer()
        .hover(move |s| s.border_color(p.accent).bg(p.hover))
        .child(folder)
        .on_click(cx.listener(|this, _, window, cx| this.pick_addons(false, window, cx)));
        let importer = card(
            "install-mod",
            IconName::FolderInput,
            p.success,
            "Installing a mod?",
            if short {
                "Mods convert into addons"
            } else {
                "Mods convert into Swofty addons using the Simplified Importer."
            },
        )
        .border_color(p.border_variant)
        .child(
            Button::new("go-importer")
                .outline()
                .small()
                .label(if roomy {
                    "Go to Simplified Importer"
                } else {
                    "Simplified Importer"
                })
                .on_click(cx.listener(|this, _, _, cx| this.open_page(Page::Importer, cx))),
        );
        h_flex()
            .px(px(GUTTER))
            .pb(px(if short { 12. } else { 18. }))
            .gap(px(if short { 10. } else { 14. }))
            .flex_shrink_0()
            .child(addon)
            .child(importer)
    }

    fn filters(&self, query: &str, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let items: Vec<&AddonItem> = self.library.listed().collect();
        let active = !self.addons.facets.is_empty() || !query.is_empty();
        let group = |title: &'static str, facets: Vec<Facet>, cx: &mut Context<Self>| {
            let rows: Vec<AnyElement> = facets
                .into_iter()
                .filter(|f| {
                    f.group() != Group::Category
                        || self.addons.facets.contains(f)
                        || items.iter().any(|m| self.matches_facet(m, *f))
                })
                .map(|f| {
                    let n = items
                        .iter()
                        .filter(|m| self.passes(m, query, Some(f.group())) && self.matches_facet(m, f))
                        .count();
                    self.filter_row(f, n, p, cx)
                })
                .collect();
            v_flex()
                .gap(px(1.))
                .child(div().pb(px(4.)).child(label(title, p)))
                .children(rows)
        };
        let categories = Category::ALL
            .into_iter()
            .map(|c| Facet::Category(Some(c)))
            .chain([Facet::Category(None)])
            .collect();
        let types = Type::ALL.into_iter().map(Facet::Type).collect();
        let sources = Source::ALL.into_iter().map(Facet::Source).collect();
        let states = State::ALL.into_iter().map(Facet::State).collect();
        let clear = active.then(|| {
            div()
                .id("filters-clear")
                .text_size(px(12.))
                .text_color(p.accent)
                .cursor_pointer()
                .hover(move |s| s.text_color(p.accent_hover))
                .child("Clear")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.addons.facets.clear();
                    this.addons.clear_search(cx);
                    cx.notify();
                }))
        });
        v_flex()
            .w(px(FILTERS_W))
            .flex_shrink_0()
            .h_full()
            .gap(px(14.))
            .child(
                Input::new(&self.addons.search).cleanable(true).prefix(
                    Icon::new(IconName::Search)
                        .size(px(14.))
                        .text_color(p.text_placeholder),
                ),
            )
            .child(
                h_flex()
                    .pb(px(8.))
                    .mb(px(-6.))
                    .border_b_1()
                    .border_color(p.border_variant)
                    .justify_between()
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .text_color(p.text_muted)
                            .child(Icon::new(IconName::Funnel).size(px(13.)))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Filters"),
                            ),
                    )
                    .children(clear),
            )
            .child(
                v_flex()
                    .id("addon-filters")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .gap(px(16.))
                    .pb(px(8.))
                    .child(group("CATEGORY", categories, cx))
                    .child(group("TYPE", types, cx))
                    .child(group("SOURCE", sources, cx))
                    .child(group("STATE", states, cx)),
            )
    }

    fn filter_row(&self, f: Facet, count: usize, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let on = self.addons.facets.contains(&f);
        let dim = count == 0 && !on;
        h_flex()
            .id(SharedString::from(format!("filter-{f:?}")))
            .h(px(26.))
            .px(px(6.))
            .gap(px(8.))
            .rounded(px(5.))
            .cursor_pointer()
            .hover(move |s| s.bg(p.hover))
            .child(checkbox(on, p))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(13.))
                    .text_color(if dim { p.text_placeholder } else { p.text })
                    .child(f.label()),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(if on { p.text_muted } else { p.text_placeholder })
                    .child(count.to_string()),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.addons.facets.remove(&f) {
                    this.addons.facets.insert(f);
                }
                cx.notify();
            }))
            .into_any_element()
    }

    fn addon_list(
        &mut self,
        query: &str,
        highlight: Option<&(String, f32)>,
        split: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (order, loaded) = self.ordered();
        let total = order.len();
        let shown: Vec<(usize, String)> = order
            .iter()
            .enumerate()
            .filter(|(_, m)| self.passes(m, query, None))
            .map(|(i, m)| (i, m.manifest.id.clone()))
            .collect();
        let vanilla = self.vanilla();
        let mut children: Vec<AnyElement> = Vec::new();
        let mut scroll_to = None;
        let mut last_group = None;
        if !vanilla && !shown.iter().any(|(i, _)| *i < loaded) {
            children.push(self.group_heading(Some(true), true, true, p, cx));
        }
        for (i, id) in &shown {
            let group = (!vanilla).then_some(*i < loaded);
            if group != last_group {
                children.push(self.group_heading(group, children.is_empty(), false, p, cx));
                last_group = group;
            }
            let fade = highlight.filter(|(h, _)| h == id).map(|(_, k)| *k);
            if fade.is_some() && self.addons.scroll_pending {
                scroll_to = Some(children.len());
            }
            let position = (*i < loaded && !vanilla).then_some(*i + 1);
            if let Some(m) = self.library.listed().find(|a| &a.manifest.id == id) {
                children.push(self.list_row(m, position, fade, p, cx));
            }
            if !split && self.open_addon.as_ref() == Some(id) {
                children.push(self.addon_detail(id, true, p, cx));
            }
        }
        if !vanilla && last_group != Some(false) {
            children.push(self.group_heading(Some(false), children.is_empty(), true, p, cx));
        }
        if let Some(ix) = scroll_to {
            self.addons.scroll_pending = false;
            if split {
                self.addons.scroll.scroll_to_item(ix);
            } else {
                self.addons.scroll.scroll_to_top_of_item(ix);
            }
        }
        let header = h_flex()
            .h(px(28.))
            .gap(px(8.))
            .flex_shrink_0()
            .text_size(px(12.))
            .text_color(p.text_muted)
            .child(Icon::new(IconName::Layers).size(px(13.)))
            .child(div().flex_1().min_w_0().truncate().child(if vanilla {
                format!("Switch away from {VANILLA} to use addons")
            } else {
                "Lower in the list wins when two addons change the same thing".to_owned()
            }))
            .when(shown.len() != total, |d| {
                d.child(
                    div()
                        .flex_shrink_0()
                        .text_color(p.text_placeholder)
                        .child(format!("{} of {total}", shown.len())),
                )
            });
        let body = if shown.is_empty() {
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .py(px(40.))
                .rounded(px(8.))
                .border_1()
                .border_color(p.border_variant)
                .child(badge(IconName::Search, 36., p))
                .child(
                    div()
                        .pt(px(4.))
                        .text_size(px(14.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("No addons match"),
                )
                .child(
                    Button::new("empty-clear")
                        .ghost()
                        .small()
                        .label("Clear filters")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.addons.facets.clear();
                            this.addons.clear_search(cx);
                            cx.notify();
                        })),
                )
                .into_any_element()
        } else {
            div()
                .min_h_0()
                .flex()
                .flex_col()
                .rounded(px(8.))
                .border_1()
                .border_color(p.border)
                .overflow_hidden()
                .child(
                    v_flex()
                        .id("addon-list")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .track_scroll(&self.addons.scroll)
                        .pb(px(6.))
                        .children(children),
                )
                .into_any_element()
        };
        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .gap(px(6.))
            .child(header)
            .child(body)
            .into_any_element()
    }

    /// A group's heading in the list. ON and OFF take a dragged addon, which turns
    /// it on or off; an empty group says so.
    fn group_heading(
        &self,
        group: Option<bool>,
        first: bool,
        empty: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let title = match group {
            Some(true) => format!("ON IN {} · LOAD ORDER", self.library.profile.name.to_uppercase()),
            Some(false) => "OFF".to_owned(),
            None => "INSTALLED".to_owned(),
        };
        let heading = v_flex()
            .id(SharedString::from(format!("group-{group:?}")))
            .px(px(12.))
            .pt(px(if first { 8. } else { 14. }))
            .pb(px(6.))
            .gap(px(6.))
            .flex_shrink_0()
            .child(label(title, p))
            .when(empty, |d| {
                d.child(
                    div()
                        .py(px(10.))
                        .text_size(px(12.))
                        .text_color(p.text_placeholder)
                        .child(match group {
                            Some(true) => "Drag an addon here to turn it on",
                            _ => "Drag an addon here to turn it off",
                        }),
                )
            });
        let Some(on) = group else {
            return heading.into_any_element();
        };
        heading
            .drag_over::<DraggedAddon>(move |s, _, _, _| s.bg(p.hover))
            .on_drop(
                cx.listener(move |this, d: &DraggedAddon, window, cx| this.drop_into(&d.id, on, window, cx)),
            )
            .into_any_element()
    }

    fn list_row(
        &self,
        m: &AddonItem,
        position: Option<usize>,
        fade: Option<f32>,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let man = &m.manifest;
        let id = man.id.clone();
        let selected = self.open_addon.as_ref() == Some(&id);
        let draggable = self.switchable(m);
        let pairs = self.live_pairs(m);
        let lost = pairs.iter().filter(|pr| pr.loser == id).count();
        let won = pairs.len() - lost;
        let kept = self.stock_removals(m);
        let trailing = if m.locked {
            h_flex()
                .gap(px(5.))
                .text_size(px(12.))
                .text_color(p.text_placeholder)
                .whitespace_nowrap()
                .child(Icon::new(IconName::Lock).size(px(12.)))
                .child(if self.vanilla() {
                    format!("Off in {VANILLA}")
                } else {
                    "Always on".to_owned()
                })
                .into_any_element()
        } else {
            let id = id.clone();
            self.toggle(
                format!("toggle-{id}"),
                m.enabled && !self.vanilla(),
                cx,
                move |this, on, window, cx| this.set_addon_enabled(&id, on, window, cx),
            )
        };
        let icon = match &m.icon {
            _ if man.is_builtin() => logo(28.),
            Some(icon) => addon_icon(icon, 28., p),
            None => fallback_icon(m, 28., p),
        };
        let grip = div().w(px(14.)).flex_shrink_0().when(draggable, |d| {
            d.child(
                Icon::new(IconName::GripVertical)
                    .size(px(14.))
                    .text_color(p.text_placeholder),
            )
        });
        let category = match m.category() {
            Some(c) => Some(c.label()),
            None if m.source == Source::Builtin => None,
            None => Some("Uncategorised"),
        };
        let types = Type::list(m);
        let row = h_flex()
            .id(SharedString::from(format!("addon-{id}")))
            .relative()
            .h(px(52.))
            .pl(px(6.))
            .pr(px(12.))
            .gap(px(10.))
            .flex_shrink_0()
            .border_t_1()
            .border_color(p.border_variant)
            .cursor_pointer()
            .map(|d| match (fade, selected) {
                (Some(k), _) => d.bg(p.warning.opacity(0.06 + 0.3 * k)),
                (None, true) => d.bg(p.selected),
                (None, false) => d.hover(move |s| s.bg(p.hover)),
            })
            .when_some(fade, |d, k| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(px(3.))
                        .bg(p.warning.opacity(k)),
                )
            })
            .when(selected && fade.is_none(), |d| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(px(3.))
                        .bg(p.accent),
                )
            })
            .child(grip)
            .child(
                div()
                    .w(px(14.))
                    .flex_shrink_0()
                    .text_size(px(11.))
                    .text_color(p.text_placeholder)
                    .children(position.map(|n| n.to_string())),
            )
            .child(icon)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .items_baseline()
                            .child(
                                div()
                                    .min_w_0()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(man.name.clone()),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(px(12.))
                                    .text_color(p.text_placeholder)
                                    .child(man.version.to_string()),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .text_size(px(12.))
                            .children(category.map(|c| {
                                div()
                                    .flex_shrink_0()
                                    .text_color(if m.category().is_some() {
                                        p.text_muted
                                    } else {
                                        p.text_placeholder
                                    })
                                    .child(c)
                            }))
                            .when(!types.is_empty(), |d| {
                                d.when(category.is_some(), |d| {
                                    d.child(div().text_color(p.text_placeholder).child("·"))
                                })
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .text_color(p.text_placeholder)
                                        .child(types),
                                )
                            })
                            .child(
                                div()
                                    .h(px(16.))
                                    .px(px(5.))
                                    .flex()
                                    .items_center()
                                    .flex_shrink_0()
                                    .rounded(px(3.))
                                    .border_1()
                                    .border_color(p.border)
                                    .text_size(px(10.5))
                                    .text_color(p.text_muted)
                                    .child(m.source.label()),
                            )
                            .when(!man.experimental_features.is_empty(), |d| {
                                let features = experimental_labels(m);
                                d.child(
                                    div()
                                        .id(SharedString::from(format!("experimental-{id}")))
                                        .h(px(16.))
                                        .px(px(5.))
                                        .flex()
                                        .items_center()
                                        .flex_shrink_0()
                                        .rounded(px(3.))
                                        .border_1()
                                        .border_color(p.warning.opacity(0.6))
                                        .text_size(px(10.5))
                                        .text_color(p.warning)
                                        .child("Uses experimental features")
                                        .tooltip(move |window, cx| {
                                            Tooltip::new(format!("Turns on {features} while it's on"))
                                                .build(window, cx)
                                        }),
                                )
                            }),
                    ),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .gap(px(4.))
                    .justify_end()
                    .when(self.update_ready(m), |d| {
                        d.child(pill(IconName::CircleArrowUp, "", p.accent))
                    })
                    .when(kept > 0, |d| {
                        d.child(
                            pill(
                                IconName::TriangleAlert,
                                format!("{} skipped", plural(kept, "removal", "removals")),
                                p.warning,
                            )
                            .id(SharedString::from(format!("kept-{id}")))
                            .tooltip(move |window, cx| {
                                Tooltip::new(stock_removals_text(kept)).build(window, cx)
                            }),
                        )
                    })
                    .when(lost > 0, |d| {
                        d.child(
                            pill(IconName::TriangleAlert, format!("Loses to {lost}"), p.warning)
                                .id(SharedString::from(format!("loses-{id}")))
                                .tooltip(move |window, cx| {
                                    Tooltip::new(format!(
                                        "{} lower in the load order {} some of what this addon changes",
                                        plural(lost, "addon", "addons"),
                                        if lost == 1 { "replaces" } else { "replace" }
                                    ))
                                    .build(window, cx)
                                }),
                        )
                    })
                    .when(lost == 0 && won > 0, |d| {
                        d.child(
                            pill(IconName::Layers, format!("Wins over {won}"), p.text_muted)
                                .id(SharedString::from(format!("wins-{id}")))
                                .tooltip(move |window, cx| {
                                    Tooltip::new(format!(
                                        "Replaces some of what {} higher in the load order {}",
                                        plural(won, "addon", "addons"),
                                        if won == 1 { "changes" } else { "change" }
                                    ))
                                    .build(window, cx)
                                }),
                        )
                    }),
            )
            .child(
                div()
                    .w(px(84.))
                    .flex()
                    .justify_end()
                    .flex_shrink_0()
                    .child(trailing),
            )
            .on_click(cx.listener({
                let id = id.clone();
                move |this, _, _, cx| {
                    this.open_addon = (this.open_addon.as_ref() != Some(&id)).then(|| id.clone());
                    cx.notify();
                }
            }));
        let row = match self.vanilla() {
            true => row,
            false => {
                let target = id.clone();
                row.drag_over::<DraggedAddon>(move |s, _, _, _| s.bg(p.hover).border_color(p.accent))
                    .on_drop(cx.listener(move |this, d: &DraggedAddon, window, cx| {
                        this.drop_onto(&d.id, &target, window, cx)
                    }))
            }
        };
        let row = match draggable {
            true => {
                let dragged = DraggedAddon {
                    id: id.clone(),
                    name: man.name.clone().into(),
                };
                row.on_drag(dragged, |d, _, _, cx| cx.new(|_| d.clone()))
            }
            false => row,
        };
        row.into_any_element()
    }

    fn changes_of(&mut self, id: &str) -> Changes {
        if let Some(c) = self.addons.changes.get(id) {
            return c.clone();
        }
        let Some(item) = self.library.addons.iter().find(|a| a.manifest.id == id) else {
            return Changes::default();
        };
        let c = Changes::read(&self.store, item);
        self.addons.changes.insert(id.to_owned(), c.clone());
        c
    }

    fn addon_detail(&mut self, id: &str, inline: bool, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let changes = self.changes_of(id);
        let Some(m) = self.library.addons.iter().find(|a| a.manifest.id == id) else {
            return div().into_any_element();
        };
        let man = &m.manifest;
        let made_for = man
            .compat
            .game_builds
            .iter()
            .map(|b| {
                b.to_string()
                    .trim_start_matches(['>', '<', '=', '^', '~'])
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join(", ");
        let (compat, compat_color) = match self.install.as_ref().and_then(|i| i.build_id) {
            _ if man.compat.game_builds.is_empty() => ("Works with any game version".to_owned(), p.text),
            Some(b) if man.compat.accepts_build(b) => ("Works with this game version".to_owned(), p.success),
            Some(b) => (format!("Made for game build {made_for} (yours: {b})"), p.warning),
            None => (format!("Made for game build {made_for}"), p.text),
        };
        let on = self.addon_on(m);
        let (status, status_color) = if self.vanilla() {
            (format!("Switch away from {VANILLA} to use addons"), p.warning)
        } else if on {
            ("On".to_owned(), p.success)
        } else {
            ("Off".to_owned(), p.text_muted)
        };
        let (order, loaded) = self.ordered();
        let position = order[..loaded].iter().position(|a| a.manifest.id == id);
        let movable = self.movable(m);
        let source = match (&man.origin, m.source) {
            (Some(o), Source::Importer) => match &o.file {
                Some(f) => format!("{} · from {f}", m.source.label()),
                None => m.source.label().to_owned(),
            },
            _ => m.source.label().to_owned(),
        };
        let close = (!inline).then(|| {
            Button::new("detail-close")
                .ghost()
                .small()
                .icon(IconName::X)
                .tooltip("Close")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.open_addon = None;
                    cx.notify();
                }))
        });
        let head = h_flex()
            .gap(px(12.))
            .items_start()
            .child(match &m.icon {
                _ if man.is_builtin() => logo(40.),
                Some(icon) => addon_icon(icon, 40., p),
                None => fallback_icon(m, 40., p),
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(man.name.clone()),
                    )
                    .child(div().text_size(px(13.)).text_color(p.text_muted).child(
                        match man.author.trim() {
                            "" | "Unknown" => man.version.to_string(),
                            author => format!("{} · by {author}", man.version),
                        },
                    )),
            )
            .children(close);
        let order_line = position.map(|i| {
            let (up, down) = (id.to_owned(), id.to_owned());
            h_flex()
                .gap(px(4.))
                .child(div().flex_1().child(format!("{} of {loaded}", i + 1)))
                .when(movable, |d| {
                    d.child(
                        Button::new("detail-up")
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowUp)
                            .tooltip("Move up")
                            .disabled(i <= 1)
                            .on_click(
                                cx.listener(move |this, _, window, cx| this.move_step(&up, true, window, cx)),
                            ),
                    )
                    .child(
                        Button::new("detail-down")
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowDown)
                            .tooltip("Move down")
                            .disabled(i + 1 >= loaded)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.move_step(&down, false, window, cx)
                            })),
                    )
                })
        });
        let facts = v_flex()
            .child(fact("Status", div().text_color(status_color).child(status), p))
            .when_some(order_line, |d, line| d.child(fact("Load order", line, p)))
            .when(m.category().is_some() || m.source != Source::Builtin, |d| {
                d.child(fact(
                    "Category",
                    m.category().map_or("Uncategorised", Category::label),
                    p,
                ))
            })
            .when(!man.kind.is_empty(), |d| d.child(fact("Type", Type::list(m), p)))
            .when(!man.experimental_features.is_empty(), |d| {
                d.child(fact("Experimental features", experimental_labels(m), p))
            })
            .child(fact("Source", source, p))
            .child(fact(
                "Game version",
                div().text_color(compat_color).child(compat),
                p,
            ))
            .when(!man.requires.is_empty(), |d| {
                d.child(fact(
                    "Requires",
                    man.requires
                        .iter()
                        .map(|r| format!("{} {}", self.addon_name(&r.id), r.version))
                        .collect::<Vec<_>>()
                        .join(", "),
                    p,
                ))
            })
            .when_some(man.license.clone(), |d, l| d.child(fact("License", l, p)))
            .when_some(man.homepage.clone(), |d, h| d.child(fact("Homepage", h, p)));
        let advanced_open = self.addons.advanced;
        let advanced = v_flex()
            .child(
                h_flex()
                    .id("detail-advanced")
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_size(px(12.))
                    .text_color(p.text_placeholder)
                    .hover(move |s| s.text_color(p.text_muted))
                    .child(chevron(advanced_open, p))
                    .child("Advanced")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.addons.advanced = !this.addons.advanced;
                        cx.notify();
                    })),
            )
            .when(advanced_open, |d| {
                d.child(
                    v_flex()
                        .pt(px(6.))
                        .gap(px(2.))
                        .text_size(px(12.))
                        .child(div().text_color(p.text_muted).child("Addon ID"))
                        .child(
                            div()
                                .id("detail-id")
                                .text_color(p.text_placeholder)
                                .truncate()
                                .child(man.id.clone())
                                .tooltip({
                                    let id = man.id.clone();
                                    move |window, cx| Tooltip::new(id.clone()).build(window, cx)
                                }),
                        ),
                )
            });
        let parts = changes.parts();
        let what = v_flex()
            .gap(px(8.))
            .child(label("WHAT IT CHANGES", p))
            .child(if parts.is_empty() {
                div()
                    .text_size(px(13.))
                    .text_color(p.text_placeholder)
                    .child(if man.launcher.is_some() {
                        "Adds pages and tools to the launcher"
                    } else {
                        "Nothing listed in its manifest"
                    })
                    .into_any_element()
            } else {
                h_flex()
                    .flex_wrap()
                    .gap(px(6.))
                    .children(parts.into_iter().map(|t| tag(t, p).text_color(p.text)))
                    .into_any_element()
            })
            .when(!changes.unreadable.is_empty(), |d| {
                d.child(
                    div()
                        .text_size(px(12.))
                        .text_color(p.warning)
                        .child(format!("Could not read {}", changes.unreadable.join(", "))),
                )
            });
        let mut actions: Vec<AnyElement> = Vec::new();
        if m.source == Source::DbEditor {
            let id = id.to_owned();
            actions.push(
                Button::new("detail-edit-db")
                    .outline()
                    .small()
                    .icon(IconName::Database)
                    .label("Edit in DB Editor")
                    .on_click(cx.listener(move |this, _, _, cx| this.db_edit_addon(&id, cx)))
                    .into_any_element(),
            );
        }
        let remove = (m.source != Source::Builtin && !m.locked).then(|| {
            let (id, name) = (man.id.clone(), man.name.clone());
            Button::new("detail-remove")
                .ghost()
                .small()
                .icon(IconName::Trash)
                .label("Remove")
                .text_color(p.error)
                .on_click(cx.listener(move |this, _, window, cx| this.remove_addon(&id, &name, window, cx)))
        });
        let settings = if man.options.is_empty() {
            None
        } else {
            Some(self.settings(man, p, cx).into_any_element())
        };
        let conflicts = self.conflicts_block(id, p, cx).into_any_element();
        v_flex()
            .id(SharedString::from(format!("detail-{id}")))
            .gap(px(18.))
            .map(|d| {
                if inline {
                    d.flex_shrink_0()
                        .px(px(20.))
                        .pt(px(14.))
                        .pb(px(18.))
                        .bg(p.surface)
                        .border_t_1()
                        .border_color(p.border_variant)
                } else {
                    d.w(px(DETAIL_W))
                        .flex_shrink_0()
                        .h_full()
                        .overflow_y_scroll()
                        .p(px(18.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(p.border)
                        .bg(p.surface)
                }
            })
            .when(!inline, |d| d.child(head))
            .when_some(man.description.clone(), |d, text| {
                d.child(
                    div()
                        .text_size(px(13.))
                        .line_height(px(20.))
                        .text_color(p.text_muted)
                        .child(text),
                )
            })
            .when(!actions.is_empty() || remove.is_some(), |d| {
                d.child(
                    h_flex()
                        .flex_wrap()
                        .gap(px(8.))
                        .when(!actions.is_empty(), |d| d.children(actions).child(div().flex_1()))
                        .children(remove),
                )
            })
            .child(facts)
            .child(what)
            .child(conflicts)
            .children(settings)
            .child(advanced)
            .into_any_element()
    }

    fn check_button(&self, id: &'static str, small: bool, cx: &mut Context<Self>) -> Button {
        Button::new(id)
            .outline()
            .when(small, |b| b.xsmall())
            .icon(IconName::RefreshCw)
            .label(if self.addons.checking {
                "Checking…"
            } else {
                "Check conflicts"
            })
            .tooltip("Plans this profile's build without installing it")
            .loading(self.addons.checking)
            .disabled(self.vanilla() || self.addons.checking || self.launching())
            .on_click(cx.listener(|this, _, window, cx| this.check_conflicts(window, cx)))
    }

    fn conflicts_block(&self, id: &str, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let report = self.addons.report.as_ref();
        let kept = self
            .library
            .addons
            .iter()
            .find(|a| a.manifest.id == id)
            .map_or(0, |m| self.stock_removals(m));
        let check = self.check_button("check-conflicts", true, cx);
        let note = match report {
            _ if self.vanilla() => format!("{VANILLA} has no addons to conflict"),
            None => "Not checked yet. Checking plans this profile's build without installing it.".to_owned(),
            Some(r) if !r.is_current(&self.library.profile) => format!(
                "Checked {}, before addons were turned on, off or moved. Check again to update it.",
                ago(r.built_at)
            ),
            Some(r) if self.changed_since(id, r.built_at) => format!(
                "Checked {}, before this addon last changed. Check again to include it.",
                ago(r.built_at)
            ),
            Some(r) => format!("Checked {}", ago(r.built_at)),
        };
        let stale = report
            .is_some_and(|r| !r.is_current(&self.library.profile) || self.changed_since(id, r.built_at));
        let pairs: Vec<&Pair> = self.pairs_of(id).collect();
        let rows: Vec<AnyElement> = pairs
            .iter()
            .enumerate()
            .map(|(i, pair)| self.pair_row(id, pair, i == 0, p, cx))
            .collect();
        v_flex()
            .gap(px(8.))
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(label("CONFLICTS", p))
                    .child(check),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(p.text_placeholder)
                    .child(note),
            )
            .when(kept > 0, |d| {
                d.child(
                    h_flex()
                        .gap(px(6.))
                        .items_start()
                        .text_size(px(13.))
                        .text_color(p.text_muted)
                        .child(
                            Icon::new(IconName::TriangleAlert)
                                .size(px(14.))
                                .text_color(p.warning),
                        )
                        .child(stock_removals_text(kept)),
                )
            })
            .when(report.is_some() && !stale && pairs.is_empty(), |d| {
                d.child(
                    h_flex()
                        .gap(px(6.))
                        .text_size(px(13.))
                        .text_color(p.text_muted)
                        .child(
                            Icon::new(IconName::CircleCheck)
                                .size(px(14.))
                                .text_color(p.success),
                        )
                        .child("No conflicts with other addons"),
                )
            })
            .when(!rows.is_empty(), |d| {
                d.child(
                    v_flex()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(p.border_variant)
                        .overflow_hidden()
                        .children(rows),
                )
            })
    }

    fn pair_row(&self, id: &str, pair: &Pair, first: bool, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let wins = pair.winner == id;
        let other = if wins { &pair.loser } else { &pair.winner };
        let key = (pair.winner.clone(), pair.loser.clone());
        let open = self.addons.open_pairs.contains(&key);
        let text = if wins {
            format!("Overrides {} on {}", self.addon_name(other), pair_counts(pair))
        } else {
            format!(
                "Overridden by {} on {}",
                self.addon_name(other),
                pair_counts(pair)
            )
        };
        let items = open.then(|| {
            let mut groups: Vec<(String, Vec<String>)> = Vec::new();
            for item in &pair.items {
                let (group, name) = item_group(item);
                match groups.iter_mut().find(|(g, _)| *g == group) {
                    Some((_, names)) => names.push(name),
                    None => groups.push((group, vec![name])),
                }
            }
            let details = self.addons.details.contains(&key);
            let all = self.addons.all_items.contains(&key);
            let total = pair.items.len();
            let unlisted = pair.database + pair.resources + pair.files + pair.strings;
            let link =
                |id: String, text: String, cx: &mut Context<Self>, f: fn(&mut Self, (String, String))| {
                    let key = key.clone();
                    div()
                        .id(SharedString::from(id))
                        .text_size(px(12.))
                        .text_color(p.accent)
                        .cursor_pointer()
                        .hover(move |s| s.text_color(p.accent_hover))
                        .child(text)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            f(this, key.clone());
                            cx.notify();
                        }))
                };
            let tag_id = format!("{}-{}", pair.winner, pair.loser);
            let raw = details.then(|| {
                let mut left = if all { usize::MAX } else { ITEMS_SHOWN };
                let mut out = Vec::new();
                for (g, names) in &groups {
                    if left == 0 {
                        break;
                    }
                    out.push(
                        div()
                            .pt(px(6.))
                            .text_size(px(11.))
                            .text_color(p.text_placeholder)
                            .child(g.clone()),
                    );
                    out.extend(names.iter().take(left).map(|n| {
                        div()
                            .text_size(px(12.))
                            .line_height(px(17.))
                            .text_color(p.text_muted)
                            .child(n.clone())
                    }));
                    left = left.saturating_sub(names.len());
                }
                v_flex().gap(px(1.)).children(out).when(total > ITEMS_SHOWN, |d| {
                    d.child(div().pt(px(4.)).child(link(
                        format!("pair-all-{tag_id}"),
                        if all {
                            "Show fewer".to_owned()
                        } else {
                            format!("Show all {total}")
                        },
                        cx,
                        |this, key| {
                            if !this.addons.all_items.remove(&key) {
                                this.addons.all_items.insert(key);
                            }
                        },
                    )))
                })
            });
            v_flex()
                .px(px(12.))
                .pb(px(10.))
                .pl(px(34.))
                .gap(px(4.))
                .children(groups.iter().map(|(g, names)| {
                    h_flex()
                        .gap(px(6.))
                        .text_size(px(12.))
                        .child(div().text_color(p.text).child(g.clone()))
                        .child(
                            div()
                                .text_color(p.text_placeholder)
                                .child(names.len().to_string()),
                        )
                }))
                .when(unlisted > total, |d| {
                    d.child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.text_placeholder)
                            .child(format!("{} more not listed", unlisted - total)),
                    )
                })
                .child(link(
                    format!("pair-details-{tag_id}"),
                    if details { "Hide details" } else { "Details" }.to_owned(),
                    cx,
                    |this, key| {
                        if !this.addons.details.remove(&key) {
                            this.addons.details.insert(key);
                        }
                    },
                ))
                .children(raw)
        });
        v_flex()
            .when(!first, |d| d.border_t_1().border_color(p.border_variant))
            .child(
                h_flex()
                    .id(SharedString::from(format!("pair-{}-{}", pair.winner, pair.loser)))
                    .px(px(12.))
                    .py(px(8.))
                    .gap(px(10.))
                    .items_start()
                    .cursor_pointer()
                    .hover(move |s| s.bg(p.hover))
                    .child(
                        Icon::new(if wins {
                            IconName::ArrowUp
                        } else {
                            IconName::ArrowDown
                        })
                        .size(px(14.))
                        .mt(px(2.))
                        .text_color(if wins { p.success } else { p.warning }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.))
                            .line_height(px(19.))
                            .child(text),
                    )
                    .child(chevron(open, p))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.addons.open_pairs.remove(&key) {
                            this.addons.open_pairs.insert(key.clone());
                        }
                        cx.notify();
                    })),
            )
            .children(items)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use swofty_core::profile::ProfileAddon;

    use super::pinned_delta;

    fn addons(ids: &[&str]) -> Vec<ProfileAddon> {
        ids.iter()
            .map(|id| ProfileAddon {
                id: (*id).into(),
                enabled: true,
            })
            .collect()
    }

    #[test]
    fn nothing_moves_above_swoftymod_or_simplified_for_swofty() {
        let list = addons(&["swofty.core", "teamsimplified.mods", "a", "b"]);
        assert_eq!(
            pinned_delta(&list, "b", 3, 0),
            -1,
            "b stops below the pinned addons"
        );
        assert_eq!(pinned_delta(&list, "b", 3, 2), -1);
        assert_eq!(pinned_delta(&list, "a", 2, 3), 1);
        let list = addons(&["swofty.core", "a", "teamsimplified.mods", "b"]);
        assert_eq!(
            pinned_delta(&list, "a", 1, 1),
            1,
            "an addon above them moves below"
        );
    }
}
