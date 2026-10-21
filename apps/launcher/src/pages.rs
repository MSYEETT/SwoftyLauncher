use std::rc::Rc;
use std::sync::{Arc, LazyLock};

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable, Icon, Sizable, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::manifest::{AddonManifest, AddonOption, Binds, OptionType};
use swofty_core::profile::VANILLA;
use swofty_core::store::ProfileDeletion;

use crate::library::{AddonIcon, AddonItem, ProfileItem, kind_label, plural};
use crate::shell::{Page, Prompt, Shell, SliderSlot};
use crate::theme::{Palette, palette};

pub(crate) const GUTTER: f32 = 28.;

pub(crate) fn tag(label: impl Into<SharedString>, p: Palette) -> Div {
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

fn logo(size: f32) -> Div {
    static LOGO: LazyLock<Arc<Image>> = LazyLock::new(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../assets/icon-256.png").to_vec(),
        ))
    });
    div()
        .w(px(size))
        .h(px(size))
        .flex_shrink_0()
        .child(img(LOGO.clone()).size_full())
}

pub(crate) fn wordmark(width: f32) -> Div {
    static MARK: LazyLock<Arc<Image>> = LazyLock::new(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../assets/wordmark.png").to_vec(),
        ))
    });
    div()
        .w(px(width))
        .h(px(width * 238. / 440.))
        .flex_shrink_0()
        .child(img(MARK.clone()).size_full())
}

pub(crate) fn game_art(w: f32, h: f32) -> Div {
    static ART: LazyLock<Arc<Image>> = LazyLock::new(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../assets/f1-25.png").to_vec(),
        ))
    });
    div()
        .w(px(w))
        .h(px(h))
        .flex_shrink_0()
        .overflow_hidden()
        .rounded(px(12.))
        .child(img(ART.clone()).size_full())
}

pub(crate) fn addon_icon(icon: &AddonIcon, size: f32, p: Palette) -> Div {
    match icon {
        AddonIcon::Glyph(data) => {
            tile(size, p).child(svg().data(data).size(px(size * 0.5)).text_color(p.text_muted))
        }
        AddonIcon::Image(image) => div()
            .w(px(size))
            .h(px(size))
            .flex_shrink_0()
            .rounded(px(size * 0.22))
            .overflow_hidden()
            .child(img(image.clone()).size_full()),
    }
}

pub(crate) fn monogram(text: &str, size: f32, p: Palette) -> Div {
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

pub(crate) fn fact(label: impl Into<SharedString>, value: impl IntoElement, p: Palette) -> Div {
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
                .child(label.into()),
        )
        .child(div().flex_1().min_w_0().text_size(px(13.)).child(value))
}

pub(crate) fn stat(
    title: impl Into<SharedString>,
    value: impl Into<SharedString>,
    color: Hsla,
    p: Palette,
) -> Div {
    v_flex()
        .gap(px(3.))
        .child(label(title, p))
        .child(div().text_size(px(13.)).text_color(color).child(value.into()))
}

fn summary(text: &str) -> &str {
    text.find(". ").map_or(text, |i| &text[..=i])
}

pub(crate) fn chevron(open: bool, p: Palette) -> Icon {
    Icon::new(if open {
        IconName::ChevronUp
    } else {
        IconName::ChevronDown
    })
    .size(px(15.))
    .text_color(p.text_placeholder)
}

fn deletion_summary(plan: &ProfileDeletion, p: Palette) -> impl IntoElement {
    let name = &plan.name;
    let removed = if plan.saves {
        "Its choice of addons and mods, their settings, its build cache and its saves are deleted."
    } else {
        "Its choice of addons and mods, their settings and its build cache are deleted."
    };
    v_flex()
        .gap(px(12.))
        .child(div().text_size(px(13.)).text_color(p.text).child(format!(
            "{removed} Installed addons and mods stay in the library, and other profiles don't change."
        )))
        .when_some(plan.switch_to.clone(), |d, to| {
            d.child(
                h_flex()
                    .gap(px(10.))
                    .items_start()
                    .p(px(12.))
                    .rounded(px(6.))
                    .bg(p.elevated)
                    .border_1()
                    .border_color(p.border)
                    .child(
                        Icon::new(IconName::ArrowRightLeft)
                            .size(px(15.))
                            .text_color(p.accent),
                    )
                    .child(
                        v_flex()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(p.text)
                                    .child(format!("{name} is the active profile")),
                            )
                            .child(div().text_size(px(13.)).text_color(p.text_muted).child(format!(
                                "{to} becomes active first, and Play uses it from now on."
                            ))),
                    ),
            )
        })
        .child(
            div()
                .text_size(px(13.))
                .text_color(p.text_muted)
                .child("This can't be undone."),
        )
}

