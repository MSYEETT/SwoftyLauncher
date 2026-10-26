// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career - no R&D purchases in a Real-life career (progressive_rnd_block.dll).
//
// The R&D purchase request handler (0x1412414c0; site_Career_RnDPurchaseRequestEntry, its entry:
// edx the team key, r8 the upgrade instance, r9b the option) returns 0 at once when the instance is
// null. In a Progressive career whose R&D mode is real_life (ProgressiveRndMode 1 from
// progressive.dll), a request for the player's team (ProgressivePlayerTeam) gets a null instance,
// so the game refuses it, and the game's popup says why (at most once every 10 seconds). Requests
// for other teams pass. Car progress is held at 0 in these careers anyway (progressive_rnd.dll); this
// keeps the player from spending resource points on upgrades that would do nothing.

#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include <atomic>

#include <swofty/game.hpp>
#include <swofty/mixin.hpp>
#include <swofty/popup.hpp>

namespace {

std::atomic<uint64_t> g_last_popup{0};
std::atomic<uint32_t> g_logged{0};

template <class F> F export_of(const char *name)
{
    HMODULE m = GetModuleHandleW(L"progressive.dll");
    return m ? reinterpret_cast<F>(reinterpret_cast<void *>(GetProcAddress(m, name))) : nullptr;
}

int rnd_mode()
{
    static auto fn = export_of<int (*)()>("ProgressiveRndMode");
    return fn ? fn() : 0;
}

uint32_t player_team()
{
    static auto fn = export_of<uint32_t (*)()>("ProgressivePlayerTeam");
    return fn ? fn() : 0;
}

} // namespace

SWOFTY_SITE(site_Career_RnDPurchaseRequestEntry, swofty::Context &c)
{
    if (!c.r8 || rnd_mode() != 1)
        return;
    uint32_t team = static_cast<uint32_t>(c.rdx), mine = player_team();
    if (!mine || ((team ^ mine) & 0xffff00ffu) != 0)
        return;
    c.r8 = 0;
    if (g_logged++ < 8)
        swofty::log("[progressive] R&D real life: the player's upgrade purchase for team %u refused", team >> 16);
    uint64_t now = GetTickCount64(), last = g_last_popup.load();
    if (now - last > 10000 && g_last_popup.compare_exchange_strong(last, now))
        swofty::popup::show("REAL-LIFE R&D",
                            "This Progressive Career runs each season's real car, so R&D upgrades are off.\n"
                            "Change R&D to Base or Hybrid in the Progressive Career settings for a new career.");
}

SWOFTY_ADDON()
