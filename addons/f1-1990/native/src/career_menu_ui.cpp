// SPDX-License-Identifier: AGPL-3.0-or-later
//
// F1 1990 - a third "1990 Career" tile on the career content select menu, next to the stock 2025
// and 2026 tiles. Its own DLL, so a binding it can't resolve drops only this hook.
//
// FeTile_Build_List case 6 builds the 2025 and 2026 content tiles inline; at its end
// (site_FeTile_CareerContentEnd, r14 = the TileList) FeHome_Add_CareerTile appends a third tile
// (button_3, template fe_tile_button_default, its own VM node and list entry). Its 0x148-byte home
// item is replaced, the way case 6 replaces items, by a career content item built like the 2026 one
// (FeTile_Construct_CareerItem, 0x1c8 bytes) with the 2026 tile's route (+0x178, driver_career), data group
// f1_career_1990, suffix 90 and a copy of the 2026 vtable whose populate shows the 1990 labels with
// no badge and whose press selects the 1990 car group (or the 2026 one) before the router. The
// populate loop after the site then initialises and populates all three tiles. Only when the 1990
// career content is in the database; option career_menu_ui turns it off.

#include <atomic>
#include <cstring>

#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include "content_entry.hpp"
#include "profile.hpp"
#include "weekend.hpp"

SWOFTY_SETTING(bool, career_menu_ui, true);

