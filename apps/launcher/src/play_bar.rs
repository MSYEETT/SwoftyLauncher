use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Disableable, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::game::{self, ago, duration};
use crate::shell::{Page, PlayState, Shell};
use crate::theme::Palette;

fn link(id: &'static str, label: &'static str, p: Palette) -> Stateful<Div> {
    div()
        .id(id)
        .cursor_pointer()
        .text_color(p.accent)
        .hover(move |s| s.text_color(p.accent_hover).underline())
        .child(label)
}

impl Shell {
    fn game_info(&self, p: Palette, wide: bool) -> impl IntoElement {
        let (title, line) = match &self.install {
            Some(i) => (
                format!(
                    "F1 25 · build {}",
                    i.build_id.map_or("unknown".into(), |b| b.to_string())
                ),
                i.dir.display().to_string(),
            ),
            None if self.detected => (
                "F1 25 not found".into(),
                "No Steam library has app 3059520".into(),
            ),
            None => ("F1 25".into(), "Looking for the game…".into()),
        };
        let mode = self.install.as_ref().map(|i| i.mode.clone());
        h_flex()
            .w(px(if wide { 420. } else { 330. }))
            .flex_shrink_1()
            .min_w(px(240.))
            .gap(px(14.))
            .child(
                div()
                    .size(px(52.))
                    .flex_shrink_0()
                    .rounded(px(12.))
                    .bg(p.elevated)
                    .border_1()
                    .border_color(p.border_variant)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconName::Gamepad2)
                            .size(px(26.))
                            .text_color(p.text_muted),
                    ),
            )
            .child(
                v_flex()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(17.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(title),
                    )
                    .when_some(mode, |d, m| {
                        d.child(
                            div()
                                .text_size(px(14.))
                                .text_color(p.text_muted)
                                .truncate()
                                .child(m),
                        )
                    })
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(p.text_placeholder)
                            .truncate()
                            .child(line),
                    ),
            )
    }

    fn status(&self, state: &PlayState, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let dot = |c: Hsla| div().size(px(10.)).rounded_full().bg(c).flex_shrink_0();
        let row = h_flex()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .gap(px(10.))
            .text_size(px(15.));
        if let Some(notice) = &self.notice {
            return row
                .text_color(p.error)
                .child(Icon::new(IconName::CircleAlert).size(px(18.)))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .child(format!("Launch failed: {notice}")),
                )
                .when_some(self.launch_log(), |d, log| {
                    d.child(
                        link("launch-log", "View log", p).on_click(move |_, _, cx| cx.open_with_system(&log)),
                    )
                });
        }
        match state {
            PlayState::Running(secs) => row
                .child(dot(p.success))
                .child(div().min_w_0().truncate().child(match secs {
                    Some(s) => format!("Running for {}", duration(*s)),
                    None => "Running".into(),
                }))
                .child(
                    div()
                        .text_color(p.text_placeholder)
                        .child(format!("pid {}", self.processes[0].pid)),
                ),
            PlayState::Launching => row
                .child(dot(p.warning))
                .child(format!(
                    "Starting with profile {}… {}",
                    self.library.profile.name,
                    self.launch
                        .as_ref()
                        .map(|l| duration(l.since.elapsed().as_secs()))
                        .unwrap_or_default()
                ))
                .when_some(self.launch_log(), |d, log| {
                    d.child(link("launch-log", "Log", p).on_click(move |_, _, cx| cx.open_with_system(&log)))
                }),
            PlayState::Crashed(session) => {
                let dir = session.dir.clone();
                let summary = self
                    .crash
                    .as_ref()
                    .filter(|(d, s)| *d == session.dir && !s.is_empty())
                    .map(|(_, s)| s.clone())
                    .unwrap_or_else(|| "reading the log…".into());
                let crashed = format!(
                    "Crashed {}",
                    session
                        .ended
                        .map(|e| ago(game::now_secs() - e))
                        .unwrap_or_default()
                );
                row.items_start()
                    .child(div().mt(px(7.)).child(dot(p.error)))
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                h_flex()
                                    .gap(px(14.))
                                    .child(
                                        div()
                                            .text_color(p.error)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(crashed),
                                    )
                                    .child(
                                        link("crash-log", "Open logs", p)
                                            .on_click(move |_, _, cx| cx.open_with_system(&dir)),
                                    )
                                    .child(link("dismiss", "Dismiss", p).on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.dismissed = this.session.as_ref().map(|s| s.dir.clone());
                                            cx.notify();
                                        },
                                    ))),
                            )
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(p.text_muted)
                                    .truncate()
                                    .child(summary),
                            ),
                    )
            }
            PlayState::NotInstalled => row.text_color(p.text_muted).child(dot(p.text_placeholder)).child(
                div()
                    .min_w_0()
                    .truncate()
                    .child("Install it with Steam or locate its folder"),
            ),
            PlayState::Detecting => row.text_color(p.text_muted).child("Detecting F1 25…"),
            PlayState::Ready => {
                let last = self.session.as_ref().and_then(|s| {
                    let ended = s.ended?;
                    Some(format!(
                        "Last session ended {}{}",
                        ago(game::now_secs() - ended),
                        s.exit_code
                            .map(|c| format!(" · exit code {c}"))
                            .unwrap_or_default()
                    ))
                });
                let route = match &self.install {
                    Some(i) if i.via_steam => "launches via Steam",
                    _ => "launches via Proton",
                };
                row.text_color(p.text_muted)
                    .child(dot(p.accent))
                    .child(div().min_w_0().truncate().child(match last {
                        Some(l) => l,
                        None if cfg!(windows) => "Ready".into(),
                        None => format!("Ready · {route}"),
                    }))
            }
        }
    }

    fn launch_log(&self) -> Option<std::path::PathBuf> {
        let log = self.store.root.join("state/launcher-run.log");
        log.is_file().then_some(log)
    }

    fn profile_picker(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = cx.entity().downgrade();
        let active = self.library.profile.name.clone();
        let names: Vec<String> = self.library.profiles.iter().map(|p| p.name.clone()).collect();
        Button::new("profile-picker")
            .large()
            .outline()
            .h(px(64.))
            .min_w(px(180.))
            .icon(IconName::Layers)
            .label(active.clone())
            .dropdown_caret(true)
            .disabled(matches!(
                self.play_state(),
                PlayState::Running(_) | PlayState::Launching
            ))
            .dropdown_menu_with_anchor(Anchor::BottomRight, move |mut menu, _, _| {
                menu = menu.min_w(px(240.)).label("Profile");
                for name in &names {
                    let (shell, target) = (shell.clone(), name.clone());
                    menu = menu.item(
                        PopupMenuItem::new(name.clone())
                            .checked(*name == active)
                            .on_click(move |_, window, cx| {
                                shell
                                    .update(cx, |s, cx| s.switch_profile(&target, window, cx))
                                    .ok();
                            }),
                    );
                }
                let shell = shell.clone();
                menu.separator().item(
                    PopupMenuItem::new("Manage profiles…")
                        .icon(IconName::Settings)
                        .on_click(move |_, _, cx| {
                            shell.update(cx, |s, cx| s.go(Page::Profiles, cx)).ok();
                        }),
                )
            })
    }

    fn big_button(&self, state: &PlayState, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = p.background;
        let (icon, label, bg, fg, enabled) = match state {
            PlayState::Ready | PlayState::Crashed(_) => (IconName::Play, "Play", p.accent, dark, true),
            PlayState::Launching => (
                IconName::LoaderCircle,
                "Launching…",
                p.accent.opacity(0.45),
                dark,
                false,
            ),
            PlayState::Running(_) => (
                IconName::Activity,
                "Running",
                p.success.opacity(0.16),
                p.success,
                false,
            ),
            PlayState::NotInstalled => (IconName::FolderSearch, "Locate game", p.elevated, p.text, true),
            PlayState::Detecting => (IconName::LoaderCircle, "Play", p.elevated, p.text_muted, false),
        };
        let hover = match state {
            PlayState::NotInstalled => p.selected,
            _ => p.accent_hover,
        };
        let running = matches!(state, PlayState::Running(_));
        let missing = matches!(state, PlayState::NotInstalled);
        div()
            .id("play")
            .h(px(64.))
            .w(px(260.))
            .flex_shrink_0()
            .rounded(px(12.))
            .bg(bg)
            .text_color(fg)
            .flex()
            .items_center()
            .justify_center()
            .gap(px(12.))
            .when(missing, |d| d.border_1().border_color(p.border))
            .when(running, |d| d.border_1().border_color(p.success.opacity(0.4)))
            .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)))
            .map(|d| match icon {
                IconName::LoaderCircle => d.child(Spinner::new().icon(icon).with_size(px(26.))),
                _ => d.child(Icon::new(icon).size(px(26.))),
            })
            .child(
                div()
                    .text_size(px(22.))
                    .font_weight(FontWeight::BOLD)
                    .child(label),
            )
            .when(enabled, |d| {
                d.on_click(cx.listener(move |this, _, window, cx| {
                    if missing {
                        this.locate_game(window, cx);
                    } else {
                        this.play(window, cx);
                    }
                }))
            })
    }

    pub(crate) fn play_bar(&self, p: Palette, wide: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.play_state();
        h_flex()
            .h(px(104.))
            .flex_shrink_0()
            .px(px(24.))
            .gap(px(24.))
            .bg(p.surface)
            .border_t_1()
            .border_color(p.border_variant)
            .child(self.game_info(p, wide))
            .child(self.status(&state, p, cx))
            .child(
                h_flex()
                    .gap(px(14.))
                    .flex_shrink_0()
                    .when(matches!(state, PlayState::Running(_)), |d| {
                        d.child(
                            Button::new("stop")
                                .danger()
                                .outline()
                                .large()
                                .h(px(64.))
                                .icon(IconName::Square)
                                .label("Stop")
                                .on_click(cx.listener(|this, _, window, cx| this.confirm_stop(window, cx))),
                        )
                    })
                    .when(
                        self.install.is_some() && !matches!(state, PlayState::Running(_)),
                        |d| {
                            d.child(
                                Button::new("locate")
                                    .ghost()
                                    .large()
                                    .icon(IconName::FolderSearch)
                                    .tooltip("Change game folder")
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.locate_game(window, cx)),
                                    ),
                            )
                        },
                    )
                    .child(self.profile_picker(cx))
                    .child(self.big_button(&state, p, cx)),
            )
    }
}
