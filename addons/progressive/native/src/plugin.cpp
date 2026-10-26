// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career - engine plugin for F1 25.
//
// Round 1 is the risk-first probe for experiment E1 (DESIGN.md): on every Team HQ (the core's
// career.hub entered) of a Driver Career it logs the season and the seat table, then applies
// data/probe.json once per career (probe.cpp). career.loaded logs the career; career.unloaded
// clears the per-career log state. Every game address is a KB binding resolved at load; one
// unresolved binding means the addon registers nothing.

#include <cstring>

#include <swofty/career.hpp>
#include <swofty/events.hpp>
#include <swofty/mixin.hpp>

#include "progressive.hpp"

namespace {

constexpr const char *kVersion = "0.1.0";

void on_career(const SwoftyEvent *ev, void *)
{
    auto *s = static_cast<const SwoftyCareerState *>(ev->data);
    swofty::career::State state{};
    if (s)
        state = *s;
    else
        swofty::career::state(&state);
    if (!std::strcmp(ev->name, SWOFTY_EVENT_CAREER_UNLOADED)) {
        swofty::log("[progressive] career unloaded (it was %s)",
                    swofty::career::name(static_cast<swofty::career::Mode>(ev->a)));
        pg::forget_career();
        return;
    }
    if (!std::strcmp(ev->name, SWOFTY_EVENT_CAREER_LOADED)) {
        swofty::log("[progressive] career loaded: mode %s, career %s",
                    swofty::career::name(static_cast<swofty::career::Mode>(ev->a)), state.key[0] ? state.key : "(no key yet)");
        return;
    }
    if (ev->a != 1)
        return;
    if (static_cast<swofty::career::Mode>(ev->b) != swofty::career::Mode::driver_career) {
        swofty::log("[progressive] Team HQ in %s: nothing to do outside Driver Career",
                    swofty::career::name(static_cast<swofty::career::Mode>(ev->b)));
        return;
    }
    pg::report("Team HQ", state);
    pg::probe(state);
}

void subscribe(const char *event)
{
    uint32_t token = 0;
    swofty::events::Result r = swofty::events::subscribe(event, on_career, nullptr, &token);
    swofty::log("[progressive] subscribe %s: %s", event, swofty::events::name(r));
}

} // namespace

SWOFTY_ON_LOAD
{
    swofty::log("[progressive] loaded v%s (round 1: E1 seat probe); core API v%u", kVersion,
                swofty::api ? swofty::api->version : 0u);
    subscribe(SWOFTY_EVENT_CAREER_LOADED);
    subscribe(SWOFTY_EVENT_CAREER_UNLOADED);
    subscribe(SWOFTY_EVENT_CAREER_HUB);
}

SWOFTY_ADDON()
