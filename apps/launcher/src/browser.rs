//! The Addon Browser: the addons published to the Swofty registry, to search,
//! read about, and install or update in one click.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::catalog::{self, Addon, Trouble, Update};
use swofty_core::update::current_version;

use crate::library::{Source, plural};
use crate::news::format_date;
use crate::pages::{GUTTER, badge, label, tag};
use crate::shell::{Page, Shell};
use crate::theme::Palette;

const STALE: Duration = Duration::from_secs(30 * 60);
const ICONS_DIR: &str = "icons";
const CARD_W: f32 = 330.;
const GAP: f32 = 14.;
const SIDE_W: f32 = 340.;
const NATIVE: &str = "Runs code in the game";

fn kind_label(kind: &str) -> &str {
    match kind {
        "database" => "Database",
        "erp" => "Archives",
        "text" | "ui" => "Text & UI",
        "simplified" => "Simplified",
        "files" => "Files",
        "native" => "Code",
        other => other,
    }
}

fn kinds(a: &Addon) -> String {
    let mut seen: Vec<&str> = Vec::new();
    for k in &a.kind {
        let l = kind_label(k);
        if !seen.contains(&l) {
            seen.push(l);
        }
    }
    seen.join(", ")
}

fn day(iso: &str) -> String {
    format_date(iso.get(..10).unwrap_or(iso))
}

fn host(url: &str) -> &str {
    url.split_once("://").map_or(url, |(_, rest)| rest)
}

fn web_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

fn native_badge(id: String, p: Palette) -> Stateful<Div> {
    h_flex()
        .id(SharedString::from(id))
        .h(px(20.))
        .px(px(6.))
        .gap(px(4.))
        .flex_shrink_0()
        .rounded(px(4.))
        .bg(p.warning.opacity(0.14))
        .text_color(p.warning)
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .child(Icon::new(IconName::Cpu).size(px(11.)))
        .child(NATIVE)
        .tooltip(|window, cx| {
            Tooltip::new("It ships native code that runs inside F1 25 while it's on. Install it only if you trust its author.")
                .build(window, cx)
        })
}

#[derive(Clone, Debug, PartialEq)]
enum Offer {
    Install,
    Installed,
    Update(semver::Version),
    TooOld,
    Working(Option<u64>),
}

struct Fetched {
    addons: Option<Vec<Addon>>,
    icons: Vec<(String, Arc<Image>)>,
    error: Option<Trouble>,
}

fn fetch(root: &Path, shown: Vec<Addon>, known: BTreeSet<String>) -> Fetched {
    let (addons, error) = match catalog::fetch(root) {
        Ok(addons) => (Some(addons), None),
        Err(e) => (None, Some(e)),
    };
    let dir = root.join(catalog::CACHE_DIR).join(ICONS_DIR);
    let icons = addons
        .as_ref()
        .unwrap_or(&shown)
        .iter()
        .filter_map(|a| a.icon.clone())
        .filter(|url| web_url(url) && !known.contains(url))
        .filter_map(|url| crate::news::image(&dir, &url).map(|i| (url, i)))
        .collect();
    Fetched { addons, icons, error }
}

pub(crate) struct Browser {
    pub(crate) addons: Vec<Addon>,
    pub(crate) updates: Vec<Update>,
    pub(crate) toasts: Vec<Notification>,
    icons: HashMap<String, Arc<Image>>,
    checked: Option<Instant>,
    live: bool,
    saved: bool,
    fetching: bool,
    trouble: Option<Trouble>,
    search: Entity<InputState>,
    open: Option<String>,
    working: HashMap<String, Arc<AtomicU64>>,
    ticking: bool,
    prompted: BTreeSet<(String, semver::Version)>,
    _search: Subscription,
}

