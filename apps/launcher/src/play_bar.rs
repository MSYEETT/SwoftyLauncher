use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Disableable, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::game::{self, ago, duration};
use crate::library::plural;
use crate::pages::{avatar, badge};
use crate::shell::{Page, PlayState, Shell};
use crate::theme::Palette;

pub(crate) fn link(id: &'static str, label: &'static str, p: Palette) -> Stateful<Div> {
    div()
        .id(id)
        .cursor_pointer()
        .text_size(px(13.))
        .text_color(p.text_muted)
        .underline()
        .hover(move |s| s.text_color(p.text))
        .child(label)
}

fn launch_failed(row: Div, text: impl std::fmt::Display, p: Palette) -> Div {
    let text = SharedString::from(format!("Launch failed: {text}"));
    let tip = text.clone();
    row.text_color(p.error)
        .child(Icon::new(IconName::CircleAlert).size(px(16.)))
        .child(
            div()
                .id("launch-failed")
                .min_w_0()
                .truncate()
                .child(text)
                .tooltip(move |window, cx| {
                    let tip = tip.clone();
                    Tooltip::element(move |_, _| div().max_w(px(480.)).child(tip.clone())).build(window, cx)
                }),
        )
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
            .w(px(if wide { 400. } else { 300. }))
            .flex_shrink_1()
            .min_w(px(220.))
            .gap(px(12.))
            .child(badge(IconName::Gamepad2, 36., p))
            .child(
                v_flex()
                    .min_w_0()
                    .gap(px(1.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.text_placeholder)
                            .truncate()
                            .child(match mode {
                                Some(m) => format!("{m} · {line}"),
                                None => line,
                            }),
                    ),
            )
    }

    fn status(&self, state: &PlayState, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let dot = |c: Hsla| div().size(px(7.)).rounded_full().bg(c).flex_shrink_0();
        let row = h_flex()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .gap(px(9.))
            .text_size(px(13.));
        if let Some(notice) = &self.notice {
            return launch_failed(row, notice, p).when_some(self.launch_log(), |d, log| {
                d.child(link("launch-log", "View log", p).on_click(move |_, _, cx| cx.open_with_system(&log)))
            });
        }
        let rebuild = !self.library.profile.is_vanilla() && !self.library.rebuild.is_empty();
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
                )
                .when(rebuild, |d| {
                    d.child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(p.warning)
                            .child("Restart to apply database settings"),
                    )
                }),
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
                let ending = self
                    .crash
                    .as_ref()
                    .filter(|(d, _)| *d == session.dir)
                    .map(|(_, e)| e);
                let open =
                    link("crash-log", "Open logs", p).on_click(move |_, _, cx| cx.open_with_system(&dir));
                let dismiss = link("dismiss", "Dismiss", p).on_click(cx.listener(|this, _, _, cx| {
                    this.dismissed = this.session.as_ref().map(|s| s.dir.clone());
                    cx.notify();
                }));
                if let Some(Err(failure)) = ending {
                    return launch_failed(row, failure, p).child(open).child(dismiss);
                }
                let summary = ending
                    .and_then(|e| e.as_ref().ok())
                    .filter(|s| !s.is_empty())
                    .cloned()
                    .unwrap_or_else(|| "reading the log…".into());
                let crashed = format!(
                    "Crashed {}",
                    session
                        .ended
                        .map(|e| ago(game::now_secs() - e))
                        .unwrap_or_default()
                );
                row.items_start()
                    .child(div().mt(px(6.)).child(dot(p.error)))
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                h_flex()
                                    .gap(px(12.))
                                    .child(
                                        div()
                                            .text_color(p.error)
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(crashed),
                                    )
                                    .child(open)
                                    .child(dismiss),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
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
                if rebuild {
                    return row.text_color(p.text_muted).child(dot(p.warning)).child(
                        div().min_w_0().truncate().child(format!(
                            "Ready · rebuilds {} for {} on launch",
                            self.library.profile.name,
                            plural(self.library.rebuild.len(), "changed setting", "changed settings")
                        )),
                    );
                }
                row.text_color(p.text_muted)
                    .child(dot(p.success))
                    .child(div().min_w_0().truncate().child(match last {
                        Some(l) => l,
                        None if cfg!(windows) => "Ready".into(),
                        None => format!("Ready · {route}"),
                    }))
            }
        }
    }

    fn console_toggle(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let on = self.console.is_some();
        h_flex()
            .id("console")
            .h(px(32.))
            .px(px(8.))
            .gap(px(7.))
            .rounded(px(5.))
            .cursor_pointer()
            .text_color(if on { p.text } else { p.text_muted })
            .hover(move |s| s.bg(p.hover).text_color(p.text))
            .tooltip(|window, cx| Tooltip::new("Show console").build(window, cx))
            .child(Checkbox::new("console-check").checked(on).small())
            .child(Icon::new(IconName::SquareTerminal).size(px(16.)))
            .on_click(cx.listener(move |this, _, window, cx| this.set_console(!on, window, cx)))
    }

    fn launch_log(&self) -> Option<std::path::PathBuf> {
        let log = self.store.root.join("state/launcher-run.log");
        log.is_file().then_some(log)
    }

    fn profile_picker(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = cx.entity().downgrade();
        let active = self.library.profile.name.clone();
        let summary = self.library.active().map(|a| a.summary()).unwrap_or_default();
        let profiles: Vec<(String, String)> = self
            .library
            .profiles
            .iter()
            .map(|p| (p.name.clone(), p.summary()))
            .collect();
        Button::new("profile-picker")
            .outline()
            .h(px(40.))
            .min_w(px(180.))
            .child(
                h_flex().gap(px(8.)).child(avatar(&active, 24., p)).child(
                    v_flex()
                        .items_start()
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(active.clone()),
                        )
                        .child(div().text_size(px(11.)).text_color(p.text_muted).child(summary)),
                ),
            )
            .dropdown_caret(true)
            .disabled(matches!(
                self.play_state(),
                PlayState::Running(_) | PlayState::Launching
            ))
            .dropdown_menu_with_anchor(Anchor::BottomRight, move |mut menu, _, _| {
                menu = menu.min_w(px(300.)).label("Profile");
                for (name, summary) in &profiles {
                    let (shell, target) = (shell.clone(), name.clone());
                    let (label, summary) = (name.clone(), summary.clone());
                    menu = menu.item(
                        PopupMenuItem::element(move |_, _| {
                            h_flex()
                                .w_full()
                                .gap(px(24.))
                                .justify_between()
                                .child(label.clone())
                                .child(div().text_color(p.text_muted).child(summary.clone()))
                        })
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
        let (icon, label, bg, fg, enabled) = match state {
            PlayState::Ready | PlayState::Crashed(_) => (IconName::Play, "Play", p.accent, p.on_accent, true),
            PlayState::Launching => (IconName::LoaderCircle, "Launching…", p.elevated, p.accent, false),
            PlayState::Running(_) => (IconName::Activity, "Running", p.elevated, p.success, false),
            PlayState::NotInstalled => (IconName::FolderSearch, "Locate game", p.elevated, p.text, true),
            PlayState::Detecting => (IconName::LoaderCircle, "Play", p.elevated, p.text_muted, false),
        };
        let ready = matches!(state, PlayState::Ready | PlayState::Crashed(_));
        let missing = matches!(state, PlayState::NotInstalled);
        div()
            .id("play")
            .h(px(40.))
            .w(px(168.))
            .flex_shrink_0()
            .rounded(px(5.))
            .bg(bg)
            .text_color(fg)
            .flex()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .when(!ready, |d| d.border_1().border_color(p.border))
            .when(ready, |d| d.hover(move |s| s.bg(p.accent_hover)))
            .when(missing, |d| d.hover(move |s| s.bg(p.selected)))
            .when(enabled, |d| d.cursor_pointer())
            .map(|d| match icon {
                IconName::LoaderCircle => d.child(Spinner::new().icon(icon).with_size(px(16.))),
                _ => d.child(Icon::new(icon).size(px(16.))),
            })
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
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
            .h(px(64.))
            .flex_shrink_0()
            .px(px(16.))
            .gap(px(20.))
            .bg(p.chrome)
            .border_t_1()
            .border_color(p.border_variant)
            .child(self.game_info(p, wide))
            .child(self.status(&state, p, cx))
            .child(
                h_flex()
                    .gap(px(8.))
                    .flex_shrink_0()
                    .child(self.console_toggle(p, cx))
                    .when(matches!(state, PlayState::Running(_)), |d| {
                        d.child(
                            Button::new("stop")
                                .danger()
                                .outline()
                                .h(px(40.))
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
                                    .icon(IconName::FolderSearch)
                                    .tooltip("Change game folder")
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.locate_game(window, cx)),
                                    ),
                            )
                        },
                    )
                    .child(self.profile_picker(p, cx))
                    .child(self.big_button(&state, p, cx)),
            )
    }
}
