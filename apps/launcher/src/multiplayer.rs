use std::path::Path;
use std::time::{Duration, Instant};

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::catalog::{self, Addon};
use swofty_core::install::Game;
use swofty_core::multiplayer::bridge::MemberStats;
use swofty_core::multiplayer::game::{self as mp_game, Category, Command, GameStatus, Screen};
use swofty_core::multiplayer::rooms::{Api, RoomSettings, RoomSummary};
use swofty_core::multiplayer::session::{
    FixStep, Local, MemberState, Phase, PrintMatch, RelayStatus, Session, SessionState, Setup,
};
use swofty_core::store::Store;

use crate::library::plural;
use crate::pages::{GUTTER, label, monogram, tag};
use crate::shell::{Page, Shell};
use crate::theme::{Palette, ScrollArea};

const TICK: Duration = Duration::from_millis(500);
const GAME_EVERY: Duration = Duration::from_secs(2);
const ROOMS_EVERY: Duration = Duration::from_secs(5);
const STATUS_TIMEOUT: Duration = Duration::from_secs(3);
const MIN_PLAYERS: u8 = 2;
const MAX_PLAYERS: u8 = 20;
const TRACKS_SHOWN: usize = 3;

/// Where the game is, as the page's status pill names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GameState {
    NotRunning,
    MainMenu,
    LanLobby,
    Busy,
}

impl GameState {
    pub(crate) fn of(running: bool, status: Option<&GameStatus>) -> Self {
        if !running {
            return Self::NotRunning;
        }
        match status.map(|s| s.screen) {
            Some(Screen::MainMenu) => Self::MainMenu,
            Some(Screen::LanLobby) => Self::LanLobby,
            Some(Screen::Other) | None => Self::Busy,
        }
    }

    pub(crate) fn text(self) -> &'static str {
        match self {
            Self::NotRunning => "Game not running",
            Self::MainMenu => "On the main menu",
            Self::LanLobby => "In a LAN lobby",
            Self::Busy => "Busy",
        }
    }

    fn color(self, p: Palette) -> Hsla {
        match self {
            Self::NotRunning => p.text_placeholder,
            Self::MainMenu => p.success,
            Self::LanLobby => p.accent,
            Self::Busy => p.warning,
        }
    }

    /// Why Create and Join wait, or `None` on the main menu.
    pub(crate) fn not_ready(self) -> Option<&'static str> {
        match self {
            Self::MainMenu => None,
            Self::NotRunning => Some("Start F1 25 first"),
            Self::LanLobby => Some("Leave the LAN lobby first"),
            Self::Busy => Some("Go to F1 25's main menu"),
        }
    }
}

pub(crate) fn join_blocked(room: &RoomSummary, game: GameState) -> Option<&'static str> {
    if room.members >= room.max_members {
        return Some("Room full");
    }
    game.not_ready()
}

pub(crate) fn category_label(category: &str) -> String {
    match category.parse::<Category>() {
        Ok(Category::F12026) => "F1 2026".into(),
        Ok(Category::F12025) => "F1 2025".into(),
        Err(_) => category.to_owned(),
    }
}

pub(crate) fn tracks_text(tracks: &[String]) -> Option<String> {
    if tracks.is_empty() {
        return None;
    }
    let mut text = tracks
        .iter()
        .take(TRACKS_SHOWN)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if tracks.len() > TRACKS_SHOWN {
        text.push_str(&format!(" and {} more", tracks.len() - TRACKS_SHOWN));
    }
    Some(format!("Tracks: {text}"))
}

/// The room row's second line: host, car category, the host's tracks.
pub(crate) fn room_meta(room: &RoomSummary) -> String {
    let mut parts = vec![
        format!("Hosted by {}", room.host.name),
        category_label(&room.category),
    ];
    if let Some(tracks) = room.addons.as_ref().and_then(|a| tracks_text(&a.tracks)) {
        parts.push(tracks);
    }
    parts.join(" \u{00b7} ")
}

pub(crate) fn players_text(room: &RoomSummary) -> String {
    format!("{}/{}", room.members, room.max_members)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Addons {
    Match,
    Differ,
    Unknown,
}

pub(crate) fn addons_against(room: &RoomSummary, mine: Option<&str>) -> Addons {
    match (room.digest.as_deref(), mine) {
        (Some(host), Some(mine)) if host.eq_ignore_ascii_case(mine) => Addons::Match,
        (Some(_), Some(_)) => Addons::Differ,
        _ => Addons::Unknown,
    }
}

pub(crate) fn addons_text(addons: Addons) -> &'static str {
    match addons {
        Addons::Match => "Your addons match",
        Addons::Differ => "Addons differ",
        Addons::Unknown => "Checking addons",
    }
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

pub(crate) fn phase_text(phase: &Phase) -> String {
    match phase {
        Phase::Connecting(step) => format!("Connecting: {step}"),
        Phase::WaitingForGame(why) => capitalised(why),
        Phase::InLobby => "In the lobby".into(),
        Phase::Racing => "Racing".into(),
        Phase::Ended(why) => capitalised(why),
        Phase::Failed(why) => format!("The room stopped: {why}"),
    }
}

/// What the host does next, under the phase.
pub(crate) fn phase_hint(phase: &Phase, host: bool) -> Option<&'static str> {
    match phase {
        Phase::InLobby if host => {
            Some("Pick tracks and settings in F1 25's LAN lobby, then start the race there.")
        }
        Phase::InLobby => Some("The host picks tracks and settings and starts the race."),
        Phase::Racing if host => Some("Mark the race over when everyone is back in the lobby."),
        _ => None,
    }
}

/// Retry drives the game again: offered while the session waits on the game for anything but
/// the addons, or after an error.
pub(crate) fn retry_offered(state: &SessionState) -> bool {
    match &state.phase {
        Phase::WaitingForGame(_) => state.mismatches.is_empty(),
        Phase::Connecting(_) | Phase::InLobby | Phase::Racing => state.last_error.is_some(),
        Phase::Ended(_) | Phase::Failed(_) => false,
    }
}

pub(crate) fn addon_word(m: PrintMatch) -> &'static str {
    match m {
        PrintMatch::Unknown => "Checking",
        PrintMatch::Matches => "Match",
        PrintMatch::Differs => "Differ",
    }
}

