#pragma once
#include <swofty/game/base.hpp>
#include <swofty/game/vm.hpp>

namespace swofty::game {

struct Action {
    int32_t type; // L2 experimental
    uint8_t _pad_0x4[0x4];
    NeStr *str; // L2 experimental
};
static_assert(offsetof(Action, type) == 0x0);
static_assert(offsetof(Action, str) == 0x8);

struct HomeItem {
    ItemVtbl *vtable; // L3 stable
    uint8_t _pad_0x8[0x8];
    NeStr *id; // L2 experimental
    VmPropHolder button_node; // L3 stable
};
static_assert(offsetof(HomeItem, vtable) == 0x0);
static_assert(offsetof(HomeItem, id) == 0x10);
static_assert(offsetof(HomeItem, button_node) == 0x18);

struct ItemVtbl {
    uint8_t _pad_0x0[0x20];
    void *populate; // L3 stable
    void *get_action; // L3 stable
};
static_assert(offsetof(ItemVtbl, populate) == 0x20);
static_assert(offsetof(ItemVtbl, get_action) == 0x28);

struct Tile {
    uint8_t _pad_0x0[0xd0];
    HomeItem *item; // L3 stable
};
static_assert(offsetof(Tile, item) == 0xd0);

struct TileEntry {
    uint8_t _pad_0x0[0x8];
    Tile *tile; // L3 stable
};
static_assert(offsetof(TileEntry, tile) == 0x8);

struct TileList {
    uint8_t _pad_0x0[0x78];
    TileEntry *items_begin; // L3 stable
    TileEntry *items_end; // L3 stable
};
static_assert(offsetof(TileList, items_begin) == 0x78);
static_assert(offsetof(TileList, items_end) == 0x80);

using FeTile_Build_List_t = void(TileList *self, void *ctx, void *a3, void *a4, uint8_t menu_kind, uint8_t sub_kind, uint32_t a7);
inline constexpr fn<"FeTile_Build_List", FeTile_Build_List_t> FeTile_Build_List{}; // L3 stable
using FeTile_ItemBase_Populate_t = void(HomeItem *item, void *node, uint32_t a7);
inline constexpr fn<"FeTile_ItemBase_Populate", FeTile_ItemBase_Populate_t> FeTile_ItemBase_Populate{}; // L3 stable

}
