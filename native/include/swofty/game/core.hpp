#pragma once
#include <swofty/game/base.hpp>

namespace swofty::game {

struct NeAllocator {
    NeAllocatorVtbl *vtable; // L2 experimental
};
static_assert(offsetof(NeAllocator, vtable) == 0x0);

struct NeAllocatorVtbl {
    uint8_t _pad_0x0[0x18];
    void *alloc; // L2 experimental
    uint8_t _pad_0x20[0x18];
    void *free; // L2 experimental
};
static_assert(offsetof(NeAllocatorVtbl, alloc) == 0x18);
static_assert(offsetof(NeAllocatorVtbl, free) == 0x38);

struct NeLogGate {
    uint32_t channel_hashes[373]; // L2 experimental
    uint8_t enabled; // L2 experimental
};
static_assert(offsetof(NeLogGate, channel_hashes) == 0x0);
static_assert(offsetof(NeLogGate, enabled) == 0x5d4);

struct NeMessageBus {
    uint8_t _pad_0x0[0x68];
    uint8_t active; // L2 experimental
};
static_assert(offsetof(NeMessageBus, active) == 0x68);

struct NeOptionalVec {
    NeAllocator *alloc; // L2 experimental
    void *begin; // L2 experimental
    void *end; // L2 experimental
    uint64_t count; // L2 experimental
    uint8_t engaged; // L2 experimental
    uint8_t _pad_0x21[0x7];
};
static_assert(offsetof(NeOptionalVec, alloc) == 0x0);
static_assert(offsetof(NeOptionalVec, begin) == 0x8);
static_assert(offsetof(NeOptionalVec, end) == 0x10);
static_assert(offsetof(NeOptionalVec, count) == 0x18);
static_assert(offsetof(NeOptionalVec, engaged) == 0x20);
static_assert(sizeof(NeOptionalVec) == 0x28);

struct NeSharedCtrl {
    void **vtable; // L2 experimental
    int32_t strong; // L2 experimental
    int32_t weak; // L2 experimental
};
static_assert(offsetof(NeSharedCtrl, vtable) == 0x0);
static_assert(offsetof(NeSharedCtrl, strong) == 0x8);
static_assert(offsetof(NeSharedCtrl, weak) == 0xc);

using Ne_Detach_ChildNode_t = int(void *parent, void *child);
inline constexpr fn<"Ne_Detach_ChildNode", Ne_Detach_ChildNode_t> Ne_Detach_ChildNode{}; // L2 experimental
using Ne_Format_String_t = int64_t(NeStr **out, const char *fmt, ...);
inline constexpr fn<"Ne_Format_String", Ne_Format_String_t> Ne_Format_String{}; // L2 experimental
using Ne_Get_SettingBool_t = uint64_t(void *node, const char *name, uint8_t *out, uint8_t def);
inline constexpr fn<"Ne_Get_SettingBool", Ne_Get_SettingBool_t> Ne_Get_SettingBool{}; // L2 experimental
using Ne_Get_SettingU32_t = uint64_t(void *node, const char *name, uint32_t *out, uint32_t def);
inline constexpr fn<"Ne_Get_SettingU32", Ne_Get_SettingU32_t> Ne_Get_SettingU32{}; // L2 experimental
using Ne_Log_Thunk_t = void(const char *channel, const char *fmt, ...);
inline constexpr fn<"Ne_Log_Thunk", Ne_Log_Thunk_t> Ne_Log_Thunk{}; // L2 experimental
using Ne_Make_StrFromCStr_t = uint32_t(NeStr **out, const char *s, uint32_t flags, uintptr_t len);
inline constexpr fn<"Ne_Make_StrFromCStr", Ne_Make_StrFromCStr_t> Ne_Make_StrFromCStr{}; // L3 stable
using Ne_Post_BusMessage_t = uint64_t(void *bus, void *msg);
inline constexpr fn<"Ne_Post_BusMessage", Ne_Post_BusMessage_t> Ne_Post_BusMessage{}; // L2 experimental
inline constexpr data<"g_Ne_DefaultAllocator", void *> g_Ne_DefaultAllocator{}; // L2 experimental
inline constexpr data<"g_Ne_LogGate", NeLogGate> g_Ne_LogGate{}; // L2 experimental
inline constexpr site<"site_Ne_Format_String_Entry"> site_Ne_Format_String_Entry{}; // L2 experimental
inline constexpr call<"site_Ne_HwConfigParseCall"> site_Ne_HwConfigParseCall{}; // L2 experimental
inline constexpr call<"site_Ne_HwXmlParseCall"> site_Ne_HwXmlParseCall{}; // L2 experimental
inline constexpr call<"site_Ne_HwXmlWriteCall"> site_Ne_HwXmlWriteCall{}; // L2 experimental

}