pub(crate) fn relay_word(r: RelayStatus) -> &'static str {
    match r {
        RelayStatus::None => "None",
        RelayStatus::Pending => "Pending",
        RelayStatus::Ready => "Ready",
    }
}

fn size_text(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / f64::from(1 << 20)),
        b if b >= 1 << 10 => format!("{:.0} KB", b as f64 / f64::from(1 << 10)),
        b => format!("{b} B"),
    }
}

pub(crate) fn traffic_text(t: Option<&MemberStats>) -> String {
    match t {
        Some(t) if t.packets_in + t.packets_out > 0 => format!(
            "\u{2191} {}  \u{2193} {}",
            size_text(t.bytes_out),
            size_text(t.bytes_in)
        ),
        _ => "\u{2014}".into(),
    }
}

/// The fix steps the launcher can take on its own, and the lines left to the player.
pub(crate) fn fixable(steps: &[FixStep]) -> (usize, Vec<String>) {
    let manual = steps
        .iter()
        .filter_map(|s| match s {
            FixStep::Manual { line } => Some(capitalised(line)),
            _ => None,
        })
        .collect::<Vec<_>>();
    (steps.len() - manual.len(), manual)
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Fixed {
    pub(crate) code: String,
    pub(crate) done: Vec<String>,
    pub(crate) failed: Vec<String>,
}

fn install_exact(
    store: &Store,
    profile: &str,
    catalog: &[Addon],
    id: &str,
    version: &semver::Version,
    sha256: Option<&str>,
) -> Result<(), String> {
    let addon = catalog
        .iter()
        .find(|a| a.id == id)
        .ok_or_else(|| format!("{id} is not in the Addon Browser"))?;
    let v = addon
        .versions
        .iter()
        .chain(addon.latest.iter())
        .find(|v| v.version == *version && sha256.is_none_or(|s| v.sha256.trim().eq_ignore_ascii_case(s)))
        .ok_or_else(|| format!("{} {version} is not in the Addon Browser", addon.name))?;
    catalog::requires::install(store, catalog, addon, v, &|_| {}).map_err(|e| e.to_string())?;
    let registry = store.registry().map_err(|e| e.to_string())?;
    store
        .update_profile(profile, |p| registry.set_enabled(p, id, true))
        .map_err(|e| e.to_string())
}

fn reorder(order: &[String], addons: &mut [swofty_core::profile::ProfileAddon]) {
    let slots: Vec<usize> = (0..addons.len())
        .filter(|&i| order.contains(&addons[i].id))
        .collect();
    let mut placed: Vec<_> = slots.iter().map(|&i| addons[i].clone()).collect();
    placed.sort_by_key(|a| order.iter().position(|id| *id == a.id));
    for (slot, addon) in slots.into_iter().zip(placed) {
        addons[slot] = addon;
    }
}

/// Makes `profile` match the host as far as the launcher can: exact registry versions, addons
/// turned off, options and load order. The game picks the changes up when it starts again.
fn apply_fix(
    store: &Store,
    profile: &str,
    catalog: &[Addon],
    steps: &[FixStep],
) -> (Vec<String>, Vec<String>) {
    let (mut done, mut failed) = (Vec::new(), Vec::new());
    for step in steps {
        let result = match step {
            FixStep::Install {
                id,
                name,
                version,
                sha256,
            } => install_exact(store, profile, catalog, id, version, sha256.as_deref())
                .map(|_| format!("Installed {name} {version}")),
            FixStep::Disable { id, name } => store
                .registry()
                .and_then(|r| store.update_profile(profile, |p| r.set_enabled(p, id, false)))
                .map(|_| format!("Turned off {name}"))
                .map_err(|e| e.to_string()),
            FixStep::SetOption { addon, option, value } => store
                .addons()
                .and_then(|all| {
                    let m = all
                        .get(addon)
                        .ok_or_else(|| swofty_core::Error::Other(format!("{addon} is not installed")))?;
                    let value = match value {
                        Some(v) => v.clone(),
                        None => m
                            .options
                            .iter()
                            .find(|o| o.id == *option)
                            .map(|o| o.default_value())
                            .unwrap_or_default(),
                    };
                    store.update_profile(profile, |p| p.set_option(m, option, value).map(drop))
                })
                .map(|_| format!("Set {addon}.{option} to the host's value"))
                .map_err(|e| e.to_string()),
            FixStep::Reorder { order } => store
                .update_profile(profile, |p| {
                    reorder(order, &mut p.addons);
                    Ok(())
                })
                .map(|_| "Put the addons in the host's load order".to_owned())
                .map_err(|e| e.to_string()),
            FixStep::Manual { .. } => continue,
        };
        match result {
            Ok(line) => done.push(line),
            Err(e) => failed.push(e),
        }
    }
    (done, failed)
}

fn read_local(store: &Store, profile: &str, dir: &Path) -> Result<Local, String> {
    let game = Game::locate(&store.root, dir.to_path_buf()).map_err(|e| e.to_string())?;
    let profile = store.profile(profile).map_err(|e| e.to_string())?;
    Local::read(store, &profile, &game.identity).map_err(|e| e.to_string())
}

fn read_game(dir: &Path, running: bool) -> Option<GameStatus> {
    mp_game::last_status(dir).or_else(|| {
        running
            .then(|| mp_game::send(dir, &Command::Status, STATUS_TIMEOUT).ok())
            .flatten()
            .map(|r| r.status)
    })
}

enum Start {
    Create(RoomSettings),
    Join(String),
}

pub(crate) struct Multiplayer {
    visible: bool,
    game: Option<GameStatus>,
    game_at: Option<Instant>,
    game_polling: bool,
    rooms: Vec<RoomSummary>,
    rooms_loaded: bool,
    rooms_error: Option<String>,
    rooms_at: Option<Instant>,
    rooms_polling: bool,
    digest: Option<String>,
    digest_busy: bool,
    form: bool,
    name: Entity<InputState>,
    code: Entity<InputState>,
    public: bool,
    max: u8,
    category: Category,
    starting: Option<String>,
    session: Option<Session>,
    state: Option<SessionState>,
    leaving: bool,
    fixing: bool,
    fixed: Option<Fixed>,
    error: Option<String>,
    copied: Option<Instant>,
    _inputs: [Subscription; 2],
}

impl Multiplayer {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Shell>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Room name"));
        let code = cx.new(|cx| InputState::new(window, cx).placeholder("Invite code"));
        let changed = |cx: &mut Context<Shell>, input: &Entity<InputState>| {
            cx.subscribe(input, |_, _, e: &InputEvent, cx| {
                if matches!(e, InputEvent::Change) {
                    cx.notify();
                }
            })
        };
        let inputs = [changed(cx, &name), changed(cx, &code)];
        Self {
            visible: false,
            game: None,
            game_at: None,
            game_polling: false,
            rooms: Vec::new(),
            rooms_loaded: false,
            rooms_error: None,
            rooms_at: None,
            rooms_polling: false,
            digest: None,
            digest_busy: false,
            form: false,
            name,
            code,
            public: true,
            max: 8,
            category: Category::F12026,
            starting: None,
            session: None,
            state: None,
            leaving: false,
            fixing: false,
            fixed: None,
            error: None,
            copied: None,
            _inputs: inputs,
        }
    }

    fn busy(&self) -> bool {
        self.starting.is_some() || self.leaving || self.fixing || self.session.is_some()
    }
}

