#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    unused_imports
)]
use core::ffi::{c_char, c_int, c_uint, c_void};

pub const BUILD_SHA256: &str = "d873a1c8d2044cef2fd76c99c9d5a97649c5b815d055540995105f46619865ae";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Stable,
    Experimental,
}

#[derive(Clone, Copy, Debug)]
pub struct Binding {
    pub id: &'static str,
    pub kind: &'static str,
    pub status: Status,
}

#[repr(C)]
pub struct AiHandlingConfig {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct CarPerfVehicle {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct CarPhysicsRecord {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct CarWheelState {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct DepartmentData {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct EntrantRecord {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct EntrantSetupMode {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct EntrantsManager {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct FEHomeViewModel {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct GdbTableCursor {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct NeAllocator {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct NeAllocatorVtbl {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct NeStr {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct RnDDepartmentConfig {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct UpgradeEffectDef {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct UpgradeRecord {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct VehicleIdentity {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct VehiclePackageRow {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct Action {
    pub r#type: i32,
    _pad_0x4: [u8; 0x4],
    pub str: *mut NeStr,
}
const _: () = assert!(core::mem::offset_of!(Action, r#type) == 0x0);
const _: () = assert!(core::mem::offset_of!(Action, str) == 0x8);

#[repr(C)]
pub struct VmPropHolder {
    _pad_0x0: [u8; 0x8],
    pub node: *mut c_void,
}
const _: () = assert!(core::mem::offset_of!(VmPropHolder, node) == 0x8);

#[repr(C)]
pub struct HomeItem {
    pub vtable: *mut ItemVtbl,
    _pad_0x8: [u8; 0x8],
    pub id: *mut NeStr,
    pub button_node: VmPropHolder,
}
const _: () = assert!(core::mem::offset_of!(HomeItem, vtable) == 0x0);
const _: () = assert!(core::mem::offset_of!(HomeItem, id) == 0x10);
const _: () = assert!(core::mem::offset_of!(HomeItem, button_node) == 0x18);

#[repr(C)]
pub struct ItemVtbl {
    _pad_0x0: [u8; 0x20],
    pub populate: *mut c_void,
    pub get_action: *mut c_void,
}
const _: () = assert!(core::mem::offset_of!(ItemVtbl, populate) == 0x20);
const _: () = assert!(core::mem::offset_of!(ItemVtbl, get_action) == 0x28);

#[repr(C)]
pub struct NeLogGate {
    pub channel_hashes: [u32; 373],
    pub enabled: u8,
}
const _: () = assert!(core::mem::offset_of!(NeLogGate, channel_hashes) == 0x0);
const _: () = assert!(core::mem::offset_of!(NeLogGate, enabled) == 0x5d4);

#[repr(C)]
pub struct Tile {
    _pad_0x0: [u8; 0xd0],
    pub item: *mut HomeItem,
}
const _: () = assert!(core::mem::offset_of!(Tile, item) == 0xd0);

#[repr(C)]
pub struct TileEntry {
    _pad_0x0: [u8; 0x8],
    pub tile: *mut Tile,
}
const _: () = assert!(core::mem::offset_of!(TileEntry, tile) == 0x8);

#[repr(C)]
pub struct TileList {
    _pad_0x0: [u8; 0x78],
    pub items_begin: *mut TileEntry,
    pub items_end: *mut TileEntry,
}
const _: () = assert!(core::mem::offset_of!(TileList, items_begin) == 0x78);
const _: () = assert!(core::mem::offset_of!(TileList, items_end) == 0x80);

pub type Car_Reset_LastFittedUpgrade_t = unsafe extern "C" fn(inst: *mut c_void);
pub type Career_Add_TeamModifier_t = unsafe extern "C" fn(list: *mut c_void, key: u32, value: f32);
pub type Career_Calc_UpgradeCost_t =
    unsafe extern "C" fn(list: *mut c_void, inst: *mut c_void, mods: *mut c_void, option: u32) -> i32;
pub type Career_Construct_RnDDeptCard_t = unsafe extern "C" fn(
    card: *mut c_void,
    ctx: *mut c_void,
    builder: *mut c_void,
    allocator: *mut c_void,
    name: *const c_char,
    icon: *const c_char,
    dept: u8,
) -> *mut c_void;
pub type Career_Refresh_RnDDeptMode_t = unsafe extern "C" fn(holder: *mut usize, mode: u32);
pub type Career_Refresh_UpgradeAvailability_t = unsafe extern "C" fn(upgrades: *mut c_void, day: u16);
pub type FeHome_Add_CareerTile_t = unsafe extern "C" fn(list: *mut TileList, index: u8);
pub type FeHome_Handle_Command_t =
    unsafe extern "C" fn(vm: *mut FEHomeViewModel, cmd: *mut *mut NeStr) -> u64;
pub type FeTile_Build_List_t = unsafe extern "C" fn(
    self_: *mut TileList,
    ctx: *mut c_void,
    a3: *mut c_void,
    a4: *mut c_void,
    menu_kind: u8,
    sub_kind: u8,
    a7: u32,
);
pub type FeTile_ItemBase_Populate_t = unsafe extern "C" fn(item: *mut HomeItem, node: *mut c_void, a7: u32);
pub type Ne_Format_String_t = unsafe extern "C" fn(out: *mut *mut NeStr, fmt: *const c_char, ...) -> i64;
pub type Ne_Get_SettingBool_t =
    unsafe extern "C" fn(node: *mut c_void, name: *const c_char, out: *mut u8, def: u8) -> u64;
pub type Ne_Get_SettingU32_t =
    unsafe extern "C" fn(node: *mut c_void, name: *const c_char, out: *mut u32, def: u32) -> u64;
pub type Ne_Log_Thunk_t = unsafe extern "C" fn(channel: *const c_char, fmt: *const c_char, ...);
pub type Ne_Make_StrFromCStr_t =
    unsafe extern "C" fn(out: *mut *mut NeStr, s: *const c_char, flags: u32, len: usize) -> u32;
pub type Vm_Set_Bool_t = unsafe extern "C" fn(
    registry: *mut c_void,
    parent: *mut c_void,
    name: *const c_char,
    value: *mut u8,
    overwrite: c_int,
) -> *mut c_void;
pub type Vm_Set_String_t = unsafe extern "C" fn(
    registry: *mut c_void,
    parent: *mut c_void,
    name: *const c_char,
    value: *mut *mut NeStr,
    overwrite: c_int,
) -> *mut c_void;
pub type Vm_Set_TabEnabled_t = unsafe extern "C" fn(tabs: usize, node: usize, enabled: u32);

pub mod bindings {
    use super::{Binding, Status};
    pub const Car_Reset_LastFittedUpgrade: Binding = Binding {
        id: "Car_Reset_LastFittedUpgrade",
        kind: "func",
        status: Status::Experimental,
    };
    pub const Career_Add_TeamModifier: Binding = Binding {
        id: "Career_Add_TeamModifier",
        kind: "func",
        status: Status::Experimental,
    };
    pub const Career_Calc_UpgradeCost: Binding = Binding {
        id: "Career_Calc_UpgradeCost",
        kind: "func",
        status: Status::Experimental,
    };
    pub const Career_Construct_RnDDeptCard: Binding = Binding {
        id: "Career_Construct_RnDDeptCard",
        kind: "func",
        status: Status::Experimental,
    };
    pub const Career_Refresh_RnDDeptMode: Binding = Binding {
        id: "Career_Refresh_RnDDeptMode",
        kind: "func",
        status: Status::Experimental,
    };
    pub const Career_Refresh_UpgradeAvailability: Binding = Binding {
        id: "Career_Refresh_UpgradeAvailability",
        kind: "func",
        status: Status::Experimental,
    };
    pub const FeHome_Add_CareerTile: Binding = Binding {
        id: "FeHome_Add_CareerTile",
        kind: "func",
        status: Status::Stable,
    };
    pub const FeHome_Handle_Command: Binding = Binding {
        id: "FeHome_Handle_Command",
        kind: "func",
        status: Status::Stable,
    };
    pub const FeTile_Build_List: Binding = Binding {
        id: "FeTile_Build_List",
        kind: "func",
        status: Status::Stable,
    };
    pub const FeTile_ItemBase_Populate: Binding = Binding {
        id: "FeTile_ItemBase_Populate",
        kind: "func",
        status: Status::Stable,
    };
    pub const Ne_Format_String: Binding = Binding {
        id: "Ne_Format_String",
        kind: "func",
        status: Status::Experimental,
    };
    pub const Ne_Get_SettingBool: Binding = Binding {
        id: "Ne_Get_SettingBool",
        kind: "func",
        status: Status::Experimental,
    };
    pub const Ne_Get_SettingU32: Binding = Binding {
        id: "Ne_Get_SettingU32",
        kind: "func",
        status: Status::Experimental,
    };
    pub const Ne_Log_Thunk: Binding = Binding {
        id: "Ne_Log_Thunk",
        kind: "func",
        status: Status::Experimental,
    };
    pub const Ne_Make_StrFromCStr: Binding = Binding {
        id: "Ne_Make_StrFromCStr",
        kind: "func",
        status: Status::Stable,
    };
    pub const Vm_Set_Bool: Binding = Binding {
        id: "Vm_Set_Bool",
        kind: "func",
        status: Status::Experimental,
    };
    pub const Vm_Set_String: Binding = Binding {
        id: "Vm_Set_String",
        kind: "func",
        status: Status::Experimental,
    };
    pub const Vm_Set_TabEnabled: Binding = Binding {
        id: "Vm_Set_TabEnabled",
        kind: "func",
        status: Status::Experimental,
    };
    pub const g_Career_RnDOverviewSelectedDept: Binding = Binding {
        id: "g_Career_RnDOverviewSelectedDept",
        kind: "data",
        status: Status::Experimental,
    };
    pub const g_Ne_DefaultAllocator: Binding = Binding {
        id: "g_Ne_DefaultAllocator",
        kind: "data",
        status: Status::Experimental,
    };
    pub const g_Ne_LogGate: Binding = Binding {
        id: "g_Ne_LogGate",
        kind: "data",
        status: Status::Experimental,
    };
    pub const g_Vm_Registry: Binding = Binding {
        id: "g_Vm_Registry",
        kind: "data",
        status: Status::Stable,
    };
    pub const k_Career_RnDCardColourTable: Binding = Binding {
        id: "k_Career_RnDCardColourTable",
        kind: "data",
        status: Status::Experimental,
    };
    pub const k_Career_RnDDeptColourTable: Binding = Binding {
        id: "k_Career_RnDDeptColourTable",
        kind: "data",
        status: Status::Experimental,
    };
    pub const k_Career_RnDErrorDeptName: Binding = Binding {
        id: "k_Career_RnDErrorDeptName",
        kind: "data",
        status: Status::Experimental,
    };
    pub const k_Career_UpgradeBlockNameTable: Binding = Binding {
        id: "k_Career_UpgradeBlockNameTable",
        kind: "data",
        status: Status::Experimental,
    };
    pub const k_Career_UpgradeTypeOrder: Binding = Binding {
        id: "k_Career_UpgradeTypeOrder",
        kind: "data",
        status: Status::Experimental,
    };
    pub const site_Career_ApplyUpgradeBlockNameDefault: Binding = Binding {
        id: "site_Career_ApplyUpgradeBlockNameDefault",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_ApplyUpgradeBlockNameJoin: Binding = Binding {
        id: "site_Career_ApplyUpgradeBlockNameJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_ApplyUpgradeKeyCall: Binding = Binding {
        id: "site_Career_ApplyUpgradeKeyCall",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_ApplyUpgradeKeyFilter: Binding = Binding {
        id: "site_Career_ApplyUpgradeKeyFilter",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_DeserialiseUpgradesEnd: Binding = Binding {
        id: "site_Career_DeserialiseUpgradesEnd",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_LoadUpgradeRows_DeptPath: Binding = Binding {
        id: "site_Career_LoadUpgradeRows_DeptPath",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDBoxGridFirstTab: Binding = Binding {
        id: "site_Career_RnDBoxGridFirstTab",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDBoxGridNameClamp: Binding = Binding {
        id: "site_Career_RnDBoxGridNameClamp",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDBoxGridNameIndex: Binding = Binding {
        id: "site_Career_RnDBoxGridNameIndex",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDBoxGridNameRead: Binding = Binding {
        id: "site_Career_RnDBoxGridNameRead",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDCardArtImage0: Binding = Binding {
        id: "site_Career_RnDCardArtImage0",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDCardArtImage1: Binding = Binding {
        id: "site_Career_RnDCardArtImage1",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDCardArtImage2: Binding = Binding {
        id: "site_Career_RnDCardArtImage2",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDCardArtMask0: Binding = Binding {
        id: "site_Career_RnDCardArtMask0",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDCardArtMask1: Binding = Binding {
        id: "site_Career_RnDCardArtMask1",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDCardArtMask2: Binding = Binding {
        id: "site_Career_RnDCardArtMask2",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDCardColourPick0: Binding = Binding {
        id: "site_Career_RnDCardColourPick0",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDCardColourPick1: Binding = Binding {
        id: "site_Career_RnDCardColourPick1",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDCardColourPick2: Binding = Binding {
        id: "site_Career_RnDCardColourPick2",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDCardColourPick3: Binding = Binding {
        id: "site_Career_RnDCardColourPick3",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDCardColourPick4: Binding = Binding {
        id: "site_Career_RnDCardColourPick4",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDCardDeptIcon0: Binding = Binding {
        id: "site_Career_RnDCardDeptIcon0",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDCardDeptIcon1: Binding = Binding {
        id: "site_Career_RnDCardDeptIcon1",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDCardDeptIcon2: Binding = Binding {
        id: "site_Career_RnDCardDeptIcon2",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDCompareDept: Binding = Binding {
        id: "site_Career_RnDCompareDept",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDContainerOpenMaint: Binding = Binding {
        id: "site_Career_RnDContainerOpenMaint",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDContainerOpenTail: Binding = Binding {
        id: "site_Career_RnDContainerOpenTail",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDDeptBoxGridEntry: Binding = Binding {
        id: "site_Career_RnDDeptBoxGridEntry",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDDeptBoxGridInject: Binding = Binding {
        id: "site_Career_RnDDeptBoxGridInject",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDDeptBoxLayoutProbe: Binding = Binding {
        id: "site_Career_RnDDeptBoxLayoutProbe",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDDeptPagePreQuery: Binding = Binding {
        id: "site_Career_RnDDeptPagePreQuery",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDDeptPageQueryDone: Binding = Binding {
        id: "site_Career_RnDDeptPageQueryDone",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDDeptPageTypeLoop: Binding = Binding {
        id: "site_Career_RnDDeptPageTypeLoop",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDDeptRowCategoryIcon: Binding = Binding {
        id: "site_Career_RnDDeptRowCategoryIcon",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDDeptRowTypeIcon: Binding = Binding {
        id: "site_Career_RnDDeptRowTypeIcon",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDDeptRowTypeIconJoin: Binding = Binding {
        id: "site_Career_RnDDeptRowTypeIconJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDDeptRowTypeLabel: Binding = Binding {
        id: "site_Career_RnDDeptRowTypeLabel",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDDeptRowTypeLabelJoin: Binding = Binding {
        id: "site_Career_RnDDeptRowTypeLabelJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDInProgressColourStore: Binding = Binding {
        id: "site_Career_RnDInProgressColourStore",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDInProgressPartIcon: Binding = Binding {
        id: "site_Career_RnDInProgressPartIcon",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDInProgressStrip: Binding = Binding {
        id: "site_Career_RnDInProgressStrip",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDInProgressTypeIcon: Binding = Binding {
        id: "site_Career_RnDInProgressTypeIcon",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDInProgressTypeIconJoin: Binding = Binding {
        id: "site_Career_RnDInProgressTypeIconJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDInProgressTypeLabel: Binding = Binding {
        id: "site_Career_RnDInProgressTypeLabel",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDInProgressTypeLabelJoin: Binding = Binding {
        id: "site_Career_RnDInProgressTypeLabelJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDListRowColourStore: Binding = Binding {
        id: "site_Career_RnDListRowColourStore",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDListRowDeptIcon: Binding = Binding {
        id: "site_Career_RnDListRowDeptIcon",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDListRowLightColourStore: Binding = Binding {
        id: "site_Career_RnDListRowLightColourStore",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPageModifierList: Binding = Binding {
        id: "site_Career_RnDPageModifierList",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPanelModifierList: Binding = Binding {
        id: "site_Career_RnDPanelModifierList",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPanelResearchLimit: Binding = Binding {
        id: "site_Career_RnDPanelResearchLimit",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPerfChildName: Binding = Binding {
        id: "site_Career_RnDPerfChildName",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPerfDeptColour: Binding = Binding {
        id: "site_Career_RnDPerfDeptColour",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPerfDeptLoopEnd: Binding = Binding {
        id: "site_Career_RnDPerfDeptLoopEnd",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPerfDeptName: Binding = Binding {
        id: "site_Career_RnDPerfDeptName",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPerfGotoName: Binding = Binding {
        id: "site_Career_RnDPerfGotoName",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPerfViewModelSlotDtor: Binding = Binding {
        id: "site_Career_RnDPerfViewModelSlotDtor",
        kind: "call",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPerksTitleDeptIcon: Binding = Binding {
        id: "site_Career_RnDPerksTitleDeptIcon",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPieBufSize: Binding = Binding {
        id: "site_Career_RnDPieBufSize",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPieCtorRet: Binding = Binding {
        id: "site_Career_RnDPieCtorRet",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPieFocusJoin: Binding = Binding {
        id: "site_Career_RnDPieFocusJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPieFocusNameCmp: Binding = Binding {
        id: "site_Career_RnDPieFocusNameCmp",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPieFocusRestore: Binding = Binding {
        id: "site_Career_RnDPieFocusRestore",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPieMoveAssignSlotDtor: Binding = Binding {
        id: "site_Career_RnDPieMoveAssignSlotDtor",
        kind: "call",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPieMovedFromSlotDtor: Binding = Binding {
        id: "site_Career_RnDPieMovedFromSlotDtor",
        kind: "call",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPiePopulateDeptGuard: Binding = Binding {
        id: "site_Career_RnDPiePopulateDeptGuard",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPiePopulateGate: Binding = Binding {
        id: "site_Career_RnDPiePopulateGate",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPiePopulateNextCode: Binding = Binding {
        id: "site_Career_RnDPiePopulateNextCode",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPiePopulateSlotCap: Binding = Binding {
        id: "site_Career_RnDPiePopulateSlotCap",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPieSliceDefaultColour: Binding = Binding {
        id: "site_Career_RnDPieSliceDefaultColour",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPieSliceDescFormat: Binding = Binding {
        id: "site_Career_RnDPieSliceDescFormat",
        kind: "call",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPieSliceIconArgs: Binding = Binding {
        id: "site_Career_RnDPieSliceIconArgs",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPieSliceNameJoin: Binding = Binding {
        id: "site_Career_RnDPieSliceNameJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPieTeardownSlotDtor: Binding = Binding {
        id: "site_Career_RnDPieTeardownSlotDtor",
        kind: "call",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPopupCategoryIcon: Binding = Binding {
        id: "site_Career_RnDPopupCategoryIcon",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPopupColourStore: Binding = Binding {
        id: "site_Career_RnDPopupColourStore",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPotentialPieEntry: Binding = Binding {
        id: "site_Career_RnDPotentialPieEntry",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPurchaseBackOpen: Binding = Binding {
        id: "site_Career_RnDPurchaseBackOpen",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPurchaseOptionsEntry: Binding = Binding {
        id: "site_Career_RnDPurchaseOptionsEntry",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPurchasePanelTypeLabel: Binding = Binding {
        id: "site_Career_RnDPurchasePanelTypeLabel",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPurchasePanelTypeLabelJoin: Binding = Binding {
        id: "site_Career_RnDPurchasePanelTypeLabelJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPurchaseRequestEntry: Binding = Binding {
        id: "site_Career_RnDPurchaseRequestEntry",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPurchaseScreenTypeLabel: Binding = Binding {
        id: "site_Career_RnDPurchaseScreenTypeLabel",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDPurchaseScreenTypeLabelJoin: Binding = Binding {
        id: "site_Career_RnDPurchaseScreenTypeLabelJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDResearchFocusNameCmp: Binding = Binding {
        id: "site_Career_RnDResearchFocusNameCmp",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDResearchRowCategoryIcon: Binding = Binding {
        id: "site_Career_RnDResearchRowCategoryIcon",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDResearchRowTypeIcon: Binding = Binding {
        id: "site_Career_RnDResearchRowTypeIcon",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDResearchRowTypeIconJoin: Binding = Binding {
        id: "site_Career_RnDResearchRowTypeIconJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDResearchRowTypeLabel: Binding = Binding {
        id: "site_Career_RnDResearchRowTypeLabel",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDResearchRowTypeLabelJoin: Binding = Binding {
        id: "site_Career_RnDResearchRowTypeLabelJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDResearchTypeLoop: Binding = Binding {
        id: "site_Career_RnDResearchTypeLoop",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDResultTypeIcon: Binding = Binding {
        id: "site_Career_RnDResultTypeIcon",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDResultTypeIconJoin: Binding = Binding {
        id: "site_Career_RnDResultTypeIconJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDResultTypeLabel: Binding = Binding {
        id: "site_Career_RnDResultTypeLabel",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDResultTypeLabelJoin: Binding = Binding {
        id: "site_Career_RnDResultTypeLabelJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDTileCountBody: Binding = Binding {
        id: "site_Career_RnDTileCountBody",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDTileCountExit: Binding = Binding {
        id: "site_Career_RnDTileCountExit",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDTileCountGuards: Binding = Binding {
        id: "site_Career_RnDTileCountGuards",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDTileCountIndexGuard: Binding = Binding {
        id: "site_Career_RnDTileCountIndexGuard",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDTodoTypeLabel: Binding = Binding {
        id: "site_Career_RnDTodoTypeLabel",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDTodoTypeLabelJoin: Binding = Binding {
        id: "site_Career_RnDTodoTypeLabelJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDTreeRowCountProbe: Binding = Binding {
        id: "site_Career_RnDTreeRowCountProbe",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDUpgradeInfoTypeLabel: Binding = Binding {
        id: "site_Career_RnDUpgradeInfoTypeLabel",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDUpgradeInfoTypeLabelJoin: Binding = Binding {
        id: "site_Career_RnDUpgradeInfoTypeLabelJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_RnDUpgradeListDeptIcon: Binding = Binding {
        id: "site_Career_RnDUpgradeListDeptIcon",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_TeamDeptEnumStore: Binding = Binding {
        id: "site_Career_TeamDeptEnumStore",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_TeamHqRnDPopupTypeIcon: Binding = Binding {
        id: "site_Career_TeamHqRnDPopupTypeIcon",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_TeamHqRnDPopupTypeIconJoin: Binding = Binding {
        id: "site_Career_TeamHqRnDPopupTypeIconJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_TeleFailedTypeName: Binding = Binding {
        id: "site_Career_TeleFailedTypeName",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_TeleFailedTypeNameJoin: Binding = Binding {
        id: "site_Career_TeleFailedTypeNameJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_TeleRecordTypeName: Binding = Binding {
        id: "site_Career_TeleRecordTypeName",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_TeleRecordTypeNameCall: Binding = Binding {
        id: "site_Career_TeleRecordTypeNameCall",
        kind: "call",
        status: Status::Experimental,
    };
    pub const site_Career_TeleResearchTypeName: Binding = Binding {
        id: "site_Career_TeleResearchTypeName",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_TeleResearchTypeNameCall: Binding = Binding {
        id: "site_Career_TeleResearchTypeNameCall",
        kind: "call",
        status: Status::Experimental,
    };
    pub const site_Career_Unk141814320TypeLabel: Binding = Binding {
        id: "site_Career_Unk141814320TypeLabel",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_Unk141814320TypeLabelJoin: Binding = Binding {
        id: "site_Career_Unk141814320TypeLabelJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_Unk1418169f0TypeLabel: Binding = Binding {
        id: "site_Career_Unk1418169f0TypeLabel",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_Unk1418169f0TypeLabelJoin: Binding = Binding {
        id: "site_Career_Unk1418169f0TypeLabelJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeAutoFitBody: Binding = Binding {
        id: "site_Career_UpgradeAutoFitBody",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeAutoFitDeptGate: Binding = Binding {
        id: "site_Career_UpgradeAutoFitDeptGate",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeBlockReplayEntry: Binding = Binding {
        id: "site_Career_UpgradeBlockReplayEntry",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeCardDeptIcon: Binding = Binding {
        id: "site_Career_UpgradeCardDeptIcon",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeCardTypeLabel: Binding = Binding {
        id: "site_Career_UpgradeCardTypeLabel",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeCardTypeLabelJoin: Binding = Binding {
        id: "site_Career_UpgradeCardTypeLabelJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeGraphHubTest: Binding = Binding {
        id: "site_Career_UpgradeGraphHubTest",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeGraphRegularNode: Binding = Binding {
        id: "site_Career_UpgradeGraphRegularNode",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeInitFakeDeptBranch: Binding = Binding {
        id: "site_Career_UpgradeInitFakeDeptBranch",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeInitNormal: Binding = Binding {
        id: "site_Career_UpgradeInitNormal",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeMetaTypeJoin: Binding = Binding {
        id: "site_Career_UpgradeMetaTypeJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradePushStateBody: Binding = Binding {
        id: "site_Career_UpgradePushStateBody",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradePushStateDeptGate: Binding = Binding {
        id: "site_Career_UpgradePushStateDeptGate",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeRowTypeJoin: Binding = Binding {
        id: "site_Career_UpgradeRowTypeJoin",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeSetEnabledBody: Binding = Binding {
        id: "site_Career_UpgradeSetEnabledBody",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeSetEnabledDeptGate: Binding = Binding {
        id: "site_Career_UpgradeSetEnabledDeptGate",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeSetFittedBody: Binding = Binding {
        id: "site_Career_UpgradeSetFittedBody",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeSetFittedDeptGate: Binding = Binding {
        id: "site_Career_UpgradeSetFittedDeptGate",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeSetLockedBody: Binding = Binding {
        id: "site_Career_UpgradeSetLockedBody",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_UpgradeSetLockedDeptGate: Binding = Binding {
        id: "site_Career_UpgradeSetLockedDeptGate",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_VehicleFillOrderEnd: Binding = Binding {
        id: "site_Career_VehicleFillOrderEnd",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_VehicleFillOrderStart: Binding = Binding {
        id: "site_Career_VehicleFillOrderStart",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_WeekendObjectiveNext: Binding = Binding {
        id: "site_Career_WeekendObjectiveNext",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Career_WeekendObjectiveType: Binding = Binding {
        id: "site_Career_WeekendObjectiveType",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_FeHome_AfterCareerCall: Binding = Binding {
        id: "site_FeHome_AfterCareerCall",
        kind: "site",
        status: Status::Stable,
    };
    pub const site_Gdb_DeptCountStore: Binding = Binding {
        id: "site_Gdb_DeptCountStore",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Ne_Format_String_Entry: Binding = Binding {
        id: "site_Ne_Format_String_Entry",
        kind: "site",
        status: Status::Experimental,
    };
    pub const site_Ne_HwConfigParseCall: Binding = Binding {
        id: "site_Ne_HwConfigParseCall",
        kind: "call",
        status: Status::Experimental,
    };
    pub const site_Ne_HwXmlParseCall: Binding = Binding {
        id: "site_Ne_HwXmlParseCall",
        kind: "call",
        status: Status::Experimental,
    };
    pub const site_Ne_HwXmlWriteCall: Binding = Binding {
        id: "site_Ne_HwXmlWriteCall",
        kind: "call",
        status: Status::Experimental,
    };
    pub const site_Sess_AiVehicleRegisterCall: Binding = Binding {
        id: "site_Sess_AiVehicleRegisterCall",
        kind: "call",
        status: Status::Experimental,
    };
    pub const vt_Career_RnDDeptCard: Binding = Binding {
        id: "vt_Career_RnDDeptCard",
        kind: "vtable",
        status: Status::Experimental,
    };
}
