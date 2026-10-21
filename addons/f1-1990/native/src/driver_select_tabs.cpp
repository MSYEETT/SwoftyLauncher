// SPDX-License-Identifier: AGPL-3.0-or-later
//
// F1 1990 - no F2 and Icons tabs on the Choose Driver screen of a new 1990 Driver Career. Its own
// DLL, so a binding it can't resolve drops only this hook.
//
// The driver_career_driver_select_header view model builds its tabs with the shared tab builder
// 0x144251f40 from the content group driver_career_driver_select_header (header_f1,
// header_selected_team_icon, header_f2, header_icon). The redirect of that call
// (site_Game_DriverSelectHeader_BuildTabs) gives a 1990 wizard a copy of the group without
// header_f2 and header_icon, so the header has the first two tabs only. The view model also
// sets tab states by fixed index (Game_DriverSelectHeader_SetTabState, e.g. tabs 0, 1 and 3 off), so
// that wrap skips an index past the tabs the header has. A wizard is a 1990 one when the last career
// tile pressed was the 1990 tile (F1990_CareerWizard1990, exported by f1_1990_ui.dll) and no other
// career is loaded.

#include <windows.h>

#include <atomic>
#include <cstring>
#include <vector>

#include <swofty/career.hpp>
#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include "career_1990.hpp"
#include "content_tabs.hpp"

namespace {

using f1990::at;

constexpr size_t kTabSize = 0x70;
constexpr const char *kDropped[] = {"header_f2", "header_icon"};

struct Group {
    uint8_t head[0x08];
    const uint8_t *begin, *end;
};

std::vector<uint8_t> g_entries;
std::atomic<uint32_t> g_builds, g_skips;

bool wizard_1990()
{
    static int (*pressed)() = [] {
        HMODULE ui = GetModuleHandleW(L"f1_1990_ui.dll");
        auto fn = ui ? reinterpret_cast<int (*)()>(GetProcAddress(ui, "F1990_CareerWizard1990")) : nullptr;
        if (!fn)
            swofty::log("[f1-1990] driver select: f1_1990_ui.dll is not loaded; the header keeps its tabs");
        return fn;
    }();
    if (!pressed || !pressed())
        return false;
    void *career = swofty::career::entity();
    return !career || f1990::is_1990_career(career);
}

}

SWOFTY_REDIRECT(site_Game_DriverSelectHeader_BuildTabs, int64_t, void *tabs, void *alloc, void *node, const void *group)
{
    if (!group || !wizard_1990())
        return orig(tabs, alloc, node, group);
    int dropped = f1990::drop_content_tabs(group, kDropped, g_entries);
    if (dropped <= 0) {
        if (g_builds++ < 4)
            swofty::log("[f1-1990] driver select: the header group %s; its tabs stay",
                        dropped < 0 ? "does not look like a content group" : "has no F2 or Icons tab");
        return orig(tabs, alloc, node, group);
    }
    Group copy;
    std::memcpy(&copy, group, sizeof copy);
    copy.begin = g_entries.data();
    copy.end = g_entries.data() + g_entries.size();
    if (g_builds++ < 4)
        swofty::log("[f1-1990] driver select: 1990 wizard; header tabs %zu of %zu (F2 and Icons hidden)",
                    g_entries.size() / f1990::kContentTabSize, g_entries.size() / f1990::kContentTabSize + dropped);
    return orig(tabs, alloc, node, &copy);
}

SWOFTY_WRAP(Game_DriverSelectHeader_SetTabState, void, void *header, uint64_t tab, uint8_t content_visible,
            uint8_t enabled)
{
    auto *b = header ? at<const uint8_t *>(header, 0x2b8) : nullptr;
    auto *e = header ? at<const uint8_t *>(header, 0x2c0) : nullptr;
    if (!b || e <= b || tab >= size_t(e - b) / kTabSize) {
        if (g_skips++ < 8)
            swofty::log("[f1-1990] driver select: tab %llu state skipped, the header has %zu tab(s)",
                        (unsigned long long)tab, b && e > b ? size_t(e - b) / kTabSize : size_t(0));
        return;
    }
    orig(header, tab, content_visible, enabled);
}

SWOFTY_ADDON()
