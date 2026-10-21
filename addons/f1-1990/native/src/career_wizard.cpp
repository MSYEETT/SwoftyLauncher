// SPDX-License-Identifier: AGPL-3.0-or-later
//
// F1 1990 - the new 1990 Driver Career wizard without the Skip to F1 / Start in F2 choice and without
// the team-icons step (My Team / Konnersport). Its own DLL, so a binding it can't resolve drops only
// these hooks; option career_wizard_1990 turns them off.
//
// Every caller of Game_CareerWizard_StartsInF1 passes the wizard state, whose +0x128 is the data group
// of the content tile that opened the wizard (f1_career_2025, f1_career_2026 or f1_career_1990). When
// it answers yes, the content-select router fires career_wizard_driver_career_to_f1 (straight to the
// season length step), the step machine leaves out the team-icons step after the settings, and the
// driver select confirm takes the F1 path, as Skip to F1 does. The wrap answers yes for
// f1_career_1990; the 2025 and 2026 wizards get the game's answer.

#include <atomic>
#include <cstring>

#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include "career_group.hpp"

SWOFTY_SETTING(bool, career_wizard_1990, true);

namespace {

std::atomic<uint32_t> g_logged;

}

SWOFTY_WRAP(Game_CareerWizard_StartsInF1, bool, void *entrants, void *wizard_data, const char *entrant_list)
{
    bool stock = orig(entrants, wizard_data, entrant_list);
    if (!career_wizard_1990 || !entrant_list || std::strcmp(entrant_list, f1990::kCareerGroup1990))
        return stock;
    if (g_logged++ < 8)
        swofty::log("[f1-1990] wizard: %s starts in F1, no F2 choice and no team-icons step (game said %s)",
                    entrant_list, stock ? "yes" : "no");
    return true;
}

SWOFTY_ON_LOAD
{
    swofty::log("[f1-1990] career wizard loaded: 1990 wizard steps %s", career_wizard_1990 ? "on" : "off (career_wizard_1990)");
}

SWOFTY_ADDON()
