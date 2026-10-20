// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Career Mode Editor - engine plugin for F1 25.
//
// Edits a running career from the launcher's Career Editor page. The page writes edit batches
// into the addon's storage folder; this plugin drains them on the flow thread while the career
// is in a safe state, applies each op through the game's own code, and publishes a snapshot of
// the career back (bridge.cpp). The core's career library (swofty/career.hpp) tracks the flow
// states, career-loaded and the mode; state.cpp turns that into the safe state, and this file logs
// the core's career events. The frame callback is registered at load; if the core refuses it, the
// feature modules' game hooks (ce::ensure_frame) and the core's career events retry it until it
// takes. The feature modules (economy, rnd, drivers, season, weather) register
// their ops and sections through ce.hpp. Every game address is a KB binding resolved at load; one
// unresolved binding means the addon registers nothing.

#include <atomic>
#include <cstring>
#include <mutex>

#include <swofty/career.hpp>
#include <swofty/events.hpp>
#include <swofty/mixin.hpp>

#include "bridge.hpp"

namespace {

std::atomic<bool> g_frame_registered, g_frame_live;
std::mutex g_frame_lock;
bool g_frame_refusal_logged;

void on_frame(double, void *)
{
    if (!g_frame_live.exchange(true))
        ce::log("frame callback live: the mailbox is running; core career library %s",
                swofty::career::available() ? "available" : "unavailable");
    ce::bridge::frame();
}

void register_frame(const char *retry)
{
    if (g_frame_registered)
        return;
    std::lock_guard<std::mutex> g(g_frame_lock);
    if (g_frame_registered)
        return;
    uint32_t token = 0;
    swofty::events::Result r = swofty::events::on_frame(on_frame, nullptr, &token);
    if (r != swofty::events::Result::ok) {
        if (!g_frame_refusal_logged)
            ce::log("frame callback %s %s%s: the mailbox stays idle; the editor's game hooks and the core's career "
                    "events retry it",
                    swofty::events::name(r), retry ? "on retry from " : "at load", retry ? retry : "");
        g_frame_refusal_logged = true;
        return;
    }
    g_frame_registered = true;
    ce::log("frame callback ok (token %u)%s%s", token, retry ? " on retry from " : "", retry ? retry : "");
}

const char *mode_of(const SwoftyEvent *ev)
{
    auto *s = static_cast<const SwoftyCareerState *>(ev->data);
    return s ? swofty::career::name(swofty::career::mode(*s)) : "?";
}

void on_career(const SwoftyEvent *ev, void *)
{
    register_frame("a career event");
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

namespace ce {

void ensure_frame()
{
    register_frame("a game hook");
}

} // namespace ce

SWOFTY_ON_LOAD
{
    ce::log("loaded v%s; API v%u; game base 0x%llx", ce::kVersion, swofty::api ? swofty::api->version : 0u,
            swofty::api ? (unsigned long long)swofty::api->module_base() : 0ull);
    ce::bridge::start();
    if (!swofty::events::available())
        ce::log("events library reports unavailable at load; registering the frame callback anyway");
    register_frame(nullptr);
    subscribe(SWOFTY_EVENT_CAREER_LOADED);
    subscribe(SWOFTY_EVENT_CAREER_UNLOADED);
    subscribe(SWOFTY_EVENT_CAREER_HUB);
}

SWOFTY_ADDON()
