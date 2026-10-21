use std::ffi::{c_char, c_int, c_void};

pub const SWOFTY_API_VERSION: u32 = 8;
pub const INIT_SYMBOLS: [&str; 2] = ["SwoftyAddonInit", "SwoftyModInit"];
pub const API_SYMBOLS: [&str; 2] = ["SwoftyAddonApi", "SwoftyModApi"];
pub const HEADER: &str = include_str!("../include/swofty_api.h");
pub const MIXIN_HEADER: &str = include_str!("../include/swofty/mixin.hpp");
pub const ASSETS_HEADER: &str = include_str!("../include/swofty/assets.hpp");
pub const EVENTS_HEADER: &str = include_str!("../include/swofty/events.hpp");
pub const OVERLAY_HEADER: &str = include_str!("../include/swofty/overlay.hpp");
pub const TELEMETRY_HEADER: &str = include_str!("../include/swofty/telemetry.hpp");
pub const DB_HEADER: &str = include_str!("../include/swofty/db.hpp");
pub const STORAGE_HEADER: &str = include_str!("../include/swofty/storage.hpp");
pub const CAREER_HEADER: &str = include_str!("../include/swofty/career.hpp");
pub const MEM_HEADER: &str = include_str!("../include/swofty/mem.hpp");
pub const POPUP_HEADER: &str = include_str!("../include/swofty/popup.hpp");
pub const LANG_HEADER: &str = include_str!("../include/swofty/lang.hpp");

pub const CAREER_START_PICK: u32 = 0;
pub const CAREER_START_NEW: u32 = 1;
pub const CAREER_START_CONTINUE: u32 = 2;

pub const MIXIN_WRAP: u32 = 1;
pub const MIXIN_SITE: u32 = 2;
pub const MIXIN_VTABLE: u32 = 3;
pub const MIXIN_REDIRECT: u32 = 4;
pub const MIXIN_PATCH: u32 = 5;
pub const MIXIN_ON_LOAD: u32 = 6;
pub const MIXIN_OVERWRITE: u32 = 0x100;

pub const SETTING_TOGGLE: u32 = 1;
pub const SETTING_SLIDER: u32 = 2;
pub const SETTING_CHOICE: u32 = 3;

pub const ASSET_OK: c_int = 0;
pub const ASSET_LATE: c_int = 1;
pub const ASSET_UNAVAILABLE: c_int = 2;
pub const ASSET_INVALID: c_int = 3;

pub const DB_INT: u8 = 0;
pub const DB_FLOAT: u8 = 1;
pub const DB_STRING: u8 = 2;
pub const DB_ENUM: u8 = 3;
pub const DB_ID: u8 = 4;
pub const DB_BOOL: u8 = 5;

pub const TELEMETRY_VERSION: u32 = 1;

pub type LogFn = unsafe extern "C" fn(fmt: *const c_char, ...);
pub type ModuleBaseFn = unsafe extern "C" fn() -> usize;
pub type ScanFn =
    unsafe extern "C" fn(section: *const c_char, pattern: *const c_char, out: *mut usize) -> c_int;
pub type ResolveFn = unsafe extern "C" fn(id: *const c_char) -> usize;
pub type RegisterMixinsFn = unsafe extern "C" fn(mixins: *const SwoftyMixin, count: usize) -> c_int;
pub type FieldsFn = unsafe extern "C" fn(obj: *const c_void, size: usize) -> *mut c_void;
pub type SettingFn =
    unsafe extern "C" fn(id: *const c_char, kind: u32, out: *mut SwoftySettingValue) -> c_int;
pub type SettingChangedFn =
    unsafe extern "C" fn(id: *const c_char, value: *const SwoftySettingValue, user: *mut c_void);
