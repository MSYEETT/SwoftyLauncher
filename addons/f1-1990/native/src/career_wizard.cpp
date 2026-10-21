// SPDX-License-Identifier: AGPL-3.0-or-later
//
// F1 1990 - the new 1990 Driver Career wizard: Recommended or Custom settings, then Choose Driver
// with the official 1990 drivers. Its own DLL, so a binding it can't resolve drops only these hooks;
// option career_wizard_1990 turns them off. Each hook acts only on a wizard whose content data
// group (wizard state +0x128, copied from the Career tile) is f1_career_1990; the 2025 and 2026
// wizards keep every step.
//
// - Game_CareerWizard_StartsInF1 answers yes: no Skip to F1 / Start in F2 step, no team-icons step
//   (My Team / Konnersport), and the driver select takes the F1 path.
// - The Career tile router (FeTile_Route_CareerItem) fires career_wizard_driver_career_1990 instead
//   of career_wizard_driver_career_to_f1. erp/wizard.json adds that route to the content select's
//   grouping: it opens the settings step, so there is no Full Season / Custom Season choice and the
//   career runs the full 1990 calendar.
// - Game_CareerWizard_PushNextStep: after the settings, the classic drivers step (14, Add Driver
//   Icons To Market) becomes the confirm step (6).
// - At site_Game_CareerWizardConfirm_NextRoute the confirm step's route driver_career_driver_type_select
//   (Official Driver / Custom Driver) becomes driver_career_driver_select, the route the game takes
//   for a team-icon career: Choose Driver opens directly.

#include <atomic>
#include <cstring>

#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include "career_group.hpp"

SWOFTY_SETTING(bool, career_wizard_1990, true);

namespace {

using f1990::at;
using swofty::game::Action;
using swofty::game::HomeItem;
using swofty::game::NeStr;

constexpr char kToF1Route[] = "career_wizard_driver_career_to_f1";
constexpr char kSettingsRoute[] = "career_wizard_driver_career_1990";
constexpr char kDriverTypeRoute[] = "driver_career_driver_type_select";
constexpr char kDriverSelectRoute[] = "driver_career_driver_select";
constexpr uint8_t kStepClassicDrivers = 14;
constexpr uint8_t kStepConfirm = 6;

std::atomic<uint32_t> g_starts_logged, g_route_logged, g_step_logged, g_confirm_logged;

bool is_1990_group(const char *group)
{
    return group && !std::strcmp(group, f1990::kCareerGroup1990);
}

bool is_1990_wizard(const void *wizard)
{
    return wizard && is_1990_group(at<const char *>(wizard, 0x128));
}

const char *text(const NeStr *s)
{
    return s ? s->chars : "";
}

size_t steps(const void *wizard)
{
    auto *b = at<const uint8_t *>(wizard, 0xe0), *e = at<const uint8_t *>(wizard, 0xe8);
    return b && e > b ? size_t(e - b) : 0;
}

}

SWOFTY_WRAP(Game_CareerWizard_StartsInF1, bool, void *entrants, void *wizard_data, const char *entrant_list)
{
    bool stock = orig(entrants, wizard_data, entrant_list);
    if (!career_wizard_1990 || !is_1990_group(entrant_list))
        return stock;
    if (g_starts_logged++ < 8)
        swofty::log("[f1-1990] wizard: %s starts in F1, no F2 choice and no team-icons step (game said %s)",
                    entrant_list, stock ? "yes" : "no");
    return true;
}

SWOFTY_WRAP(FeTile_Route_CareerItem, Action *, HomeItem *item, Action *out)
{
    Action *r = orig(item, out);
    auto *group = item ? at<const NeStr *>(item, 0x180) : nullptr;
    if (!career_wizard_1990 || !out || !group || !is_1990_group(group->chars))
        return r;
    if (out->type != 1 || std::strcmp(text(out->str), kToF1Route)) {
        if (g_route_logged++ < 4)
            swofty::log("[f1-1990] wizard: 1990 tile route \"%s\" (type %d) left as it is", text(out->str), out->type);
        return r;
    }
    swofty::game::Ne_Make_StrFromCStr(&out->str, kSettingsRoute, 0, 0);
    if (g_route_logged++ < 4)
        swofty::log("[f1-1990] wizard: 1990 tile opens the settings step (%s -> %s)", kToF1Route, text(out->str));
    return r;
}

SWOFTY_WRAP(Game_CareerWizard_PushNextStep, void, void *wizard)
{
    size_t before = wizard ? steps(wizard) : 0;
    orig(wizard);
    if (!career_wizard_1990 || !is_1990_wizard(wizard) || steps(wizard) != before + 1)
        return;
    auto *last = at<uint8_t *>(wizard, 0xe8) - 1;
    if (*last != kStepClassicDrivers)
        return;
    *last = kStepConfirm;
    if (g_step_logged++ < 4)
        swofty::log("[f1-1990] wizard: classic drivers step (%u) replaced by the confirm step (%u)", kStepClassicDrivers,
                    kStepConfirm);
}

SWOFTY_SITE(site_Game_CareerWizardConfirm_NextRoute, swofty::Context &c)
{
    auto *route = reinterpret_cast<const char *>(c.rdx);
    auto *vm = reinterpret_cast<const uint8_t *>(c.rsi);
    if (!career_wizard_1990 || !route || !vm || std::strcmp(route, kDriverTypeRoute) ||
        !is_1990_wizard(at<const void *>(vm, 0x270)))
        return;
    c.rdx = reinterpret_cast<uintptr_t>(kDriverSelectRoute);
    if (g_confirm_logged++ < 4)
        swofty::log("[f1-1990] wizard: confirm goes to %s, no driver type step", kDriverSelectRoute);
}

SWOFTY_ON_LOAD
{
    swofty::log("[f1-1990] career wizard loaded: 1990 wizard steps %s", career_wizard_1990 ? "on" : "off (career_wizard_1990)");
}

SWOFTY_ADDON()