pub(crate) fn names(title: impl Into<SharedString>, items: &[String], p: Palette) -> Div {
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
    pub(crate) fn header(
        &self,
        title: impl Into<SharedString>,
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
                            .child(title.into()),
                    )
                    .child(div().text_size(px(13.)).text_color(p.text_muted).child(subtitle))
                    .when_some(self.library.error.clone(), |d, e| {
                        d.child(div().text_size(px(13.)).text_color(p.error).child(e))
                    }),
            )
            .child(h_flex().gap(px(8.)).pb(px(2.)).child(actions))
    }

    pub(crate) fn list(
        &self,
        id: impl Into<ElementId>,
        rows: Vec<AnyElement>,
        p: Palette,
    ) -> impl IntoElement {
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

    pub(crate) fn empty(
        &self,
        icon: IconName,
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
        action: impl IntoElement,
        p: Palette,
    ) -> AnyElement {
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .pb(px(48.))
            .child(badge(icon, 44., p))
            .child(
                div()
                    .pt(px(8.))
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title.into()),
            )
            .child(
                div()
                    .max_w(px(400.))
                    .text_center()
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .text_color(p.text_muted)
                    .child(body.into()),
            )
            .child(div().pt(px(8.)).child(action))
            .into_any_element()
    }

    pub(crate) fn row(&self, first: bool, open: bool, p: Palette) -> Div {
        v_flex()
            .flex_shrink_0()
            .when(!first, |d| d.border_t_1().border_color(p.border_variant))
            .when(open, |d| d.bg(p.surface))
    }

    pub(crate) fn row_head(&self, id: String, open: bool, p: Palette) -> Stateful<Div> {
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
        change: impl Fn(&mut Self, bool, &mut Window, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        let switch = Switch::new(SharedString::from(id.clone()))
            .checked(on)
            .small()
            .disabled(self.vanilla())
            .on_change(cx.listener(move |this, on: &bool, window, cx| change(this, *on, window, cx)));
        div()
            .id(SharedString::from(format!("{id}-hold")))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(switch)
            .into_any_element()
    }

    pub(crate) fn vanilla(&self) -> bool {
        self.library.profile.is_vanilla()
    }

    fn install_addon(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt(
            Prompt {
                title: "Install addon",
                description: "An addon folder that contains swofty.json.".into(),
                initial: String::new(),
                placeholder: "/path/to/my-addon",
                ok: "Install",
                browse: Some(PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some("Install".into()),
                }),
                action: Rc::new(|store, value| {
                    let m = store
                        .install_addon(std::path::Path::new(value))
                        .map_err(|e| e.to_string())?;
                    Ok(format!("Installed {} {}", m.name, m.version))
                }),
            },
            window,
            cx,
        );
    }

    fn state_line(&self, on: bool, what: &str, p: Palette) -> (String, Hsla) {
        if self.vanilla() {
            (format!("Switch away from {VANILLA} to use {what}"), p.warning)
        } else if on {
            ("On".to_owned(), p.success)
        } else {
            ("Off".to_owned(), p.text_muted)
        }
    }

    pub(crate) fn addons_page(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let tracks_page = self.library.tracks_page;
        let listed = self.library.listed().count();
        let hidden = self.library.addons.len() - listed;
        let installed = plural(listed, "addon installed", "addons installed");
        let subtitle = if self.vanilla() {
            format!("{installed} · {VANILLA} plays the game without them")
        } else {
            format!(
                "{installed} · {} on in {}",
                self.library.enabled_addons(),
                self.library.profile.name
            )
        };
        let actions = Button::new("install-addon")
            .outline()
            .icon(IconName::Plus)
            .label("Install addon…")
            .on_click(cx.listener(|this, _, window, cx| this.install_addon(window, cx)));
        let rows: Vec<AnyElement> = self
            .library
            .listed()
            .enumerate()
            .map(|(i, m)| {
                let open = self.open_addon.as_ref() == Some(&m.manifest.id);
                self.addon_row(m, i == 0, open, p, cx)
            })
            .collect();
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
        let tracks = tracks_page.filter(|_| hidden > 0).map(|i| {
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
                        .on_click(cx.listener(move |this, _, _, cx| this.go(Page::Section(i), cx))),
                )
        });
        v_flex()
            .size_full()
            .child(self.header(Page::Addons.label(), subtitle, actions, p))
            .children(pending)
            .children(tracks)
            .child(self.list("addons", rows, p))
            .into_any_element()
    }

    fn addon_row(
        &self,
        m: &AddonItem,
        first: bool,
        open: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let man = &m.manifest;
        let id = man.id.clone();
        let builtin = man.is_builtin();
        let trailing = if m.locked {
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
            self.toggle(
                format!("toggle-{id}"),
                m.enabled,
                cx,
                move |this, on, window, cx| this.set_addon_enabled(&id, on, window, cx),
            )
        };
        let head = self
            .row_head(format!("addon-{id}"), open, p)
            .child(match &m.icon {
                _ if builtin => logo(32.),
                Some(icon) => addon_icon(icon, 32., p),
                None => badge(IconName::Package, 32., p),
            })
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
                this.open_addon = (this.open_addon.as_ref() != Some(&id)).then(|| id.clone());
                cx.notify();
            }));
        self.row(first, open, p)
            .child(head)
            .when(open, |d| d.child(self.addon_body(m, p, cx)))
            .into_any_element()
    }

    fn addon_body(&self, m: &AddonItem, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let man = &m.manifest;
        let remove = (!man.is_builtin() && !m.locked).then(|| {
            let (id, name) = (man.id.clone(), man.name.clone());
            h_flex().justify_end().pt(px(2.)).child(
                h_flex()
                    .id(SharedString::from(format!("remove-{}", man.id)))
                    .gap(px(7.))
                    .px(px(12.))
                    .py(px(7.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(p.error)
                    .text_color(p.error)
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .hover(move |s| s.bg(p.hover))
                    .child(Icon::new(IconName::Trash).size(px(14.)))
                    .child("Remove addon")
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.remove_addon(&id, &name, window, cx)),
                    ),
            )
        });
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
        let (status, status_color) = if m.locked && !self.vanilla() {
            ("On".to_owned(), p.success)
        } else {
            self.state_line(m.enabled, "addons", p)
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
            .when(!man.options.is_empty(), |d| d.child(self.settings(man, p, cx)))
            .children(remove)
    }

    fn settings(&self, man: &AddonManifest, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let customised = !self.vanilla() && self.library.profile.options.contains_key(&man.id);
        let reset = customised.then(|| {
            let man = man.clone();
            div()
                .id(SharedString::from(format!("reset-{}", man.id)))
                .cursor_pointer()
                .text_size(px(12.))
                .text_color(p.text_muted)
                .hover(move |s| s.text_color(p.text))
                .child("Reset to defaults")
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.update_profile(window, cx, |p| p.reset_options(&man).map(drop))
                }))
        });
        let rows: Vec<AnyElement> = man
            .options
            .iter()
            .enumerate()
            .map(|(i, o)| self.setting_row(man, o, i == 0, p, cx))
            .collect();
        v_flex()
            .gap(px(6.))
            .child(
                h_flex()
                    .max_w(px(760.))
                    .justify_between()
                    .child(label("SETTINGS", p))
                    .children(reset),
            )
            .child(
                v_flex()
                    .max_w(px(760.))
                    .rounded(px(5.))
                    .border_1()
                    .border_color(p.border_variant)
                    .children(rows),
            )
    }

    fn setting_row(
        &self,
        man: &AddonManifest,
        o: &AddonOption,
        first: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = format!("{}.{}", man.id, o.id);
        let pending = !self.vanilla() && self.library.rebuild.contains(&key);
        let (hint, hint_color) = match o.binds {
            Binds::Native => ("Applies live", p.text_placeholder),
            Binds::Db if pending => ("Rebuilds on next launch", p.warning),
            Binds::Db => ("Applies after a rebuild", p.text_placeholder),
        };
        let value = self.library.profile.option(man, o);
        let control = match o.kind {
            OptionType::Toggle => {
                let (man, id) = (man.clone(), o.id.clone());
                self.toggle(
                    format!("setting-{key}"),
                    value == true,
                    cx,
                    move |this, on, window, cx| {
                        this.update_profile(window, cx, |p| p.set_option(&man, &id, on.into()).map(drop))
                    },
                )
            }
            OptionType::Choice => self.choice(&key, man, o, &value, cx),
            OptionType::Slider => self.slider(&key, o, &value, p, cx),
        };
        h_flex()
            .px(px(12.))
            .py(px(10.))
            .gap(px(24.))
            .when(!first, |d| d.border_t_1().border_color(p.border_variant))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        h_flex()
                            .gap(px(10.))
                            .items_baseline()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(o.title().to_owned()),
                            )
                            .child(div().text_size(px(12.)).text_color(hint_color).child(hint)),
                    )
                    .when_some(o.description.clone(), |d, t| {
                        d.child(
                            div()
                                .text_size(px(12.))
                                .line_height(px(18.))
                                .text_color(p.text_muted)
                                .child(t),
                        )
                    }),
            )
            .child(div().flex_shrink_0().child(control))
            .into_any_element()
    }

    fn choice(
        &self,
        key: &str,
        man: &AddonManifest,
        o: &AddonOption,
        value: &serde_json::Value,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let states = o.states();
        let current = o.index(value);
        let shell = cx.entity().downgrade();
        let (man, id) = (man.clone(), o.id.clone());
        Button::new(SharedString::from(format!("setting-{key}")))
            .outline()
            .small()
            .min_w(px(168.))
            .label(o.text(value))
            .dropdown_caret(true)
            .disabled(self.vanilla())
            .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                for (i, s) in states.iter().enumerate() {
                    let (shell, man, id, v) = (shell.clone(), man.clone(), id.clone(), s.value.clone());
                    menu = menu.item(
                        PopupMenuItem::new(s.text.clone())
                            .checked(current == Some(i))
                            .on_click(move |_, window, cx| {
                                let v = v.clone();
                                shell
                                    .update(cx, |this, cx| {
                                        this.update_profile(window, cx, |p| {
                                            p.set_option(&man, &id, v).map(drop)
                                        })
                                    })
                                    .ok();
                            }),
                    );
                }
                menu
            })
            .into_any_element()
    }

    fn slider(
        &self,
        key: &str,
        o: &AddonOption,
        value: &serde_json::Value,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(slot) = self.sliders.get(key) else {
            return div().into_any_element();
        };
        let at = slot.state.read(cx).value().start().round().max(0.) as usize;
        let text = o
            .states()
            .get(at)
            .map_or_else(|| o.text(value), |s| s.text.clone());
        h_flex()
            .w(px(260.))
            .gap(px(12.))
            .child(Slider::new(&slot.state).disabled(self.vanilla()).flex_1())
            .child(
                div()
                    .w(px(48.))
                    .flex()
                    .justify_end()
                    .text_size(px(13.))
                    .text_color(p.text)
                    .child(text),
            )
            .into_any_element()
    }

    pub(crate) fn sync_sliders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(m) = self
            .open_addon
            .as_ref()
            .and_then(|id| self.library.addons.iter().find(|m| &m.manifest.id == id))
        else {
            return;
        };
        let man = m.manifest.clone();
        for o in man.options.iter().filter(|o| o.kind == OptionType::Slider) {
            let key = format!("{}.{}", man.id, o.id);
            let at = o.index(&self.library.profile.option(&man, o)).unwrap_or(0);
            if let Some(slot) = self.sliders.get_mut(&key) {
                if slot.synced != at {
                    slot.synced = at;
                    slot.state.update(cx, |s, cx| s.set_value(at as f32, window, cx));
                }
                continue;
            }
            let last = o.states().len().saturating_sub(1) as f32;
            let state = cx.new(|_| {
                SliderState::new()
                    .min(0.)
                    .max(last)
                    .step(1.)
                    .default_value(at as f32)
            });
            let (man, id) = (man.clone(), o.id.clone());
            let changes = cx.subscribe_in(&state, window, move |this, _, ev: &SliderEvent, window, cx| {
                let SliderEvent::Release(v) = ev else {
                    return;
                };
                let at = v.start().round().max(0.) as usize;
                let Some(state) = man
                    .options
                    .iter()
                    .find(|o| o.id == id)
                    .and_then(|o| o.states().into_iter().nth(at))
                else {
                    return;
                };
                this.update_profile(window, cx, |p| p.set_option(&man, &id, state.value).map(drop));
            });
            self.sliders.insert(
                key,
                SliderSlot {
                    state,
                    synced: at,
                    _changes: changes,
                },
            );
        }
    }

    pub(crate) fn profiles_page(&mut self, p: Palette, wide: bool, cx: &mut Context<Self>) -> AnyElement {
        let subtitle =
            "Each profile is its own set of addons and mods. Play starts F1 25 with the active one.".into();
        let actions = Button::new("new-profile")
            .outline()
            .icon(IconName::Plus)
            .label("New profile")
            .on_click(cx.listener(|this, _, window, cx| this.profile_prompt(None, false, window, cx)));
        let active = self.library.profile.name.clone();
        let deletable = self.library.profiles.iter().filter(|i| !i.vanilla()).count() > 1;
        let mut cards: Vec<AnyElement> = self
            .library
            .profiles
            .iter()
            .map(|item| self.profile_card(item, item.name == active, deletable, p, cx))
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
                        .child("Starts with SwoftyMod and Simplified for Swofty"),
                )
                .on_click(cx.listener(|this, _, window, cx| this.profile_prompt(None, false, window, cx)))
                .into_any_element(),
        );
        v_flex()
            .size_full()
            .child(self.header(Page::Profiles.label(), subtitle, actions, p))
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
                description: "Letters, digits, - and _. Each profile keeps its own addons, mods and saves."
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

    fn game_busy(&self) -> bool {
        !self.processes.is_empty() || self.launch.is_some()
    }

    fn confirm_delete_profile(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let name = name.to_owned();
        let plan = if self.game_busy() {
            Err("Close F1 25 before deleting a profile.".to_owned())
        } else {
            self.store.plan_profile_deletion(&name).map_err(|e| e.to_string())
        };
        let plan = match plan {
            Ok(plan) => plan,
            Err(reason) => {
                window.open_alert_dialog(cx, move |alert, _, _| {
                    alert
                        .title(format!("Can't delete {name}"))
                        .description(reason.clone())
                });
                return;
            }
        };
        let p = palette(cx);
        let shell = cx.entity().downgrade();
        let store = self.store.clone();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (shell, store, name) = (shell.clone(), store.clone(), name.clone());
            alert
                .width(px(480.))
                .title(format!("Delete profile {name}?"))
                .child(deletion_summary(&plan, p))
                .ok_text("Delete profile")
                .ok_variant(ButtonVariant::Danger)
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    let busy = shell.upgrade().is_some_and(|s| s.read(cx).game_busy());
                    let result = if busy {
                        Err("Close F1 25 before deleting a profile.".to_owned())
                    } else {
                        store.delete_profile(&name).map_err(|e| e.to_string())
                    };
                    match result {
                        Ok(done) => {
                            let text = match &done.switch_to {
                                Some(to) => format!("Deleted profile {name}. {to} is now active."),
                                None => format!("Deleted profile {name}"),
                            };
                            window.push_notification(Notification::success(text), cx);
                        }
                        Err(e) => window.push_notification(Notification::error(e), cx),
                    }
                    shell.update(cx, |s, cx| s.reload(cx)).ok();
                    true
                })
        });
    }

    fn profile_card(
        &self,
        item: &ProfileItem,
        active: bool,
        deletable: bool,
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
                        .child("No addons, no mods"),
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
                .child(names("ADDONS", &item.addons, p))
                .children(
                    item.summaries
                        .iter()
                        .map(|s| names(s.label.to_uppercase(), &s.items, p)),
                )
        };
        let button = |id: &str| Button::new(SharedString::from(format!("{id}-{name}")));
        let (rename, duplicate, switch, delete) = (name.clone(), name.clone(), name.clone(), name.clone());
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
                .child(
                    button("delete")
                        .ghost()
                        .small()
                        .icon(IconName::Trash)
                        .tooltip(if deletable {
                            "Delete"
                        } else {
                            "Your only profile can't be deleted"
                        })
                        .disabled(!deletable)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.confirm_delete_profile(&delete, window, cx)
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
        let moved = self.page != page;
        self.page = page;
        if let Page::Section(i) = page {
            self.fetch_views(Some(i), cx);
        } else if moved {
            self.reload(cx);
        }
        cx.notify();
    }
}