impl Browser {
    pub(crate) fn new(root: &Path, window: &mut Window, cx: &mut Context<Shell>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search the Addon Browser"));
        let changed = cx.subscribe(&search, |_, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                cx.notify();
            }
        });
        let cached = catalog::cached(root);
        Self {
            saved: cached.is_some(),
            addons: cached.unwrap_or_default(),
            updates: Vec::new(),
            toasts: Vec::new(),
            icons: HashMap::new(),
            checked: None,
            live: false,
            fetching: false,
            trouble: None,
            search,
            open: None,
            working: HashMap::new(),
            ticking: false,
            prompted: BTreeSet::new(),
            _search: changed,
        }
    }

    fn stale(&self) -> bool {
        !self.fetching && self.checked.is_none_or(|t| t.elapsed() >= STALE)
    }

    fn subtitle(&self) -> String {
        if self.fetching {
            return "Checking the registry…".into();
        }
        let base = catalog::base_url();
        if self.live {
            return format!(
                "{} from {}",
                plural(self.addons.len(), "addon", "addons"),
                host(&base)
            );
        }
        let shown = if self.saved {
            "showing the addons saved last time"
        } else {
            "no addons saved yet"
        };
        match self.trouble {
            Some(Trouble::Failed(_)) => format!("Couldn't load the registry · {shown}"),
            _ => format!("Offline · {shown}"),
        }
    }

    fn find(&self, id: &str) -> Option<&Addon> {
        self.addons.iter().find(|a| a.id == id)
    }
}

