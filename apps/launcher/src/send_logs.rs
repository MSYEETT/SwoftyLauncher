use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Disableable, Icon, Sizable, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::send_logs::{self, Bundle, GameInfo, MESSAGE_MAX, Report, Stage};
use swofty_core::store::Store;

use crate::shell::Shell;
use crate::theme::{Palette, palette};

pub const LABEL: &str = "Send Logs to Developers";

const WHAT_IS_SENT: &str = "Your launcher and game logs. They can include folder paths with your computer's \
                            user name, your hardware, and your installed mods and profiles. Passwords and \
                            tokens in the launcher's settings are left out.";

#[derive(Clone, PartialEq)]
enum Phase {
    Editing,
    Working,
    Sent(usize),
    Failed(String),
}

pub struct SendLogs {
    store: Store,
    game: GameInfo,
    name: Entity<InputState>,
    message: Entity<TextareaState>,
    phase: Phase,
    invalid: Option<String>,
    stage: Arc<Mutex<Stage>>,
    _work: Option<Task<()>>,
    _poll: Option<Task<()>>,
}

impl SendLogs {
    fn new(store: Store, game: GameInfo, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let remembered = store.config().discord_username.unwrap_or_default();
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("e.g. swofty")
                .default_value(remembered)
        });
        let message = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Optional: what you were doing and what happened")
                .rows(4)
        });
        cx.subscribe(&name, |f: &mut Self, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) && f.invalid.take().is_some() {
                cx.notify();
            }
        })
        .detach();
        cx.subscribe(&message, |f: &mut Self, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                f.invalid = None;
                cx.notify();
            }
        })
        .detach();
        Self {
            store,
            game,
            name,
            message,
            phase: Phase::Editing,
            invalid: None,
            stage: Arc::new(Mutex::new(Stage::Collecting)),
            _work: None,
            _poll: None,
        }
    }

    fn working(&self) -> bool {
        self.phase == Phase::Working
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.phase, Phase::Working | Phase::Sent(_)) {
            return;
        }
        let name = send_logs::check_name(&self.name.read(cx).value());
        let message = send_logs::check_message(&self.message.read(cx).value());
        let (name, message) = match (name, message) {
            (Ok(n), Ok(m)) => (n, m),
            (Err(e), _) | (_, Err(e)) => {
                self.invalid = Some(e);
                cx.notify();
                return;
            }
        };
        self.invalid = None;
        self.phase = Phase::Working;
        if let Ok(mut s) = self.stage.lock() {
            *s = Stage::Collecting;
        }
        let (store, stage) = (self.store.clone(), self.stage.clone());
        let report = Report {
            name: name.clone(),
            message,
            game: self.game.clone(),
        };
        self._work = Some(cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let bundle = Bundle::collect(&store, &report);
                    let progress = |s: Stage| {
                        if let Ok(mut slot) = stage.lock() {
                            *slot = s;
                        }
                    };
                    send_logs::send(&bundle, &progress).map_err(|e| e.to_string())
                })
                .await;
            this.update(cx, |f, cx| f.finish(name, result, cx)).ok();
        }));
        self._poll = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(150)).await;
                let Ok(working) = this.update(cx, |f, cx| {
                    cx.notify();
                    f.working()
                }) else {
                    break;
                };
                if !working {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn finish(&mut self, name: String, result: Result<usize, String>, cx: &mut Context<Self>) {
        self.phase = match result {
            Ok(parts) => {
                let mut config = self.store.config();
                if config.discord_username.as_deref() != Some(name.as_str()) {
                    config.discord_username = Some(name);
                    if let Err(e) = self.store.save_config(&config) {
                        eprintln!("swofty-launcher: could not remember the Discord username: {e}");
                    }
                }
                Phase::Sent(parts)
            }
            Err(e) => Phase::Failed(e),
        };
        cx.notify();
    }

    fn status(&self, p: Palette) -> Option<Div> {
        let row = || h_flex().gap(px(8.)).items_start().text_size(px(13.));
        let text = |t: String| div().flex_1().min_w_0().child(t);
        match &self.phase {
            Phase::Editing => self.invalid.clone().map(|e| {
                row()
                    .text_color(p.error)
                    .child(Icon::new(IconName::CircleAlert).size(px(15.)))
                    .child(text(e))
            }),
            Phase::Working => {
                let stage = self.stage.lock().map(|s| s.to_string()).unwrap_or_default();
                Some(
                    row()
                        .text_color(p.text_muted)
                        .child(Spinner::new().with_size(px(15.)))
                        .child(text(stage)),
                )
            }
            Phase::Sent(parts) => Some(
                row()
                    .text_color(p.success)
                    .child(Icon::new(IconName::CircleCheck).size(px(15.)))
                    .child(text(match parts {
                        1 => "Sent — thanks!".into(),
                        n => format!("Sent in {n} parts — thanks!"),
                    })),
            ),
            Phase::Failed(e) => Some(
                row()
                    .text_color(p.error)
                    .child(Icon::new(IconName::CircleAlert).size(px(15.)))
                    .child(text(format!("Not sent. {e}"))),
            ),
        }
    }

    fn footer(&self, form: Entity<Self>) -> DialogFooter {
        let close = |label: &'static str| {
            Button::new("send-logs-cancel")
                .label(label)
                .on_click(|_, window, cx| window.close_dialog(cx))
        };
        match &self.phase {
            Phase::Sent(_) => DialogFooter::new().child(
                Button::new("send-logs-done")
                    .primary()
                    .label("Done")
                    .on_click(|_, window, cx| window.close_dialog(cx)),
            ),
            phase => {
                let working = *phase == Phase::Working;
                DialogFooter::new()
                    .child(close(if working { "Close" } else { "Cancel" }))
                    .child(
                        Button::new("send-logs-send")
                            .primary()
                            .icon(IconName::Send)
                            .label(match phase {
                                Phase::Working => "Sending…",
                                Phase::Failed(_) => "Try again",
                                _ => "Send",
                            })
                            .loading(working)
                            .disabled(working)
                            .on_click(move |_, window, cx| {
                                form.update(cx, |f, cx| f.submit(window, cx));
                            }),
                    )
            }
        }
    }
}

