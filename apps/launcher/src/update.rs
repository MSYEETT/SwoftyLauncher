use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, Sizable, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use semver::Version;
use swofty_core::managed;
use swofty_core::update::{self, Install, Release};
use swofty_core::updater::{self, Gate, Part, Phase, Track};

use crate::play_bar::link;
use crate::sections::{Reporter, Shared};
use crate::shell::{PlayState, Shell};
use crate::theme::Palette;

const TICK: Duration = Duration::from_secs(5);
const RESTART_DELAY: Duration = Duration::from_millis(1500);
pub const BUTTON_WIDTH: f32 = 204.;

pub struct Updates {
    pub copy: Result<Install, String>,
    pub launcher: Track,
    pub addon: Track,
    pub addon_installed: Option<Version>,
    pub phase: Phase,
    pub checking: bool,
    pub adding: bool,
    pub progress: Option<(f32, String)>,
    pub restarting: Vec<(Part, Version)>,
    pub expected: Option<String>,
}

impl Updates {
    pub fn new() -> Self {
        let copy = Install::current();
        if let Ok(i) = &copy
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
        Self {
            copy,
            launcher: Track::default(),
            addon: Track::default(),
            addon_installed: None,
            phase: Phase::Idle,
            checking: false,
            adding: false,
            progress: None,
            restarting: Vec::new(),
            expected: std::env::var(updater::UPDATED_ENV).ok().filter(|v| !v.is_empty()),
        }
    }

    fn launcher_offer(&self) -> Option<&Release> {
        self.copy
            .is_ok()
            .then(|| self.launcher.offer(Some(&update::current_version())))
            .flatten()
    }

    fn addon_offer(&self) -> Option<&Release> {
        self.addon_installed
            .as_ref()
            .and_then(|v| self.addon.offer(Some(v)))
    }

    pub fn pending(&self) -> Vec<(Part, Release)> {
        updater::plan(self.addon_offer(), self.launcher_offer())
    }

    fn pending_versions(&self) -> Vec<(Part, Version)> {
        self.pending()
            .iter()
            .filter_map(|(part, r)| r.version().map(|v| (*part, v)))
            .collect()
    }

    fn blocked_notice(&self) -> Option<(Version, String, &str)> {
        let reason = self.copy.as_ref().err()?;
        let version = self.launcher.version()?;
        (version > update::current_version()).then(|| {
            (
                version,
                self.launcher.tag.clone().unwrap_or_default(),
                reason.as_str(),
            )
        })
    }
}

fn log_change(part: Part, before: &Option<String>, after: &Option<String>) {
    if before != after
        && let Some(e) = after
    {
        eprintln!("swofty-launcher: {} update check: {e}", part.name());
    }
}

fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