pub type WatchSettingFn =
    unsafe extern "C" fn(id: *const c_char, f: Option<SettingChangedFn>, user: *mut c_void) -> c_int;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyMixin {
    pub target: *const c_char,
    pub kind: u32,
    pub priority: i32,
    pub handler: *mut c_void,
    pub orig: *mut *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwoftySettingValue {
    pub kind: u32,
    pub toggle: i32,
    pub slider: f64,
    pub choice: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyFragment {
    pub file: *const c_char,
    pub fourcc: *const c_char,
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyAssets {
    pub available: Option<unsafe extern "C" fn(feature: *const c_char) -> c_int>,
    pub register_group: Option<
        unsafe extern "C" fn(
            group: *const c_char,
            erps: *const *const c_char,
            erp_count: usize,
            deps: *const *const c_char,
            dep_count: usize,
            templ: *const c_char,
        ) -> c_int,
    >,
    pub put_resource: Option<
        unsafe extern "C" fn(
            uri: *const c_char,
            fragments: *const SwoftyFragment,
            fragment_count: usize,
            erps: *const *const c_char,
            erp_count: usize,
        ) -> c_int,
    >,
    pub db_set:
        Option<unsafe extern "C" fn(key: *const c_char, kind: *const c_char, value: *const c_char) -> c_int>,
    pub db_set_raw:
        Option<unsafe extern "C" fn(hash: u64, tag: u8, value: *const c_void, size: usize) -> c_int>,
    pub lang_set: Option<
        unsafe extern "C" fn(language: *const c_char, key: *const c_char, text: *const c_char) -> c_int,
    >,
    pub lang_table: Option<unsafe extern "C" fn(language: *const c_char, file: *const c_char) -> c_int>,
    pub overlay_file: Option<unsafe extern "C" fn(game_path: *const c_char, file: *const c_char) -> c_int>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyEvent {
    pub name: *const c_char,
    pub frame: u64,
    pub time: f64,
    pub a: i64,
    pub b: i64,
    pub data: *const c_void,
}

pub type SwoftyEventFn = unsafe extern "C" fn(ev: *const SwoftyEvent, user: *mut c_void);
pub type SwoftyFrameFn = unsafe extern "C" fn(dt: f64, user: *mut c_void);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyEvents {
    pub available: Option<unsafe extern "C" fn() -> c_int>,
    pub subscribe: Option<
        unsafe extern "C" fn(
            event: *const c_char,
            f: Option<SwoftyEventFn>,
            user: *mut c_void,
            token: *mut u32,
        ) -> c_int,
    >,
    pub on_frame:
        Option<unsafe extern "C" fn(f: Option<SwoftyFrameFn>, user: *mut c_void, token: *mut u32) -> c_int>,
    pub unsubscribe: Option<unsafe extern "C" fn(token: u32) -> c_int>,
}

#[repr(C)]
pub struct SwoftyPaint {
    _private: [u8; 0],
}

pub type SwoftyPaintFn = unsafe extern "C" fn(paint: *mut SwoftyPaint, user: *mut c_void);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyOverlay {
    pub available: Option<unsafe extern "C" fn() -> c_int>,
    pub on_paint:
        Option<unsafe extern "C" fn(f: Option<SwoftyPaintFn>, user: *mut c_void, token: *mut u32) -> c_int>,
    pub remove: Option<unsafe extern "C" fn(token: u32) -> c_int>,
    pub size: Option<unsafe extern "C" fn(paint: *const SwoftyPaint, width: *mut f32, height: *mut f32)>,
    pub rect:
        Option<unsafe extern "C" fn(paint: *mut SwoftyPaint, x: f32, y: f32, w: f32, h: f32, rgba: u32)>,
    pub rect_outline: Option<
        unsafe extern "C" fn(
            paint: *mut SwoftyPaint,
            x: f32,
            y: f32,
            w: f32,
            h: f32,
            thickness: f32,
            rgba: u32,
        ),
    >,
    pub line: Option<
        unsafe extern "C" fn(
            paint: *mut SwoftyPaint,
            x0: f32,
            y0: f32,
            x1: f32,
            y1: f32,
            thickness: f32,
            rgba: u32,
        ),
    >,
    pub text: Option<
        unsafe extern "C" fn(
            paint: *mut SwoftyPaint,
            x: f32,
            y: f32,
            size: f32,
            rgba: u32,
            utf8: *const c_char,
        ),
    >,
    pub measure: Option<
        unsafe extern "C" fn(
            paint: *const SwoftyPaint,
            size: f32,
            utf8: *const c_char,
            width: *mut f32,
            height: *mut f32,
        ),
    >,
    pub load_image: Option<unsafe extern "C" fn(id: *const c_char, file: *const c_char) -> c_int>,
    pub image: Option<
        unsafe extern "C" fn(
            paint: *mut SwoftyPaint,
            id: *const c_char,
            x: f32,
            y: f32,
            w: f32,
            h: f32,
            tint: u32,
        ) -> c_int,
    >,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyCarState {
    pub version: u32,
    pub flags: u32,
    pub speed: f32,
    pub rpm: f32,
    pub max_rpm: f32,
    pub gear: i32,
    pub throttle: f32,
    pub brake: f32,
    pub clutch: f32,
    pub steer: f32,
    pub tyre_temp: [f32; 4],
    pub tyre_wear: [f32; 4],
    pub brake_temp: [f32; 4],
    pub fuel: f32,
    pub fuel_laps: f32,
    pub position: i32,
    pub lap: u32,
    pub sector: u32,
    pub last_lap_time: f32,
    pub current_lap_time: f32,
    pub best_lap_time: f32,
    pub sector_time: [f32; 3],
    pub lap_distance: f32,
    pub world_pos: [f32; 3],
    pub world_vel: [f32; 3],
    pub reserved: [f32; 8],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftySessionState {
    pub version: u32,
    pub session_type: u32,
    pub track: u32,
    pub flag: u32,
    pub weather: u32,
    pub total_laps: u32,
    pub participant_count: u32,
    pub player_index: u32,
    pub session_time: f32,
    pub session_time_left: f32,
    pub track_temp: f32,
    pub air_temp: f32,
    pub reserved: [f32; 8],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyParticipant {
    pub version: u32,
    pub flags: u32,
    pub name: [c_char; 48],
    pub team: u32,
    pub position: i32,
    pub lap: u32,
    pub lap_distance: f32,
    pub best_lap_time: f32,
    pub reserved: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyTelemetryFeed {
    pub version: u32,
    pub size: u32,
    pub seq: u64,
    pub frame: u64,
    pub session: SwoftySessionState,
    pub cars: [SwoftyCarState; 24],
    pub participants: [SwoftyParticipant; 24],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyTelemetry {
    pub available: Option<unsafe extern "C" fn() -> c_int>,
    pub session: Option<unsafe extern "C" fn(out: *mut SwoftySessionState) -> c_int>,
    pub car: Option<unsafe extern "C" fn(index: u32, out: *mut SwoftyCarState) -> c_int>,
    pub participant: Option<unsafe extern "C" fn(index: u32, out: *mut SwoftyParticipant) -> c_int>,
    pub shared_memory: Option<unsafe extern "C" fn(name: *mut *const c_char) -> c_int>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyDbValue {
    pub tag: u8,
    pub i: i64,
    pub f: f64,
    pub s: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyDbCursor {
    pub table: u64,
    pub index: u64,
    pub count: u64,
    pub row: *const c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyDb {
    pub available: Option<unsafe extern "C" fn() -> c_int>,
    pub get: Option<unsafe extern "C" fn(key: *const c_char, out: *mut SwoftyDbValue) -> c_int>,
    pub get_hash: Option<unsafe extern "C" fn(hash: u64, out: *mut SwoftyDbValue) -> c_int>,
    pub int_at: Option<unsafe extern "C" fn(key: *const c_char, out: *mut i64) -> c_int>,
    pub float_at: Option<unsafe extern "C" fn(key: *const c_char, out: *mut f64) -> c_int>,
    pub string_at: Option<unsafe extern "C" fn(key: *const c_char, out: *mut *const c_char) -> c_int>,
    pub bool_at: Option<unsafe extern "C" fn(key: *const c_char, out: *mut i32) -> c_int>,
    pub open: Option<unsafe extern "C" fn(table: *const c_char, cursor: *mut SwoftyDbCursor) -> c_int>,
    pub row: Option<
        unsafe extern "C" fn(
            table: *const c_char,
            label: *const c_char,
            cursor: *mut SwoftyDbCursor,
        ) -> c_int,
    >,
    pub step: Option<unsafe extern "C" fn(cursor: *mut SwoftyDbCursor) -> c_int>,
    pub cell: Option<
        unsafe extern "C" fn(
            cursor: *const SwoftyDbCursor,
            field: *const c_char,
            out: *mut SwoftyDbValue,
        ) -> c_int,
    >,
    pub row_name: Option<unsafe extern "C" fn(cursor: *const SwoftyDbCursor) -> *const c_char>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyStorage {
    pub available: Option<unsafe extern "C" fn() -> c_int>,
    pub read: Option<
        unsafe extern "C" fn(path: *const c_char, buf: *mut c_void, cap: usize, len: *mut usize) -> c_int,
    >,
    pub write: Option<unsafe extern "C" fn(path: *const c_char, data: *const c_void, len: usize) -> c_int>,
    pub append: Option<unsafe extern "C" fn(path: *const c_char, data: *const c_void, len: usize) -> c_int>,
    pub remove: Option<unsafe extern "C" fn(path: *const c_char) -> c_int>,
    pub size: Option<unsafe extern "C" fn(path: *const c_char, out: *mut u64) -> c_int>,
    pub list: Option<
        unsafe extern "C" fn(dir: *const c_char, buf: *mut c_char, cap: usize, len: *mut usize) -> c_int,
    >,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyFlowState {
    pub hash: u32,
    pub from: u32,
    pub to: u32,
    pub named: u32,
    pub transitions: u64,
    pub name: [c_char; 64],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyCareerState {
    pub loaded: u32,
    pub mode: u32,
    pub hub: u32,
    pub humans: u32,
    pub player_driver: u32,
    pub player_team: u32,
    pub has_player: u32,
    pub components: u32,
    pub component_mode: u32,
    pub team_hq_mode: u32,
    pub flow_career: u32,
    pub reserved: u32,
    pub key: [c_char; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyCareer {
    pub available: Option<unsafe extern "C" fn() -> c_int>,
    pub state: Option<unsafe extern "C" fn(out: *mut SwoftyCareerState) -> c_int>,
    pub flow: Option<unsafe extern "C" fn(out: *mut SwoftyFlowState) -> c_int>,
    pub flow_status: Option<unsafe extern "C" fn(hash: u32, status: *mut u32, age_ms: *mut u64) -> c_int>,
    pub entity: Option<unsafe extern "C" fn() -> *mut c_void>,
    pub component: Option<unsafe extern "C" fn(type_name: *const c_char) -> *mut c_void>,
    pub component_by_desc: Option<unsafe extern "C" fn(desc: *const c_void) -> *mut c_void>,
    pub post: Option<unsafe extern "C" fn(msg: *mut c_void) -> c_int>,
    pub block: Option<unsafe extern "C" fn(mode: u32, title: *const c_char, body: *const c_char) -> c_int>,
    pub unblock: Option<unsafe extern "C" fn(mode: u32) -> c_int>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyCareerStart {
    pub mode: u32,
    pub kind: u32,
    pub blocked: u32,
    pub reserved: u32,
    pub route: [c_char; 64],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyPopup {
    pub available: Option<unsafe extern "C" fn() -> c_int>,
    pub show: Option<unsafe extern "C" fn(title: *const c_char, body: *const c_char) -> c_int>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyLang {
    pub available: Option<unsafe extern "C" fn() -> c_int>,
    pub override_set: Option<
        unsafe extern "C" fn(language: *const c_char, key: *const c_char, text: *const c_char) -> c_int,
    >,
    pub override_clear: Option<unsafe extern "C" fn(language: *const c_char, key: *const c_char) -> c_int>,
    pub override_clear_all: Option<unsafe extern "C" fn() -> c_int>,
    pub refresh: Option<unsafe extern "C" fn() -> c_int>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwoftyApi {
    pub version: u32,
    pub log: Option<LogFn>,
    pub module_base: Option<ModuleBaseFn>,
    pub scan: Option<ScanFn>,
    pub resolve: Option<ResolveFn>,
    pub register_mixins: Option<RegisterMixinsFn>,
    pub fields: Option<FieldsFn>,
    pub setting: Option<SettingFn>,
    pub watch_setting: Option<WatchSettingFn>,
    pub assets: *const SwoftyAssets,
    pub events: *const SwoftyEvents,
    pub overlay: *const SwoftyOverlay,
    pub telemetry: *const SwoftyTelemetry,
    pub db: *const SwoftyDb,
    pub storage: *const SwoftyStorage,
    pub career: *const SwoftyCareer,
    pub popup: *const SwoftyPopup,
    pub lang: *const SwoftyLang,
}

pub type SwoftyAddonInitFn = unsafe extern "C" fn(api: *const SwoftyApi) -> c_int;
pub type SwoftyModInitFn = SwoftyAddonInitFn;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Capability {
    Log,
    Scan,
    Hook,
    ReadDb,
    Options,
    OnFrame,
    Assets,
    Events,
    Overlay,
    Telemetry,
    Storage,
    Career,
    Popup,
    Lang,
}

impl Capability {
    pub const ALL: [Capability; 14] = [
        Self::Log,
        Self::Scan,
        Self::Hook,
        Self::ReadDb,
        Self::Options,
        Self::OnFrame,
        Self::Assets,
        Self::Events,
        Self::Overlay,
        Self::Telemetry,
        Self::Storage,
        Self::Career,
        Self::Popup,
        Self::Lang,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Log => "log",
            Self::Scan => "scan",
            Self::Hook => "hook",
            Self::ReadDb => "read-db",
            Self::Options => "options",
            Self::OnFrame => "on-frame",
            Self::Assets => "assets",
            Self::Events => "events",
            Self::Overlay => "overlay",
            Self::Telemetry => "telemetry",
            Self::Storage => "storage",
            Self::Career => "career",
            Self::Popup => "popup",
            Self::Lang => "lang",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == s)
    }

    pub fn available_in(self, api_version: u32) -> bool {
        match self {
            Self::Log | Self::Scan => api_version >= 1,
            Self::Hook => api_version >= 2,
            Self::Options => api_version >= 3,
            Self::Assets => api_version >= 4,
            Self::ReadDb | Self::OnFrame | Self::Events | Self::Overlay | Self::Telemetry | Self::Storage => {
                api_version >= 5
            }
            Self::Career => api_version >= 6,
            Self::Popup => api_version >= 7,
            Self::Lang => api_version >= 8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_header() {
        assert_eq!(std::mem::size_of::<SwoftyApi>(), 144);
        assert_eq!(std::mem::offset_of!(SwoftyApi, career), 120);
        assert_eq!(std::mem::offset_of!(SwoftyApi, popup), 128);
        assert_eq!(std::mem::offset_of!(SwoftyApi, lang), 136);
        assert_eq!(std::mem::size_of::<SwoftyLang>(), 40);
        assert_eq!(std::mem::size_of::<SwoftyCareer>(), 80);
        assert_eq!(std::mem::offset_of!(SwoftyCareer, block), 64);
        assert_eq!(std::mem::size_of::<SwoftyCareerStart>(), 80);
        assert_eq!(std::mem::offset_of!(SwoftyCareerStart, route), 16);
        assert_eq!(std::mem::size_of::<SwoftyPopup>(), 16);
        assert_eq!(std::mem::size_of::<SwoftyFlowState>(), 88);
        assert_eq!(std::mem::offset_of!(SwoftyFlowState, name), 24);
        assert_eq!(std::mem::size_of::<SwoftyCareerState>(), 80);
        assert_eq!(std::mem::offset_of!(SwoftyCareerState, key), 48);
        assert_eq!(std::mem::offset_of!(SwoftyApi, assets), 72);
        assert_eq!(std::mem::offset_of!(SwoftyApi, events), 80);
        assert_eq!(std::mem::offset_of!(SwoftyApi, overlay), 88);
        assert_eq!(std::mem::offset_of!(SwoftyApi, telemetry), 96);
        assert_eq!(std::mem::offset_of!(SwoftyApi, db), 104);
        assert_eq!(std::mem::offset_of!(SwoftyApi, storage), 112);
        assert_eq!(std::mem::size_of::<SwoftyAssets>(), 64);
        assert_eq!(std::mem::offset_of!(SwoftyAssets, db_set_raw), 32);
        assert_eq!(std::mem::offset_of!(SwoftyAssets, overlay_file), 56);
        assert_eq!(std::mem::size_of::<SwoftyFragment>(), 24);
        assert_eq!(std::mem::size_of::<SwoftyEvent>(), 48);
        assert_eq!(std::mem::size_of::<SwoftyEvents>(), 32);
        assert_eq!(std::mem::size_of::<SwoftyOverlay>(), 88);
        assert_eq!(std::mem::size_of::<SwoftyCarState>(), 192);
        assert_eq!(std::mem::size_of::<SwoftySessionState>(), 80);
        assert_eq!(std::mem::size_of::<SwoftyParticipant>(), 92);
        assert_eq!(std::mem::size_of::<SwoftyTelemetryFeed>(), 6920);
        assert_eq!(std::mem::offset_of!(SwoftyTelemetryFeed, cars), 104);
        assert_eq!(std::mem::size_of::<SwoftyTelemetry>(), 40);
        assert_eq!(std::mem::size_of::<SwoftyDbValue>(), 32);
        assert_eq!(std::mem::offset_of!(SwoftyDbValue, i), 8);
        assert_eq!(std::mem::offset_of!(SwoftyDbValue, s), 24);
        assert_eq!(std::mem::size_of::<SwoftyDbCursor>(), 32);
        assert_eq!(std::mem::size_of::<SwoftyDb>(), 96);
        assert_eq!(std::mem::size_of::<SwoftyStorage>(), 56);
        assert_eq!(std::mem::offset_of!(SwoftyApi, log), 8);
        assert_eq!(std::mem::offset_of!(SwoftyApi, scan), 24);
        assert_eq!(std::mem::offset_of!(SwoftyApi, fields), 48);
        assert_eq!(std::mem::offset_of!(SwoftyApi, setting), 56);
        assert_eq!(std::mem::offset_of!(SwoftyApi, watch_setting), 64);
        assert_eq!(std::mem::size_of::<SwoftyMixin>(), 32);
        assert_eq!(std::mem::size_of::<SwoftySettingValue>(), 24);
        assert_eq!(std::mem::offset_of!(SwoftySettingValue, slider), 8);
        assert_eq!(std::mem::offset_of!(SwoftySettingValue, choice), 16);
        assert!(HEADER.contains(&format!("#define SWOFTY_API_VERSION {SWOFTY_API_VERSION}")));
        for name in [
            "SwoftyAddonInitFn",
            "SwoftyModInitFn",
            "SWOFTY_MIXIN_ON_LOAD = 6",
            "SWOFTY_SETTING_CHOICE = 3",
            "watch_setting",
            "SWOFTY_ASSET_UNAVAILABLE = 2",
            "const SwoftyAssets *assets;",
            "const SwoftyEvents *events;",
            "const SwoftyOverlay *overlay;",
            "const SwoftyTelemetry *telemetry;",
            "const SwoftyDb *db;",
            "const SwoftyStorage *storage;",
            "const SwoftyCareer *career;",
            "const SwoftyPopup *popup;",
            "const SwoftyLang *lang;",
            "} SwoftyLang;",
            "#define SWOFTY_EVENT_CAREER_STARTING \"career.starting\"",
            "SWOFTY_CAREER_START_CONTINUE = 2",
            "} SwoftyCareerStart;",
            "#define SWOFTY_EVENT_CAREER_HUB \"career.hub\"",
            "} SwoftyCareerState;",
            "#define SWOFTY_EVENT_SESSION_START \"session.start\"",
            "#define SWOFTY_TELEMETRY_VERSION 1",
            "SWOFTY_DB_BOOL = 5",
            "} SwoftyTelemetryFeed;",
        ] {
            assert!(HEADER.contains(name), "{name}");
        }
        for name in [
            "SWOFTY_SETTING(",
            "SWOFTY_ON_CHANGE(",
            "struct setting",
            "SWOFTY_ADDON(",
            "SWOFTY_MOD(",
        ] {
            assert!(MIXIN_HEADER.contains(name), "{name}");
        }
    }

    #[test]
    fn template_headers_match() {
        assert_eq!(
            include_str!("../../../templates/addon/native/include/swofty_api.h"),
            HEADER
        );
        assert_eq!(
            include_str!("../../../templates/addon/native/include/swofty/mixin.hpp"),
            MIXIN_HEADER
        );
        assert_eq!(
            include_str!("../../../templates/addon/native/include/swofty/assets.hpp"),
            ASSETS_HEADER
        );
        assert_eq!(
            include_str!("../../../templates/addon/native/include/swofty/events.hpp"),
            EVENTS_HEADER
        );
        assert_eq!(
            include_str!("../../../templates/addon/native/include/swofty/overlay.hpp"),
            OVERLAY_HEADER
        );
        assert_eq!(
            include_str!("../../../templates/addon/native/include/swofty/telemetry.hpp"),
            TELEMETRY_HEADER
        );
        assert_eq!(
            include_str!("../../../templates/addon/native/include/swofty/db.hpp"),
            DB_HEADER
        );
        assert_eq!(
            include_str!("../../../templates/addon/native/include/swofty/storage.hpp"),
            STORAGE_HEADER
        );
        assert_eq!(
            include_str!("../../../templates/addon/native/include/swofty/career.hpp"),
            CAREER_HEADER
        );
        assert_eq!(
            include_str!("../../../templates/addon/native/include/swofty/mem.hpp"),
            MEM_HEADER
        );
        assert_eq!(
            include_str!("../../../templates/addon/native/include/swofty/popup.hpp"),
            POPUP_HEADER
        );
        assert_eq!(
            include_str!("../../../templates/addon/native/include/swofty/lang.hpp"),
            LANG_HEADER
        );
    }

    #[test]
    fn capability_names_round_trip() {
        for c in Capability::ALL {
            assert_eq!(Capability::parse(c.as_str()), Some(c));
        }
        assert!(Capability::Assets.available_in(4));
        assert!(!Capability::Assets.available_in(3));
        for c in [
            Capability::ReadDb,
            Capability::OnFrame,
            Capability::Events,
            Capability::Overlay,
            Capability::Telemetry,
            Capability::Storage,
        ] {
            assert!(c.available_in(5));
            assert!(!c.available_in(4));
        }
        assert!(Capability::Career.available_in(6));
        assert!(!Capability::Career.available_in(5));
        assert!(Capability::Popup.available_in(7));
        assert!(!Capability::Popup.available_in(6));
        assert!(Capability::Lang.available_in(8));
        assert!(!Capability::Lang.available_in(7));
    }

    #[test]
    fn assets_header_wraps_every_call() {
        for name in [
            "enum class Result { ok, late, unavailable, invalid };",
            "bool available(",
            "Result register_group(",
            "Result put_resource(",
            "Result db_set(std::string_view key",
            "Result db_set(uint64_t hash",
            "Result lang_set(",
            "Result lang_table(",
            "Result overlay_file(",
        ] {
            assert!(ASSETS_HEADER.contains(name), "{name}");
        }
    }

    #[test]
    fn stdlib_headers_wrap_every_call() {
        for name in [
            "namespace swofty::events {",
            "bool available(",
            "Result subscribe(",
            "Result on_frame(",
            "Result unsubscribe(",
        ] {
            assert!(EVENTS_HEADER.contains(name), "{name}");
        }
        for name in [
            "namespace swofty::overlay {",
            "Result on_paint(",
            "void rect(",
            "void text(",
            "Result load_image(",
            "Result image(",
        ] {
            assert!(OVERLAY_HEADER.contains(name), "{name}");
        }
        for name in [
            "namespace swofty::telemetry {",
            "Result session(",
            "Result car(",
            "Result participant(",
            "Result shared_memory(",
        ] {
            assert!(TELEMETRY_HEADER.contains(name), "{name}");
        }
        for name in [
            "namespace swofty::db {",
            "Result get(",
            "Result int_at(",
            "Result row(",
            "Result step(",
            "Result cell(",
        ] {
            assert!(DB_HEADER.contains(name), "{name}");
        }
        for name in [
            "namespace swofty::storage {",
            "Result read(",
            "Result write(",
            "Result append(",
            "Result remove(",
            "Result list(",
        ] {
            assert!(STORAGE_HEADER.contains(name), "{name}");
        }
    }
}