fn field(title: &'static str, hint: Option<SharedString>, p: Palette) -> Div {
    h_flex()
        .justify_between()
        .text_size(px(13.))
        .child(div().font_weight(FontWeight::MEDIUM).child(title))
        .children(hint.map(|h| div().text_size(px(12.)).text_color(p.text_placeholder).child(h)))
}

impl Render for SendLogs {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let locked = matches!(self.phase, Phase::Working | Phase::Sent(_));
        let count = self.message.read(cx).value().chars().count();
        v_flex()
            .gap(px(14.))
            .child(
                v_flex()
                    .gap(px(6.))
                    .child(field(
                        "Your Discord username",
                        Some("So the developers can reply".into()),
                        p,
                    ))
                    .child(Input::new(&self.name).disabled(locked)),
            )
            .child(
                v_flex()
                    .gap(px(6.))
                    .child(field(
                        "What went wrong?",
                        Some(format!("{count}/{MESSAGE_MAX}").into()),
                        p,
                    ))
                    .child(Textarea::new(&self.message).h(px(96.)).disabled(locked)),
            )
            .child(
                h_flex()
                    .gap(px(10.))
                    .items_start()
                    .p(px(12.))
                    .rounded(px(6.))
                    .bg(p.elevated)
                    .border_1()
                    .border_color(p.border)
                    .child(Icon::new(IconName::Info).size(px(15.)).text_color(p.accent))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("What gets sent"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(p.text_muted)
                                    .child(WHAT_IS_SENT),
                            ),
                    ),
            )
            .children(self.status(p))
    }
}

impl Shell {
    pub(crate) fn send_logs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let game = GameInfo {
            dir: self.install.as_ref().map(|i| i.dir.clone()),
            build_id: self.install.as_ref().and_then(|i| i.build_id),
            version: self.install.as_ref().and_then(|i| i.exe_version.clone()),
        };
        let store = self.store.clone();
        let form = cx.new(|cx| SendLogs::new(store, game, window, cx));
        let focus = std::cell::Cell::new(true);
        window.open_alert_dialog(cx, move |alert, window, cx| {
            if focus.replace(false) {
                let name = form.read(cx).name.clone();
                name.update(cx, |s, cx| s.focus(window, cx));
            }
            let submit = form.clone();
            let footer = form.read(cx).footer(form.clone());
            alert
                .width(px(560.))
                .title(LABEL)
                .description("Sends your logs to the developers on Discord, so they can see what went wrong.")
                .child(form.clone())
                .footer(footer)
                .on_ok(move |_, window, cx| {
                    submit.update(cx, |f, cx| f.submit(window, cx));
                    false
                })
        });
    }

    pub(crate) fn send_logs_button(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = send_logs::available();
        h_flex()
            .id("send-logs")
            .min_h(px(36.))
            .px(px(10.))
            .py(px(6.))
            .gap(px(9.))
            .rounded(px(8.))
            .border_1()
            .border_color(p.border_variant)
            .bg(p.surface)
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .map(|d| {
                if enabled {
                    d.cursor_pointer()
                        .text_color(p.text_muted)
                        .hover(move |s| s.bg(p.hover).border_color(p.border).text_color(p.text))
                        .on_click(cx.listener(|this, _, window, cx| this.send_logs(window, cx)))
                } else {
                    d.text_color(p.text_placeholder)
                }
            })
            .child(Icon::new(IconName::Send).size(px(15.)).flex_shrink_0())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(if enabled { LABEL } else { send_logs::UNAVAILABLE }),
            )
    }
}
