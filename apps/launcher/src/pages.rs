use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable, Icon, Sizable, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::profile::{Profile, VANILLA};

use crate::library::{MapItem, ModItem, ProfileItem, kind_label, plural};
use crate::shell::{Page, Prompt, Shell};
use crate::theme::Palette;

pub(crate) const GUTTER: f32 = 28.;

fn tag(label: impl Into<SharedString>, p: Palette) -> Div {
    div()
        .px(px(6.))
        .h(px(20.))
        .flex()
        .items_center()
        .rounded(px(4.))
        .border_1()
        .border_color(p.border)
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.text_muted)
        .whitespace_nowrap()
        .child(label.into())
}

fn tile(size: f32, p: Palette) -> Div {
    div()
        .size(px(size))
        .flex_shrink_0()
        .rounded(px(5.))
        .bg(p.elevated)
        .border_1()
        .border_color(p.border_variant)
        .flex()
        .items_center()
        .justify_center()
}

pub(crate) fn badge(icon: IconName, size: f32, p: Palette) -> Div {
    tile(size, p).child(Icon::new(icon).size(px(size * 0.5)).text_color(p.text_muted))
}

fn monogram(text: &str, size: f32, p: Palette) -> Div {
    let letter: String = text.chars().take(1).flat_map(char::to_uppercase).collect();
    tile(size, p)
        .text_size(px(size * 0.42))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.text_muted)
        .child(letter)
}

pub(crate) fn avatar(name: &str, size: f32, p: Palette) -> Div {
    if name == VANILLA {
        badge(IconName::ShieldCheck, size, p)
    } else {
        monogram(name, size, p)
    }
}

pub(crate) fn label(title: impl Into<SharedString>, p: Palette) -> Div {
    div()
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.text_placeholder)
        .child(title.into())
}

fn fact(label: &'static str, value: impl IntoElement, p: Palette) -> Div {
    h_flex()
        .py(px(6.))
        .gap(px(12.))
        .border_b_1()
        .border_color(p.border_variant)
        .child(
            div()
                .w(px(104.))
                .flex_shrink_0()
                .text_size(px(13.))
                .text_color(p.text_muted)
                .child(label),
        )
        .child(div().flex_1().min_w_0().text_size(px(13.)).child(value))
}

fn stat(title: &'static str, value: impl Into<SharedString>, color: Hsla, p: Palette) -> Div {
    v_flex()
        .gap(px(3.))
        .child(label(title, p))
        .child(div().text_size(px(13.)).text_color(color).child(value.into()))
}

fn summary(text: &str) -> &str {
    text.find(". ").map_or(text, |i| &text[..=i])
}

fn chevron(open: bool, p: Palette) -> Icon {
    Icon::new(if open {
        IconName::ChevronUp
    } else {
        IconName::ChevronDown
    })
    .size(px(15.))
    .text_color(p.text_placeholder)
}

fn names(title: &'static str, items: &[String], p: Palette) -> Div {
    const SHOWN: usize = 6;
    let text = if items.is_empty() {
        "None".to_owned()
    } else {
        let mut t = items.iter().take(SHOWN).cloned().collect::<Vec<_>>().join(", ");
        if items.len() > SHOWN {
            t.push_str(&format!(" and {} more", items.len() - SHOWN));
        }
        t
    };
    v_flex().gap(px(3.)).child(label(title, p)).child(
        div()
            .text_size(px(13.))
            .line_height(px(19.))
            .text_color(if items.is_empty() {
                p.text_placeholder
            } else {
                p.text
            })
            .child(text),
    )
}