namespace {

using f1990::at;
using swofty::game::Action;
using swofty::game::HomeItem;
using swofty::game::NeAllocator;
using swofty::game::NeStr;
using swofty::game::Tile;
using swofty::game::TileList;

constexpr uint8_t kIndex = 3;
constexpr size_t kItemSize = 0x1c8;
constexpr size_t kSlots = 22;
constexpr size_t kPopulateSlot = 4;
constexpr size_t kPressSlot = 5;
constexpr size_t kNotificationSlot = 0x78 / 8;
constexpr uint16_t kCarGroup1990 = 2;
constexpr char kTitle[] = "lng_f1990_career_content";
constexpr char kDescription[] = "lng_f1990_career_content_description";

void **g_vtable;
std::atomic<uint32_t> g_builds, g_presses;

void set_text(void *node, const char *name, const char *text)
{
    NeStr *s = nullptr;
    swofty::game::Ne_Make_StrFromCStr(&s, text, 0, 0);
    swofty::game::Vm_Set_String(*swofty::game::g_Vm_Registry, node, name, &s, 1);
    swofty::game::Ne_Make_StrFromCStr(&s, nullptr, 0, 0);
}

void populate(HomeItem *item, void *node, uint32_t a7)
{
    swofty::game::FeTile_CareerDlcItem_Populate.get()(item, node, a7);
    auto *vm = static_cast<uint8_t *>(item->button_node.node);
    if (!vm)
        return;
    set_text(vm + 8, "title", kTitle);
    set_text(vm + 8, "description", kDescription);
    auto **vt = *reinterpret_cast<void ***>(item);
    reinterpret_cast<void (*)(HomeItem *, int)>(vt[kNotificationSlot])(item, 0);
}

Action *press(HomeItem *item, Action *out)
{
    auto *system = static_cast<const uint8_t *>(*swofty::game::g_FeTile_CustomisationSystem);
    auto *b = system ? at<const uint8_t *>(system, 0x230) : nullptr;
    auto *e = system ? at<const uint8_t *>(system, 0x238) : nullptr;
    f1990::ContentPick pick = f1990::pick_content(b, e, kCarGroup1990);
    *swofty::game::g_FeTile_SelectedCareerContent = pick.id;
    if (g_presses++ < 4)
        swofty::log("[f1-1990] career menu: 1990 Career pressed; content 0x%08x (%s)", pick.id,
                    pick.kind == f1990::ContentPick::exact ? "1990 car group"
                    : pick.kind == f1990::ContentPick::dlc ? "no 1990 car group, the 2026 one"
                                                           : "no content entry");
    return swofty::game::FeTile_Route_CareerItem(item, out);
}

void set_string(void *field, const char *text)
{
    swofty::game::Ne_Make_StrFromCStr(static_cast<NeStr **>(field), text, 0, 0);
}

HomeItem *build_item(NeAllocator *alloc, HomeItem *dlc)
{
    auto alloc_fn = reinterpret_cast<void *(*)(NeAllocator *, size_t, size_t)>(alloc->vtable->alloc);
    auto *item = static_cast<HomeItem *>(alloc_fn(alloc, kItemSize, 0x10));
    if (!item)
        return nullptr;
    const char *id = dlc->id ? dlc->id->chars : "career_dlc_content";
    swofty::game::FeTile_Construct_CareerItem(item, alloc, id);
    auto *raw = reinterpret_cast<uint8_t *>(item);
    auto *route = at<const uint8_t *>(dlc, 0x178);
    set_string(raw + 0x178, route && route[9] ? reinterpret_cast<const char *>(route + 9) : "driver_career");
    std::memset(raw + 0x1b8, 0, 0x10);
    set_string(raw + 0x1c0, "f1_2026");
    set_string(raw + 0x180, "f1_career_1990");
    set_string(raw + 0x188, "90");
    raw[0x190] = 0;
    if (!g_vtable) {
        auto **src = *reinterpret_cast<void ***>(dlc);
        auto **copy = new void *[kSlots + 1];
        std::memcpy(copy, src - 1, (kSlots + 1) * sizeof(void *));
        copy[1 + kPopulateSlot] = reinterpret_cast<void *>(&populate);
        copy[1 + kPressSlot] = reinterpret_cast<void *>(&press);
        g_vtable = copy + 1;
    }
    *reinterpret_cast<void ***>(item) = g_vtable;
    return item;
}

bool is_dlc_item(const HomeItem *item)
{
    auto *const *vt = item ? *reinterpret_cast<void *const *const *>(item) : nullptr;
    return vt && vt[kPopulateSlot] == reinterpret_cast<void *>(swofty::game::FeTile_CareerDlcItem_Populate.address());
}

void add_tile(TileList *list)
{
    if (!list || !list->items_begin || list->items_end <= list->items_begin)
        return;
    Tile *last = list->items_end[-1].tile;
    HomeItem *dlc = last ? last->item : nullptr;
    if (!is_dlc_item(dlc)) {
        swofty::log("[f1-1990] career menu: the content list does not end with the 2026 tile; no 1990 tile");
        return;
    }
    size_t before = size_t(list->items_end - list->items_begin);
    swofty::game::FeHome_Add_CareerTile(list, kIndex);
    size_t after = list->items_begin ? size_t(list->items_end - list->items_begin) : 0;
    Tile *tile = after == before + 1 ? list->items_end[-1].tile : nullptr;
    if (!tile || !tile->item || !tile->allocator) {
        swofty::log("[f1-1990] career menu: the tile adder gave %zu -> %zu items; no 1990 tile", before, after);
        return;
    }
    NeAllocator *alloc = at<NeAllocator *>(list, 0x08);
    HomeItem *item = alloc ? build_item(alloc, dlc) : nullptr;
    if (!item) {
        swofty::log("[f1-1990] career menu: could not build the 1990 content item; the third tile stays a Career tile");
        return;
    }
    HomeItem *old = tile->item;
    NeAllocator *old_alloc = tile->allocator;
    tile->item = item;
    tile->allocator = alloc;
    reinterpret_cast<void (*)(HomeItem *, int)>(old->vtable->destroy)(old, 0);
    reinterpret_cast<void (*)(NeAllocator *, void *)>(old_alloc->vtable->free)(old_alloc, old);
    if (g_builds++ < 4)
        swofty::log("[f1-1990] career menu: 1990 Career tile added as button_%u (items %zu -> %zu)", kIndex, before,
                    after);
}

}

SWOFTY_SITE(site_FeTile_CareerContentEnd, swofty::Context &c)
{
    if (!career_menu_ui || f1990::has_1990_content("career menu") != 1)
        return;
    add_tile(reinterpret_cast<TileList *>(c.r14));
}

SWOFTY_ON_LOAD
{
    swofty::log("[f1-1990] career menu UI loaded: 1990 Career tile %s", career_menu_ui ? "on" : "off (career_menu_ui)");
}

SWOFTY_ADDON()
