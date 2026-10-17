use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable, Icon, Sizable, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::baseline::Identity;
use swofty_core::import::{self, Route};
use swofty_core::install::Game;
use swofty_core::launcher::Progress;
use swofty_core::profile::Profile;
use swofty_core::registry::{Host, Registry};
use swofty_core::store::Store;
use swofty_core::view::{self, Art, Block, Body, ButtonKind, Control, Effect, Event, Row, Style, Tone};

use crate::game::Install;
use crate::library::plural;
use crate::pages::{GUTTER, badge, chevron, fact, label, monogram, names, stat, tag};
use crate::shell::Shell;
use crate::theme::Palette;

type Shared = Arc<Mutex<Option<(f32, String)>>>;

struct Reporter(Shared);

impl Progress for Reporter {
    fn report(&self, done: f32, text: &str) {
        if let Ok(mut p) = self.0.lock() {
            *p = Some((done, text.to_owned()));
        }
    }
}

fn game(store: &Store, install: Option<&Install>) -> Option<Game> {
    let install = install?;
    let identity = Identity::detect_cached(&store.root, &install.dir, install.build_id).ok()?;
    Some(Game {
        dir: install.dir.clone(),
        user_reg: None,
        identity,
    })
}

fn host(store: &Store, addon: &str, profile: &Profile, game: Option<&Game>) -> Result<Host, String> {
    Host::new(store, addon, profile, game).map_err(|e| e.to_string())
}

pub(crate) fn icon(i: view::Icon) -> IconName {
    use view::Icon as I;
    match i {
        I::Activity => IconName::Activity,
        I::ArrowRightLeft => IconName::ArrowRightLeft,
        I::ChevronDown => IconName::ChevronDown,
        I::ChevronUp => IconName::ChevronUp,
        I::CircleAlert => IconName::CircleAlert,
        I::CircleCheck => IconName::CircleCheck,
        I::Copy => IconName::Copy,
        I::Eraser => IconName::Eraser,
        I::FolderOpen => IconName::FolderOpen,
        I::Gamepad => IconName::Gamepad2,
        I::Layers => IconName::Layers,
        I::Lock => IconName::Lock,
        I::Map => IconName::Map,
        I::Package => IconName::Package,
        I::Pencil => IconName::Pencil,
        I::Play => IconName::Play,
        I::Plus => IconName::Plus,
        I::Puzzle => IconName::Puzzle,
        I::Settings => IconName::Settings,
        I::ShieldCheck => IconName::ShieldCheck,
        I::Trash => IconName::Trash,
        I::Upload => IconName::Upload,
    }
}

fn tone(t: Tone, p: Palette) -> Hsla {
    match t {
        Tone::Neutral => p.text,
        Tone::Muted => p.text_muted,
        Tone::Accent => p.accent,
        Tone::Good => p.success,
        Tone::Warn => p.warning,
        Tone::Bad => p.error,
    }
}

