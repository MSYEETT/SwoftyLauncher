use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariant};
use gpui_kit::component::dialog::AlertDialog;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::slider::SliderState;
use gpui_kit::component::{Icon, TitleBar, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::crash::StartFailure;
use swofty_core::process::{GameProcess, stop_game};
use swofty_core::recovery::{self, Blocked, Notice};
use swofty_core::saves::{self, SavesWarning};
use swofty_core::session::{LaunchMode, SessionRecord};
use swofty_core::store::Store;
use swofty_core::view::View;

use crate::game::{self, Install, Launch, Probe};
use crate::library::{AddonIcon, Library};
use crate::pages::{addon_icon, label};
use crate::theme::{Palette, palette};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Addons,
    Profiles,
    Section(usize),
}

impl Page {
    pub fn label(self) -> &'static str {
        match self {
            Self::Addons => "Addons",
            Self::Profiles => "Profiles",
            Self::Section(_) => "",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Self::Addons => IconName::Puzzle,
            Self::Profiles => IconName::Layers,
            Self::Section(_) => IconName::Package,
        }
    }
}

pub enum PlayState {
    Detecting,
    NotInstalled,
    Ready,
    Launching,
    Restoring,
    Running,
    Crashed(SessionRecord),
}

pub struct Shell {
    pub(crate) store: Store,
    pub(crate) page: Page,
    pub(crate) library: Library,
    pub(crate) open_addon: Option<String>,
    pub(crate) install: Option<Install>,
    pub(crate) detected: bool,
    pub(crate) rediscover: bool,
    pub(crate) processes: Vec<GameProcess>,
    pub(crate) session: Option<SessionRecord>,
    pub(crate) crash: Option<(PathBuf, Result<String, StartFailure>)>,
    pub(crate) dismissed: Option<PathBuf>,
    pub(crate) launch: Option<Launch>,
    pub(crate) notice: Option<String>,
    pub(crate) blocked: Option<Blocked>,
    pub(crate) recovered_seen: f64,
    pub(crate) restoring: bool,
    pub(crate) sliders: HashMap<String, SliderSlot>,
    pub(crate) requests: Option<SystemTime>,
    pub(crate) console: Option<AnyWindowHandle>,
    pub(crate) views: HashMap<String, Result<View, String>>,
    pub(crate) views_profile: String,
    pub(crate) view_gen: u64,
    pub(crate) view_gens: HashMap<String, u64>,
    pub(crate) busy: HashSet<String>,
    pub(crate) open_row: Option<String>,
    pub(crate) progress: Option<(f32, String)>,
    pub(crate) dropping: bool,
    pub(crate) sidebar_list: bool,
    pub(crate) vr_profiles: BTreeSet<String>,
    pub(crate) start_steam: bool,
    pub(crate) updates: crate::update::Updates,
    pub(crate) forms: HashMap<String, crate::blocks::FormSlot>,
    pub(crate) searches: HashMap<String, crate::blocks::SearchSlot>,
    pub(crate) sorts: HashMap<String, (String, bool)>,
    pub(crate) cell: Option<crate::blocks::CellEdit>,
    pub(crate) pending_cells: HashMap<String, String>,
    pub(crate) content_width: f32,
    pub(crate) ticking: bool,
    painted: Instant,
    pub(crate) live_n: u64,
    _console_closed: Subscription,
    _poll: Task<()>,
    _live: Task<()>,
    _updates: Task<()>,
}

pub struct SliderSlot {
    pub state: Entity<SliderState>,
    pub synced: usize,
    pub _changes: Subscription,
}

pub type Action = Rc<dyn Fn(&Store, &str) -> Result<String, String>>;

pub struct Prompt {
    pub title: &'static str,
    pub description: String,
    pub initial: String,
    pub placeholder: &'static str,
    pub ok: &'static str,
    pub browse: Option<PathPromptOptions>,
    pub action: Action,
}

fn section_label(title: &'static str, p: Palette) -> impl IntoElement {
    div()
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.text_placeholder)
        .child(title)
}

