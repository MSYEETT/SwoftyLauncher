// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Classic Seasons - one Grand Prix / Time Trial category box per season.
//
// The DynamicEraSelectTabViewModel builds its boxes from a fixed list in FeTile_Build_EraTiles, so a season's
// game_setup.scenarios row never gets a box from the database alone. At site_FeTile_EraTilesBuilt, after the last
// stock box, this adds one box per season whose entrant list the screen's mode offers under the season's own setup
// mode (FeTile_Is_EraScenarioAvailable_Thunk, the check the stock boxes use). A box's command is its entrant list,
// which the press handler raises as entrant_list_selected, the same way it does for the stock F1 2025 box.
//
// All seasons share one setup mode, cs_f1, whose entrant setup mode lists every season: one more setup mode is all
// the setup refresh's fixed-size mode list holds. Time Trial loads every car of every list the entrant setup mode
// offers, and the Grand Prix takes its first list, so when a season box is pressed
// (Game_F1WorldActivityContainer_SelectEntrantList) the entrant setup mode offers only that season's list until another
// box is pressed or the boxes are built again.

#include <cstdint>
#include <cstdio>
#include <cstring>
#include <iterator>

#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include "seasons.hpp"

namespace {

using namespace swofty::game;

constexpr const char *kImage = "#overrides.home_screen.tiles_solo_6x6[f1]";
constexpr size_t kTiles = 0x690;
constexpr size_t kAllocator = 0x270;
constexpr size_t kMode = 0x670;
constexpr size_t kTileBegin = 0x6d8;
constexpr size_t kTileEnd = 0x6e0;
constexpr size_t kTileEntry = 0x10;

template <class T> T at(void *base, size_t off)
{
    return *reinterpret_cast<T *>(static_cast<uint8_t *>(base) + off);
}

constexpr size_t kContainerMode = 0x2f0;
constexpr size_t kModeSetup = 0x88;
constexpr size_t kSetupModes = 0x260;
constexpr size_t kSetupModesVfunc = 0x1d0;
constexpr size_t kSetupModeName = 0xd8;
constexpr size_t kEntrantSetups = 0x10;
constexpr size_t kEntrantSetupCount = 0x70;
constexpr size_t kLists = 0x130;
constexpr size_t kListCount = 0x230;
constexpr size_t kListSlots = 32;
constexpr size_t kListName = 0x28;
constexpr int64_t kEntrantSetupSlots = 12;

struct Narrowed {
    uint8_t *setup;
    void *hit;
    int64_t count;
    void *lists[kListSlots];
};

Narrowed g_narrowed[4];
size_t g_narrowed_count;
int g_logged;
int g_narrow_logged;

const char *name_of(void *object, size_t off)
{
    auto *holder = object ? at<uint8_t *>(object, off) : nullptr;
    return holder ? at<const char *>(holder, 0x10) : nullptr;
}

template <class F> void for_each_classic_setup(void *mode, F f)
{
    auto *owner = mode ? at<uint8_t *>(mode, kModeSetup) : nullptr;
    auto *modes = owner ? at<uint8_t *>(owner, kSetupModes) : nullptr;
    if (!modes)
        return;
    auto get = reinterpret_cast<uint8_t *(*)(void *)>((*reinterpret_cast<void ***>(modes))[kSetupModesVfunc / 8]);
    uint8_t *vec = get(modes);
    if (!vec)
        return;
    for (auto **p = at<uint8_t **>(vec, 8), **e = at<uint8_t **>(vec, 0x10); p != e; ++p) {
        const char *name = name_of(*p, kSetupModeName);
        if (!name || std::strcmp(name, classic_seasons::kSetupMode))
            continue;
        int64_t n = at<int64_t>(*p, kEntrantSetupCount);
        auto **setups = reinterpret_cast<uint8_t **>(*p + kEntrantSetups);
        for (int64_t i = 0; i < n && i < kEntrantSetupSlots; ++i)
            f(setups[i]);
    }
}

void restore()
{
    for (size_t i = 0; i < g_narrowed_count; ++i) {
        Narrowed &n = g_narrowed[i];
        auto **lists = reinterpret_cast<void **>(n.setup + kLists);
        if (at<int64_t>(n.setup, kListCount) != 1 || lists[0] != n.hit)
            continue;
        std::memcpy(n.setup + kLists, n.lists, size_t(n.count) * sizeof(void *));
        *reinterpret_cast<int64_t *>(n.setup + kListCount) = n.count;
    }
    g_narrowed_count = 0;
}

void narrow(void *mode, const char *list)
{
    restore();
    for_each_classic_setup(mode, [&](uint8_t *setup) {
        int64_t count = at<int64_t>(setup, kListCount);
        if (count <= 1 || count > int64_t(kListSlots) || g_narrowed_count == std::size(g_narrowed))
            return;
        auto **lists = reinterpret_cast<void **>(setup + kLists);
        void *hit = nullptr;
        for (int64_t i = 0; i < count && !hit; ++i) {
            const char *name = name_of(lists[i], kListName);
            if (name && !std::strcmp(name, list))
                hit = lists[i];
        }
        if (!hit)
            return;
        Narrowed &n = g_narrowed[g_narrowed_count++];
        n.setup = setup;
        n.hit = hit;
        n.count = count;
        std::memcpy(n.lists, lists, size_t(count) * sizeof(void *));
        lists[0] = hit;
        *reinterpret_cast<int64_t *>(setup + kListCount) = 1;
        if (g_narrow_logged++ < 12)
            swofty::log("[classic-seasons] %s offers only %s (of %lld lists)", classic_seasons::kSetupMode, list,
                        (long long)count);
    });
}

}

SWOFTY_SITE(site_FeTile_EraTilesBuilt, swofty::Context &c)
{
    auto *vm = reinterpret_cast<uint8_t *>(c.rsi);
    if (!vm)
        return;
    restore();
    void *mode = at<void *>(vm, kMode);
    void *alloc = at<void *>(vm, kAllocator);
    auto *begin = at<uint8_t *>(vm, kTileBegin);
    auto *end = at<uint8_t *>(vm, kTileEnd);
    size_t count = end > begin ? size_t(end - begin) / kTileEntry : 0;
    size_t stock = count;
    for (const auto &s : classic_seasons::kSeasons) {
        if (!mode || !FeTile_Is_EraScenarioAvailable_Thunk(mode, s.setup_mode, s.entrant_list))
            continue;
        char node[0x20];
        std::snprintf(node, sizeof node, "select_tile_%zu", count);
        FeTile_Add_EraTile(vm + kTiles, alloc, uint8_t(count), node, s.entrant_list, 1, s.title, s.entrant_list, kImage,
                           0);
        count++;
    }
    if (g_logged++ < 6)
        swofty::log("[classic-seasons] era select: %zu stock box(es), %zu season box(es) added", stock, count - stock);
}

SWOFTY_WRAP(Game_F1WorldActivityContainer_SelectEntrantList, void, void *container, const char *entrant_list)
{
    if (container && entrant_list && !std::strncmp(entrant_list, "cs_f1_", 6))
        narrow(at<void *>(container, kContainerMode), entrant_list);
    else
        restore();
    orig(container, entrant_list);
}

SWOFTY_ON_LOAD
{
    swofty::log("[classic-seasons] loaded: %zu season(s)", std::size(classic_seasons::kSeasons));
}

SWOFTY_ADDON()