fn due(at: Option<Instant>, every: Duration, now: Instant) -> bool {
    at.is_none_or(|at| now.duration_since(at) >= every)
}

impl Shell {
    pub(crate) fn mp_allowed(&self) -> bool {
        self.mp.as_ref().is_some_and(|m| m.allowed)
    }

    pub(crate) fn watch_multiplayer(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update(cx, |s, cx| s.mp_tick(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn mp_tick(&mut self, cx: &mut Context<Self>) {
        if self.page != Page::Multiplayer || !self.mp_allowed() {
            self.mp_page.visible = false;
            return;
        }
        let now = Instant::now();
        let m = &mut self.mp_page;
        if !m.visible {
            m.visible = true;
            m.rooms_at = None;
            m.game_at = None;
            m.digest = None;
        }
        if let Some(session) = &m.session {
            let state = session.state();
            if m.state.as_ref() != Some(&state) {
                m.state = Some(state);
                cx.notify();
            }
        }
        if m.copied
            .is_some_and(|at| now.duration_since(at) > Duration::from_secs(2))
        {
            m.copied = None;
            cx.notify();
        }
        if m.session.is_none() && !m.game_polling && due(m.game_at, GAME_EVERY, now) {
            self.mp_poll_game(cx);
        }
        let m = &self.mp_page;
        if m.session.is_none() && !m.rooms_polling && due(m.rooms_at, ROOMS_EVERY, now) {
            self.mp_poll_rooms(cx);
        }
        let m = &self.mp_page;
        if m.digest.is_none() && !m.digest_busy && m.session.is_none() {
            self.mp_read_digest(cx);
        }
    }

    fn mp_poll_game(&mut self, cx: &mut Context<Self>) {
        let Some(dir) = self.install.as_ref().map(|i| i.dir.clone()) else {
            self.mp_page.game = None;
            self.mp_page.game_at = Some(Instant::now());
            return;
        };
        let running = !self.processes.is_empty();
        self.mp_page.game_polling = true;
        cx.spawn(async move |this, cx| {
            let status = cx
                .background_executor()
                .spawn(async move { read_game(&dir, running) })
                .await;
            this.update(cx, |s, cx| {
                let m = &mut s.mp_page;
                m.game_polling = false;
                m.game_at = Some(Instant::now());
                if m.game != status {
                    m.game = status;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn mp_poll_rooms(&mut self, cx: &mut Context<Self>) {
        let Some(token) = self.mp.as_ref().map(|m| m.token.clone()) else {
            return;
        };
        self.mp_page.rooms_polling = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { Api::new(&token).rooms() })
                .await;
            this.update(cx, |s, cx| {
                let m = &mut s.mp_page;
                m.rooms_polling = false;
                m.rooms_at = Some(Instant::now());
                match result {
                    Ok(rooms) => {
                        m.rooms = rooms;
                        m.rooms_loaded = true;
                        m.rooms_error = None;
                    }
                    Err(e) => m.rooms_error = Some(e.message),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn mp_read_digest(&mut self, cx: &mut Context<Self>) {
        let Some(dir) = self.install.as_ref().map(|i| i.dir.clone()) else {
            return;
        };
        let (store, profile) = (self.store.clone(), self.library.profile.name.clone());
        self.mp_page.digest_busy = true;
        cx.spawn(async move |this, cx| {
            let digest = cx
                .background_executor()
                .spawn(async move { read_local(&store, &profile, &dir).map(|l| l.digest()) })
                .await;
            this.update(cx, |s, cx| {
                s.mp_page.digest_busy = false;
                match digest {
                    Ok(d) => s.mp_page.digest = Some(d),
                    Err(e) => eprintln!("swofty-launcher: couldn't read this profile's addons: {e}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn mp_game_state(&self) -> GameState {
        let status = match &self.mp_page.state {
            Some(state) if self.mp_page.session.is_some() => state.game.as_ref(),
            _ => self.mp_page.game.as_ref(),
        };
        GameState::of(!self.processes.is_empty(), status)
    }

    fn mp_start(&mut self, start: Start, cx: &mut Context<Self>) {
        if self.mp_page.busy() {
            return;
        }
        let Some(token) = self.mp.as_ref().map(|m| m.token.clone()) else {
            return;
        };
        let Some(dir) = self.install.as_ref().map(|i| i.dir.clone()) else {
            self.mp_page.error = Some("F1 25 was not found. Locate it from the play bar first.".into());
            cx.notify();
            return;
        };
        let (store, profile) = (self.store.clone(), self.library.profile.name.clone());
        let m = &mut self.mp_page;
        m.starting = Some(match &start {
            Start::Create(_) => "create".into(),
            Start::Join(code) => code.clone(),
        });
        m.error = None;
        m.fixed = None;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let local = read_local(&store, &profile, &dir)?;
                    let setup = Setup::new(Api::new(&token), &dir, local);
                    Ok::<_, String>(match start {
                        Start::Create(settings) => Session::create(setup, settings),
                        Start::Join(code) => Session::join(setup, &code),
                    })
                })
                .await;
            this.update(cx, |s, cx| {
                let m = &mut s.mp_page;
                m.starting = None;
                match result {
                    Ok(session) => {
                        m.state = Some(session.state());
                        m.session = Some(session);
                        m.form = false;
                    }
                    Err(e) => m.error = Some(format!("Couldn't read your addons: {e}")),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn mp_create(&mut self, cx: &mut Context<Self>) {
        let typed = self.mp_page.name.read(cx).value().trim().to_owned();
        let name = if typed.is_empty() {
            self.mp_default_name()
        } else {
            typed
        };
        let settings = RoomSettings {
            name: Some(name),
            public: self.mp_page.public,
            max_members: self.mp_page.max,
            category: self.mp_page.category,
        };
        self.mp_start(Start::Create(settings), cx);
    }

    fn mp_default_name(&self) -> String {
        let you = self.mp.as_ref().map_or("Your", |m| m.name.as_str());
        format!("{you}'s room")
    }

    fn mp_open_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.mp_page.form = !self.mp_page.form;
        if self.mp_page.form && self.mp_page.name.read(cx).value().is_empty() {
            let name = self.mp_default_name();
            self.mp_page
                .name
                .update(cx, |s, cx| s.set_value(name, window, cx));
        }
        cx.notify();
    }

    fn mp_join_code(&mut self, cx: &mut Context<Self>) {
        let code = self.mp_page.code.read(cx).value().trim().to_owned();
        if !code.is_empty() {
            self.mp_start(Start::Join(code), cx);
        }
    }

    pub(crate) fn mp_leave(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.mp_page.session.take() else {
            return;
        };
        self.mp_page.leaving = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .spawn(async move { session.leave() })
                .await;
            this.update(cx, |s, cx| {
                let m = &mut s.mp_page;
                m.leaving = false;
                m.state = None;
                m.rooms_at = None;
                m.game_at = None;
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn mp_fix(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.mp_page.session.take() else {
            return;
        };
        let code = self
            .mp_page
            .state
            .as_ref()
            .and_then(|s| s.room.as_ref())
            .map(|r| r.code.clone())
            .unwrap_or_default();
        let steps = session.fix_plan();
        let (store, profile) = (self.store.clone(), self.library.profile.name.clone());
        let listing = self.browser.addons.clone();
        self.mp_page.fixing = true;
        self.mp_page.error = None;
        cx.spawn(async move |this, cx| {
            let (done, failed) = cx
                .background_executor()
                .spawn(async move {
                    session.leave();
                    let listing = if listing.is_empty() {
                        catalog::cached(&store.root).unwrap_or_default()
                    } else {
                        listing
                    };
                    apply_fix(&store, &profile, &listing, &steps)
                })
                .await;
            this.update(cx, |s, cx| {
                let m = &mut s.mp_page;
                m.fixing = false;
                m.state = None;
                m.digest = None;
                m.rooms_at = None;
                m.fixed = Some(Fixed { code, done, failed });
                s.reload(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn mp_copy_code(&mut self, code: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(code));
        self.mp_page.copied = Some(Instant::now());
        cx.notify();
    }

    pub(crate) fn multiplayer_page(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let in_room = self.mp_page.session.is_some() || self.mp_page.leaving || self.mp_page.fixing;
        match self.mp_page.state.clone() {
            Some(state) if in_room => self.mp_room(&state, p, cx),
            _ => self.mp_lobby(p, cx),
        }
    }

    fn game_pill(&self, p: Palette) -> Div {
        let game = self.mp_game_state();
        h_flex()
            .h(px(28.))
            .px(px(10.))
            .gap(px(7.))
            .flex_shrink_0()
            .rounded(px(14.))
            .border_1()
            .border_color(p.border)
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.text_muted)
            .child(div().size(px(7.)).rounded_full().bg(game.color(p)))
            .child(game.text())
    }

    fn mp_lobby(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let who = self
            .mp
            .as_ref()
            .map_or_else(String::new, |m| format!("Signed in as {}. ", m.name));
        let subtitle = format!("{who}Join a room, or create one and pick tracks in F1 25's LAN lobby.");
        let actions = h_flex()
            .gap(px(8.))
            .items_center()
            .child(self.game_pill(p))
            .child(
                Button::new("mp-create-open")
                    .outline()
                    .icon(if self.mp_page.form {
                        IconName::X
                    } else {
                        IconName::Plus
                    })
                    .label(if self.mp_page.form { "Close" } else { "Create room" })
                    .on_click(cx.listener(|this, _, window, cx| this.mp_open_form(window, cx))),
            );
        let body = if !self.mp_page.rooms_loaded && self.mp_page.rooms_error.is_none() {
            div()
                .px(px(GUTTER))
                .pt(px(8.))
                .text_size(px(13.))
                .text_color(p.text_muted)
                .child("Loading rooms\u{2026}")
                .into_any_element()
        } else if self.mp_page.rooms.is_empty() {
            let body = match &self.mp_page.rooms_error {
                Some(e) => format!("Couldn't load the room list: {e}"),
                None => "Create one, or join a private room with its code.".into(),
            };
            self.empty(IconName::Users, "No open rooms", body, div(), p)
        } else {
            let rows = self
                .mp_page
                .rooms
                .clone()
                .iter()
                .enumerate()
                .map(|(i, r)| self.room_row(r, i == 0, p, cx))
                .collect();
            self.list("mp-rooms", rows, p).into_any_element()
        };
        v_flex()
            .size_full()
            .child(self.header(Page::Multiplayer.label(), subtitle, actions, p))
            .children(self.mp_page.fixed.clone().map(|f| self.fixed_card(&f, p, cx)))
            .when(self.mp_page.form, |d| d.child(self.create_form(p, cx)))
            .child(self.rooms_bar(p, cx))
            .child(body)
            .into_any_element()
    }

    fn rooms_bar(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let game = self.mp_game_state();
        let typed = !self.mp_page.code.read(cx).value().trim().is_empty();
        let joining = self
            .mp_page
            .starting
            .as_ref()
            .filter(|s| s.as_str() != "create")
            .is_some();
        let count = if self.mp_page.rooms_loaded {
            format!("OPEN ROOMS \u{00b7} {}", self.mp_page.rooms.len())
        } else {
            "OPEN ROOMS".into()
        };
        v_flex()
            .px(px(GUTTER))
            .pb(px(8.))
            .gap(px(6.))
            .flex_shrink_0()
            .child(
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .child(label(count, p))
                    .when_some(
                        self.mp_page
                            .rooms_error
                            .clone()
                            .filter(|_| self.mp_page.rooms_loaded),
                        |d, e| {
                            d.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.error)
                                    .truncate()
                                    .child(format!("Couldn't refresh: {e}")),
                            )
                        },
                    )
                    .child(div().flex_1())
                    .children(
                        game.not_ready()
                            .filter(|_| typed)
                            .map(|why| div().text_size(px(12.)).text_color(p.text_placeholder).child(why)),
                    )
                    .child(
                        div().w(px(180.)).child(
                            Input::new(&self.mp_page.code)
                                .small()
                                .disabled(self.mp_page.busy()),
                        ),
                    )
                    .child(
                        Button::new("mp-join-code")
                            .outline()
                            .small()
                            .label("Join with code")
                            .loading(joining)
                            .disabled(!typed || game.not_ready().is_some() || self.mp_page.busy())
                            .on_click(cx.listener(|this, _, _, cx| this.mp_join_code(cx))),
                    ),
            )
            .when_some(self.mp_page.error.clone(), |d, e| {
                d.child(div().text_size(px(13.)).text_color(p.error).child(e))
            })
    }

    fn room_row(&self, room: &RoomSummary, first: bool, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let game = self.mp_game_state();
        let blocked = join_blocked(room, game);
        let addons = addons_against(room, self.mp_page.digest.as_deref());
        let (icon, color) = match addons {
            Addons::Match => (IconName::CircleCheck, p.success),
            Addons::Differ => (IconName::TriangleAlert, p.warning),
            Addons::Unknown => (IconName::CircleDot, p.text_placeholder),
        };
        let joining = self.mp_page.starting.as_deref() == Some(room.code.as_str());
        let code = room.code.clone();
        self.row(first, false, p)
            .child(
                h_flex()
                    .h(px(58.))
                    .px(px(14.))
                    .gap(px(12.))
                    .child(monogram(&room.host.name, 32., p))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(room.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.text_muted)
                                    .truncate()
                                    .child(room_meta(room)),
                            ),
                    )
                    .child(
                        h_flex()
                            .w(px(64.))
                            .flex_shrink_0()
                            .gap(px(6.))
                            .text_size(px(13.))
                            .text_color(p.text_muted)
                            .child(Icon::new(IconName::Users).size(px(14.)))
                            .child(players_text(room)),
                    )
                    .child(
                        h_flex()
                            .w(px(150.))
                            .flex_shrink_0()
                            .gap(px(6.))
                            .text_size(px(12.))
                            .text_color(p.text_muted)
                            .child(Icon::new(icon).size(px(14.)).text_color(color))
                            .child(addons_text(addons)),
                    )
                    .child(
                        h_flex()
                            .w(px(220.))
                            .flex_shrink_0()
                            .justify_end()
                            .gap(px(10.))
                            .children(blocked.map(|why| {
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.text_placeholder)
                                    .whitespace_nowrap()
                                    .child(why)
                            }))
                            .child(
                                Button::new(SharedString::from(format!("mp-join-{}", room.code)))
                                    .small()
                                    .primary()
                                    .label("Join")
                                    .loading(joining)
                                    .disabled(blocked.is_some() || self.mp_page.busy())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.mp_start(Start::Join(code.clone()), cx)
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn create_form(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let m = &self.mp_page;
        let blocked = self.mp_game_state().not_ready();
        let creating = m.starting.as_deref() == Some("create");
        let shell = cx.entity().downgrade();
        let players = Button::new("mp-max")
            .outline()
            .small()
            .w(px(160.))
            .label(plural(usize::from(m.max), "player", "players"))
            .dropdown_caret(true)
            .dropdown_menu_with_anchor(Anchor::TopLeft, move |mut menu, _, _| {
                for n in MIN_PLAYERS..=MAX_PLAYERS {
                    let shell = shell.clone();
                    menu = menu.item(
                        PopupMenuItem::new(plural(usize::from(n), "player", "players")).on_click(
                            move |_, _, cx| {
                                shell
                                    .update(cx, |s, cx| {
                                        s.mp_page.max = n;
                                        cx.notify();
                                    })
                                    .ok();
                            },
                        ),
                    );
                }
                menu
            });
        let shell = cx.entity().downgrade();
        let current = m.category;
        let cars = Button::new("mp-category")
            .outline()
            .small()
            .w(px(160.))
            .label(category_label(current.as_str()))
            .dropdown_caret(true)
            .dropdown_menu_with_anchor(Anchor::TopLeft, move |mut menu, _, _| {
                for category in [Category::F12026, Category::F12025] {
                    let shell = shell.clone();
                    menu = menu.item(
                        PopupMenuItem::new(category_label(category.as_str()))
                            .checked(category == current)
                            .on_click(move |_, _, cx| {
                                shell
                                    .update(cx, |s, cx| {
                                        s.mp_page.category = category;
                                        cx.notify();
                                    })
                                    .ok();
                            }),
                    );
                }
                menu
            });
        let public = h_flex()
            .gap(px(10.))
            .child(
                Switch::new("mp-public")
                    .checked(m.public)
                    .small()
                    .on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.mp_page.public = *on;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(p.text_muted)
                    .child(if m.public {
                        "Listed: anyone can see it and join"
                    } else {
                        "Private: only people with the code can join"
                    }),
            );
        let row = |title: &'static str, control: AnyElement| {
            h_flex()
                .gap(px(12.))
                .child(
                    div()
                        .w(px(104.))
                        .flex_shrink_0()
                        .text_size(px(13.))
                        .text_color(p.text_muted)
                        .child(title),
                )
                .child(h_flex().flex_1().min_w_0().child(control))
        };
        div().px(px(GUTTER)).pb(px(16.)).flex_shrink_0().child(
            v_flex()
                .max_w(px(760.))
                .p(px(16.))
                .gap(px(12.))
                .rounded(px(6.))
                .border_1()
                .border_color(p.border)
                .bg(p.surface)
                .child(label("NEW ROOM", p))
                .child(row(
                    "Name",
                    div()
                        .w(px(320.))
                        .child(Input::new(&m.name).small())
                        .into_any_element(),
                ))
                .child(row("Visibility", public.into_any_element()))
                .child(row("Players", players.into_any_element()))
                .child(row("Cars", cars.into_any_element()))
                .child(
                    h_flex()
                        .gap(px(10.))
                        .pt(px(4.))
                        .child(
                            Button::new("mp-create")
                                .primary()
                                .small()
                                .label("Create room")
                                .loading(creating)
                                .disabled(blocked.is_some() || m.busy())
                                .on_click(cx.listener(|this, _, _, cx| this.mp_create(cx))),
                        )
                        .child(div().text_size(px(12.)).text_color(p.text_placeholder).child(
                            blocked.unwrap_or("F1 25 opens its LAN lobby; pick tracks and settings there."),
                        )),
                ),
        )
    }

    fn fixed_card(&self, f: &Fixed, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let blocked = self.mp_game_state().not_ready();
        let code = f.code.clone();
        let ok = f.failed.is_empty();
        let (title, icon, color) = if ok {
            ("Addons updated", IconName::CircleCheck, p.success)
        } else {
            ("Some changes failed", IconName::TriangleAlert, p.warning)
        };
        div()
            .px(px(GUTTER))
            .pb(px(16.))
            .flex_shrink_0()
            .child(
                h_flex()
                    .max_w(px(760.))
                    .p(px(12.))
                    .gap(px(10.))
                    .items_start()
                    .rounded(px(6.))
                    .bg(p.elevated)
                    .border_1()
                    .border_color(p.border)
                    .child(Icon::new(icon).size(px(15.)).text_color(color))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .children(f.done.iter().chain(&f.failed).map(|l| {
                                div()
                                    .text_size(px(13.))
                                    .text_color(p.text_muted)
                                    .child(l.clone())
                            }))
                            .child(div().text_size(px(13.)).child(
                                "Close F1 25 and press Play so the game loads the new addons, then join again.",
                            )),
                    )
                    .when(!code.is_empty(), |d| {
                        d.child(
                            v_flex()
                                .items_end()
                                .gap(px(4.))
                                .child(
                                    Button::new("mp-rejoin")
                                        .small()
                                        .primary()
                                        .label("Join again")
                                        .disabled(blocked.is_some() || self.mp_page.busy())
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.mp_start(Start::Join(code.clone()), cx)
                                        })),
                                )
                                .children(blocked.map(|why| {
                                    div()
                                        .text_size(px(12.))
                                        .text_color(p.text_placeholder)
                                        .child(why)
                                })),
                        )
                    })
                    .child(
                        Button::new("mp-fixed-close")
                            .ghost()
                            .small()
                            .icon(IconName::X)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mp_page.fixed = None;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }

    fn mp_room(&mut self, state: &SessionState, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let m = &self.mp_page;
        let over = state.phase.is_over();
        let room = state.room.clone();
        let title = room
            .as_ref()
            .map_or_else(|| "Opening the room".to_owned(), |r| r.name.clone());
        let subtitle = room.as_ref().map_or_else(
            || "Setting up\u{2026}".to_owned(),
            |r| {
                let mut parts = vec![
                    category_label(&r.category),
                    format!("up to {}", plural(r.max_members as usize, "player", "players")),
                    if r.public { "Listed" } else { "Private" }.to_owned(),
                ];
                if state.host {
                    parts.push("You host".into());
                }
                parts.join(" \u{00b7} ")
            },
        );
        let code = room.as_ref().map(|r| r.code.clone());
        let actions = h_flex()
            .gap(px(8.))
            .items_center()
            .child(self.game_pill(p))
            .when_some(code, |d, code| {
                let copy = code.clone();
                d.child(
                    Button::new("mp-copy-code")
                        .outline()
                        .icon(if m.copied.is_some() {
                            IconName::Check
                        } else {
                            IconName::Copy
                        })
                        .label(code)
                        .tooltip("Copy the invite code")
                        .on_click(cx.listener(move |this, _, _, cx| this.mp_copy_code(copy.clone(), cx))),
                )
            })
            .child(
                Button::new("mp-leave")
                    .outline()
                    .icon(IconName::LogOut)
                    .label(if m.leaving {
                        "Leaving\u{2026}"
                    } else if over {
                        "Back to rooms"
                    } else {
                        "Leave"
                    })
                    .disabled(m.leaving || m.fixing || m.session.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.mp_leave(cx))),
            );
        let started = room.as_ref().is_some_and(|r| r.started);
        let race = (state.host && !over && m.session.is_some()).then(|| {
            h_flex()
                .gap(px(8.))
                .child(
                    Button::new("mp-race-start")
                        .outline()
                        .small()
                        .icon(IconName::Flag)
                        .label("Race started")
                        .disabled(started)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(s) = &this.mp_page.session {
                                s.mark_race(true);
                            }
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("mp-race-over")
                        .outline()
                        .small()
                        .icon(IconName::FlagTriangleRight)
                        .label("Race over")
                        .disabled(!started)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(s) = &this.mp_page.session {
                                s.mark_race(false);
                            }
                            cx.notify();
                        })),
                )
        });
        let (icon, color) = match &state.phase {
            Phase::InLobby | Phase::Racing => (IconName::CircleCheck, p.success),
            Phase::Failed(_) => (IconName::CircleAlert, p.error),
            Phase::Ended(_) => (IconName::Info, p.text_muted),
            _ => (IconName::LoaderCircle, p.accent),
        };
        let status = v_flex()
            .max_w(px(760.))
            .p(px(14.))
            .gap(px(10.))
            .rounded(px(6.))
            .border_1()
            .border_color(p.border)
            .child(
                h_flex()
                    .gap(px(10.))
                    .items_start()
                    .child(Icon::new(icon).size(px(16.)).text_color(color))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(3.))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(phase_text(&state.phase)),
                            )
                            .children(
                                phase_hint(&state.phase, state.host)
                                    .map(|h| div().text_size(px(13.)).text_color(p.text_muted).child(h)),
                            )
                            .children(state.last_error.clone().filter(|_| !over).map(|e| {
                                div()
                                    .text_size(px(13.))
                                    .text_color(p.error)
                                    .child(capitalised(&e))
                            })),
                    )
                    .when(retry_offered(state) && m.session.is_some(), |d| {
                        d.child(
                            Button::new("mp-retry")
                                .outline()
                                .small()
                                .icon(IconName::RefreshCw)
                                .label("Retry")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(s) = &this.mp_page.session {
                                        s.retry();
                                    }
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .children(race);
        let mismatches = (!state.mismatches.is_empty()).then(|| self.mismatch_card(state, p, cx));
        let members: Vec<AnyElement> = state
            .members
            .iter()
            .enumerate()
            .map(|(i, member)| member_row(member, i == 0, p))
            .collect();
        v_flex()
            .size_full()
            .child(self.header(title, subtitle, actions, p))
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .scroll_y("mp-room")
                    .px(px(GUTTER))
                    .pt(px(4.))
                    .pb(px(24.))
                    .gap(px(16.))
                    .child(status)
                    .children(mismatches)
                    .child(
                        v_flex()
                            .gap(px(6.))
                            .child(label(format!("PLAYERS \u{00b7} {}", state.members.len()), p))
                            .child(if members.is_empty() {
                                div()
                                    .text_size(px(13.))
                                    .text_color(p.text_muted)
                                    .child("Nobody is here yet.")
                                    .into_any_element()
                            } else {
                                v_flex()
                                    .rounded(px(6.))
                                    .border_1()
                                    .border_color(p.border)
                                    .overflow_hidden()
                                    .child(members_head(p))
                                    .children(members)
                                    .into_any_element()
                            }),
                    ),
            )
            .into_any_element()
    }

    fn mismatch_card(&self, state: &SessionState, p: Palette, cx: &mut Context<Self>) -> Div {
        let steps = swofty_core::multiplayer::session::fix_plan(&state.mismatches);
        let (auto, manual) = fixable(&steps);
        let fixing = self.mp_page.fixing;
        v_flex()
            .max_w(px(760.))
            .p(px(14.))
            .gap(px(10.))
            .rounded(px(6.))
            .bg(p.warning.opacity(0.08))
            .border_1()
            .border_color(p.warning.opacity(0.4))
            .child(
                h_flex()
                    .gap(px(10.))
                    .child(
                        Icon::new(IconName::TriangleAlert)
                            .size(px(15.))
                            .text_color(p.warning),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!(
                                "{} from the host's",
                                plural(state.mismatches.len(), "thing differs", "things differ")
                            )),
                    ),
            )
            .child(v_flex().gap(px(4.)).children(state.mismatches.iter().map(|mm| {
                h_flex()
                    .gap(px(8.))
                    .items_start()
                    .text_size(px(13.))
                    .child(
                        Icon::new(if mm.fixable {
                            IconName::Wrench
                        } else {
                            IconName::CircleAlert
                        })
                        .size(px(13.))
                        .mt(px(3.))
                        .text_color(p.text_placeholder),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(capitalised(mm.line().trim_end_matches(" (fixable)"))),
                    )
            })))
            .child(
                h_flex()
                    .gap(px(10.))
                    .pt(px(2.))
                    .when(auto > 0, |d| {
                        d.child(
                            Button::new("mp-fix")
                                .primary()
                                .small()
                                .icon(IconName::Wrench)
                                .label("Fix & join")
                                .loading(fixing)
                                .disabled(fixing)
                                .on_click(cx.listener(|this, _, _, cx| this.mp_fix(cx))),
                        )
                    })
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.text_muted)
                            .child(if manual.is_empty() {
                                "Installs the host's exact versions. F1 25 then needs a restart to load them."
                            } else if auto > 0 {
                                "Fixes what it can; change the rest yourself. F1 25 then needs a restart."
                            } else {
                                "Change these yourself, then join again."
                            }),
                    ),
            )
    }
}

fn members_head(p: Palette) -> Div {
    let col = |w: f32, t: &'static str| {
        div()
            .w(px(w))
            .flex_shrink_0()
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.text_placeholder)
            .child(t)
    };
    h_flex()
        .h(px(30.))
        .px(px(14.))
        .gap(px(12.))
        .bg(p.surface)
        .border_b_1()
        .border_color(p.border_variant)
        .child(div().w(px(28.)).flex_shrink_0())
        .child(
            div()
                .flex_1()
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.text_placeholder)
                .child("PLAYER"),
        )
        .child(col(96., "ADDRESS"))
        .child(col(96., "STATUS"))
        .child(col(80., "ADDONS"))
        .child(col(72., "RELAY"))
        .child(col(140., "TRAFFIC"))
}

fn member_row(m: &MemberState, first: bool, p: Palette) -> AnyElement {
    let cell = |w: f32| {
        div()
            .w(px(w))
            .flex_shrink_0()
            .text_size(px(13.))
            .text_color(p.text_muted)
            .truncate()
    };
    let dot = |color: Hsla, text: &'static str| {
        h_flex()
            .gap(px(6.))
            .child(div().size(px(6.)).rounded_full().bg(color))
            .child(text)
    };
    let addons = match m.fingerprint {
        PrintMatch::Matches => p.success,
        PrintMatch::Differs => p.warning,
        PrintMatch::Unknown => p.text_placeholder,
    };
    let relay = match m.relay {
        RelayStatus::Ready => p.success,
        RelayStatus::Pending => p.warning,
        RelayStatus::None => p.text_placeholder,
    };
    h_flex()
        .h(px(48.))
        .px(px(14.))
        .gap(px(12.))
        .when(!first, |d| d.border_t_1().border_color(p.border_variant))
        .child(monogram(&m.name, 28., p))
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .child(m.name.clone()),
                )
                .when(m.host, |d| d.child(tag("Host", p)))
                .when(m.me, |d| d.child(tag("You", p))),
        )
        .child(cell(96.).child(m.ip.clone()))
        .child(cell(96.).child(if m.connected {
            dot(p.success, "Connected")
        } else {
            dot(p.text_placeholder, "Offline")
        }))
        .child(cell(80.).child(dot(addons, addon_word(m.fingerprint))))
        .child(cell(72.).child(dot(relay, relay_word(m.relay))))
        .child(cell(140.).child(traffic_text(m.traffic.as_ref())))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use swofty_core::multiplayer::bridge::MemberStats;
    use swofty_core::multiplayer::game::{GameStatus, Role, Screen};
    use swofty_core::multiplayer::rooms::{RoomAddons, RoomSummary, User};
    use swofty_core::multiplayer::session::{FixStep, Phase};
    use swofty_core::profile::ProfileAddon;

    use super::{
        Addons, GameState, addons_against, addons_text, category_label, fixable, join_blocked, phase_hint,
        phase_text, players_text, reorder, room_meta, traffic_text,
    };

    fn status(screen: Screen) -> GameStatus {
        GameStatus {
            screen,
            in_session: false,
            role: Role::None,
            configured: false,
            self_member: 0,
            base: 0,
        }
    }

    fn room(members: u32, max: u32, digest: Option<&str>, tracks: &[&str]) -> RoomSummary {
        RoomSummary {
            code: "F1-7KQ2MX".into(),
            name: "Sunday league".into(),
            host: User {
                id: "u1".into(),
                name: "swofty".into(),
                avatar: None,
            },
            members,
            max_members: max,
            category: "f1-2026".into(),
            digest: digest.map(str::to_owned),
            addons: Some(RoomAddons {
                count: 4,
                tracks: tracks.iter().map(|t| (*t).to_owned()).collect(),
            }),
            created_at: 0,
        }
    }

    #[test]
    fn names_where_the_game_is() {
        assert_eq!(
            GameState::of(false, Some(&status(Screen::MainMenu))),
            GameState::NotRunning
        );
        assert_eq!(
            GameState::of(true, Some(&status(Screen::MainMenu))),
            GameState::MainMenu
        );
        assert_eq!(
            GameState::of(true, Some(&status(Screen::LanLobby))),
            GameState::LanLobby
        );
        assert_eq!(GameState::of(true, Some(&status(Screen::Other))), GameState::Busy);
        assert_eq!(GameState::of(true, None), GameState::Busy);
        assert_eq!(GameState::NotRunning.text(), "Game not running");
        assert_eq!(GameState::MainMenu.not_ready(), None);
    }

    #[test]
    fn join_waits_for_the_main_menu_and_a_free_seat() {
        let open = room(3, 8, None, &[]);
        assert_eq!(join_blocked(&open, GameState::MainMenu), None);
        assert_eq!(
            join_blocked(&open, GameState::NotRunning),
            Some("Start F1 25 first")
        );
        assert_eq!(
            join_blocked(&open, GameState::Busy),
            Some("Go to F1 25's main menu")
        );
        assert_eq!(
            join_blocked(&open, GameState::LanLobby),
            Some("Leave the LAN lobby first")
        );
        assert_eq!(
            join_blocked(&room(8, 8, None, &[]), GameState::MainMenu),
            Some("Room full")
        );
    }

    #[test]
    fn describes_a_room() {
        let r = room(3, 8, None, &["Okayama", "Bahrain 2024", "Monza 1990", "Imola"]);
        assert_eq!(players_text(&r), "3/8");
        assert_eq!(
            room_meta(&r),
            "Hosted by swofty \u{00b7} F1 2026 \u{00b7} Tracks: Okayama, Bahrain 2024, Monza 1990 and 1 more"
        );
        assert_eq!(
            room_meta(&room(1, 2, None, &[])),
            "Hosted by swofty \u{00b7} F1 2026"
        );
        assert_eq!(category_label("f1-2025"), "F1 2025");
        assert_eq!(category_label("karts"), "karts");
    }

    #[test]
    fn compares_addons_by_digest() {
        let r = room(1, 8, Some("sha256:ABC"), &[]);
        assert_eq!(addons_against(&r, Some("sha256:abc")), Addons::Match);
        assert_eq!(addons_against(&r, Some("sha256:def")), Addons::Differ);
        assert_eq!(addons_against(&r, None), Addons::Unknown);
        assert_eq!(addons_against(&room(1, 8, None, &[]), Some("x")), Addons::Unknown);
        assert_eq!(addons_text(Addons::Match), "Your addons match");
        assert_eq!(addons_text(Addons::Differ), "Addons differ");
    }

    #[test]
    fn words_the_phase() {
        assert_eq!(
            phase_text(&Phase::Connecting("opening the relay".into())),
            "Connecting: opening the relay"
        );
        assert_eq!(
            phase_text(&Phase::WaitingForGame("go to the game's main menu".into())),
            "Go to the game's main menu"
        );
        assert_eq!(
            phase_text(&Phase::Ended("the host closed the room".into())),
            "The host closed the room"
        );
        assert!(phase_hint(&Phase::InLobby, true).unwrap().contains("tracks"));
        assert_eq!(phase_hint(&Phase::Racing, false), None);
    }

    #[test]
    fn shows_traffic_once_there_is_some() {
        assert_eq!(traffic_text(None), "\u{2014}");
        let t = MemberStats {
            packets_out: 10,
            bytes_out: 3 << 20,
            packets_in: 4,
            bytes_in: 2048,
        };
        assert_eq!(traffic_text(Some(&t)), "\u{2191} 3.0 MB  \u{2193} 2 KB");
    }

    #[test]
    fn splits_fixes_from_manual_steps() {
        let steps = vec![
            FixStep::Disable {
                id: "a".into(),
                name: "A".into(),
            },
            FixStep::Manual {
                line: "game build 1 is not the host's 2".into(),
            },
        ];
        assert_eq!(
            fixable(&steps),
            (1, vec!["Game build 1 is not the host's 2".to_owned()])
        );
    }

    #[test]
    fn puts_the_shared_addons_in_the_hosts_order() {
        let addon = |id: &str| ProfileAddon {
            id: id.into(),
            enabled: true,
            unknown: Default::default(),
        };
        let mut addons = vec![addon("core"), addon("b"), addon("x"), addon("a")];
        reorder(&["a".into(), "b".into()], &mut addons);
        let ids: Vec<&str> = addons.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["core", "a", "x", "b"]);
    }
}
