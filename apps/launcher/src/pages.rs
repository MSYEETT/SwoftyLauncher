use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable, Icon, Sizable, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::manifest::ModManifest;
use swofty_core::map::TrackPackage;
use swofty_core::profile::VANILLA;

use crate::library::{MapItem, ModItem, ProfileItem, kind_label};
use crate::shell::{Page, Prompt, Shell};
use crate::theme::Palette;

fn tag(label: impl Into<SharedString>, p: Palette) -> Div {
    div()
        .px(px(9.))
        .py(px(3.))
        .rounded(px(6.))
        .bg(p.elevated)
        .border_1()
        .border_color(p.border_variant)
        .text_size(px(13.))
        .text_color(p.text_muted)
        .child(label.into())
}

fn tile(icon: IconName, size: f32, color: Hsla, p: Palette) -> Div {
    div()
        .size(px(size))
        .flex_shrink_0()
        .rounded(px(size * 0.22))
        .bg(color.opacity(0.14))
        .border_1()
        .border_color(color.opacity(0.28))
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(icon).size(px(size * 0.5)).text_color(color))
        .text_color(p.text)
}

fn section(title: &'static str, p: Palette) -> Div {
    div()
        .pt(px(28.))
        .pb(px(10.))
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.text_placeholder)
        .child(title)
}

