use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable, Icon, Sizable, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::manifest::Kind;
use swofty_core::profile::{Profile, VANILLA};

use crate::library::{MapItem, ModItem, ProfileItem, kind_label, plural};
use crate::shell::{Page, Prompt, Shell};
use crate::theme::{Palette, deepen, glow, gradient, hue};

fn chip(label: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .px(px(8.))
        .py(px(2.))
        .rounded(px(6.))
        .bg(color.opacity(0.14))
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(label.into())
}

fn tile(size: f32, color: Hsla) -> Div {
    div()
        .size(px(size))
        .flex_shrink_0()
        .rounded(px(size * 0.28))
        .bg(gradient(color, deepen(color)))
        .shadow(glow(color, 0.22))
        .flex()
        .items_center()
        .justify_center()
}

pub(crate) fn badge(icon: IconName, size: f32, color: Hsla, p: Palette) -> Div {
    tile(size, color).child(Icon::new(icon).size(px(size * 0.5)).text_color(p.on_color))
}

fn monogram(text: &str, size: f32, color: Hsla, p: Palette) -> Div {
    let letter: String = text.chars().take(1).flat_map(char::to_uppercase).collect();
    tile(size, color)
        .text_size(px(size * 0.44))
        .font_weight(FontWeight::BOLD)
        .text_color(p.on_color)
        .child(letter)
}

pub(crate) fn avatar(name: &str, size: f32, p: Palette) -> Div {
    if name == VANILLA {
        badge(IconName::ShieldCheck, size, p.text_placeholder, p)
    } else {
        monogram(name, size, hue(name), p)
    }
}

fn section(title: &'static str, p: Palette) -> Div {
    div()
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.text_placeholder)
        .child(title)
}

fn fact(label: &'static str, value: impl IntoElement, p: Palette) -> Div {
    h_flex()
        .py(px(7.))
        .gap(px(12.))
        .border_b_1()
        .border_color(p.border_variant)
        .child(
            div()
                .w(px(112.))
                .flex_shrink_0()
                .text_size(px(13.))
                .text_color(p.text_muted)
                .child(label),
        )
        .child(div().flex_1().min_w_0().text_size(px(13.)).child(value))
}

fn stat(label: &'static str, value: impl Into<SharedString>, color: Hsla, p: Palette) -> Div {
    v_flex()
        .gap(px(3.))
        .child(section(label, p))
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
    .size(px(16.))
    .text_color(p.text_placeholder)
}

fn kind_color(kind: Kind, p: Palette) -> Hsla {
    match kind {
        Kind::Database => hue("database"),
        Kind::Erp => p.warning,
        Kind::Ui => p.mods,
        Kind::Files => p.maps,
        Kind::Native => p.profiles,
    }
}

fn names(title: &'static str, items: &[String], color: Hsla, p: Palette) -> Div {
    const SHOWN: usize = 6;
    v_flex()
        .gap(px(6.))
        .child(section(title, p))
        .child(if items.is_empty() {
            div()
                .text_size(px(13.))
                .text_color(p.text_placeholder)
                .child("None")
        } else {
            h_flex()
                .flex_wrap()
                .gap(px(6.))
                .children(items.iter().take(SHOWN).map(|n| chip(n.clone(), color)))
                .when(items.len() > SHOWN, |d| {
                    d.child(chip(format!("+{} more", items.len() - SHOWN), p.text_muted))
                })
        })
}