impl Shell {
    fn header(
        &self,
        page: Page,
        subtitle: String,
        actions: impl IntoElement,
        p: Palette,
    ) -> impl IntoElement {
        h_flex()
            .px(px(GUTTER))
            .pt(px(22.))
            .pb(px(16.))
            .gap(px(16.))
            .items_end()
            .flex_shrink_0()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(3.))
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(page.label()),
                    )
                    .child(div().text_size(px(13.)).text_color(p.text_muted).child(subtitle))
                    .when_some(self.library.error.clone(), |d, e| {
                        d.child(div().text_size(px(13.)).text_color(p.error).child(e))
                    }),
            )
            .child(h_flex().gap(px(8.)).pb(px(2.)).child(actions))
    }

    fn list(&self, id: &'static str, rows: Vec<AnyElement>, p: Palette) -> impl IntoElement {
        v_flex()
            .id(id)
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(GUTTER))
            .pt(px(4.))
            .pb(px(24.))
            .child(
                v_flex()
                    .flex_shrink_0()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(p.border)
                    .overflow_hidden()
                    .children(rows),
            )
    }

    fn empty(
        &self,
        page: Page,
        title: &'static str,
        body: &'static str,
        action: impl IntoElement,
        p: Palette,
    ) -> AnyElement {
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .pb(px(48.))
            .child(badge(page.icon(), 44., p))
            .child(
                div()
                    .pt(px(8.))
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .max_w(px(400.))
                    .text_center()
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .text_color(p.text_muted)
                    .child(body),
            )
            .child(div().pt(px(8.)).child(action))
            .into_any_element()
    }

    fn row(&self, first: bool, open: bool, p: Palette) -> Div {
        v_flex()
            .flex_shrink_0()
            .when(!first, |d| d.border_t_1().border_color(p.border_variant))
            .when(open, |d| d.bg(p.surface))
    }

    fn row_head(&self, id: String, open: bool, p: Palette) -> Stateful<Div> {
        h_flex()
            .id(SharedString::from(id))
            .h(px(54.))
            .px(px(14.))
            .gap(px(12.))
            .cursor_pointer()
            .when(!open, |d| d.hover(move |s| s.bg(p.hover)))
    }

    fn toggle(
        &self,
        id: String,
        on: bool,
        cx: &mut Context<Self>,
        set: impl Fn(&mut Profile, bool) -> swofty_core::Result<()> + 'static,
    ) -> AnyElement {
        let switch = Switch::new(SharedString::from(id.clone()))
            .checked(on)
            .small()
            .disabled(self.vanilla())
            .on_change(cx.listener(move |this, on: &bool, window, cx| {
                let on = *on;
                this.update_profile(window, cx, |p| set(p, on));
            }));
        div()
            .id(SharedString::from(format!("{id}-hold")))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(switch)
            .into_any_element()
    }

    fn vanilla(&self) -> bool {
        self.library.profile.is_vanilla()
    }

    fn install_mod(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt(
            Prompt {
                title: "Install mod",
                description: "A mod folder that contains swofty.json.".into(),
                initial: String::new(),
                placeholder: "/path/to/my-mod",
                ok: "Install",
                browse: Some(PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some("Install".into()),
                }),
                action: Rc::new(|store, value| {
                    let m = store
                        .install_mod(std::path::Path::new(value))
                        .map_err(|e| e.to_string())?;
                    Ok(format!("Installed {} {}", m.name, m.version))
                }),
            },
            window,
            cx,
        );
    }

    fn import_map(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt(
            Prompt {
                title: "Import map",
                description: "A track package zip, or the download that contains one.".into(),
                initial: String::new(),
                placeholder: "/path/to/track.zip",
                ok: "Import",
                browse: Some(PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: Some("Import".into()),
                }),
                action: Rc::new(|store, value| {
                    let m = store
                        .import_map(std::path::Path::new(value))
                        .map_err(|e| e.to_string())?;
                    Ok(format!("Imported {}", m.display_name()))
                }),
            },
            window,
            cx,
        );
    }

    fn state_line(&self, on: bool, what: &str, p: Palette) -> (String, Hsla) {
        let profile = &self.library.profile.name;
        if self.vanilla() {
            (format!("Switch away from {VANILLA} to use {what}"), p.warning)
        } else if on {
            (format!("On in {profile}"), p.success)
        } else {
            (format!("Off in {profile}"), p.text_muted)
        }
    }

    pub(crate) fn mods_page(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let installed = plural(self.library.mods.len(), "mod installed", "mods installed");
        let subtitle = if self.vanilla() {
            format!("{installed} · {VANILLA} plays the game without them")
        } else {
            format!(
                "{installed} · {} on in {}",
                self.library.enabled_mods(),
                self.library.profile.name
            )
        };
        let actions = Button::new("install-mod")
            .outline()
            .icon(IconName::Plus)
            .label("Install mod…")
            .on_click(cx.listener(|this, _, window, cx| this.install_mod(window, cx)));
        let rows: Vec<AnyElement> = self
            .library
            .mods
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let open = self.open_mod.as_ref() == Some(&m.manifest.id);
                self.mod_row(m, i == 0, open, p, cx)
            })
            .collect();
        v_flex()
            .size_full()
            .child(self.header(Page::Mods, subtitle, actions, p))
            .child(self.list("mods", rows, p))
            .into_any_element()
    }

    fn mod_row(
        &self,
        m: &ModItem,
        first: bool,
        open: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let man = &m.manifest;
        let id = man.id.clone();
        let builtin = man.is_builtin();
        let trailing = if builtin {
            let label = if self.vanilla() {
                format!("Off in {VANILLA}")
            } else {
                "Always on".to_owned()
            };
            h_flex()
                .gap(px(5.))
                .text_size(px(12.))
                .text_color(p.text_placeholder)
                .whitespace_nowrap()
                .child(Icon::new(IconName::Lock).size(px(12.)))
                .child(label)
                .into_any_element()
        } else {
            let id = id.clone();
            self.toggle(format!("toggle-{id}"), m.enabled, cx, move |p, on| {
                p.set_enabled(&id, on)
            })
        };
        let head = self
            .row_head(format!("mod-{id}"), open, p)
            .child(badge(
                if builtin {
                    IconName::Sparkles
                } else {
                    IconName::Package
                },
                32.,
                p,
            ))
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
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(man.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.text_placeholder)
                                    .child(man.version.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(p.text_muted)
                            .truncate()
                            .child(
                                man.description
                                    .as_deref()
                                    .map_or(man.id.clone(), |d| summary(d).to_owned()),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .w(px(200.))
                    .gap(px(4.))
                    .flex_shrink_0()
                    .children(man.kind.iter().map(|k| tag(kind_label(*k), p))),
            )
            .child(div().w(px(96.)).flex().justify_end().child(trailing))
            .child(chevron(open, p))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_mod = (this.open_mod.as_ref() != Some(&id)).then(|| id.clone());
                cx.notify();
            }));
        self.row(first, open, p)
            .child(head)
            .when(open, |d| d.child(self.mod_body(m, p)))
            .into_any_element()
    }

    fn mod_body(&self, m: &ModItem, p: Palette) -> impl IntoElement {
        let man = &m.manifest;
        let builds = if man.compat.game_builds.is_empty() {
            "any".to_owned()
        } else {
            man.compat
                .game_builds
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        };
        let (compat, compat_color) = match self.install.as_ref().and_then(|i| i.build_id) {
            Some(b) if man.compat.accepts_build(b) => (format!("{builds} · matches {b}"), p.success),
            Some(b) => (format!("{builds} · not tested on {b}"), p.warning),
            None => (builds, p.text),
        };
        let ops = &man.operations;
        let changes: Vec<String> = [
            (ops.database.len(), "database edit", "database edits"),
            (ops.erp.len(), "ERP asset", "ERP assets"),
            (ops.ui.len(), "UI patch", "UI patches"),
            (ops.files.len(), "file", "files"),
            (ops.native.len(), "native plugin", "native plugins"),
        ]
        .into_iter()
        .filter(|(n, ..)| *n > 0)
        .map(|(n, one, many)| plural(n, one, many))
        .collect();
        let (status, status_color) = if man.is_builtin() && !self.vanilla() {
            ("On in every modded profile".to_owned(), p.success)
        } else {
            self.state_line(m.enabled, "mods", p)
        };
        let more = man.description.clone().filter(|d| summary(d).len() < d.len());
        v_flex()
            .gap(px(16.))
            .pl(px(58.))
            .pr(px(20.))
            .pt(px(14.))
            .pb(px(18.))
            .border_t_1()
            .border_color(p.border_variant)
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_x(px(32.))
                    .gap_y(px(12.))
                    .child(stat("STATUS", status, status_color, p))
                    .child(stat("VERSION", man.version.to_string(), p.text, p))
                    .child(stat("AUTHOR", man.author.clone(), p.text, p))
                    .when_some(man.license.clone(), |d, l| d.child(stat("LICENSE", l, p.text, p)))
                    .child(stat("GAME BUILDS", compat, compat_color, p))
                    .when(!man.requires.is_empty(), |d| {
                        d.child(stat(
                            "REQUIRES",
                            man.requires
                                .iter()
                                .map(|r| format!("{} {}", r.id, r.version))
                                .collect::<Vec<_>>()
                                .join(", "),
                            p.text,
                            p,
                        ))
                    })
                    .child(stat("ID", man.id.clone(), p.text_muted, p))
                    .when_some(man.homepage.clone(), |d, h| {
                        d.child(stat("HOMEPAGE", h, p.text_muted, p))
                    }),
            )
            .when_some(more, |d, text| {
                d.child(
                    div()
                        .max_w(px(720.))
                        .text_size(px(13.))
                        .line_height(px(20.))
                        .text_color(p.text_muted)
                        .child(text),
                )
            })
            .when(!changes.is_empty(), |d| {
                d.child(
                    v_flex()
                        .gap(px(3.))
                        .child(label("WHAT IT CHANGES", p))
                        .child(div().text_size(px(13.)).child(changes.join(", "))),
                )
            })
            .when(!man.options.is_empty(), |d| {
                d.child(
                    v_flex()
                        .gap(px(6.))
                        .child(label("OPTIONS", p))
                        .children(man.options.iter().map(|o| {
                            v_flex()
                                .max_w(px(720.))
                                .px(px(12.))
                                .py(px(8.))
                                .gap(px(2.))
                                .rounded(px(5.))
                                .border_1()
                                .border_color(p.border_variant)
                                .child(
                                    h_flex()
                                        .gap(px(8.))
                                        .items_baseline()
                                        .child(
                                            div()
                                                .text_size(px(13.))
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(o.label.clone().unwrap_or_else(|| o.id.clone())),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(p.text_placeholder)
                                                .child(format!("default {}", o.default)),
                                        ),
                                )
                                .when_some(o.description.clone(), |d, t| {
                                    d.child(div().text_size(px(13.)).text_color(p.text_muted).child(t))
                                })
                        })),
                )
            })
    }

    pub(crate) fn maps_page(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let imported = plural(self.library.maps.len(), "map imported", "maps imported");
        let subtitle = if self.vanilla() {
            format!("{imported} · {VANILLA} plays the game without them")
        } else {
            format!(
                "{imported} · {} on in {}",
                self.library.maps.iter().filter(|m| m.enabled).count(),
                self.library.profile.name
            )
        };
        let import = |id: &'static str, cx: &mut Context<Self>| {
            Button::new(id)
                .outline()
                .icon(IconName::Upload)
                .label("Import map…")
                .on_click(cx.listener(|this, _, window, cx| this.import_map(window, cx)))
        };
        let header = self.header(Page::Maps, subtitle, import("import-map", cx), p);
        if self.library.maps.is_empty() {
            let cta = import("import-map-empty", cx);
            return v_flex()
                .size_full()
                .child(header)
                .child(self.empty(
                    Page::Maps,
                    "No maps yet",
                    "Import a track package to add a circuit. It stays in your library and can be switched on per profile.",
                    cta,
                    p,
                ))
                .into_any_element();
        }
        let rows: Vec<AnyElement> = self
            .library
            .maps
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let open = self.open_map.as_ref() == Some(&m.package.track_code);
                self.map_row(m, i == 0, open, p, cx)
            })
            .collect();
        v_flex()
            .size_full()
            .child(header)
            .child(self.list("maps", rows, p))
            .into_any_element()
    }

    fn map_row(
        &self,
        m: &MapItem,
        first: bool,
        open: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let pkg = &m.package;
        let code = pkg.track_code.clone();
        let lead = match m.art.flag.clone() {
            Some(flag) => div()
                .w(px(32.))
                .h(px(32.))
                .flex_shrink_0()
                .rounded(px(5.))
                .overflow_hidden()
                .border_1()
                .border_color(p.border_variant)
                .child(img(flag).size_full().object_fit(ObjectFit::Cover))
                .into_any_element(),
            None => monogram(pkg.display_name(), 32., p).into_any_element(),
        };
        let line = [
            pkg.activation.copy_slot.as_ref().map(|s| format!("Replaces {s}")),
            pkg.author_name.as_ref().map(|a| format!("by {a}")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        let toggle = {
            let code = code.clone();
            self.toggle(format!("map-{code}"), m.enabled, cx, move |p, on| {
                p.set_map(&code, on)
            })
        };
        let head = self
            .row_head(format!("map-row-{code}"), open, p)
            .child(lead)
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
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(pkg.display_name().to_owned()),
                            )
                            .when_some(pkg.version.clone(), |d, v| {
                                d.child(div().text_size(px(12.)).text_color(p.text_placeholder).child(v))
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(p.text_muted)
                            .truncate()
                            .child(if line.is_empty() { code.clone() } else { line }),
                    ),
            )
            .child(div().w(px(96.)).flex().justify_end().child(toggle))
            .child(chevron(open, p))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_map = (this.open_map.as_ref() != Some(&code)).then(|| code.clone());
                cx.notify();
            }));
        self.row(first, open, p)
            .child(head)
            .when(open, |d| d.child(self.map_body(m, p, cx)))
            .into_any_element()
    }

    fn map_body(&self, m: &MapItem, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let pkg = &m.package;
        let a = &pkg.activation;
        let code = pkg.track_code.clone();
        let preview = div()
            .w(px(384.))
            .h(px(216.))
            .flex_shrink_0()
            .rounded(px(5.))
            .overflow_hidden()
            .border_1()
            .border_color(p.border)
            .map(|d| match m.art.preview.clone() {
                Some(path) => d.child(img(path).size_full().object_fit(ObjectFit::Cover)),
                None => d
                    .bg(p.elevated)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(monogram(pkg.display_name(), 56., p)),
            });
        h_flex()
            .items_start()
            .gap(px(24.))
            .pl(px(58.))
            .pr(px(20.))
            .pt(px(14.))
            .pb(px(18.))
            .border_t_1()
            .border_color(p.border_variant)
            .child(preview)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .max_w(px(560.))
                    .child({
                        let (status, color) = self.state_line(m.enabled, "maps", p);
                        fact("Status", div().text_color(color).child(status), p)
                    })
                    .child(fact("Track code", pkg.track_code.clone(), p))
                    .when_some(a.copy_from_track.clone(), |d, t| d.child(fact("Based on", t, p)))
                    .when_some(a.copy_slot.clone(), |d, t| d.child(fact("Replaces slot", t, p)))
                    .child(fact(
                        "Database",
                        plural(
                            a.overrides.len() + a.track_overrides.len(),
                            "value changed",
                            "values changed",
                        ),
                        p,
                    ))
                    .child(fact("Archive", pkg.erp.clone(), p))
                    .when_some(pkg.author_name.clone(), |d, a| d.child(fact("Author", a, p)))
                    .child(
                        h_flex().pt(px(14.)).child(
                            Button::new("remove-map")
                                .danger()
                                .ghost()
                                .small()
                                .icon(IconName::Trash)
                                .label("Remove from library")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    if let Err(e) = this.store.remove_map(&code) {
                                        window.push_notification(Notification::error(e.to_string()), cx);
                                    }
                                    this.reload(cx);
                                })),
                        ),
                    ),
            )
    }

    pub(crate) fn profiles_page(&mut self, p: Palette, wide: bool, cx: &mut Context<Self>) -> AnyElement {
        let subtitle =
            "Each profile is its own set of mods and maps. Play starts F1 25 with the active one.".into();
        let actions = Button::new("new-profile")
            .outline()
            .icon(IconName::Plus)
            .label("New profile")
            .on_click(cx.listener(|this, _, window, cx| this.profile_prompt(None, false, window, cx)));
        let active = self.library.profile.name.clone();
        let mut cards: Vec<AnyElement> = self
            .library
            .profiles
            .iter()
            .map(|item| self.profile_card(item, item.name == active, p, cx))
            .collect();
        cards.push(
            v_flex()
                .id("new-profile-card")
                .min_h(px(180.))
                .items_center()
                .justify_center()
                .gap(px(4.))
                .rounded(px(6.))
                .border_1()
                .border_dashed()
                .border_color(p.border)
                .cursor_pointer()
                .text_color(p.text_muted)
                .hover(move |s| s.bg(p.surface).text_color(p.text))
                .child(Icon::new(IconName::Plus).size(px(18.)))
                .child(
                    div()
                        .pt(px(6.))
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .child("New profile"),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(p.text_placeholder)
                        .child("Starts with SwoftyMod and nothing else"),
                )
                .on_click(cx.listener(|this, _, window, cx| this.profile_prompt(None, false, window, cx)))
                .into_any_element(),
        );
        v_flex()
            .size_full()
            .child(self.header(Page::Profiles, subtitle, actions, p))
            .child(
                div()
                    .id("profiles")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(GUTTER))
                    .pt(px(4.))
                    .pb(px(24.))
                    .child(
                        div()
                            .grid()
                            .grid_cols(if wide { 3 } else { 2 })
                            .gap(px(12.))
                            .children(cards),
                    ),
            )
            .into_any_element()
    }

    fn profile_prompt(
        &mut self,
        from: Option<String>,
        rename: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (title, ok, initial) = match (&from, rename) {
            (Some(f), true) => ("Rename profile", "Rename", f.clone()),
            (Some(f), false) => ("Duplicate profile", "Duplicate", format!("{f}-copy")),
            (None, _) => ("New profile", "Create", String::new()),
        };
        self.prompt(
            Prompt {
                title,
                description: "Letters, digits, - and _. Each profile keeps its own mods, maps and saves."
                    .into(),
                initial,
                placeholder: "my-profile",
                ok,
                browse: None,
                action: Rc::new(move |store, name| {
                    let result = match (&from, rename) {
                        (Some(f), true) => store.rename_profile(f, name),
                        (f, _) => store.create_profile(name, f.as_deref()),
                    };
                    result.map_err(|e| e.to_string())?;
                    Ok(format!("Saved profile {name}"))
                }),
            },
            window,
            cx,
        );
    }

    fn profile_card(
        &self,
        item: &ProfileItem,
        active: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = item.name.clone();
        let vanilla = item.vanilla();
        let contents = if vanilla {
            v_flex()
                .gap(px(3.))
                .child(label("CONTENTS", p))
                .child(
                    div()
                        .text_size(px(13.))
                        .line_height(px(19.))
                        .child("No mods, no maps"),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(p.text_muted)
                        .child("Plays F1 25 exactly as shipped."),
                )
        } else {
            v_flex()
                .gap(px(12.))
                .child(names("MODS", &item.mods, p))
                .child(names("MAPS", &item.maps, p))
        };
        let button = |id: &str| Button::new(SharedString::from(format!("{id}-{name}")));
        let (rename, duplicate, switch) = (name.clone(), name.clone(), name.clone());
        let footer = h_flex()
            .gap(px(2.))
            .pt(px(12.))
            .border_t_1()
            .border_color(p.border_variant)
            .child(if active {
                h_flex()
                    .gap(px(6.))
                    .text_size(px(13.))
                    .text_color(p.text_muted)
                    .child(
                        Icon::new(IconName::CircleCheck)
                            .size(px(14.))
                            .text_color(p.accent),
                    )
                    .child("Used when you press Play")
                    .into_any_element()
            } else {
                button("use")
                    .outline()
                    .small()
                    .icon(IconName::ArrowRightLeft)
                    .label("Switch to this")
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.switch_profile(&switch, window, cx)),
                    )
                    .into_any_element()
            })
            .child(div().flex_1())
            .when(!vanilla, |d| {
                d.child(
                    button("rename")
                        .ghost()
                        .small()
                        .icon(IconName::Pencil)
                        .tooltip("Rename")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.profile_prompt(Some(rename.clone()), true, window, cx)
                        })),
                )
                .child(
                    button("duplicate")
                        .ghost()
                        .small()
                        .icon(IconName::Copy)
                        .tooltip("Duplicate")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.profile_prompt(Some(duplicate.clone()), false, window, cx)
                        })),
                )
            });
        v_flex()
            .size_full()
            .p(px(16.))
            .gap(px(14.))
            .rounded(px(6.))
            .border_1()
            .border_color(if active { p.accent } else { p.border })
            .child(
                h_flex()
                    .gap(px(10.))
                    .child(avatar(&name, 32., p))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.text_muted)
                                    .child(item.summary()),
                            ),
                    )
                    .when(active, |d| {
                        d.child(
                            h_flex()
                                .gap(px(6.))
                                .text_size(px(12.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(p.accent)
                                .child(div().size(px(6.)).rounded_full().bg(p.accent))
                                .child("Active"),
                        )
                    }),
            )
            .child(div().flex_1().child(contents))
            .child(footer)
            .into_any_element()
    }

    pub(crate) fn go(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }
}
