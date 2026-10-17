use std::collections::HashSet;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Icon, Sizable, TitleBar, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::data::{self, ModRow};
use crate::theme::{Palette, palette};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Mods,
    Maps,
    Profiles,
    Play,
    Settings,
}

impl Section {
    fn label(self) -> &'static str {
        match self {
            Self::Mods => "Mods",
            Self::Maps => "Maps",
            Self::Profiles => "Profiles",
            Self::Play => "Play",
            Self::Settings => "Settings",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Mods => IconName::Puzzle,
            Self::Maps => IconName::Map,
            Self::Profiles => IconName::Layers,
            Self::Play => IconName::Play,
            Self::Settings => IconName::Settings,
        }
    }
}

pub struct Shell {
    section: Section,
    profile: SharedString,
    disabled: HashSet<&'static str>,
}

impl Shell {
    pub fn new() -> Self {
        Self {
            section: Section::Mods,
            profile: "default".into(),
            disabled: HashSet::new(),
        }
    }

    fn title_bar(&self, p: Palette) -> impl IntoElement {
        TitleBar::new().child(
            h_flex()
                .gap_2()
                .text_sm()
                .child(Icon::new(IconName::Gamepad2).small().text_color(p.accent))
                .child(div().text_color(p.text).child("Swofty Mod Launcher"))
                .child(div().text_color(p.text_placeholder).child("/"))
                .child(
                    h_flex()
                        .gap_1()
                        .text_color(p.text_muted)
                        .child(Icon::new(IconName::Layers).xsmall())
                        .child(self.profile.clone()),
                ),
        )
    }