fn art(a: &Art, p: Palette) -> AnyElement {
    match a {
        Art::None => badge(IconName::Package, 32., p).into_any_element(),
        Art::Monogram(text) => monogram(text, 32., p).into_any_element(),
        Art::Image(path) => div()
            .size(px(32.))
            .flex_shrink_0()
            .rounded(px(5.))
            .overflow_hidden()
            .border_1()
            .border_color(p.border_variant)
            .child(img(path.clone()).size_full().object_fit(ObjectFit::Cover))
            .into_any_element(),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

impl Shell {
    pub(crate) fn refresh_views(&mut self, cx: &mut Context<Self>) {
        let store = self.store.clone();
        let library = &self.library;
        let registry = library.registry.clone();
        let profile = library.profile.clone();
        let install = self.install.clone();
        let sections: Vec<(String, String)> = library
            .sections
            .iter()
            .map(|s| (s.addon.clone(), s.spec.id.clone()))
            .collect();
        if sections.is_empty() {
            self.views.clear();
            return;
        }
        cx.spawn(async move |this, cx| {
            let views = cx
                .background_executor()
                .spawn(async move {
                    let game = game(&store, install.as_ref());
                    sections
                        .into_iter()
                        .map(|(addon, section)| {
                            let view = registry
                                .get(&addon)
                                .ok_or_else(|| format!("{addon} is gone"))
                                .and_then(|e| {
                                    let host = host(&store, &addon, &profile, game.as_ref())?;
                                    e.addon.view(&host.cx(), &section).map_err(|e| e.to_string())
                                });
                            (format!("{addon}/{section}"), view)
                        })
                        .collect()
                })
                .await;
            this.update(cx, |s, cx| {
                s.views = views;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn send(
        &mut self,
        addon: &str,
        section: &str,
        control: String,
        event: Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self.library.registry.get(addon).cloned() else {
            return;
        };
        let store = self.store.clone();
        let profile = self.library.profile.clone();
        let install = self.install.clone();
        let (addon, section) = (addon.to_owned(), section.to_owned());
        self.busy.insert(control.clone());
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let id = addon.clone();
            let effect = cx
                .background_executor()
                .spawn(async move {
                    let game = game(&store, install.as_ref());
                    let host = host(&store, &addon, &profile, game.as_ref())?;
                    entry
                        .addon
                        .event(&host.cx(), &section, event)
                        .map_err(|e| e.to_string())
                })
                .await;
            this.update_in(cx, |s, window, cx| {
                s.busy.remove(&control);
                s.effect(&id, effect, window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn effect(
        &mut self,
        addon: &str,
        effect: Result<Effect, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match effect {
            Ok(Effect::None) => cx.notify(),
            Ok(Effect::Reload) => self.reload(cx),
            Ok(Effect::Notice(text)) => {
                window.push_notification(Notification::info(text), cx);
                self.reload(cx);
            }
            Ok(Effect::SetAddonEnabled(on)) => self.update_profile(window, cx, |p| p.set_enabled(addon, on)),
            Err(e) => {
                window.push_notification(Notification::error(e), cx);
                cx.notify();
            }
        }
    }

    pub(crate) fn drop_paths(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        self.dropping = false;
        if paths.is_empty() {
            return;
        }
        let store = self.store.clone();
        let registry = self.library.registry.clone();
        let profile = self.library.profile.clone();
        let install = self.install.clone();
        let shared: Shared = Arc::new(Mutex::new(Some((
            0.,
            format!("Importing {}", plural(paths.len(), "item", "items")),
        ))));
        self.progress = shared.lock().ok().and_then(|p| p.clone());
        let reporter = Reporter(shared.clone());
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let now = shared.lock().ok().and_then(|p| p.clone());
                let done = now.is_none();
                if this
                    .update(cx, |s, cx| {
                        s.progress = now;
                        cx.notify();
                    })
                    .is_err()
                    || done
                {
                    break;
                }
            }
        })
        .detach();
        cx.spawn_in(window, async move |this, cx| {
            let (done, failed) = cx
                .background_executor()
                .spawn(async move {
                    let result = import_all(&store, &registry, &profile, install.as_ref(), &paths, &reporter);
                    if let Ok(mut p) = reporter.0.lock() {
                        *p = None;
                    }
                    result
                })
                .await;
            this.update_in(cx, |s, window, cx| {
                if !done.is_empty() {
                    window.push_notification(Notification::success(done.join("\n")), cx);
                }
                if !failed.is_empty() {
                    window.push_notification(Notification::error(failed.join("\n")), cx);
                }
                s.rediscover = true;
                s.reload(cx);
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn section_page(&mut self, index: usize, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(section) = self.library.sections.get(index) else {
            return div().into_any_element();
        };
        let (addon, id, title) = (
            section.addon.clone(),
            section.spec.id.clone(),
            section.spec.title.clone(),
        );
        let section_icon = icon(section.spec.icon);
        let view = match self.views.get(&format!("{addon}/{id}")) {
            Some(Ok(v)) => v.clone(),
            Some(Err(e)) => {
                return v_flex()
                    .size_full()
                    .child(self.header(title, String::new(), div(), p))
                    .child(self.empty(section_icon, "This page could not load", e.clone(), div(), p))
                    .into_any_element();
            }
            None => {
                return v_flex()
                    .size_full()
                    .child(self.header(title, "Loading…".into(), div(), p))
                    .into_any_element();
            }
        };
        let actions: Vec<AnyElement> = view
            .actions
            .iter()
            .map(|b| self.button(&addon, &id, b, false, cx))
            .collect();
        let notices: Vec<AnyElement> = view
            .notices
            .iter()
            .enumerate()
            .map(|(i, n)| {
                h_flex()
                    .id(SharedString::from(format!("notice-{i}")))
                    .mx(px(GUTTER))
                    .mb(px(10.))
                    .px(px(12.))
                    .py(px(8.))
                    .gap(px(10.))
                    .flex_shrink_0()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(p.border_variant)
                    .bg(p.surface)
                    .text_size(px(13.))
                    .child(
                        Icon::new(IconName::CircleAlert)
                            .size(px(14.))
                            .text_color(tone(n.tone, p)),
                    )
                    .child(div().flex_1().min_w_0().text_color(p.text).child(n.text.clone()))
                    .children(n.action.as_ref().map(|b| self.button(&addon, &id, b, true, cx)))
                    .into_any_element()
            })
            .collect();
        let body = match &view.body {
            Body::List { rows, reorder } => {
                let rows: Vec<AnyElement> = rows
                    .iter()
                    .enumerate()
                    .map(|(i, r)| self.view_row(&addon, &id, r, i, rows.len(), *reorder, p, cx))
                    .collect();
                self.list(SharedString::from(format!("section-{addon}-{id}")), rows, p)
                    .into_any_element()
            }
            Body::Empty { title, text, action } => {
                let action = action
                    .as_ref()
                    .map(|b| self.button(&addon, &id, b, false, cx))
                    .unwrap_or_else(|| div().into_any_element());
                self.empty(section_icon, title.clone(), text.clone(), action, p)
            }
            Body::Blocks(blocks) => v_flex()
                .id(SharedString::from(format!("section-{addon}-{id}")))
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px(px(GUTTER))
                .pt(px(4.))
                .pb(px(24.))
                .gap(px(16.))
                .children(blocks.iter().map(|b| self.block(&addon, &id, b, p, cx)))
                .into_any_element(),
        };
        v_flex()
            .size_full()
            .child(self.header(
                title,
                view.subtitle.clone(),
                h_flex().gap(px(8.)).children(actions),
                p,
            ))
            .children(notices)
            .child(body)
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn view_row(
        &self,
        addon: &str,
        section: &str,
        r: &Row,
        index: usize,
        count: usize,
        reorder: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = format!("{addon}/{section}/{}", r.id);
        let open = self.open_row.as_ref() == Some(&key);
        let expandable = !r.detail.is_empty() || reorder;
        let trailing = r.trailing.as_ref().map(|c| self.control(addon, section, c, cx));
        let toggle_key = key.clone();
        let head = self
            .row_head(format!("row-{key}"), open, p)
            .child(art(&r.art, p))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(r.title.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(p.text_muted)
                            .truncate()
                            .child(r.subtitle.clone()),
                    ),
            )
            .child(
                h_flex().gap(px(4.)).flex_shrink_0().children(
                    r.tags
                        .iter()
                        .map(|t| tag(t.text.clone(), p).text_color(tone(t.tone, p))),
                ),
            )
            .children(trailing.map(|t| div().w(px(96.)).flex().justify_end().child(t)))
            .when(expandable, |d| d.child(chevron(open, p)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_row = (this.open_row.as_ref() != Some(&toggle_key)).then(|| toggle_key.clone());
                cx.notify();
            }));
        let moves = reorder.then(|| {
            let mv = |label: &'static str, ic: IconName, to: usize, enabled: bool, cx: &mut Context<Self>| {
                let (addon, section, row) = (addon.to_owned(), section.to_owned(), r.id.clone());
                let control = format!("{key}/move-{label}");
                Button::new(SharedString::from(control.clone()))
                    .outline()
                    .small()
                    .icon(ic)
                    .label(label)
                    .disabled(!enabled || self.busy.contains(&control))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let event = Event::Move { row: row.clone(), to };
                        this.send(&addon, &section, control.clone(), event, window, cx)
                    }))
            };
            h_flex()
                .gap(px(6.))
                .child(mv(
                    "Move up",
                    IconName::ArrowUp,
                    index.saturating_sub(1),
                    index > 0,
                    cx,
                ))
                .child(mv(
                    "Move down",
                    IconName::ArrowDown,
                    index + 1,
                    index + 1 < count,
                    cx,
                ))
        });
        self.row(index == 0, open, p)
            .child(head)
            .when(open, |d| {
                d.child(
                    v_flex()
                        .gap(px(16.))
                        .pl(px(58.))
                        .pr(px(20.))
                        .pt(px(14.))
                        .pb(px(18.))
                        .border_t_1()
                        .border_color(p.border_variant)
                        .children(r.detail.iter().map(|b| self.block(addon, section, b, p, cx)))
                        .children(moves),
                )
            })
            .into_any_element()
    }

    fn block(&self, addon: &str, section: &str, b: &Block, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        match b {
            Block::Stats(facts) => h_flex()
                .flex_wrap()
                .gap_x(px(32.))
                .gap_y(px(12.))
                .children(
                    facts
                        .iter()
                        .map(|f| stat(f.label.to_uppercase(), f.value.clone(), p.text, p)),
                )
                .into_any_element(),
            Block::Facts(facts) => v_flex()
                .max_w(px(560.))
                .children(facts.iter().map(|f| fact(f.label.clone(), f.value.clone(), p)))
                .into_any_element(),
            Block::Label(text) => label(text.to_uppercase(), p).into_any_element(),
            Block::Text { text, tone: t } => div()
                .max_w(px(720.))
                .text_size(px(13.))
                .line_height(px(20.))
                .text_color(tone(*t, p))
                .child(text.clone())
                .into_any_element(),
            Block::Names { label, items } => names(label.to_uppercase(), items, p).into_any_element(),
            Block::Table { columns, rows } => {
                let cell = |text: String, head: bool| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(if head { 11. } else { 13. }))
                        .text_color(if head { p.text_placeholder } else { p.text })
                        .child(text)
                };
                v_flex()
                    .max_w(px(760.))
                    .rounded(px(5.))
                    .border_1()
                    .border_color(p.border_variant)
                    .child(
                        h_flex()
                            .px(px(12.))
                            .py(px(6.))
                            .gap(px(12.))
                            .children(columns.iter().map(|c| cell(c.to_uppercase(), true))),
                    )
                    .children(rows.iter().map(|row| {
                        h_flex()
                            .px(px(12.))
                            .py(px(6.))
                            .gap(px(12.))
                            .border_t_1()
                            .border_color(p.border_variant)
                            .children(row.iter().map(|c| cell(c.clone(), false)))
                    }))
                    .into_any_element()
            }
            Block::Buttons(buttons) => h_flex()
                .gap(px(6.))
                .children(buttons.iter().map(|b| self.button(addon, section, b, true, cx)))
                .into_any_element(),
        }
    }

    fn control(&self, addon: &str, section: &str, c: &Control, cx: &mut Context<Self>) -> AnyElement {
        match c {
            Control::Switch { id, on, enabled } => {
                let control = format!("{addon}/{section}/{id}");
                let hold = SharedString::from(format!("hold-{control}"));
                let (addon, section, id) = (addon.to_owned(), section.to_owned(), id.clone());
                let switch = Switch::new(SharedString::from(format!("switch-{control}")))
                    .checked(*on)
                    .small()
                    .disabled(!enabled || self.busy.contains(&control))
                    .on_change(cx.listener(move |this, on: &bool, window, cx| {
                        let event = Event::Toggle {
                            id: id.clone(),
                            on: *on,
                        };
                        this.send(&addon, &section, control.clone(), event, window, cx)
                    }));
                div()
                    .id(hold)
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(switch)
                    .into_any_element()
            }
            Control::Choice { id, options, current } => {
                let control = format!("{addon}/{section}/{id}");
                let shell = cx.entity().downgrade();
                let (options, current) = (options.clone(), *current);
                let (addon, section, id) = (addon.to_owned(), section.to_owned(), id.clone());
                Button::new(SharedString::from(format!("choice-{control}")))
                    .outline()
                    .small()
                    .min_w(px(140.))
                    .label(options.get(current).cloned().unwrap_or_default())
                    .dropdown_caret(true)
                    .disabled(self.busy.contains(&control))
                    .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                        for (i, text) in options.iter().enumerate() {
                            let (shell, addon, section, id, control) = (
                                shell.clone(),
                                addon.clone(),
                                section.clone(),
                                id.clone(),
                                control.clone(),
                            );
                            menu =
                                menu.item(PopupMenuItem::new(text.clone()).checked(i == current).on_click(
                                    move |_, window, cx| {
                                        let event = Event::Choose {
                                            id: id.clone(),
                                            index: i,
                                        };
                                        shell
                                            .update(cx, |this, cx| {
                                                this.send(
                                                    &addon,
                                                    &section,
                                                    control.clone(),
                                                    event,
                                                    window,
                                                    cx,
                                                )
                                            })
                                            .ok();
                                    },
                                ));
                        }
                        menu
                    })
                    .into_any_element()
            }
            Control::Button(b) => self.button(addon, section, b, true, cx),
        }
    }

    fn button(
        &self,
        addon: &str,
        section: &str,
        b: &view::Button,
        small: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let control = format!("{addon}/{section}/{}", b.id);
        let mut button = Button::new(SharedString::from(format!("button-{control}")))
            .label(b.label.clone())
            .loading(self.busy.contains(&control))
            .disabled(self.busy.contains(&control));
        button = match b.style {
            Style::Primary => button.primary(),
            Style::Outline => button.outline(),
            Style::Ghost => button.ghost(),
            Style::Danger => button.danger().ghost(),
        };
        if small {
            button = button.small();
        }
        if let Some(i) = b.icon {
            button = button.icon(icon(i));
        }
        let (addon, section, kind, id) = (addon.to_owned(), section.to_owned(), b.kind.clone(), b.id.clone());
        button
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                let click = Event::Click { id: id.clone() };
                match &kind {
                    ButtonKind::Event => this.send(&addon, &section, control.clone(), click, window, cx),
                    ButtonKind::Confirm { title, text } => {
                        let shell = cx.entity().downgrade();
                        let (title, text) = (
                            SharedString::from(title.clone()),
                            SharedString::from(text.clone()),
                        );
                        let (addon, section, control, id) =
                            (addon.clone(), section.clone(), control.clone(), id.clone());
                        window.open_alert_dialog(cx, move |alert, _, _| {
                            let (shell, addon, section, control, id) = (
                                shell.clone(),
                                addon.clone(),
                                section.clone(),
                                control.clone(),
                                id.clone(),
                            );
                            alert
                                .title(title.clone())
                                .description(text.clone())
                                .show_cancel(true)
                                .on_ok(move |_, window, cx| {
                                    let click = Event::Click { id: id.clone() };
                                    shell
                                        .update(cx, |this, cx| {
                                            this.send(&addon, &section, control.clone(), click, window, cx)
                                        })
                                        .ok();
                                    true
                                })
                        });
                    }
                    ButtonKind::Pick {
                        title,
                        folders,
                        multiple,
                        ..
                    } => {
                        let rx = cx.prompt_for_paths(PathPromptOptions {
                            files: true,
                            directories: *folders,
                            multiple: *multiple,
                            prompt: Some(SharedString::from(title.clone())),
                        });
                        let (addon, section, control, id) =
                            (addon.clone(), section.clone(), control.clone(), id.clone());
                        cx.spawn_in(window, async move |this, cx| {
                            if let Ok(Ok(Some(paths))) = rx.await {
                                this.update_in(cx, |s, window, cx| {
                                    let event = Event::Picked { id, paths };
                                    s.send(&addon, &section, control, event, window, cx)
                                })
                                .ok();
                            }
                        })
                        .detach();
                    }
                }
            }))
            .into_any_element()
    }

    pub(crate) fn drop_overlay(&self, p: Palette) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .p(px(16.))
            .bg(p.background.opacity(0.85))
            .child(
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap(px(8.))
                    .rounded(px(10.))
                    .border_2()
                    .border_dashed()
                    .border_color(p.accent)
                    .child(badge(IconName::Upload, 44., p).text_color(p.accent))
                    .child(
                        div()
                            .pt(px(8.))
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Drop to import"),
                    )
                    .child(
                        div()
                            .max_w(px(420.))
                            .text_center()
                            .text_size(px(13.))
                            .line_height(px(19.))
                            .text_color(p.text_muted)
                            .child("Addon folders, .swoftymod files, track packages and the files your addons import."),
                    ),
            )
    }

    pub(crate) fn progress_card(&self, p: Palette) -> Option<impl IntoElement> {
        let (done, text) = self.progress.clone()?;
        Some(
            v_flex()
                .absolute()
                .right(px(16.))
                .bottom(px(84.))
                .w(px(320.))
                .p(px(12.))
                .gap(px(8.))
                .rounded(px(6.))
                .border_1()
                .border_color(p.border)
                .bg(p.elevated)
                .child(div().text_size(px(13.)).truncate().child(text))
                .child(
                    div()
                        .h(px(4.))
                        .w_full()
                        .rounded_full()
                        .bg(p.border_variant)
                        .child(
                            div()
                                .h_full()
                                .w(relative(done.clamp(0., 1.)))
                                .rounded_full()
                                .bg(p.accent),
                        ),
                ),
        )
    }
}

