use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{Disableable, Icon, Sizable, WindowExt, h_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::update::{self, Install};

use crate::play_bar::link;
use crate::sections::{Reporter, Shared};
use crate::shell::{PlayState, Shell};
use crate::theme::Palette;

#[derive(Clone, Copy, PartialEq)]
pub enum Stage {
    Available,
    Installing,
    Installed,
}

pub struct Update {
    pub version: String,
    pub tag: String,
    pub install: Result<Install, String>,
    pub stage: Stage,
}

impl Shell {
    pub(crate) fn check_for_update(&mut self, cx: &mut Context<Self>) {
        let install = Install::current();
        if let Ok(i) = &install
            && i.staging().exists()
        {
            let i = i.clone();
            std::thread::spawn(move || {
                for _ in 0..3 {
                    if i.cleanup() {
                        break;
                    }
                    std::thread::sleep(Duration::from_secs(5));
                }
            });
        }
        let updates = self.store.config().updates;
        if !updates.check {
            return;
        }
        let due = updates.due(update::now_secs());
        cx.spawn(async move |this, cx| {
            let fetched = if due {
                cx.background_executor()
                    .spawn(async { update::fetch_latest(update::CHECK_TIMEOUT).ok() })
                    .await
            } else {
                None
            };
            this.update(cx, |s, cx| {
                let mut config = s.store.config();
                if let Some(release) = &fetched {
                    config.updates.record(&release.tag_name, update::now_secs());
                    s.store.save_config(&config).ok();
                }
                s.update = config
                    .updates
                    .offer(&update::current_version())
                    .map(|version| Update {
                        version: version.to_string(),
                        tag: config.updates.latest.clone().unwrap_or_default(),
                        install,
                        stage: Stage::Available,
                    });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn start_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.play_state(), PlayState::Running | PlayState::Launching) {
            window.push_notification(Notification::error("Close F1 25 before updating."), cx);
            return;
        }
        let Some(u) = self.update.as_mut() else {
            return;
        };
        let Ok(install) = u.install.clone() else {
            return;
        };
        u.stage = Stage::Installing;
        let shared: Shared = Arc::new(Mutex::new(Some((0., format!("Downloading {}", u.tag)))));
        let reporter = Reporter(shared.clone());
        self.follow_progress(shared, cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let result = update::fetch_latest(Duration::from_secs(30))
                        .and_then(|r| install.update(&r, &reporter).map(|v| (v, r.tag_name)));
                    if let Ok(mut p) = reporter.0.lock() {
                        *p = None;
                    }
                    result
                })
                .await;
            this.update_in(cx, |s, window, cx| {
                let Some(u) = s.update.as_mut() else {
                    return;
                };
                match result {
                    Ok((version, tag)) => {
                        window.push_notification(
                            Notification::success(format!("Swofty Launcher {version} is installed")),
                            cx,
                        );
                        (u.version, u.tag, u.stage) = (version.to_string(), tag, Stage::Installed);
                    }
                    Err(e) => {
                        window
                            .push_notification(Notification::error(e.to_string()).title("Update failed"), cx);
                        u.stage = Stage::Available;
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn skip_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(u) = self.update.take() else {
            return;
        };
        let mut config = self.store.config();
        config.updates.skip = Some(u.version);
        if let Err(e) = self.store.save_config(&config) {
            window.push_notification(Notification::error(e.to_string()), cx);
        }
        cx.notify();
    }

    fn restart(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Ok(install)) = self.update.as_ref().map(|u| &u.install) else {
            return;
        };
        match std::process::Command::new(install.launcher()).spawn() {
            Ok(_) => cx.quit(),
            Err(e) => window.push_notification(
                Notification::error(format!("{}: {e}", install.launcher().display()))
                    .title("Could not restart"),
                cx,
            ),
        }
    }

    pub(crate) fn update_bar(&self, p: Palette, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let u = self.update.as_ref()?;
        let version = &u.version;
        let notes = update::notes_url(&u.tag);
        let text = match (u.stage, &u.install) {
            (Stage::Installed, _) => format!("Swofty Launcher {version} is installed. Restart to use it."),
            (Stage::Installing, _) => format!("Updating to Swofty Launcher {version}…"),
            (Stage::Available, Ok(_)) => format!(
                "Swofty Launcher {version} is available. You have {}.",
                update::current_version()
            ),
            (Stage::Available, Err(reason)) => format!("Swofty Launcher {version} is available ({reason})."),
        };
        let open_notes = {
            let notes = notes.clone();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| cx.open_url(&notes)
        };
        let action = match (u.stage, u.install.is_ok()) {
            (Stage::Installed, _) => Button::new("update-restart")
                .primary()
                .small()
                .icon(IconName::RotateCw)
                .label("Restart")
                .on_click(cx.listener(|this, _, window, cx| this.restart(window, cx))),
            (Stage::Installing, _) => Button::new("update-running")
                .primary()
                .small()
                .loading(true)
                .disabled(true)
                .label("Updating…"),
            (Stage::Available, true) => Button::new("update-install")
                .primary()
                .small()
                .icon(IconName::Download)
                .label("Update")
                .on_click(cx.listener(|this, _, window, cx| this.start_update(window, cx))),
            (Stage::Available, false) => Button::new("update-download")
                .outline()
                .small()
                .icon(IconName::ExternalLink)
                .label("Download")
                .on_click(open_notes.clone()),
        };
        Some(
            h_flex()
                .id("update-bar")
                .flex_shrink_0()
                .px(px(16.))
                .py(px(8.))
                .gap(px(12.))
                .bg(p.surface)
                .border_t_1()
                .border_color(p.border_variant)
                .text_size(px(13.))
                .child(
                    Icon::new(IconName::CircleArrowUp)
                        .size(px(16.))
                        .text_color(p.accent),
                )
                .child(div().flex_1().min_w_0().truncate().child(text))
                .child(link("update-notes", "Release notes", p).on_click(open_notes))
                .when(u.stage == Stage::Available, |d| {
                    d.child(
                        link("update-skip", "Skip this version", p)
                            .on_click(cx.listener(|this, _, window, cx| this.skip_update(window, cx))),
                    )
                })
                .child(action),
        )
    }
}