fn field(label: &'static str, value: impl Into<SharedString>, p: Palette) -> Div {
    h_flex()
        .items_start()
        .py(px(10.))
        .gap(px(16.))
        .border_b_1()
        .border_color(p.border_variant)
        .child(
            div()
                .w(px(170.))
                .flex_shrink_0()
                .text_size(px(15.))
                .text_color(p.text_muted)
                .child(label),
        )
        .child(div().flex_1().min_w_0().text_size(px(15.)).child(value.into()))
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

impl Shell {
    fn header(
        &self,
        title: &'static str,
        subtitle: String,
        actions: impl IntoElement,
        p: Palette,
    ) -> impl IntoElement {
        h_flex()
            .px(px(36.))
            .pt(px(28.))
            .pb(px(24.))
            .gap(px(24.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(p.border_variant)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(28.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(div().text_size(px(15.)).text_color(p.text_muted).child(subtitle))
                    .when_some(self.library.error.clone(), |d, e| {
                        d.child(div().text_size(px(15.)).text_color(p.error).child(e))
                    }),
            )
            .child(h_flex().gap(px(10.)).child(actions))
    }

    fn empty(
        &self,
        icon: IconName,
        title: &'static str,
        body: &'static str,
        action: impl IntoElement,
        p: Palette,
    ) -> AnyElement {
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .gap(px(14.))
            .child(tile(icon, 88., p.accent, p))
            .child(
                div()
                    .pt(px(8.))
                    .text_size(px(22.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .max_w(px(520.))
                    .text_center()
                    .text_size(px(16.))
                    .text_color(p.text_muted)
                    .child(body),
            )
            .child(div().pt(px(8.)).child(action))
            .into_any_element()
    }

    fn row_shell(&self, id: SharedString, selected: bool, p: Palette) -> Stateful<Div> {
        h_flex()
            .id(id)
            .p(px(16.))
            .gap(px(16.))
            .rounded(px(10.))
            .border_1()
            .cursor_pointer()
            .map(|d| {
                if selected {
                    d.bg(p.selected).border_color(p.accent.opacity(0.6))
                } else {
                    d.bg(p.surface)
                        .border_color(p.border_variant)
                        .hover(move |s| s.bg(p.hover).border_color(p.border))
                }
            })
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
                description: "A track package zip with manifest.json and its .erp at the root.".into(),
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

    pub(crate) fn mods_page(&mut self, p: Palette, wide: bool, cx: &mut Context<Self>) -> AnyElement {
        let profile = self.library.profile.name.clone();
        let subtitle = if self.vanilla() {
            format!(
                "{} installed · {VANILLA} runs the game without mods",
                self.library.mods.len()
            )
        } else {
            format!(
                "{} installed · {} enabled in {profile}",
                self.library.mods.len(),
                self.library.enabled_mods()
            )
        };
        let actions = Button::new("install-mod")
            .primary()
            .large()
            .icon(IconName::Plus)
            .label("Install mod…")
            .on_click(cx.listener(|this, _, window, cx| this.install_mod(window, cx)));
        let selected = self
            .selected_mod
            .clone()
            .filter(|id| self.library.mods.iter().any(|m| &m.manifest.id == id))
            .or_else(|| self.library.mods.first().map(|m| m.manifest.id.clone()));
        let rows: Vec<AnyElement> = self
            .library
            .mods
            .iter()
            .map(|m| self.mod_row(m, selected.as_deref() == Some(m.manifest.id.as_str()), p, cx))
            .collect();
        let detail = self
            .library
            .mods
            .iter()
            .find(|m| Some(&m.manifest.id) == selected.as_ref())
            .map(|m| self.mod_detail(m, p, cx));
        v_flex()
            .size_full()
            .child(self.header("Mods", subtitle, actions, p))
            .child(self.split(rows, detail, wide, p))
            .into_any_element()
    }

    fn split(
        &self,
        rows: Vec<AnyElement>,
        detail: Option<AnyElement>,
        wide: bool,
        p: Palette,
    ) -> impl IntoElement {
        h_flex()
            .flex_1()
            .min_h_0()
            .items_start()
            .child(
                v_flex()
                    .id("list")
                    .w(px(if wide { 620. } else { 440. }))
                    .flex_shrink_0()
                    .h_full()
                    .overflow_y_scroll()
                    .p(px(20.))
                    .gap(px(10.))
                    .children(rows),
            )
            .child(
                v_flex()
                    .id("detail")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .border_l_1()
                    .border_color(p.border_variant)
                    .px(px(40.))
                    .py(px(32.))
                    .children(detail),
            )
    }

    fn mod_switch(&self, m: &ModItem, cx: &mut Context<Self>) -> AnyElement {
        let id = m.manifest.id.clone();
        Switch::new(SharedString::from(format!("toggle-{id}")))
            .large()
            .checked(m.enabled)
            .disabled(self.vanilla())
            .on_change(cx.listener(move |this, on: &bool, window, cx| {
                let (id, on) = (id.clone(), *on);
                this.update_profile(window, cx, |p| p.set_enabled(&id, on));
            }))
            .into_any_element()
    }

    fn builtin_badge(&self, p: Palette) -> Div {
        h_flex()
            .gap(px(6.))
            .px(px(10.))
            .py(px(4.))
            .rounded(px(6.))
            .bg(p.accent.opacity(0.12))
            .text_size(px(13.))
            .text_color(p.accent)
            .child(Icon::new(IconName::Lock).size(px(14.)))
            .child("Built in")
    }

    fn mod_row(&self, m: &ModItem, selected: bool, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let man = &m.manifest;
        let id = man.id.clone();
        self.row_shell(SharedString::from(format!("mod-{id}")), selected, p)
            .child(tile(
                if man.is_builtin() {
                    IconName::Sparkles
                } else {
                    IconName::Package
                },
                48.,
                if man.is_builtin() { p.accent } else { p.success },
                p,
            ))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(4.))
                    .child(
                        h_flex()
                            .gap(px(10.))
                            .child(
                                div()
                                    .text_size(px(17.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(man.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(p.text_placeholder)
                                    .child(man.version.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(15.))
                            .text_color(p.text_muted)
                            .truncate()
                            .child(man.description.clone().unwrap_or_else(|| man.id.clone())),
                    )
                    .child(
                        h_flex()
                            .pt(px(4.))
                            .gap(px(6.))
                            .children(man.kind.iter().map(|k| tag(kind_label(*k), p))),
                    ),
            )
            .child(if man.is_builtin() {
                self.builtin_badge(p).into_any_element()
            } else {
                self.mod_switch(m, cx)
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected_mod = Some(id.clone());
                cx.notify();
            }))
            .into_any_element()
    }

    fn mod_detail(&self, m: &ModItem, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let man: &ModManifest = &m.manifest;
        let build = self.install.as_ref().and_then(|i| i.build_id);
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
        let compat = match build {
            Some(b) if man.compat.accepts_build(b) => format!("{builds} (matches build {b})"),
            Some(b) => format!("{builds} (not tested on build {b})"),
            None => builds,
        };
        let ops = &man.operations;
        let counts = [
            (ops.database.len(), "database"),
            (ops.erp.len(), "ERP"),
            (ops.ui.len(), "UI"),
            (ops.files.len(), "file"),
            (ops.native.len(), "native"),
        ]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, k)| format!("{n} {k}"))
        .collect::<Vec<_>>()
        .join(" · ");
        let state = if man.is_builtin() {
            "Always on in every modded profile".to_owned()
        } else if self.vanilla() {
            format!("Switch away from {VANILLA} to enable mods")
        } else if m.enabled {
            format!("Enabled in {}", self.library.profile.name)
        } else {
            format!("Disabled in {}", self.library.profile.name)
        };
        v_flex()
            .max_w(px(860.))
            .child(
                h_flex()
                    .gap(px(20.))
                    .child(tile(
                        if man.is_builtin() {
                            IconName::Sparkles
                        } else {
                            IconName::Package
                        },
                        76.,
                        if man.is_builtin() { p.accent } else { p.success },
                        p,
                    ))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(6.))
                            .child(
                                div()
                                    .text_size(px(28.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(man.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .text_color(p.text_muted)
                                    .child(format!("Version {} · by {}", man.version, man.author)),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .mt(px(24.))
                    .p(px(16.))
                    .gap(px(16.))
                    .rounded(px(10.))
                    .bg(p.surface)
                    .border_1()
                    .border_color(p.border_variant)
                    .child(div().flex_1().text_size(px(16.)).child(state))
                    .child(if man.is_builtin() {
                        self.builtin_badge(p).into_any_element()
                    } else {
                        self.mod_switch(m, cx)
                    }),
            )
            .when_some(man.description.clone(), |d, text| {
                d.child(
                    div()
                        .pt(px(24.))
                        .text_size(px(16.))
                        .line_height(px(25.))
                        .child(text),
                )
            })
            .child(
                h_flex()
                    .pt(px(16.))
                    .gap(px(8.))
                    .children(man.kind.iter().map(|k| tag(kind_label(*k), p))),
            )
            .child(section("DETAILS", p))
            .child(field("ID", man.id.clone(), p))
            .when_some(man.license.clone(), |d, l| d.child(field("License", l, p)))
            .child(field("Game builds", compat, p))
            .when(!man.requires.is_empty(), |d| {
                d.child(field(
                    "Requires",
                    man.requires
                        .iter()
                        .map(|r| format!("{} {}", r.id, r.version))
                        .collect::<Vec<_>>()
                        .join(", "),
                    p,
                ))
            })
            .when(!counts.is_empty(), |d| d.child(field("Operations", counts, p)))
            .when_some(man.homepage.clone(), |d, h| d.child(field("Homepage", h, p)))
            .when(!man.options.is_empty(), |d| {
                d.child(section("OPTIONS", p))
                    .children(man.options.iter().map(|o| {
                        v_flex()
                            .py(px(12.))
                            .gap(px(4.))
                            .border_b_1()
                            .border_color(p.border_variant)
                            .child(
                                h_flex()
                                    .gap(px(10.))
                                    .child(
                                        div()
                                            .text_size(px(16.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(o.label.clone().unwrap_or_else(|| o.id.clone())),
                                    )
                                    .child(tag(format!("default {}", o.default), p)),
                            )
                            .when_some(o.description.clone(), |d, t| {
                                d.child(div().text_size(px(15.)).text_color(p.text_muted).child(t))
                            })
                    }))
            })
            .into_any_element()
    }

    pub(crate) fn maps_page(&mut self, p: Palette, wide: bool, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.library.maps.iter().filter(|m| m.enabled).count();
        let subtitle = format!(
            "{} imported · {enabled} enabled in {}",
            self.library.maps.len(),
            self.library.profile.name
        );
        let import = |id: &'static str, cx: &mut Context<Self>| {
            Button::new(id)
                .primary()
                .large()
                .icon(IconName::Upload)
                .label("Import map…")
                .on_click(cx.listener(|this, _, window, cx| this.import_map(window, cx)))
        };
        let header = self.header("Maps", subtitle, import("import-map", cx), p);
        if self.library.maps.is_empty() {
            let cta = import("import-map-empty", cx);
            return v_flex()
                .size_full()
                .child(header)
                .child(self.empty(
                    IconName::Map,
                    "No maps yet",
                    "Import a track package zip to add a circuit. It is stored in your library and can be switched on per profile.",
                    cta,
                    p,
                ))
                .into_any_element();
        }
        let selected = self
            .selected_map
            .clone()
            .filter(|c| self.library.maps.iter().any(|m| &m.package.track_code == c))
            .or_else(|| self.library.maps.first().map(|m| m.package.track_code.clone()));
        let rows: Vec<AnyElement> = self
            .library
            .maps
            .iter()
            .map(|m| {
                self.map_row(
                    m,
                    selected.as_deref() == Some(m.package.track_code.as_str()),
                    p,
                    cx,
                )
            })
            .collect();
        let detail = self
            .library
            .maps
            .iter()
            .find(|m| Some(&m.package.track_code) == selected.as_ref())
            .map(|m| self.map_detail(m, p, cx));
        v_flex()
            .size_full()
            .child(header)
            .child(self.split(rows, detail, wide, p))
            .into_any_element()
    }

    fn map_switch(&self, m: &MapItem, cx: &mut Context<Self>) -> AnyElement {
        let code = m.package.track_code.clone();
        Switch::new(SharedString::from(format!("map-{code}")))
            .large()
            .checked(m.enabled)
            .disabled(self.vanilla())
            .on_change(cx.listener(move |this, on: &bool, window, cx| {
                let (code, on) = (code.clone(), *on);
                this.update_profile(window, cx, |p| p.set_map(&code, on));
            }))
            .into_any_element()
    }

    fn map_row(&self, m: &MapItem, selected: bool, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let pkg: &TrackPackage = &m.package;
        let code = pkg.track_code.clone();
        let by = pkg
            .author_name
            .as_deref()
            .map(|a| format!(" · by {a}"))
            .unwrap_or_default();
        self.row_shell(SharedString::from(format!("map-row-{code}")), selected, p)
            .child(tile(IconName::Route, 48., p.warning, p))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(4.))
                    .child(
                        h_flex()
                            .gap(px(10.))
                            .child(
                                div()
                                    .text_size(px(17.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(pkg.display_name().to_owned()),
                            )
                            .when_some(pkg.version.clone(), |d, v| {
                                d.child(div().text_size(px(14.)).text_color(p.text_placeholder).child(v))
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(15.))
                            .text_color(p.text_muted)
                            .truncate()
                            .child(format!("{}{by}", pkg.track_code)),
                    ),
            )
            .child(self.map_switch(m, cx))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected_map = Some(code.clone());
                cx.notify();
            }))
            .into_any_element()
    }

    fn map_detail(&self, m: &MapItem, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let pkg = &m.package;
        let a = &pkg.activation;
        let code = pkg.track_code.clone();
        let state = if self.vanilla() {
            format!("Switch away from {VANILLA} to enable maps")
        } else if m.enabled {
            format!("Enabled in {}", self.library.profile.name)
        } else {
            format!("Disabled in {}", self.library.profile.name)
        };
        v_flex()
            .max_w(px(860.))
            .child(
                h_flex()
                    .gap(px(20.))
                    .child(tile(IconName::Route, 76., p.warning, p))
                    .child(
                        v_flex()
                            .flex_1()
                            .gap(px(6.))
                            .child(
                                div()
                                    .text_size(px(28.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(pkg.display_name().to_owned()),
                            )
                            .child(
                                div().text_size(px(16.)).text_color(p.text_muted).child(
                                    [
                                        pkg.version.as_ref().map(|v| format!("Version {v}")),
                                        pkg.author_name.as_ref().map(|a| format!("by {a}")),
                                    ]
                                    .into_iter()
                                    .flatten()
                                    .collect::<Vec<_>>()
                                    .join(" · "),
                                ),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .mt(px(24.))
                    .p(px(16.))
                    .gap(px(16.))
                    .rounded(px(10.))
                    .bg(p.surface)
                    .border_1()
                    .border_color(p.border_variant)
                    .child(div().flex_1().text_size(px(16.)).child(state))
                    .child(self.map_switch(m, cx)),
            )
            .child(section("DETAILS", p))
            .child(field("Track code", pkg.track_code.clone(), p))
            .when_some(a.copy_from_track.clone(), |d, t| d.child(field("Based on", t, p)))
            .when_some(a.copy_slot.clone(), |d, t| d.child(field("Replaces slot", t, p)))
            .child(field("Archive", pkg.erp.clone(), p))
            .child(field(
                "Overrides",
                plural(
                    a.overrides.len() + a.track_overrides.len(),
                    "database value",
                    "database values",
                ),
                p,
            ))
            .when(!pkg.copy_directories.is_empty(), |d| {
                d.child(field(
                    "Extra folders",
                    plural(pkg.copy_directories.len(), "folder", "folders"),
                    p,
                ))
            })
            .child(
                h_flex().pt(px(28.)).child(
                    Button::new("remove-map")
                        .danger()
                        .outline()
                        .icon(IconName::Trash)
                        .label("Remove from library")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Err(e) = this.store.remove_map(&code) {
                                window.push_notification(Notification::error(e.to_string()), cx);
                            }
                            this.reload(cx);
                        })),
                ),
            )
            .into_any_element()
    }

    pub(crate) fn profiles_page(&mut self, p: Palette, wide: bool, cx: &mut Context<Self>) -> AnyElement {
        let active = self.library.profile.name.clone();
        let subtitle = format!(
            "{} · {active} is active",
            plural(self.library.profiles.len(), "profile", "profiles")
        );
        let actions = Button::new("new-profile")
            .primary()
            .large()
            .icon(IconName::Plus)
            .label("New profile")
            .on_click(cx.listener(|this, _, window, cx| this.profile_prompt(None, false, window, cx)));
        let cards: Vec<AnyElement> = self
            .library
            .profiles
            .iter()
            .map(|item| self.profile_card(item, item.name == active, p, cx))
            .collect();
        let columns = if wide { 3 } else { 2 };
        v_flex()
            .size_full()
            .child(self.header("Profiles", subtitle, actions, p))
            .child(
                div()
                    .id("profiles")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(28.))
                    .child(div().grid().grid_cols(columns).gap(px(16.)).children(cards)),
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
        let builtin = name == VANILLA;
        let summary = if builtin {
            "The unmodded game".to_owned()
        } else {
            format!(
                "{} · {}",
                plural(item.mods.len(), "mod", "mods"),
                plural(item.maps, "map", "maps")
            )
        };
        let listing = if builtin {
            "No mods and no maps. Use it to play F1 25 as shipped.".to_owned()
        } else {
            item.mods.join(", ")
        };
        let button = |id: &'static str| Button::new(SharedString::from(format!("{id}-{name}")));
        let (rename, duplicate, switch) = (name.clone(), name.clone(), name.clone());
        v_flex()
            .p(px(22.))
            .gap(px(14.))
            .rounded(px(12.))
            .bg(p.surface)
            .border_1()
            .border_color(if active {
                p.accent.opacity(0.6)
            } else {
                p.border_variant
            })
            .child(
                h_flex()
                    .gap(px(14.))
                    .child(tile(
                        if builtin {
                            IconName::Shield
                        } else {
                            IconName::Layers
                        },
                        44.,
                        if active { p.accent } else { p.text_muted },
                        p,
                    ))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(20.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(name.clone()),
                            )
                            .child(div().text_size(px(15.)).text_color(p.text_muted).child(summary)),
                    )
                    .when(active, |d| {
                        d.child(
                            h_flex()
                                .gap(px(6.))
                                .px(px(10.))
                                .py(px(4.))
                                .rounded(px(6.))
                                .bg(p.success.opacity(0.14))
                                .text_size(px(13.))
                                .text_color(p.success)
                                .child(Icon::new(IconName::CircleCheck).size(px(14.)))
                                .child("Active"),
                        )
                    })
                    .when(builtin && !active, |d| d.child(self.builtin_badge(p))),
            )
            .child(
                div()
                    .min_h(px(44.))
                    .text_size(px(15.))
                    .line_height(px(22.))
                    .text_color(p.text_muted)
                    .child(listing),
            )
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(
                        button("switch")
                            .when(!active, |b| b.primary())
                            .disabled(active)
                            .icon(IconName::Check)
                            .label(if active { "In use" } else { "Use profile" })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.switch_profile(&switch, window, cx)
                            })),
                    )
                    .when(!builtin, |d| {
                        d.child(
                            button("rename")
                                .ghost()
                                .icon(IconName::Pencil)
                                .label("Rename")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.profile_prompt(Some(rename.clone()), true, window, cx)
                                })),
                        )
                    })
                    .child(
                        button("duplicate")
                            .ghost()
                            .icon(IconName::Copy)
                            .label("Duplicate")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.profile_prompt(Some(duplicate.clone()), false, window, cx)
                            })),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn go(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }
}