fn terms_box(p: Palette) -> impl IntoElement {
    v_flex()
        .gap(px(6.))
        .p(px(12.))
        .rounded(px(6.))
        .bg(p.elevated)
        .border_1()
        .border_color(p.border)
        .child(
            h_flex()
                .gap(px(8.))
                .items_center()
                .text_color(p.accent)
                .child(Icon::new(IconName::CircleAlert).size(px(15.)))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.text)
                        .child("Outside EA's terms of service"),
                ),
        )
        .child(div().text_size(px(13.)).text_color(p.text).child(
            "EA's terms don't cover playing without EA AntiCheat, even offline. We know of \
                 no action taken against players for offline modding, but EA can still act on \
                 accounts under its terms.",
        ))
}

fn offline_terms(p: Palette) -> impl IntoElement {
    v_flex()
        .gap(px(14.))
        .child(div().text_size(px(13.)).text_color(p.text_muted).child(
            "Modded sessions run F1 25 without EA AntiCheat so the loader \
                 and addons can work. Everything is undone when the game exits.",
        ))
        .child(terms_box(p))
        .child(
            v_flex()
                .gap(px(7.))
                .child(section_label("OFFLINE ONLY", p))
                .child(div().text_size(px(13.)).text_color(p.text).child(
                    "Multiplayer is unavailable and cannot be enabled. The component is \
                         downloaded automatically and verified against pinned SHA-256 digests.",
                )),
        )
}

// Times shown as "5 min ago" stay current without a repaint every probe.
const REPAINT: Duration = Duration::from_secs(30);

