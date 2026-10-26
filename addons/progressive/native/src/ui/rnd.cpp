// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career - Real-life R&D (progressive_rnd.dll, DESIGN.md D9). In a Progressive career
// whose R&D mode is real_life, the car is the season's car and nothing on top: the vehicle upgrade
// fill (Game_Rnd_FillVehicleUpgrades 0x141238cb0, per car at session entry) gets progress 0 for
// every upgrade type from Game_Rnd_CalcUpgradeTypeProgress (0x143526260). Only calls made inside
// the fill are changed: the same function also feeds power-unit wear (0x1411cd6da) and the R&D
// screens, which keep the game's answer. The mode comes from progressive.dll
// (ProgressiveRndMode: 0 not a Progressive career, 1 real_life, 3 hybrid), set at Team HQ from the
// career's store entry and at once when the R&D option changes while the career is loaded.

#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include <atomic>

#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

namespace {

thread_local int g_fill_depth = 0;
std::atomic<uint32_t> g_frozen{0};

int rnd_mode()
{
    static int (*fn)() = nullptr;
    static bool looked = false;
    if (!looked) {
        HMODULE m = GetModuleHandleW(L"progressive.dll");
        fn = m ? reinterpret_cast<int (*)()>(reinterpret_cast<void *>(GetProcAddress(m, "ProgressiveRndMode"))) : nullptr;
        looked = m != nullptr;
    }
    return fn ? fn() : 0;
}

} // namespace

SWOFTY_WRAP(Game_Rnd_FillVehicleUpgrades, void, void *component, uint8_t car, uint8_t a3, uint8_t a4)
{
    bool freeze = rnd_mode() == 1;
    if (freeze)
        ++g_fill_depth;
    orig(component, car, a3, a4);
    if (freeze) {
        --g_fill_depth;
        if (g_frozen++ < 4)
            swofty::log("[progressive] R&D real life: car %u's upgrade progress held at 0", car);
    }
}

SWOFTY_WRAP(Game_Rnd_CalcUpgradeTypeProgress, float, void *const *instances, uint8_t type, uint8_t state, const void *meta)
{
    if (g_fill_depth > 0)
        return 0.0f;
    return orig(instances, type, state, meta);
}

SWOFTY_ON_LOAD
{
    swofty::log("[progressive] R&D: real-life freeze armed (only in Progressive careers set to real_life)");
}

SWOFTY_ADDON()