impl Shell {
    fn header(
        &self,
        page: Page,
        subtitle: String,
        actions: impl IntoElement,
        p: Palette,
    ) -> impl IntoElement {
        let color = page.color(p);
        h_flex()
            .px(px(32.))
            .pt(px(26.))
            .pb(px(22.))
            .gap(px(16.))
            .flex_shrink_0()
            .child(badge(page.icon(), 46., color, p))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(24.))
                            .font_weight(FontWeight::BOLD)
                            .child(page.label()),
                    )
                    .child(div().text_size(px(13.)).text_color(p.text_muted).child(subtitle))
                    .when_some(self.library.error.clone(), |d, e| {
                        d.child(div().text_size(px(13.)).text_color(p.error).child(e))
                    }),
            )
            .child(h_flex().gap(px(8.)).child(actions))
    }

    fn list(&self, id: &'static str, rows: Vec<AnyElement>) -> impl IntoElement {
        v_flex()
            .id(id)
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(32.))
            .pt(px(4.))
            .pb(px(28.))
            .gap(px(10.))
            .children(rows)
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
            .gap(px(10.))
            .pb(px(48.))
            .child(badge(page.icon(), 64., page.color(p), p))
            .child(
                div()
                    .pt(px(8.))
                    .text_size(px(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .max_w(px(440.))
                    .text_center()
                    .text_color(p.text_muted)
                    .child(body),
            )
            .child(div().pt(px(6.)).child(action))
            .into_any_element()
    }

    fn card(&self, open: bool, color: Hsla, p: Palette) -> Div {
        v_flex()
            .flex_shrink_0()
            .rounded(px(14.))
            .border_1()
            .bg(p.surface)
            .overflow_hidden()
            .map(|d| {
                if open {
                    d.border_color(color.opacity(0.5)).shadow(glow(color, 0.14))
                } else {
                    d.border_color(p.border_variant).shadow(glow(p.background, 0.6))
                }
            })
    }

    fn card_head(&self, id: String, open: bool, color: Hsla, p: Palette) -> Stateful<Div> {
        h_flex()
            .id(SharedString::from(id))
            .px(px(16.))
            .py(px(12.))
            .gap(px(14.))
            .cursor_pointer()
            .map(|d| {
                if open {
                    d.bg(color.opacity(0.07))
                } else {
                    d.hover(move |s| s.bg(p.hover))
                }
            })
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
            .primary()
            .icon(IconName::Plus)
            .label("Install mod…")
            .on_click(cx.listener(|this, _, window, cx| this.install_mod(window, cx)));
        let rows: Vec<AnyElement> = self
            .library
            .mods
            .iter()
            .map(|m| self.mod_card(m, self.open_mod.as_ref() == Some(&m.manifest.id), p, cx))
            .collect();
        v_flex()
            .size_full()
            .child(self.header(Page::Mods, subtitle, actions, p))
            .child(self.list("mods", rows))
            .into_any_element()
    }

    fn mod_card(&self, m: &ModItem, open: bool, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let man = &m.manifest;
        let id = man.id.clone();
        let builtin = man.is_builtin();
        let color = if builtin { p.brand } else { hue(&man.id) };
        let trailing = if builtin {
            let (label, tint) = if self.vanilla() {
                (format!("Off in {VANILLA}"), p.text_muted)
            } else {
                ("Always on".to_owned(), p.brand)
            };
            h_flex()
                .gap(px(5.))
                .px(px(9.))
                .py(px(3.))
                .rounded(px(7.))
                .bg(tint.opacity(0.14))
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(tint)
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
            .card_head(format!("mod-{id}"), open, color, p)
            .child(badge(
                if builtin {
                    IconName::Sparkles
                } else {
                    IconName::Package
                },
                40.,
                color,
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
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(man.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.text_placeholder)
                                    .child(format!("v{}", man.version)),
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
                    .gap(px(6.))
                    .flex_shrink_0()
                    .children(man.kind.iter().map(|k| chip(kind_label(*k), kind_color(*k, p)))),
            )
            .child(trailing)
            .child(chevron(open, p))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_mod = (this.open_mod.as_ref() != Some(&id)).then(|| id.clone());
                cx.notify();
            }));
        self.card(open, color, p)
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
        let changes: Vec<(String, Hsla)> = [
            (
                ops.database.len(),
                "database edit",
                "database edits",
                Kind::Database,
            ),
            (ops.erp.len(), "ERP asset", "ERP assets", Kind::Erp),
            (ops.ui.len(), "UI patch", "UI patches", Kind::Ui),
            (ops.files.len(), "file", "files", Kind::Files),
            (ops.native.len(), "native plugin", "native plugins", Kind::Native),
        ]
        .into_iter()
        .filter(|(n, ..)| *n > 0)
        .map(|(n, one, many, k)| (plural(n, one, many), kind_color(k, p)))
        .collect();
        let (status, status_color) = if man.is_builtin() && !self.vanilla() {
            ("On in every modded profile".to_owned(), p.success)
        } else {
            self.state_line(m.enabled, "mods", p)
        };
        let more = man.description.clone().filter(|d| summary(d).len() < d.len());
        v_flex()
            .gap(px(18.))
            .px(px(20.))
            .pt(px(16.))
            .pb(px(20.))
            .border_t_1()
            .border_color(p.border_variant)
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_x(px(36.))
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
                        d.child(stat("HOMEPAGE", h, p.accent, p))
                    }),
            )
            .when_some(more, |d, text| {
                d.child(div().max_w(px(820.)).line_height(px(21.)).child(text))
            })
            .when(!changes.is_empty(), |d| {
                d.child(
                    v_flex().gap(px(8.)).child(section("WHAT IT CHANGES", p)).child(
                        h_flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .children(changes.into_iter().map(|(label, c)| chip(label, c))),
                    ),
                )
            })
            .when(!man.options.is_empty(), |d| {
                d.child(
                    v_flex()
                        .gap(px(8.))
                        .child(section("OPTIONS", p))
                        .children(man.options.iter().map(|o| {
                            v_flex()
                                .max_w(px(820.))
                                .px(px(12.))
                                .py(px(9.))
                                .gap(px(3.))
                                .rounded(px(9.))
                                .bg(p.elevated)
                                .child(
                                    h_flex()
                                        .gap(px(8.))
                                        .child(
                                            div()
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(o.label.clone().unwrap_or_else(|| o.id.clone())),
                                        )
                                        .child(chip(format!("default {}", o.default), p.accent)),
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
                .primary()
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
            .map(|m| {
                let open = self.open_map.as_ref() == Some(&m.package.track_code);
                self.map_card(m, open, p, cx)
            })
            .collect();
        v_flex()
            .size_full()
            .child(header)
            .child(self.list("maps", rows))
            .into_any_element()
    }

    fn map_card(&self, m: &MapItem, open: bool, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let pkg = &m.package;
        let code = pkg.track_code.clone();
        let color = hue(&code);
        let lead = match m.art.flag.clone() {
            Some(flag) => div()
                .w(px(56.))
                .h(px(36.))
                .flex_shrink_0()
                .rounded(px(7.))
                .overflow_hidden()
                .border_1()
                .border_color(p.border)
                .shadow(glow(color, 0.18))
                .child(img(flag).size_full().object_fit(ObjectFit::Cover))
                .into_any_element(),
            None => monogram(pkg.display_name(), 40., color, p).into_any_element(),
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
            .card_head(format!("map-row-{code}"), open, color, p)
            .child(lead)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(pkg.display_name().to_owned()),
                            )
                            .when_some(pkg.version.clone(), |d, v| {
                                d.child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(p.text_placeholder)
                                        .child(format!("v{v}")),
                                )
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
            .child(toggle)
            .child(chevron(open, p))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_map = (this.open_map.as_ref() != Some(&code)).then(|| code.clone());
                cx.notify();
            }));
        self.card(open, color, p)
            .child(head)
            .when(open, |d| d.child(self.map_body(m, color, p, cx)))
            .into_any_element()
    }

    fn map_body(&self, m: &MapItem, color: Hsla, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let pkg = &m.package;
        let a = &pkg.activation;
        let code = pkg.track_code.clone();
        let preview = div()
            .w(px(432.))
            .h(px(243.))
            .flex_shrink_0()
            .rounded(px(12.))
            .overflow_hidden()
            .border_1()
            .border_color(p.border)
            .shadow(glow(color, 0.18))
            .map(|d| match m.art.preview.clone() {
                Some(path) => d.child(img(path).size_full().object_fit(ObjectFit::Cover)),
                None => d
                    .bg(gradient(color.opacity(0.5), deepen(color).opacity(0.25)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(monogram(pkg.display_name(), 72., color, p)),
            });
        h_flex()
            .items_start()
            .gap(px(28.))
            .px(px(20.))
            .pt(px(18.))
            .pb(px(20.))
            .border_t_1()
            .border_color(p.border_variant)
            .child(preview)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
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
                        h_flex().pt(px(16.)).child(
                            Button::new("remove-map")
                                .danger()
                                .outline()
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
            .primary()
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
                .min_h(px(200.))
                .items_center()
                .justify_center()
                .gap(px(8.))
                .rounded(px(15.))
                .border_2()
                .border_dashed()
                .border_color(p.border)
                .cursor_pointer()
                .hover(move |s| {
                    s.border_color(p.profiles.opacity(0.6))
                        .bg(p.profiles.opacity(0.05))
                })
                .child(badge(IconName::Plus, 40., p.profiles, p))
                .child(
                    div()
                        .pt(px(4.))
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("New profile"),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(p.text_muted)
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
                    .px(px(32.))
                    .pt(px(4.))
                    .pb(px(28.))
                    .child(
                        div()
                            .grid()
                            .grid_cols(if wide { 3 } else { 2 })
                            .gap(px(16.))
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
                .gap(px(6.))
                .child(
                    h_flex()
                        .gap(px(8.))
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(
                            Icon::new(IconName::CircleOff)
                                .size(px(16.))
                                .text_color(p.text_muted),
                        )
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
                .child(names("MODS", &item.mods, p.mods, p))
                .child(names("MAPS", &item.maps, p.maps, p))
        };
        let button = |id: &str| Button::new(SharedString::from(format!("{id}-{name}")));
        let (rename, duplicate, switch) = (name.clone(), name.clone(), name.clone());
        let footer = h_flex()
            .gap(px(4.))
            .pt(px(12.))
            .border_t_1()
            .border_color(p.border_variant)
            .child(if active {
                h_flex()
                    .gap(px(6.))
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.success)
                    .child(Icon::new(IconName::CircleCheck).size(px(15.)))
                    .child("Used when you press Play")
                    .into_any_element()
            } else {
                button("use")
                    .primary()
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
                        .icon(IconName::Pencil)
                        .tooltip("Rename")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.profile_prompt(Some(rename.clone()), true, window, cx)
                        })),
                )
                .child(
                    button("duplicate")
                        .ghost()
                        .icon(IconName::Copy)
                        .tooltip("Duplicate")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.profile_prompt(Some(duplicate.clone()), false, window, cx)
                        })),
                )
            });
        let card = v_flex()
            .size_full()
            .p(px(18.))
            .gap(px(16.))
            .rounded(px(14.))
            .bg(p.surface)
            .child(
                h_flex()
                    .gap(px(12.))
                    .child(avatar(&name, 42., p))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(17.))
                                    .font_weight(FontWeight::BOLD)
                                    .truncate()
                                    .child(name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(p.text_muted)
                                    .child(item.summary()),
                            ),
                    )
                    .when(active, |d| {
                        d.child(
                            h_flex()
                                .gap(px(5.))
                                .px(px(10.))
                                .py(px(4.))
                                .rounded(px(999.))
                                .bg(p.brand_gradient())
                                .text_size(px(12.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(p.on_color)
                                .child(Icon::new(IconName::Check).size(px(12.)))
                                .child("Active"),
                        )
                    }),
            )
            .child(div().flex_1().child(contents))
            .child(footer);
        div()
            .p(px(1.5))
            .rounded(px(15.5))
            .map(|d| {
                if active {
                    d.bg(gradient(p.brand, p.accent)).shadow(glow(p.brand, 0.2))
                } else {
                    d.bg(p.border_variant)
                }
            })
            .child(card)
            .into_any_element()
    }

    pub(crate) fn go(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }
}
