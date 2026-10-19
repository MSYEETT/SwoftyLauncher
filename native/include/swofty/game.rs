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

pub mod bindings {
    use super::{Binding, Status};
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
    pub const site_FeHome_AfterCareerCall: Binding = Binding {
        id: "site_FeHome_AfterCareerCall",
        kind: "site",
        status: Status::Stable,
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
}