impl Shell {
    pub(crate) fn watch_catalog(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(STALE).await;
                if this.update(cx, |s, cx| s.fetch_catalog(true, cx)).is_err() {
                    break;
                }
            }
        })
    }

    pub(crate) fn fetch_catalog(&mut self, prompt: bool, cx: &mut Context<Self>) {
        if self.browser.fetching {
            return;
        }
        self.browser.fetching = true;
        let root = self.store.root.clone();
        let shown = self.browser.addons.clone();
        let known = self.browser.icons.keys().cloned().collect();
        cx.spawn(async move |this, cx| {
            let fetched = cx
                .background_executor()
                .spawn(async move { fetch(&root, shown, known) })
                .await;
            this.update(cx, |s, cx| {
                let b = &mut s.browser;
                b.fetching = false;
                b.checked = Some(Instant::now());
                if let Some(e) = &fetched.error {
                    eprintln!("swofty-launcher: could not check the addon registry: {e}");
                }
                b.trouble = fetched.error;
                if let Some(addons) = fetched.addons {
                    b.addons = addons;
                    b.live = true;
                    b.saved = true;
                }
                b.icons.extend(fetched.icons);
                s.sync_catalog_updates();
                if prompt {
                    s.prompt_updates(cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn sync_catalog_updates(&mut self) {
        let installed = self
            .library
            .addons
            .iter()
            .filter(|a| a.source != Source::Builtin)
            .map(|a| &a.manifest);
        self.browser.updates = catalog::updates(installed, &self.browser.addons, &current_version());
    }

    pub(crate) fn catalog_update(&self, id: &str) -> Option<&Update> {
        self.browser.updates.iter().find(|u| u.id == id)
    }

    fn prompt_updates(&mut self, cx: &mut Context<Self>) {
        let fresh: Vec<(String, semver::Version)> = self
            .browser
            .updates
            .iter()
            .map(|u| (u.id.clone(), u.to.version.clone()))
            .filter(|k| !self.browser.prompted.contains(k))
            .collect();
        if fresh.is_empty() {
            return;
        }
        self.browser.prompted.extend(fresh);
        if self.page == Page::Browser {
            return;
        }
        let n = self.browser.updates.len();
        let text = format!("{} available", plural(n, "addon update", "addon updates"));
        let shell = cx.entity().downgrade();
        let note = Notification::info(text)
            .title("Addon updates")
            .action(move |_, _, cx| {
                let shell = shell.clone();
                let note = cx.entity().downgrade();
                Button::new("open-addon-browser")
                    .outline()
                    .small()
                    .label("Open Addon Browser")
                    .on_click(move |_, window, cx| {
                        shell.update(cx, |s, cx| s.open_page(Page::Browser, cx)).ok();
                        note.update(cx, |n, cx| n.dismiss(window, cx)).ok();
                    })
            });
        self.browser.toasts.push(note);
    }

    fn offer(&self, a: &Addon) -> Offer {
        if let Some(bytes) = self.browser.working.get(&a.id) {
            let size = a.offered(&current_version()).map_or(0, |v| v.size);
            let done = bytes.load(Ordering::Relaxed);
            return Offer::Working((size > 0).then(|| (done * 100 / size).min(100)));
        }
        let installed = self.library.addons.iter().find(|m| m.manifest.id == a.id);
        match (a.offered(&current_version()), installed) {
            (Some(v), Some(m)) if v.version > m.manifest.version => Offer::Update(v.version.clone()),
            (_, Some(_)) => Offer::Installed,
            (Some(_), None) => Offer::Install,
            (None, None) => Offer::TooOld,
        }
    }

    pub(crate) fn install_from_catalog(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.browser.working.contains_key(id) {
            return;
        }
        let Some(addon) = self.browser.find(id).cloned() else {
            return;
        };
        let Some(version) = addon.offered(&current_version()).cloned() else {
            return;
        };
        let updating = self.library.addons.iter().any(|m| m.manifest.id == addon.id);
        let bytes = Arc::new(AtomicU64::new(0));
        self.browser.working.insert(addon.id.clone(), bytes.clone());
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let (job, v) = (addon.clone(), version.clone());
            let result = cx
                .background_executor()
                .spawn(
                    async move { catalog::install(&store, &job, &v, &|n| bytes.store(n, Ordering::Relaxed)) },
                )
                .await;
            this.update(cx, |s, cx| {
                s.browser.working.remove(&addon.id);
                let note = match result {
                    Ok(m) => Notification::success(format!(
                        "{} {} {}",
                        if updating { "Updated" } else { "Installed" },
                        m.name,
                        m.version
                    )),
                    Err(e) => Notification::error(e.to_string()).title(format!(
                        "Could not {} {}",
                        if updating { "update" } else { "install" },
                        addon.name
                    )),
                };
                s.browser.toasts.push(note);
                s.reload(cx);
            })
            .ok();
        })
        .detach();
        self.tick_browser(cx);
        cx.notify();
    }

    fn update_all(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<String> = self.browser.updates.iter().map(|u| u.id.clone()).collect();
        for id in ids {
            self.install_from_catalog(&id, cx);
        }
    }

    fn tick_browser(&mut self, cx: &mut Context<Self>) {
        if self.browser.ticking {
            return;
        }
        self.browser.ticking = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(150)).await;
                let done = this
                    .update(cx, |s, cx| {
                        let done = s.browser.working.is_empty();
                        if done {
                            s.browser.ticking = false;
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

    pub(crate) fn offer_button(&self, a: &Addon, id: String, cx: &mut Context<Self>) -> Button {
        let target = a.id.clone();
        let button = Button::new(SharedString::from(id)).small();
        match self.offer(a) {
            Offer::Install => button.primary().icon(IconName::Download).label("Install"),
            Offer::Update(v) => button
                .primary()
                .icon(IconName::CircleArrowUp)
                .label(format!("Update to v{v}")),
            Offer::Installed => {
                return button
                    .outline()
                    .icon(IconName::Check)
                    .label("Installed")
                    .disabled(true);
            }
            Offer::TooOld => {
                return button.outline().label("Needs a newer launcher").disabled(true);
            }
            Offer::Working(pct) => {
                return button
                    .primary()
                    .loading(true)
                    .label(match pct {
                        Some(n) if n < 100 => format!("Downloading… {n}%"),
                        _ => "Installing…".to_owned(),
                    })
                    .disabled(true);
            }
        }
        .on_click(cx.listener(move |this, _, _, cx| {
            cx.stop_propagation();
            this.install_from_catalog(&target, cx);
        }))
    }

    fn catalog_icon(&self, a: &Addon, size: f32, p: Palette) -> Div {
        match a.icon.as_ref().and_then(|u| self.browser.icons.get(u)) {
            Some(image) => div()
                .size(px(size))
                .flex_shrink_0()
                .rounded(px(size * 0.22))
                .overflow_hidden()
                .child(img(image.clone()).size_full()),
            None => badge(IconName::Package, size, p),
        }
    }

    pub(crate) fn browser_page(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        if self.browser.stale() {
            self.fetch_catalog(false, cx);
        }
        let n = self.browser.updates.len();
        let pending = self
            .browser
            .updates
            .iter()
            .filter(|u| !self.browser.working.contains_key(&u.id))
            .count();
        let actions = h_flex()
            .gap(px(8.))
            .when(n > 0, |d| {
                d.child(
                    Button::new("browser-update-all")
                        .primary()
                        .icon(IconName::CircleArrowUp)
                        .label(format!("Update all ({n})"))
                        .disabled(pending == 0)
                        .on_click(cx.listener(|this, _, _, cx| this.update_all(cx))),
                )
            })
            .child(
                Button::new("browser-refresh")
                    .ghost()
                    .small()
                    .icon(IconName::RefreshCw)
                    .label("Refresh")
                    .disabled(self.browser.fetching)
                    .on_click(cx.listener(|this, _, _, cx| this.fetch_catalog(false, cx))),
            );
        let open = self
            .browser
            .open
            .clone()
            .filter(|id| self.browser.find(id).is_some());
        let content = match open {
            Some(id) => self.browser_detail(&id, p, cx),
            None => self.browser_grid(p, cx),
        };
        v_flex()
            .size_full()
            .child(self.header(Page::Browser.label(), self.browser.subtitle(), actions, p))
            .child(content)
            .into_any_element()
    }

    fn browser_grid(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let query = self.browser.search.read(cx).value().trim().to_lowercase();
        let shown: Vec<&Addon> = self.browser.addons.iter().filter(|a| a.matches(&query)).collect();
        let bar = h_flex()
            .px(px(GUTTER))
            .pb(px(16.))
            .gap(px(12.))
            .flex_shrink_0()
            .items_center()
            .child(
                div().w(px(320.)).child(
                    Input::new(&self.browser.search).cleanable(true).prefix(
                        Icon::new(IconName::Search)
                            .size(px(14.))
                            .text_color(p.text_placeholder),
                    ),
                ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(p.text_placeholder)
                    .child(if query.is_empty() {
                        plural(shown.len(), "addon", "addons")
                    } else {
                        format!("{} of {}", shown.len(), self.browser.addons.len())
                    }),
            );
        if shown.is_empty() {
            let (title, body) = if self.browser.addons.is_empty() {
                (
                    "No addons to show yet",
                    if self.browser.fetching {
                        "Checking the registry for addons…"
                    } else {
                        "The registry couldn't be reached. Refresh to try again."
                    },
                )
            } else {
                ("No addons match", "Try another name, author or category.")
            };
            return v_flex()
                .flex_1()
                .min_h_0()
                .child(bar)
                .child(self.empty(IconName::PackageSearch, title, body, div(), p))
                .into_any_element();
        }
        let columns = ((self.content_width + GAP) / (CARD_W + GAP)).floor().max(1.) as usize;
        let width = ((self.content_width - GAP * (columns - 1) as f32) / columns as f32).floor() - 1.;
        let mut rows = Vec::new();
        for chunk in shown.chunks(columns) {
            let cards: Vec<AnyElement> = chunk.iter().map(|a| self.browser_card(a, width, p, cx)).collect();
            rows.push(
                h_flex()
                    .flex_shrink_0()
                    .items_stretch()
                    .gap(px(GAP))
                    .children(cards)
                    .into_any_element(),
            );
        }
        v_flex()
            .flex_1()
            .min_h_0()
            .child(bar)
            .child(
                v_flex()
                    .id("browser-cards")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .px(px(GUTTER))
                    .pb(px(24.))
                    .gap(px(GAP))
                    .children(rows),
            )
            .into_any_element()
    }

    fn tags(&self, a: &Addon, id: String, p: Palette) -> Option<Div> {
        let category = a.category_label();
        let types = kinds(a);
        let native = a.native(&current_version());
        (category.is_some() || !types.is_empty() || native).then(|| {
            h_flex()
                .flex_wrap()
                .gap(px(6.))
                .when_some(category, |d, c| d.child(tag(c, p).text_color(p.text)))
                .when(!types.is_empty(), |d| d.child(tag(types, p)))
                .when(native, |d| d.child(native_badge(id, p)))
        })
    }

    fn browser_card(&self, a: &Addon, width: f32, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let launcher = current_version();
        let offered = a.offered(&launcher).or_else(|| a.newest());
        let updating = self.catalog_update(&a.id).is_some();
        let id = a.id.clone();
        let stats = [
            offered.map(|v| format!("v{}", v.version)),
            Some(plural(a.downloads as usize, "download", "downloads")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        v_flex()
            .id(SharedString::from(format!("browser-card-{}", a.id)))
            .w(px(width))
            .flex_none()
            .p(px(16.))
            .gap(px(12.))
            .rounded(px(10.))
            .border_1()
            .border_color(if updating { p.accent.opacity(0.6) } else { p.border })
            .bg(p.surface)
            .cursor_pointer()
            .hover(move |s| s.border_color(p.accent).bg(p.hover))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.browser.open = Some(id.clone());
                cx.notify();
            }))
            .child(
                h_flex()
                    .gap(px(12.))
                    .items_start()
                    .child(self.catalog_icon(a, 48., p))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(a.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.text_muted)
                                    .truncate()
                                    .child(match a.author.trim() {
                                        "" => a.id.clone(),
                                        author => format!("by {author}"),
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .h(px(38.))
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .text_color(p.text_muted)
                    .line_clamp(2)
                    .text_ellipsis()
                    .child(a.summary.clone()),
            )
            .children(self.tags(a, format!("browser-native-{}", a.id), p))
            .child(div().flex_1())
            .child(
                h_flex()
                    .gap(px(10.))
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.))
                            .text_color(p.text_placeholder)
                            .child(stats),
                    )
                    .child(self.offer_button(a, format!("browser-offer-{}", a.id), cx)),
            )
            .into_any_element()
    }

    fn browser_detail(&self, id: &str, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(a) = self.browser.find(id) else {
            return div().into_any_element();
        };
        let launcher = current_version();
        let mono = cx.theme().mono_font_family.clone();
        let installed = self
            .library
            .addons
            .iter()
            .find(|m| m.manifest.id == a.id)
            .map(|m| m.manifest.version.clone());
        let offered = a.offered(&launcher);
        let stats = [
            offered.or_else(|| a.newest()).map(|v| format!("v{}", v.version)),
            Some(plural(a.downloads as usize, "download", "downloads")),
            (!a.updated_at.is_empty()).then(|| format!("Updated {}", day(&a.updated_at))),
            installed.as_ref().map(|v| format!("v{v} installed")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        let back = h_flex()
            .id("browser-back")
            .gap(px(6.))
            .flex_shrink_0()
            .cursor_pointer()
            .text_size(px(13.))
            .text_color(p.text_muted)
            .hover(move |s| s.text_color(p.text))
            .child(Icon::new(IconName::ArrowLeft).size(px(14.)))
            .child("All addons")
            .on_click(cx.listener(|this, _, _, cx| {
                this.browser.open = None;
                cx.notify();
            }));
        let homepage = a
            .homepage
            .clone()
            .filter(|u| u.starts_with("https://"))
            .map(|url| {
                Button::new("browser-homepage")
                    .ghost()
                    .small()
                    .icon(IconName::ExternalLink)
                    .label("Homepage")
                    .on_click(move |_, _, cx| cx.open_url(&url))
            });
        let hero =
            h_flex()
                .gap(px(16.))
                .items_start()
                .flex_shrink_0()
                .child(self.catalog_icon(a, 64., p))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(6.))
                        .child(
                            div()
                                .text_size(px(22.))
                                .line_height(px(28.))
                                .font_weight(FontWeight::BOLD)
                                .child(a.name.clone()),
                        )
                        .child(div().text_size(px(13.)).text_color(p.text_muted).child(
                            match a.author.trim() {
                                "" => a.id.clone(),
                                author => format!("by {author} · {}", a.id),
                            },
                        ))
                        .children(self.tags(a, "browser-detail-native".into(), p))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(p.text_placeholder)
                                .child(stats),
                        ),
                )
                .child(
                    h_flex()
                        .gap(px(8.))
                        .flex_shrink_0()
                        .children(homepage)
                        .child(self.offer_button(a, "browser-detail-offer".into(), cx)),
                );
        let native_note = a.native(&launcher).then(|| {
            h_flex()
                .gap(px(10.))
                .items_start()
                .p(px(12.))
                .rounded(px(8.))
                .bg(p.warning.opacity(0.08))
                .border_1()
                .border_color(p.warning.opacity(0.4))
                .child(
                    Icon::new(IconName::TriangleAlert)
                        .size(px(15.))
                        .text_color(p.warning),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(NATIVE),
                        )
                        .child(div().text_size(px(13.)).text_color(p.text_muted).child(
                            "This addon ships native code (a .dll) that runs inside F1 25 while it's on. \
                             Install it only if you trust its author.",
                        )),
                )
        });
        let description = if a.description.trim().is_empty() {
            vec![
                div()
                    .text_color(p.text.opacity(0.84))
                    .child(if a.summary.is_empty() {
                        "No description yet.".to_owned()
                    } else {
                        a.summary.clone()
                    })
                    .into_any_element(),
            ]
        } else {
            crate::news::render_markdown(&format!("browser-{}", a.id), &a.description, p, &mono)
        };
        let about = v_flex()
            .flex_1()
            .min_w_0()
            .p(px(24.))
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .text_size(px(14.))
            .line_height(px(22.))
            .child(div().pb(px(12.)).child(label("ABOUT", p)))
            .children(description);
        let listed = a.listed();
        let count = listed.len();
        let versions: Vec<AnyElement> = listed
            .into_iter()
            .enumerate()
            .map(|(i, v)| {
                let fits = v.accepts(&launcher);
                let current = installed.as_ref() == Some(&v.version);
                v_flex()
                    .px(px(16.))
                    .py(px(12.))
                    .gap(px(6.))
                    .when(i > 0, |d| d.border_t_1().border_color(p.border_variant))
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(format!("v{}", v.version)),
                            )
                            .when(current, |d| d.child(tag("Installed", p).text_color(p.success)))
                            .when(v.native, |d| d.child(tag("Code", p).text_color(p.warning)))
                            .child(div().flex_1())
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.text_placeholder)
                                    .child(day(&v.published_at)),
                            ),
                    )
                    .when(!fits, |d| {
                        d.child(div().text_size(px(12.)).text_color(p.warning).child(format!(
                            "Needs launcher {}",
                            v.launcher.as_ref().map_or_else(String::new, |r| r.to_string())
                        )))
                    })
                    .child(
                        v_flex()
                            .text_size(px(13.))
                            .line_height(px(19.))
                            .when(v.changelog.trim().is_empty(), |d| {
                                d.child(div().text_color(p.text_placeholder).child("No changelog"))
                            })
                            .when(!v.changelog.trim().is_empty(), |d| {
                                d.children(crate::news::render_markdown(
                                    &format!("browser-{}-{}", a.id, v.version),
                                    &v.changelog,
                                    p,
                                    &mono,
                                ))
                            }),
                    )
                    .into_any_element()
            })
            .collect();
        let changelog = v_flex()
            .w(px(SIDE_W))
            .flex_shrink_0()
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .overflow_hidden()
            .child(
                h_flex()
                    .px(px(16.))
                    .h(px(46.))
                    .items_center()
                    .border_b_1()
                    .border_color(p.border)
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Versions"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.text_placeholder)
                            .child(plural(count, "version", "versions")),
                    ),
            )
            .children(versions);
        v_flex()
            .id("browser-detail")
            .flex_1()
            .min_h_0()
            .overflow_y_scrollbar()
            .px(px(GUTTER))
            .pb(px(24.))
            .gap(px(18.))
            .child(back)
            .child(hero)
            .children(native_note)
            .child(h_flex().items_start().gap(px(16.)).child(about).child(changelog))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{host, kind_label};

    #[test]
    fn names_kinds_and_hosts() {
        assert_eq!(kind_label("erp"), "Archives");
        assert_eq!(kind_label("native"), "Code");
        assert_eq!(host("https://api.f1.swofty.net"), "api.f1.swofty.net");
        assert_eq!(host("http://localhost:8471"), "localhost:8471");
    }
}
