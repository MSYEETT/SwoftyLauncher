// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Career Mode Editor - engine plugin for F1 25.
//
// Edits a running career from the launcher's Career Editor page. The page writes edit batches
// into the addon's storage folder; this plugin drains them on the flow thread while the career
// is in a safe state, applies each op through the game's own code, and publishes a snapshot of
// the career back (bridge.cpp). The core's career library (swofty/career.hpp) tracks the flow
// states, career-loaded and the mode; state.cpp turns that into the safe state, and this file logs
// the core's career events. The feature modules (economy, rnd, drivers, season, weather) register
// their ops and sections through ce.hpp. Every game address is a KB binding resolved at load; one
// unresolved binding means the addon registers nothing.

#include <cstring>

#include <swofty/career.hpp>
#include <swofty/events.hpp>
#include <swofty/mixin.hpp>

#include "bridge.hpp"

namespace {

void on_frame(double, void *)
{
    ce::bridge::frame();
}

const char *mode_of(const SwoftyEvent *ev)
{
    auto *s = static_cast<const SwoftyCareerState *>(ev->data);
    return s ? swofty::career::name(swofty::career::mode(*s)) : "?";
}

void on_career(const SwoftyEvent *ev, void *)
{
    auto *s = static_cast<const SwoftyCareerState *>(ev->data);
    const char *key = s && s->key[0] ? s->key : "(not yet known)";
    if (!std::strcmp(ev->name, SWOFTY_EVENT_CAREER_LOADED))
        ce::log("core: career loaded, mode %s, career %s", mode_of(ev), key);
    else if (!std::strcmp(ev->name, SWOFTY_EVENT_CAREER_UNLOADED))
        ce::log("core: career unloaded (it was %s)",
                swofty::career::name(static_cast<swofty::career::Mode>(ev->a)));
    else
        ce::log("core: Team HQ %s (%s)", ev->a ? "entered" : "left",
                swofty::career::name(static_cast<swofty::career::Mode>(ev->b)));
}

void subscribe(const char *event)
{
    uint32_t token = 0;
    swofty::events::Result r = swofty::events::subscribe(event, on_career, nullptr, &token);
    ce::log("subscribe %s: %s (token %u)", event, swofty::events::name(r), token);
}

} // namespace

SWOFTY_ON_LOAD
{
    ce::log("loaded v%s; API v%u; game base 0x%llx", ce::kVersion, swofty::api ? swofty::api->version : 0u,
            swofty::api ? (unsigned long long)swofty::api->module_base() : 0ull);
    ce::bridge::start();
    if (!swofty::events::available()) {
        ce::log("events library unavailable: no frame callback, the mailbox stays idle");
        return;
    }
    uint32_t token = 0;
    swofty::events::Result r = swofty::events::on_frame(on_frame, nullptr, &token);
    ce::log("frame callback %s (token %u)", swofty::events::name(r), token);
    ce::log("core career library %s", swofty::career::available() ? "available" : "unavailable");
    subscribe(SWOFTY_EVENT_CAREER_LOADED);
    subscribe(SWOFTY_EVENT_CAREER_UNLOADED);
    subscribe(SWOFTY_EVENT_CAREER_HUB);
}

SWOFTY_ADDON()