impl Shell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::open_default().with_builtins(swofty_builtins::all());
        let migrated = store.migrate();
        match store.seed_bundled() {
            Ok(notes) => notes.iter().for_each(|n| eprintln!("swofty-launcher: {n}")),
            Err(e) => eprintln!("swofty-launcher: could not install the bundled addons: {e}"),
        }
        let recovered_seen = Notice::read(&store.root).map_or(0.0, |n| n.at);
        let stale = store.clone();
        cx.background_executor()
            .spawn(async move {
                match game::recover_stale(&stale) {
                    Ok(lines) => lines.iter().for_each(|l| eprintln!("swofty-launcher: {l}")),
                    Err(e) => eprintln!("swofty-launcher: could not check for an unfinished session: {e}"),
                }
            })
            .detach();
        let poll = cx.spawn_in(window, async move |this, cx| {
            loop {
                let Ok((store, rediscover, seen)) = this.update(cx, |s, _| {
                    let r = std::mem::take(&mut s.rediscover);
                    (s.store.clone(), r, s.requests)
                }) else {
                    break;
                };
                let probe = cx
                    .background_executor()
                    .spawn(async move { game::probe(&store, rediscover, seen) })
                    .await;
                if this
                    .update_in(cx, |s, window, cx| s.apply(probe, window, cx))
                    .is_err()
                {
                    break;
                }
                cx.background_executor().timer(Duration::from_millis(1000)).await;
            }
        });
        let rediscover = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(10)).await;
                if this.update(cx, |s, _| s.rediscover = true).is_err() {
                    break;
                }
            }
        });
        rediscover.detach();
        let live = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(250)).await;
                if this.update(cx, |s, cx| s.live_poll(cx)).is_err() {
                    break;
                }
            }
        });
        let updates = Self::watch_updates(cx);
        let shell = cx.weak_entity();
        let console_closed = cx.on_window_closed(move |cx, id| {
            shell.update(cx, |s, cx| s.console_closed(id, cx)).ok();
        });
        let mut library = Library::load(&store);
        match migrated {
            Ok(notes) => notes.iter().for_each(|n| eprintln!("swofty-launcher: {n}")),
            Err(e) => library.error = Some(e.to_string()),
        }
        let config = store.config();
        let (sidebar_list, vr_profiles) = (config.sidebar_list, config.vr_profiles);
        let start_steam = config.launch.start_steam;
        let mut shell = Self {
            library,
            store,
            page: Page::Addons,
            open_addon: None,
            install: None,
            detected: false,
            rediscover: true,
            processes: Vec::new(),
            session: None,
            crash: None,
            dismissed: None,
            launch: None,
            notice: None,
            blocked: None,
            recovered_seen,
            restoring: false,
            sliders: HashMap::new(),
            requests: None,
            console: None,
            views: HashMap::new(),
            views_profile: String::new(),
            view_gen: 0,
            view_gens: HashMap::new(),
            busy: HashSet::new(),
            open_row: None,
            progress: None,
            dropping: false,
            sidebar_list,
            vr_profiles,
            start_steam,
            updates: crate::update::Updates::new(),
            forms: HashMap::new(),
            searches: HashMap::new(),
            sorts: HashMap::new(),
            cell: None,
            pending_cells: HashMap::new(),
            content_width: 900.,
            ticking: false,
            painted: Instant::now(),
            live_n: 0,
            _console_closed: console_closed,
            _poll: poll,
            _live: live,
            _updates: updates,
        };
        shell.refresh_views(cx);
        shell
    }

    pub(crate) fn restore_console(&mut self, cx: &mut Context<Self>) {
        if self.store.config().console {
            self.console = crate::console::open(self.store.clone(), cx.weak_entity(), cx);
            cx.notify();
        }
    }

    pub(crate) fn set_console(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        match self.console.take() {
            Some(handle) if !on => cx.defer(move |cx| {
                handle.update(cx, |_, window, _| window.remove_window()).ok();
            }),
            Some(handle) => self.console = Some(handle),
            None if on => {
                self.console = crate::console::open(self.store.clone(), cx.weak_entity(), cx);
            }
            None => {}
        }
        let mut config = self.store.config();
        config.console = self.console.is_some();
        if let Err(e) = self.store.save_config(&config) {
            window.push_notification(Notification::error(e.to_string()), cx);
        }
        cx.notify();
    }

    fn console_closed(&mut self, id: WindowId, cx: &mut Context<Self>) {
        if self.console.is_some_and(|h| h.window_id() == id) {
            self.console = None;
            let mut config = self.store.config();
            config.console = false;
            self.store.save_config(&config).ok();
            cx.notify();
        }
    }

    fn apply(&mut self, probe: Probe, window: &mut Window, cx: &mut Context<Self>) {
        let before = (self.requests, self.processes.clone(), self.session.clone());
        let was_live = self.launch.is_some() || !self.processes.is_empty();
        self.requests = probe.requests;
        if let Some(n) = probe.recovered.filter(|n| n.at > self.recovered_seen) {
            self.recovered_seen = n.at;
            let mut text: Vec<String> = n.lines.iter().take(4).cloned().collect();
            if n.lines.len() > text.len() {
                text.push(format!(
                    "and {} more; the launch log has them all",
                    n.lines.len() - text.len()
                ));
            }
            window.push_notification(Notification::info(text.join("\n")).title(recovery::HEADLINE), cx);
        }
        if let Some(install) = probe.install {
            let changed = !self.detected || self.install != install;
            self.install = install;
            self.detected = true;
            if changed || probe.merged {
                self.reload(cx);
            }
        } else if probe.merged {
            self.reload(cx);
        }
        self.processes = probe.processes;
        if let Some(launch) = self.launch.as_mut() {
            launch.saw_game |= !self.processes.is_empty();
            launch.stage = probe.launch_stage;
            if let Some(ok) = launch.finished() {
                if !ok && !launch.saw_game {
                    self.blocked = Blocked::read(&self.store.root);
                    self.notice = Some(match &self.blocked {
                        Some(b) if b.files.len() == 1 => {
                            "a file Swofty did not put in the game folder is in the way".to_owned()
                        }
                        Some(b) => format!(
                            "{} files Swofty did not put in the game folder are in the way",
                            b.files.len()
                        ),
                        None => launch.failure(),
                    });
                }
                self.launch = None;
            }
        }
        if let Some(s) = probe.session.as_ref().filter(|s| s.crashed())
            && self.crash.as_ref().is_none_or(|(d, _)| *d != s.dir)
        {
            let session = s.clone();
            let game_dir = self.install.as_ref().map(|i| i.dir.clone());
            self.crash = Some((s.dir.clone(), Ok(String::new())));
            cx.spawn(async move |this, cx| {
                let dir = session.dir.clone();
                let summary = cx
                    .background_executor()
                    .spawn(async move { game::crash_summary(&session, game_dir.as_deref()) })
                    .await;
                this.update(cx, |s, cx| {
                    s.crash = Some((dir, summary));
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        self.session = probe.session;
        let live = was_live || self.launch.is_some() || !self.processes.is_empty();
        if live
            || before != (self.requests, self.processes.clone(), self.session.clone())
            || self.painted.elapsed() >= REPAINT
        {
            cx.notify();
        }
    }

    pub(crate) fn reload(&mut self, cx: &mut Context<Self>) {
        self.library = Library::load(&self.store);
        let config = self.store.config();
        self.vr_profiles = config.vr_profiles;
        self.start_steam = config.launch.start_steam;
        if matches!(self.page, Page::Section(i) if i >= self.library.sections.len()) {
            self.page = Page::Addons;
        }
        self.refresh_views(cx);
        cx.notify();
    }

    pub(crate) fn play_state(&self) -> PlayState {
        if !self.processes.is_empty() {
            return PlayState::Running;
        }
        if let Some(launch) = &self.launch {
            return if launch.saw_game {
                PlayState::Restoring
            } else {
                PlayState::Launching
            };
        }
        if self.restoring {
            return PlayState::Restoring;
        }
        if !self.detected {
            return PlayState::Detecting;
        }
        if self.install.is_none() {
            return PlayState::NotInstalled;
        }
        match &self.session {
            Some(s)
                if s.crashed()
                    && self.dismissed.as_ref() != Some(&s.dir)
                    && s.ended.is_some_and(|e| game::now_secs() - e < 1800.0) =>
            {
                PlayState::Crashed(s.clone())
            }
            _ => PlayState::Ready,
        }
    }

    pub(crate) fn play(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(install) = self.install.clone() else {
            return;
        };
        if !swofty_core::offline::is_accepted(&self.store.root) {
            self.confirm_offline_play(window, cx);
            return;
        }
        self.notice = None;
        self.blocked = None;
        Blocked::clear(&self.store.root).ok();
        self.dismissed = self.session.as_ref().map(|s| s.dir.clone());
        match Launch::start(
            &self.store,
            &self.library.profile.name,
            &install,
            self.launch_mode(),
        ) {
            Ok(l) => self.launch = Some(l),
            Err(e) => window.push_notification(Notification::error(e).title("Could not launch F1 25"), cx),
        }
        cx.notify();
    }

    pub(crate) fn launch_mode(&self) -> LaunchMode {
        if self.vr_profiles.contains(&self.library.profile.name) {
            LaunchMode::Vr
        } else {
            LaunchMode::Normal
        }
    }

    pub(crate) fn set_start_steam(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        let mut config = self.store.config();
        config.launch.start_steam = on;
        if let Err(e) = self.store.save_config(&config) {
            window.push_notification(Notification::error(e.to_string()), cx);
        }
        self.start_steam = config.launch.start_steam;
        cx.notify();
    }

    pub(crate) fn set_launch_mode(&mut self, mode: LaunchMode, window: &mut Window, cx: &mut Context<Self>) {
        let mut config = self.store.config();
        config.set_launch_mode(&self.library.profile.name, mode);
        if let Err(e) = self.store.save_config(&config) {
            window.push_notification(Notification::error(e.to_string()), cx);
        }
        self.vr_profiles = config.vr_profiles;
        cx.notify();
    }

    /// First launch: the offline play terms, shown as a dialog on Play.
    /// Accepting records consent and launches; declining changes nothing.
    pub(crate) fn confirm_offline_play(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shell = cx.entity().downgrade();
        let store = self.store.clone();
        let p = palette(cx);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (shell, store) = (shell.clone(), store.clone());
            alert
                .width(px(560.))
                .title("Modded play runs offline")
                .child(offline_terms(p))
                .ok_text("Accept and launch")
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    if let Err(e) = swofty_core::offline::record_accept(&store.root) {
                        window.push_notification(Notification::error(e.to_string()), cx);
                        return false;
                    }
                    shell.update(cx, |s, cx| s.play(window, cx)).ok();
                    true
                })
        });
    }

    pub(crate) fn confirm_restore(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(blocked) = self.blocked.clone() else {
            return;
        };
        let shell = cx.entity().downgrade();
        let explain = blocked.explain(&self.store.root);
        let files: Vec<String> = blocked.files.iter().map(|f| f.path.clone()).collect();
        let p = palette(cx);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let shell = shell.clone();
            let shown = files.len().min(6);
            let list = v_flex()
                .gap(px(2.))
                .p(px(10.))
                .rounded(px(6.))
                .bg(p.elevated)
                .border_1()
                .border_color(p.border)
                .text_size(px(12.))
                .children(files[..shown].iter().map(|f| div().child(f.clone())))
                .when(files.len() > shown, |d| {
                    d.child(
                        div()
                            .text_color(p.text_muted)
                            .child(format!("and {} more", files.len() - shown)),
                    )
                });
            alert
                .width(px(600.))
                .title("Restore the game folder?")
                .child(
                    v_flex()
                        .gap(px(12.))
                        .child(div().text_size(px(13.)).child(
                            "F1 25 did not start because these files are where the profile puts its \
                             own, and Swofty did not put them there. They may be mods copied in by \
                             hand, so they were left untouched.",
                        ))
                        .child(list)
                        .child(div().text_size(px(13.)).child(format!("{explain}."))),
                )
                .ok_text("Restore game folder")
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    shell.update(cx, |s, cx| s.restore_game_folder(window, cx)).ok();
                    true
                })
        });
    }

    pub(crate) fn restore_game_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(install) = self.install.clone() else {
            return;
        };
        let store = self.store.clone();
        self.notice = None;
        self.blocked = None;
        self.restoring = true;
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { game::restore(&store, install.dir) })
                .await;
            this.update_in(cx, |s, window, cx| {
                s.restoring = false;
                match result {
                    Ok(text) => window
                        .push_notification(Notification::success(text).title("Game folder restored"), cx),
                    Err(e) => window.push_notification(Notification::error(e).title("Restore failed"), cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn confirm_stop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.open_alert_dialog(cx, |alert, _, _| {
            alert
                .title("Stop F1 25?")
                .description("The game closes immediately. Unsaved progress is lost.")
                .ok_text("Stop game")
                .show_cancel(true)
                .on_ok(|_, window, cx| {
                    if let Err(e) = stop_game() {
                        window.push_notification(Notification::error(e.to_string()), cx);
                    }
                    true
                })
        });
    }

    pub(crate) fn set_addon_enabled(
        &mut self,
        id: &str,
        on: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let registry = self.library.registry.clone();
        let warning = (!on)
            .then(|| registry.turn_off_warning(&self.library.profile, id))
            .flatten();
        let Some(warning) = warning else {
            self.update_profile(window, cx, |p| registry.set_enabled(p, id, on));
            return;
        };
        let shell = cx.entity().downgrade();
        let p = palette(cx);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (shell, id) = (shell.clone(), warning.id.clone());
            saves_alert(alert, &warning, None, p)
                .ok_text("Turn off anyway")
                .on_ok(move |_, window, cx| {
                    shell
                        .update(cx, |s, cx| {
                            let registry = s.library.registry.clone();
                            s.update_profile(window, cx, |p| registry.set_enabled(p, &id, false));
                        })
                        .ok();
                    true
                })
        });
        cx.notify();
    }

    pub(crate) fn remove_addon(&mut self, id: &str, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        const REMOVES: &str = "Its files are deleted and it is taken out of every profile. \
                               You can install it again later.";
        let shell = cx.entity().downgrade();
        let store = self.store.clone();
        let id = id.to_owned();
        let name = name.to_owned();
        let warning = self
            .library
            .addons
            .iter()
            .find(|a| a.manifest.id == id)
            .and_then(|a| SavesWarning::of(&a.manifest));
        let p = palette(cx);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (shell, store, id, name) = (shell.clone(), store.clone(), id.clone(), name.clone());
            let alert = match &warning {
                Some(w) => saves_alert(alert, w, Some(REMOVES), p).ok_text("Remove anyway"),
                None => alert
                    .title(format!("Remove {name}?"))
                    .description(REMOVES)
                    .ok_text("Remove addon"),
            };
            alert.show_cancel(true).on_ok(move |_, window, cx| {
                match store.remove_addon(&id) {
                    Ok(()) => {
                        window.push_notification(Notification::success(format!("Removed {name}")), cx);
                        shell
                            .update(cx, |s, cx| {
                                s.open_addon = None;
                                s.reload(cx);
                            })
                            .ok();
                    }
                    Err(e) => {
                        window.push_notification(Notification::error(e.to_string()), cx);
                    }
                }
                true
            })
        });
    }

    pub(crate) fn update_profile(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut swofty_core::profile::Profile) -> swofty_core::Result<()>,
    ) {
        if let Err(e) = self.store.update_profile(&self.library.profile.name, f) {
            window.push_notification(Notification::error(e.to_string()), cx);
        }
        self.reload(cx);
    }

    pub(crate) fn switch_profile(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(e) = self.store.set_active_profile(name) {
            window.push_notification(Notification::error(e.to_string()), cx);
        }
        self.reload(cx);
    }

    pub(crate) fn locate_game(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.install.as_ref().map(|i| i.dir.display().to_string());
        self.prompt(
            Prompt {
                title: "Locate F1 25",
                description: format!("The folder that contains {}.", swofty_core::GAME_EXE),
                initial: current.unwrap_or_default(),
                placeholder: "/path/to/steamapps/common/F1 25",
                ok: "Use this folder",
                browse: Some(PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some("Use folder".into()),
                }),
                action: Rc::new(|store, value| {
                    let dir = PathBuf::from(value);
                    if !dir.join(swofty_core::GAME_EXE).is_file() {
                        return Err(format!("{} is not in {value}", swofty_core::GAME_EXE));
                    }
                    let mut config = store.config();
                    config.game_dir = Some(dir);
                    store.save_config(&config).map_err(|e| e.to_string())?;
                    Ok("Game folder saved".into())
                }),
            },
            window,
            cx,
        );
    }

    pub(crate) fn prompt(&mut self, prompt: Prompt, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(prompt.placeholder)
                .default_value(prompt.initial.clone())
        });
        let shell = cx.entity().downgrade();
        let store = self.store.clone();
        let focus = std::cell::Cell::new(true);
        window.open_alert_dialog(cx, move |alert, window, cx| {
            if focus.replace(false) {
                input.update(cx, |s, cx| s.focus(window, cx));
            }
            let field = input.clone();
            let browse = prompt.browse.clone().map(|options| {
                Button::new("browse")
                    .icon(IconName::FolderOpen)
                    .label("Browse…")
                    .on_click(move |_, window, cx| {
                        let rx = cx.prompt_for_paths(options.clone());
                        let field = field.clone();
                        window
                            .spawn(cx, async move |cx| {
                                if let Ok(Ok(Some(paths))) = rx.await
                                    && let Some(path) = paths.first()
                                {
                                    let path = path.display().to_string();
                                    cx.update(|window, cx| {
                                        field.update(cx, |s, cx| s.set_value(path, window, cx))
                                    })
                                    .ok();
                                }
                            })
                            .detach();
                    })
            });
            let (input, shell, store, action) =
                (input.clone(), shell.clone(), store.clone(), prompt.action.clone());
            alert
                .width(px(560.))
                .title(prompt.title)
                .description(prompt.description.clone())
                .child(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .child(div().flex_1().child(Input::new(&input)))
                        .children(browse),
                )
                .ok_text(prompt.ok)
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    let value = input.read(cx).value().trim().to_owned();
                    match action(&store, &value) {
                        Ok(message) => {
                            shell
                                .update(cx, |s, cx| {
                                    s.rediscover = true;
                                    s.reload(cx)
                                })
                                .ok();
                            window.push_notification(Notification::success(message), cx);
                            true
                        }
                        Err(e) => {
                            window.push_notification(Notification::error(e), cx);
                            false
                        }
                    }
                })
        });
    }

    fn title_bar(&self, p: Palette) -> impl IntoElement {
        TitleBar::new().bg(p.chrome).child(
            h_flex().flex_1().justify_end().pr(px(8.)).child(
                div()
                    .text_size(px(12.))
                    .text_color(p.text_placeholder)
                    .child(concat!("v", env!("CARGO_PKG_VERSION"))),
            ),
        )
    }

    fn nav_item(
        &self,
        page: Page,
        title: SharedString,
        art: TileArt,
        count: Option<usize>,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let active = self.page == page;
        if self.sidebar_list {
            return h_flex()
                .id(SharedString::from(format!("nav-{title}")))
                .h(px(46.))
                .px(px(8.))
                .gap(px(10.))
                .rounded(px(8.))
                .border_1()
                .cursor_pointer()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .map(|d| {
                    if active {
                        d.bg(p.selected).border_color(p.accent).text_color(p.text)
                    } else {
                        d.bg(p.surface)
                            .border_color(p.border_variant)
                            .text_color(p.text_muted)
                            .hover(move |s| s.bg(p.hover).border_color(p.border).text_color(p.text))
                    }
                })
                .child(tile_art(art, 30., active, p))
                .child(div().flex_1().truncate().child(title))
                .children(count.map(|n| {
                    div()
                        .text_size(px(12.))
                        .text_color(p.text_placeholder)
                        .child(n.to_string())
                }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.page = page;
                    cx.notify();
                }))
                .into_any_element();
        }
        v_flex()
            .id(SharedString::from(format!("nav-{title}")))
            .relative()
            .w(px(95.))
            .h(px(92.))
            .items_center()
            .justify_center()
            .gap(px(8.))
            .rounded(px(10.))
            .border_1()
            .cursor_pointer()
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .map(|d| {
                if active {
                    d.bg(p.selected).border_color(p.accent).text_color(p.text)
                } else {
                    d.bg(p.surface)
                        .border_color(p.border_variant)
                        .text_color(p.text_muted)
                        .hover(move |s| s.bg(p.hover).border_color(p.border).text_color(p.text))
                }
            })
            .child(tile_art(art, 40., active, p))
            .child(div().max_w_full().px(px(6.)).truncate().child(title))
            .children(count.map(|n| {
                div()
                    .absolute()
                    .top(px(6.))
                    .right(px(8.))
                    .text_size(px(11.))
                    .text_color(p.text_placeholder)
                    .child(n.to_string())
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.page = page;
                cx.notify();
            }))
            .into_any_element()
    }

    fn core_nav(&self, page: Page, count: usize, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        self.nav_item(
            page,
            page.label().into(),
            TileArt::Icon(page.icon()),
            Some(count),
            p,
            cx,
        )
    }

    fn sidebar(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let list = self.sidebar_list;
        let wrap = move |items: Vec<AnyElement>| -> AnyElement {
            if list {
                v_flex().gap(px(6.)).children(items).into_any_element()
            } else {
                h_flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .children(items)
                    .into_any_element()
            }
        };
        let mut groups: Vec<(String, Vec<AnyElement>)> = Vec::new();
        for (i, s) in self.library.sections.iter().enumerate() {
            let addon = self.library.addons.iter().find(|a| a.manifest.id == s.addon);
            if i == 0 || self.library.sections[i - 1].addon != s.addon {
                let name = addon.map_or_else(|| s.addon.clone(), |a| a.manifest.name.clone());
                groups.push((name, Vec::new()));
            }
            let tile = self.nav_item(
                Page::Section(i),
                s.spec.title.clone().into(),
                match addon.and_then(|a| a.icon.clone()) {
                    Some(AddonIcon::Glyph(data)) => TileArt::Glyph(data),
                    Some(AddonIcon::Image(image)) => TileArt::Image(image),
                    None => TileArt::Icon(crate::sections::icon(s.spec.icon)),
                },
                self.section_count(i),
                p,
                cx,
            );
            if let Some((_, items)) = groups.last_mut() {
                items.push(tile);
            }
        }
        v_flex()
            .w(px(220.))
            .h_full()
            .flex_shrink_0()
            .px(px(10.))
            .pb(px(10.))
            .gap(px(8.))
            .bg(p.chrome)
            .pt(px(4.))
            .child(
                h_flex()
                    .justify_center()
                    .pt(px(2.))
                    .pb(px(16.))
                    .child(crate::pages::wordmark(148.)),
            )
            .child(wrap(vec![
                self.core_nav(Page::Profiles, self.library.profiles.len(), p, cx),
                self.core_nav(Page::Addons, self.library.addons.len(), p, cx),
            ]))
            .children(groups.into_iter().map(|(name, items)| {
                v_flex()
                    .gap(px(6.))
                    .pt(px(6.))
                    .child(label(name.to_uppercase(), p).px(px(4.)))
                    .child(wrap(items))
            }))
            .child(div().flex_1())
            .child(self.discord_button(p))
            .child(self.send_logs_button(p, cx))
            .child(
                v_flex()
                    .gap(px(6.))
                    .pt(px(4.))
                    .pb(px(2.))
                    .child(label("ACTIVE PROFILE", p).px(px(4.)))
                    .child(self.profile_picker(p, cx)),
            )
    }
}

