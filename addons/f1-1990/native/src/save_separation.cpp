// SPDX-License-Identifier: AGPL-3.0-or-later
//
// F1 1990 - its own profile save.
//
// Every career slot lives in the one profile save, which the game names with the literal "profile"
// (Steam files savegame@profile#*). The profile save and load requests build their save job through
// Game_SaveMgr_CreateSaveJob; this wrap renames that save to "f1990" (savegame@f1990#*), so the 1990
// profile starts its own game profile and never opens the stock one. Replay saves (highlights%03d)
// keep their names. If the addon's hooks don't install, plugin.cpp logs SAVE SEPARATION OFF.

#include <atomic>

#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include "save_name.hpp"

namespace {

std::atomic<uint32_t> g_redirects;

}

SWOFTY_WRAP(Game_SaveMgr_CreateSaveJob, void *, void *jobs, uint64_t kind, uint32_t type, const char *save_name,
            uint64_t a5, uint64_t a6, uint64_t a7, uint64_t a8, uint64_t a9)
{
    const char *name = f1990::save_name_for(save_name);
    if (name != save_name)
        swofty::log("[f1-1990] save: profile -> %s (job kind %llu type %u, #%u)", name, (unsigned long long)kind, type,
                    ++g_redirects);
    return orig(jobs, kind, type, name, a5, a6, a7, a8, a9);
}

SWOFTY_ON_LOAD
{
    swofty::log("[f1-1990] save separation on: the profile save is \"%s\" (savegame@%s#*) in this profile",
                f1990::kProfileSave1990, f1990::kProfileSave1990);
}