fn import_all(
    store: &Store,
    registry: &Registry,
    profile: &Profile,
    install: Option<&Install>,
    paths: &[PathBuf],
    progress: &Reporter,
) -> (Vec<String>, Vec<String>) {
    let (batches, unclaimed) = import::route_all(registry, paths);
    let mut done = Vec::new();
    let mut failed: Vec<String> = unclaimed
        .iter()
        .map(|p| format!("No addon imports {}", file_name(p)))
        .collect();
    let mut game_files = None;
    for (route, paths) in batches {
        match route {
            Route::Addon => {
                for path in paths {
                    match store.install_addon(&path) {
                        Ok(m) => done.push(format!("Installed {} {}", m.name, m.version)),
                        Err(e) => failed.push(format!("{}: {e}", file_name(&path))),
                    }
                }
            }
            Route::Map => {
                for path in paths {
                    match store.import_map(&path) {
                        Ok(m) => done.push(format!("Imported {}", m.display_name())),
                        Err(e) => failed.push(format!("{}: {e}", file_name(&path))),
                    }
                }
            }
            Route::Importer(id) => {
                let Some(importer) = registry.get(&id).and_then(|e| e.addon.importer()) else {
                    continue;
                };
                let game = game_files.get_or_insert_with(|| game(store, install));
                let result = host(store, &id, profile, game.as_ref()).and_then(|h| {
                    importer
                        .import(&h.cx(), &paths, progress)
                        .map_err(|e| e.to_string())
                });
                match result {
                    Ok(lines) => done.extend(lines),
                    Err(e) => failed.push(e),
                }
            }
        }
    }
    (done, failed)
}
