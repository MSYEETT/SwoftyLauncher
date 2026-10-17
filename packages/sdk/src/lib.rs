use std::ffi::{c_char, c_int, c_void};

pub const SWOFTY_API_VERSION: u32 = 4;
pub const INIT_SYMBOLS: [&str; 2] = ["SwoftyAddonInit", "SwoftyModInit"];
pub const API_SYMBOLS: [&str; 2] = ["SwoftyAddonApi", "SwoftyModApi"];
pub const HEADER: &str = include_str!("../include/swofty_api.h");
pub const MIXIN_HEADER: &str = include_str!("../include/swofty/mixin.hpp");
pub const ASSETS_HEADER: &str = include_str!("../include/swofty/assets.hpp");

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
}

impl Capability {
    pub const ALL: [Capability; 7] = [
        Self::Log,
        Self::Scan,
        Self::Hook,
        Self::ReadDb,
        Self::Options,
        Self::OnFrame,
        Self::Assets,
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
            Self::ReadDb | Self::OnFrame => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_header() {
        assert_eq!(std::mem::size_of::<SwoftyApi>(), 80);
        assert_eq!(std::mem::offset_of!(SwoftyApi, assets), 72);
        assert_eq!(std::mem::size_of::<SwoftyAssets>(), 64);
        assert_eq!(std::mem::offset_of!(SwoftyAssets, db_set_raw), 32);
        assert_eq!(std::mem::offset_of!(SwoftyAssets, overlay_file), 56);
        assert_eq!(std::mem::size_of::<SwoftyFragment>(), 24);
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
    }

    #[test]
    fn capability_names_round_trip() {
        for c in Capability::ALL {
            assert_eq!(Capability::parse(c.as_str()), Some(c));
        }
        assert!(Capability::Assets.available_in(4));
        assert!(!Capability::Assets.available_in(3));
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
}