enum TileArt {
    Icon(IconName),
    Glyph(Arc<[u8]>),
    Image(Arc<Image>),
}

fn tile_art(art: TileArt, size: f32, active: bool, p: Palette) -> Div {
    let color = if active { p.on_accent } else { p.text_muted };
    let frame = || {
        div()
            .size(px(size))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(size * 0.25))
            .bg(if active { p.accent } else { p.hover })
    };
    match art {
        TileArt::Image(image) => addon_icon(&AddonIcon::Image(image), size, p),
        TileArt::Glyph(data) => frame().child(svg().data(&data).size(px(size * 0.5)).text_color(color)),
        TileArt::Icon(icon) => frame().child(Icon::new(icon).size(px(size * 0.5)).text_color(color)),
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.painted = Instant::now();
        let p = palette(cx);
        let wide = window.viewport_size().width > px(1500.);
        self.content_width =
            (f32::from(window.viewport_size().width) - 220. - 2. * crate::pages::GUTTER).max(320.);
        self.sync_sliders(window, cx);
        self.sync_blocks(window, cx);
        let content = match self.page {
            Page::Addons => self.addons_page(p, cx),
            Page::Profiles => self.profiles_page(p, wide, cx),
            Page::Section(i) => self.section_page(i, p, cx),
        };
        if self.dropping && !cx.has_active_drag() {
            self.dropping = false;
        }
        v_flex()
            .id("shell")
            .relative()
            .size_full()
            .on_drag_move::<ExternalPaths>(cx.listener(|this, _, _, cx| {
                if !this.dropping {
                    this.dropping = true;
                    cx.notify();
                }
            }))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                this.drop_paths(paths.paths().to_vec(), window, cx)
            }))
            .bg(p.background)
            .text_color(p.text)
            .text_size(px(14.))
            .child(self.title_bar(p))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .bg(p.chrome)
                    .child(self.sidebar(p, cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .h_full()
                            .min_w_0()
                            .overflow_hidden()
                            .rounded_tl(px(16.))
                            .bg(p.background)
                            .child(content),
                    ),
            )
            .children(self.update_bar(p))
            .child(self.play_bar(p, wide, cx))
            .children(self.progress_card(p))
            .when(self.dropping, |d| d.child(self.drop_overlay(p)))
    }
}

fn saves_alert(alert: AlertDialog, w: &SavesWarning, then: Option<&'static str>, p: Palette) -> AlertDialog {
    let note = w.message.clone().map(|m| {
        h_flex()
            .gap(px(10.))
            .items_start()
            .p(px(12.))
            .rounded(px(6.))
            .bg(p.elevated)
            .border_1()
            .border_color(p.border)
            .child(
                Icon::new(IconName::CircleAlert)
                    .size(px(15.))
                    .text_color(p.warning),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.text)
                            .child(w.name.clone()),
                    )
                    .child(div().text_size(px(13.)).text_color(p.text_muted).child(m)),
            )
    });
    alert
        .width(px(480.))
        .title(saves::TITLE)
        .child(
            v_flex()
                .gap(px(12.))
                .child(div().text_size(px(13.)).text_color(p.text).child(saves::BODY))
                .children(note)
                .when_some(then, |d, t| {
                    d.child(div().text_size(px(13.)).text_color(p.text_muted).child(t))
                }),
        )
        .ok_variant(ButtonVariant::Danger)
        .show_cancel(true)
}
