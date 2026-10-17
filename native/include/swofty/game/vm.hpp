#pragma once
#include <swofty/game/base.hpp>

namespace swofty::game {

struct VmPropHolder {
    uint8_t _pad_0x0[0x8];
    void *node; // L3 stable
};
static_assert(offsetof(VmPropHolder, node) == 0x8);

using Vm_Set_Bool_t = void *(void *registry, void *parent, const char *name, uint8_t *value, int overwrite);
inline constexpr fn<"Vm_Set_Bool", Vm_Set_Bool_t> Vm_Set_Bool{}; // L2 experimental
using Vm_Set_String_t = void *(void *registry, void *parent, const char *name, NeStr **value, int overwrite);
inline constexpr fn<"Vm_Set_String", Vm_Set_String_t> Vm_Set_String{}; // L2 experimental
inline constexpr data<"g_Vm_Registry", void *> g_Vm_Registry{}; // L3 stable

}