    fn nav_item(&self, section: Section, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.section == section;
        h_flex()
            .id(section.label())
            .h(px(26.))
            .px_2()
            .gap_2()
            .rounded(px(4.))
            .text_sm()
            .cursor_pointer()
            .text_color(if selected { p.text } else { p.text_muted })
            .when(selected, |d| d.bg(p.selected))
            .when(!selected, |d| d.hover(move |s| s.bg(p.hover)))
            .child(Icon::new(section.icon()).small())
            .child(section.label())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.section = section;
                cx.notify();
            }))
    }

    fn sidebar(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let heading = |label: &'static str| {
            div()
                .px_2()
                .pt_3()
                .pb_1()
                .text_xs()
                .text_color(p.text_placeholder)
                .child(label)
        };
        v_flex()
            .w(px(224.))
            .h_full()
            .flex_shrink_0()
            .p_1()
            .gap_px()
            .bg(p.surface)
            .border_r_1()
            .border_color(p.border_variant)
            .child(heading("LIBRARY"))
            .child(self.nav_item(Section::Mods, p, cx))
            .child(self.nav_item(Section::Maps, p, cx))
            .child(heading("LAUNCHER"))
            .child(self.nav_item(Section::Profiles, p, cx))
            .child(self.nav_item(Section::Play, p, cx))
            .child(self.nav_item(Section::Settings, p, cx))
    }

    fn pane_header(&self, p: Palette) -> impl IntoElement {
        h_flex()
            .h(px(34.))
            .px_3()
            .gap_2()
            .flex_shrink_0()
            .bg(p.surface)
            .border_b_1()
            .border_color(p.border_variant)
            .text_sm()
            .child(Icon::new(self.section.icon()).small().text_color(p.text_muted))
            .child(div().text_color(p.text).child(self.section.label()))
    }

    fn mod_row(&self, row: &'static ModRow, p: Palette, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let enabled = row.builtin || !self.disabled.contains(row.id);
        h_flex()
            .id(row.id)
            .px_3()
            .py_2()
            .gap_3()
            .border_b_1()
            .border_color(p.border_variant)
            .hover(move |s| s.bg(p.hover))
            .child(
                Icon::new(if row.builtin {
                    IconName::Pin
                } else {
                    IconName::Package
                })
                .small()
                .text_color(if row.builtin { p.accent } else { p.text_muted }),
            )
            .child(
                v_flex()
                    .flex_1()
                    .child(
                        h_flex()
                            .gap_2()
                            .text_sm()
                            .child(div().text_color(p.text).child(row.name))
                            .child(div().text_color(p.text_placeholder).child(row.version)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(p.text_muted)
                            .child(format!("{} · {}", row.id, row.kinds)),
                    ),
            )
            .map(|d| {
                if row.builtin {
                    d.child(
                        h_flex()
                            .gap_1()
                            .text_xs()
                            .text_color(p.text_muted)
                            .child(Icon::new(IconName::Lock).xsmall())
                            .child("Built in · always on"),
                    )
                } else {
                    let id = row.id;
                    d.child(
                        Switch::new(SharedString::from(format!("toggle-{id}")))
                            .checked(enabled)
                            .on_change(cx.listener(move |this, on: &bool, _, cx| {
                                if *on {
                                    this.disabled.remove(id);
                                } else {
                                    this.disabled.insert(id);
                                }
                                cx.notify();
                            })),
                    )
                }
            })
    }

    fn mods_view(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let rows: Vec<_> = data::MODS.iter().map(|row| self.mod_row(row, p, cx)).collect();
        v_flex().children(rows).into_any_element()
    }

    fn maps_view(&self, p: Palette) -> AnyElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .child(Icon::new(IconName::Map).large().text_color(p.text_placeholder))
            .child(div().text_color(p.text).child("No maps installed"))
            .child(
                div()
                    .text_sm()
                    .text_color(p.text_muted)
                    .child("Track packages: manifest.json, <code>.erp and ui/*.json in one zip"),
            )
            .child(
                Button::new("import-map")
                    .primary()
                    .icon(IconName::Upload)
                    .label("Import map (.zip)")
                    .on_click(|_, _, _| {}),
            )
            .into_any_element()
    }

    fn profiles_view(&self, p: Palette) -> AnyElement {
        v_flex()
            .children(data::PROFILES.iter().map(|row| {
                let active = self.profile.as_ref() == row.name;
                h_flex()
                    .px_3()
                    .py_2()
                    .gap_3()
                    .border_b_1()
                    .border_color(p.border_variant)
                    .child(Icon::new(IconName::Layers).small().text_color(p.text_muted))
                    .child(
                        v_flex()
                            .flex_1()
                            .child(div().text_sm().text_color(p.text).child(row.name))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(p.text_muted)
                                    .child(format!("{} mods · {} maps", row.mods, row.maps)),
                            ),
                    )
                    .when(active, |d| {
                        d.child(
                            h_flex()
                                .gap_1()
                                .text_xs()
                                .text_color(p.success)
                                .child(Icon::new(IconName::CircleCheck).xsmall())
                                .child("Active"),
                        )
                    })
            }))
            .into_any_element()
    }

    fn play_view(&self, p: Palette) -> AnyElement {
        v_flex()
            .size_full()
            .child(
                h_flex()
                    .px_3()
                    .py_1p5()
                    .gap_2()
                    .bg(p.warning.opacity(0.12))
                    .border_b_1()
                    .border_color(p.warning.opacity(0.4))
                    .text_xs()
                    .text_color(p.warning)
                    .child(Icon::new(IconName::ShieldCheck).xsmall())
                    .child(format!(
                        "MODDED SESSION · OFFLINE · anticheat not loaded · profile {}",
                        self.profile
                    )),
            )
            .child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(
                        Button::new("play")
                            .primary()
                            .large()
                            .icon(IconName::Play)
                            .label("Play modded (offline)")
                            .on_click(|_, _, _| {}),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(p.text_muted)
                            .child(format!("F1 25 · profile {}", self.profile)),
                    ),
            )
            .into_any_element()
    }

    fn settings_view(&self, p: Palette) -> AnyElement {
        v_flex()
            .children(data::SETTINGS.iter().enumerate().map(|(i, (label, value))| {
                h_flex()
                    .px_3()
                    .py_2()
                    .gap_3()
                    .border_b_1()
                    .border_color(p.border_variant)
                    .child(div().w(px(160.)).text_sm().text_color(p.text_muted).child(*label))
                    .child(div().flex_1().text_sm().text_color(p.text).child(*value))
                    .child(
                        Button::new(("browse", i))
                            .small()
                            .icon(IconName::FolderOpen)
                            .label("Browse")
                            .on_click(|_, _, _| {}),
                    )
            }))
            .into_any_element()
    }

    fn status_bar(&self, p: Palette) -> impl IntoElement {
        h_flex()
            .h(px(26.))
            .px_2()
            .flex_shrink_0()
            .justify_between()
            .bg(p.chrome)
            .border_t_1()
            .border_color(p.border_variant)
            .text_xs()
            .text_color(p.text_muted)
            .child(
                h_flex()
                    .gap_1()
                    .child(Icon::new(IconName::HardDrive).xsmall())
                    .child("F1 25 not detected"),
            )
            .child(
                h_flex()
                    .gap_1()
                    .child(Icon::new(IconName::WifiOff).xsmall())
                    .child(format!("swofty {}", env!("CARGO_PKG_VERSION"))),
            )
    }
}

impl Render for Shell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let content = match self.section {
            Section::Mods => self.mods_view(p, cx),
            Section::Maps => self.maps_view(p),
            Section::Profiles => self.profiles_view(p),
            Section::Play => self.play_view(p),
            Section::Settings => self.settings_view(p),
        };
        v_flex()
            .size_full()
            .bg(p.background)
            .text_color(p.text)
            .child(self.title_bar(p))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .child(self.sidebar(p, cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .h_full()
                            .min_w_0()
                            .child(self.pane_header(p))
                            .child(
                                div()
                                    .id("pane")
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .child(content),
                            ),
                    ),
            )
            .child(self.status_bar(p))
    }
}
