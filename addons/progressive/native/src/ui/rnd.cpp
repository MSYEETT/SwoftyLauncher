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
// In-season development (development_logic.hpp): inside the fill, the Front and Rear Downforce types
// instead get the progress progressive.dll worked out for the car's team at the last load or Team
// HQ (ProgressiveDevelopmentProgress), 0 when it has none. The fill passes the team's R&D model
// (Upgrades, team key at +0x0c) + 0x1a30 as the type metadata (decomp 0x141238cb0), which names the
// team here.

#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include <atomic>
#include <cstddef>

#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include "../development_logic.hpp"

namespace {

thread_local int g_fill_depth = 0;
thread_local float g_fill_dev = -1.0f;
thread_local uint32_t g_fill_team = 0;
std::atomic<uint32_t> g_frozen{0};
std::atomic<uint32_t> g_dev_lines{0};
constexpr uintptr_t kModelMeta = 0x1a30;

void *progressive_export(const char *name)
{
    HMODULE m = GetModuleHandleW(L"progressive.dll");
    return m ? reinterpret_cast<void *>(GetProcAddress(m, name)) : nullptr;
}

int rnd_mode()
{
    static int (*fn)() = nullptr;
    static bool looked = false;
    if (!looked) {
        fn = reinterpret_cast<int (*)()>(progressive_export("ProgressiveRndMode"));
        looked = GetModuleHandleW(L"progressive.dll") != nullptr;
    }
    return fn ? fn() : 0;
}

bool development(uint32_t team_key, float &progress)
{
    using Fn = int (*)(uint32_t, float *);
    static Fn fn = nullptr;
    static bool looked = false;
    if (!looked) {
        fn = reinterpret_cast<Fn>(progressive_export("ProgressiveDevelopmentProgress"));
        looked = GetModuleHandleW(L"progressive.dll") != nullptr;
    }
    return fn && fn(team_key, &progress) != 0;
}

} // namespace

SWOFTY_WRAP(Game_Rnd_FillVehicleUpgrades, void, void *component, uint8_t car, uint8_t a3, uint8_t a4)
{
    bool freeze = rnd_mode() == 1;
    if (freeze) {
        ++g_fill_depth;
        g_fill_dev = -1.0f;
        g_fill_team = 0;
    }
    orig(component, car, a3, a4);
    if (freeze) {
        --g_fill_depth;
        if (g_fill_dev >= 0.0f) {
            if (g_dev_lines++ < 200)
                swofty::log("[progressive] R&D real life: car %u (team %u) upgrade progress held at 0, downforce at "
                            "%.3f (in-season development)",
                            car, g_fill_team >> 16, g_fill_dev);
        } else if (g_frozen++ < 4) {
            swofty::log("[progressive] R&D real life: car %u's upgrade progress held at 0", car);
        }
    }
}

SWOFTY_WRAP(Game_Rnd_CalcUpgradeTypeProgress, float, void *const *instances, uint8_t type, uint8_t state, const void *meta)
{
    if (g_fill_depth > 0) {
        uint32_t key = 0;
        float p = 0.0f;
        if (!progressive::development::lever_type(type) || !meta ||
            !swofty::mem::read(reinterpret_cast<uintptr_t>(meta) - kModelMeta + offsetof(swofty::game::Upgrades, team_key),
                               key) ||
            !development(key, p))
            return 0.0f;
        g_fill_dev = p;
        g_fill_team = key;
        return p;
    }
    return orig(instances, type, state, meta);
}

SWOFTY_ON_LOAD
{
    swofty::log("[progressive] R&D: real-life freeze armed (only in Progressive careers set to real_life)");
}

SWOFTY_ADDON()
