// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Driver Career only. In My Team the addon's hooks stall career creation and
// loading at fe_career_dlc_select until the game's hang watchdog ends the game,
// so the addon holds a core career block on My Team: the core cancels the
// front-end route that would start or load a My Team career and shows the
// popup below. Driver Career routes pass untouched.
//
// A core older than API 7 has no block. The addon then logs that My Team is
// not blocked and keeps working in Driver Career as before.

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <swofty/career.hpp>
#include <swofty/events.hpp>
#include <swofty/mixin.hpp>
#include <swofty/popup.hpp>

namespace {

constexpr const char *kTitle = "My Team not available";
constexpr const char *kBody =
    "R&D: Tyres & Thermal Management only works in Driver Career. To play My Team, turn the addon off in the "
    "launcher (or use a profile without it), then relaunch.";

void on_starting(const SwoftyEvent *ev, void *)
{
    auto *s = static_cast<const SwoftyCareerStart *>(ev->data);
    if (s && s->mode == SWOFTY_CAREER_MY_TEAM)
        swofty::log("[rnd-tyres] My Team career start (route '%s') %s", s->route,
                    s->blocked ? "cancelled by the core; Driver Career only" : "NOT cancelled");
}

}

SWOFTY_ON_LOAD
{
    namespace career = swofty::career;
    uint32_t v = swofty::api ? swofty::api->version : 0;
    career::Result r = career::block(career::Mode::my_team, kTitle, kBody);
    if (r != career::Result::ok) {
        swofty::log("[rnd-tyres] My Team is NOT blocked: this core (API v%u) %s. The addon is for Driver Career only; "
                    "starting or loading a My Team career with it on can freeze the game. Update the launcher.",
                    v, v < 7 ? "has no career block (it needs API v7)" : career::name(r));
        return;
    }
    swofty::events::subscribe(SWOFTY_EVENT_CAREER_STARTING, on_starting);
    swofty::log("[rnd-tyres] My Team blocked: the core cancels starting or loading a My Team career and shows \"%s\"%s",
                kTitle,
                swofty::popup::available() ? "" : " (the popup is unavailable on this build; the route is still cancelled)");
}