impl Shell {
    pub(crate) fn watch_updates(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                if this.update(cx, |s, cx| s.tick_updates(cx)).is_err() {
                    break;
                }
                cx.background_executor().timer(TICK).await;
            }
        })
    }

    fn tick_updates(&mut self, cx: &mut Context<Self>) {
        let now = update::now_secs();
        let check_launcher = self.store.config().updates.check;
        let u = &self.updates;
        let addon_due = u.addon.probe_due(now);
        let launcher_due = check_launcher && u.launcher.probe_due(now);
        if u.checking
            || matches!(u.phase, Phase::Installing | Phase::Restarting)
            || !(addon_due || launcher_due)
        {
            return;
        }
        self.updates.checking = true;
        let (mut addon, mut launcher) = (self.updates.addon.clone(), self.updates.launcher.clone());
        let can_install = self.updates.copy.is_ok();
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let (addon, launcher, installed) = cx
                .background_executor()
                .spawn(async move {
                    let installed = managed::installed(&store);
                    if addon_due {
                        updater::check(Part::Addon, &mut addon, installed.as_ref(), true, now);
                    }
                    if launcher_due {
                        let current = update::current_version();
                        updater::check(Part::Launcher, &mut launcher, Some(&current), can_install, now);
                    }
                    (addon, launcher, installed)
                })
                .await;
            this.update(cx, |s, cx| s.checked(addon, launcher, installed, cx))
                .ok();
        })
        .detach();
    }

    fn checked(&mut self, addon: Track, launcher: Track, installed: Option<Version>, cx: &mut Context<Self>) {
        let u = &mut self.updates;
        log_change(Part::Addon, &u.addon.error, &addon.error);
        log_change(Part::Launcher, &u.launcher.error, &launcher.error);
        (u.addon, u.launcher, u.addon_installed, u.checking) = (addon, launcher, installed, false);
        if u.phase == Phase::Idle
            && let Some(offer) = u.launcher_offer()
            && u.expected.as_deref() == Some(offer.tag_name.as_str())
            && let Some(text) = updater::did_not_take(u.expected.as_deref(), &update::current_version())
        {
            u.phase = Phase::Failed(text);
        }
        if u.addon_installed.is_none()
            && !u.adding
            && u.phase == Phase::Idle
            && let Some(release) = u.addon.offer(None).cloned()
        {
            self.add_missing_addon(release, cx);
        }
        cx.notify();
    }

    fn add_missing_addon(&mut self, release: Release, cx: &mut Context<Self>) {
        self.updates.adding = true;
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(
                    async move { managed::install_release(&store, &release, &Reporter(Shared::default())) },
                )
                .await;
            this.update(cx, |s, cx| {
                s.updates.adding = false;
                match result {
                    Ok(version) => {
                        eprintln!("swofty-launcher: installed {} {version}", managed::ID);
                        s.updates.addon_installed = Some(version);
                        s.reload(cx);
                    }
                    Err(e) => eprintln!("swofty-launcher: could not install {}: {e}", managed::ID),
                }
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn update_gate(&self, state: &PlayState) -> Gate {
        let busy = matches!(
            state,
            PlayState::Running | PlayState::Launching | PlayState::Restoring | PlayState::Detecting
        );
        updater::gate(&self.updates.phase, !self.updates.pending().is_empty(), busy)
    }

    fn start_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.play_state(), PlayState::Running | PlayState::Launching) {
            window.push_notification(Notification::error("Close F1 25 before updating."), cx);
            return;
        }
        let steps = self.updates.pending();
        if steps.is_empty() {
            return;
        }
        self.updates.phase = Phase::Installing;
        let shared: Shared = Arc::new(Mutex::new(Some((0., "Starting the update".into()))));
        let reporter = Reporter(shared.clone());
        self.follow_update_progress(shared, cx);
        let store = self.store.clone();
        let copy = self.updates.copy.clone().ok();
        cx.spawn_in(window, async move |this, cx| {
            let ran = cx
                .background_executor()
                .spawn(async move {
                    let ran = updater::run(&steps, &reporter, |part, release, progress| {
                        updater::install(&store, copy.as_ref(), part, release, progress)
                    });
                    if let Ok(mut p) = reporter.0.lock() {
                        *p = None;
                    }
                    ran
                })
                .await;
            this.update_in(cx, |s, window, cx| {
                s.updates.progress = None;
                if let Some((_, version)) = ran.done.iter().find(|(part, _)| *part == Part::Addon) {
                    s.updates.addon_installed = Some(version.clone());
                    s.reload(cx);
                }
                match &ran.error {
                    None => {
                        s.updates.phase = Phase::Restarting;
                        s.updates.restarting = ran.done.clone();
                        let launcher = ran.launcher().map(|v| format!("v{v}"));
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(RESTART_DELAY).await;
                            this.update(cx, |s, cx| s.restart(launcher, cx)).ok();
                        })
                        .detach();
                    }
                    Some(e) => {
                        window.push_notification(Notification::error(e.clone()).title("Update failed"), cx);
                        s.updates.phase = Phase::Failed(format!("Update failed: {e}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn follow_update_progress(&mut self, shared: Shared, cx: &mut Context<Self>) {
        self.updates.progress = shared.lock().ok().and_then(|p| p.clone());
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let now = shared.lock().ok().and_then(|p| p.clone());
                let done = now.is_none();
                if this
                    .update(cx, |s, cx| {
                        if now.is_some() {
                            s.updates.progress = now;
                        }
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
    }

    fn restart(&mut self, launcher: Option<String>, cx: &mut Context<Self>) {
        let exe = match &self.updates.copy {
            Ok(install) => Ok(install.launcher()),
            Err(_) => std::env::current_exe(),
        };
        let mut command = match exe {
            Ok(exe) => std::process::Command::new(exe),
            Err(e) => {
                self.updates.phase = Phase::Failed(format!(
                    "Updated, but could not restart: {e}. Close and reopen the launcher."
                ));
                cx.notify();
                return;
            }
        };
        command.env_remove(update::PRETEND_ENV);
        match &launcher {
            Some(tag) => command.env(updater::UPDATED_ENV, tag),
            None => command.env_remove(updater::UPDATED_ENV),
        };
        match command.spawn() {
            Ok(_) => cx.quit(),
            Err(e) => {
                self.updates.phase = Phase::Failed(format!(
                    "Updated, but could not restart: {e}. Close and reopen the launcher."
                ));
                cx.notify();
            }
        }
    }

    pub(crate) fn update_status(
        &self,
        gate: Gate,
        row: Div,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let dot = |c: Hsla| div().size(px(7.)).rounded_full().bg(c).flex_shrink_0();
        let text = |t: String| div().min_w_0().truncate().child(t);
        let u = &self.updates;
        match gate {
            Gate::Update => {
                let changes: Vec<String> = u
                    .pending_versions()
                    .iter()
                    .map(|(part, v)| {
                        let from = match part {
                            Part::Addon => u.addon_installed.as_ref().map(ToString::to_string),
                            Part::Launcher => Some(update::current_version().to_string()),
                        };
                        match from {
                            Some(from) => format!("{} {from} → {v}", part.name()),
                            None => format!("{} {v}", part.name()),
                        }
                    })
                    .collect();
                let notes = u.pending().last().map(|(_, r)| r.notes_url());
                Some(
                    row.child(dot(p.accent))
                        .child(text(format!(
                            "Update required: {}. The launcher restarts afterwards.",
                            changes.join(" and ")
                        )))
                        .when_some(notes, |d, notes| {
                            d.child(
                                link("update-notes", "Release notes", p)
                                    .flex_shrink_0()
                                    .on_click(move |_, _, cx| cx.open_url(&notes)),
                            )
                        }),
                )
            }
            Gate::Installing => Some(row.child(dot(p.warning)).child(text(format!(
                "Updating {}. Keep the launcher open; it restarts when it's done.",
                updater::describe(&u.pending_versions())
            )))),
            Gate::Restarting => Some(row.child(dot(p.success)).child(text(format!(
                "Updated {}. Restarting the launcher…",
                updater::describe(&u.restarting)
            )))),
            Gate::Retry | Gate::Play => {
                let Phase::Failed(error) = &u.phase else {
                    return None;
                };
                if gate == Gate::Play && !u.pending().is_empty() {
                    return None;
                }
                let tip = SharedString::from(error.clone());
                Some(
                    row.text_color(p.error)
                        .child(Icon::new(IconName::CircleAlert).size(px(16.)))
                        .child(
                            div()
                                .id("update-failed")
                                .min_w_0()
                                .truncate()
                                .child(error.clone())
                                .tooltip(move |window, cx| {
                                    let tip = tip.clone();
                                    Tooltip::element(move |_, _| div().max_w(px(480.)).child(tip.clone()))
                                        .build(window, cx)
                                }),
                        )
                        .when(gate == Gate::Retry, |d| {
                            d.child(
                                link("play-anyway", "Play without updating", p)
                                    .flex_shrink_0()
                                    .on_click(cx.listener(|this, _, window, cx| this.play(window, cx))),
                            )
                        }),
                )
            }
        }
    }

    pub(crate) fn update_button(&self, gate: Gate, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let u = &self.updates;
        let summary = updater::summary(&u.pending_versions());
        let (progress, detail) = match &u.progress {
            Some((done, text)) => (Some(*done), text.clone()),
            None => (None, summary.clone()),
        };
        let (icon, label, sub, bg, fg, active) = match gate {
            Gate::Installing => (
                IconName::LoaderCircle,
                match progress {
                    Some(done) => format!("Updating {:.0}%", done * 100.),
                    None => "Updating…".into(),
                },
                detail,
                p.elevated,
                p.text,
                false,
            ),
            Gate::Restarting => (
                IconName::LoaderCircle,
                "Restarting…".into(),
                updater::summary(&u.restarting),
                p.elevated,
                p.text,
                false,
            ),
            Gate::Retry => (
                IconName::RotateCw,
                "Retry update".into(),
                summary,
                p.accent,
                p.on_accent,
                true,
            ),
            Gate::Update | Gate::Play => (
                IconName::CircleArrowUp,
                "Update".into(),
                summary,
                p.accent,
                p.on_accent,
                true,
            ),
        };
        div()
            .id("play")
            .relative()
            .overflow_hidden()
            .h(px(52.))
            .w(px(BUTTON_WIDTH))
            .flex_shrink_0()
            .rounded(px(9.))
            .bg(bg)
            .text_color(fg)
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(14.))
            .when(!active, |d| d.border_1().border_color(p.border))
            .when(active, |d| {
                d.cursor_pointer()
                    .hover(move |s| s.bg(p.accent_hover))
                    .on_click(cx.listener(|this, _, window, cx| this.start_updates(window, cx)))
            })
            .map(|d| match icon {
                IconName::LoaderCircle => d.child(Spinner::new().icon(icon).with_size(px(18.))),
                _ => d.child(Icon::new(icon).size(px(18.))),
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(label),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .truncate()
                            .text_color(if active { p.on_accent } else { p.text_muted })
                            .when(active, |d| d.opacity(0.85))
                            .child(sub),
                    ),
            )
            .when_some(progress.filter(|_| gate == Gate::Installing), |d, done| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .bottom_0()
                        .h(px(4.))
                        .w(relative(done.clamp(0., 1.)))
                        .bg(p.accent),
                )
            })
            .into_any_element()
    }

    pub(crate) fn update_bar(&self, p: Palette) -> Option<impl IntoElement> {
        if !self.store.config().updates.check {
            return None;
        }
        let (version, tag, reason) = self.updates.blocked_notice()?;
        let notes = update::notes_url(&tag);
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
                .child(div().flex_1().min_w_0().truncate().child(format!(
                    "Swofty Launcher {version} is available. {}. You have {}.",
                    sentence(reason),
                    update::current_version()
                )))
                .child(
                    link("update-release", "Release page", p)
                        .flex_shrink_0()
                        .on_click(move |_, _, cx| cx.open_url(&notes)),
                ),
        )
    }
}
