// SPDX-License-Identifier: AGPL-3.0-or-later
//
// F1 1990 - the agent (Flavio) is silent in 1990 careers only. Its own DLL, so a binding it can't
// resolve drops only this hook.
//
// Game_SpeechSystem_PostLine posts each speech line as play_speech_<speaker>_<type> on the line's
// emitter, with the speech system's callback object, whose end-of-event and duration handlers move
// the line's state; a line that is never posted never finishes, so the line is still posted. For
// the speakers agent and agent_cutscene, while the loaded career's data group is f1_career_1990,
// the emitter's vol_speech RTPC (the speech volume the audio options set) is set to 0 first through
// the emitter's own RTPC setter (vtable +0x60, the call the post makes for random_number_generator),
// so the line plays silently with its subtitles and callbacks. The 1990 state is worked out on the
// core's career.loaded event and cleared on career.unloaded. Option agent_voice_mute turns it off.

#include <atomic>
#include <cstring>
#include <mutex>
#include <set>

#include <swofty/career.hpp>
#include <swofty/events.hpp>
#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include "career_group.hpp"
#include "speech_line.hpp"

SWOFTY_SETTING(bool, agent_voice_mute, true);

namespace {

using f1990::at;

constexpr size_t kRtpcSlot = 0x60 / 8;
constexpr char kSpeechVolume[] = "vol_speech";

std::atomic<int> g_1990{-1};
std::atomic<uint32_t> g_muted_lines, g_logged;
std::mutex g_lock;
std::set<void *> g_muted_emitters;

void *registry_get(void *registry, uintptr_t desc)
{
    auto **vt = *static_cast<void ***>(registry);
    if (!vt || !vt[2])
        return nullptr;
    return reinterpret_cast<void *(*)(void *, const void *, int)>(vt[2])(registry, reinterpret_cast<const void *>(desc), 1);
}

void *setup_component(void *championship)
{
    return registry_get(static_cast<uint8_t *>(championship) + 0x208,
                        swofty::game::k_Career_CareerSetupComponentDesc.address());
}

uint8_t group_index(void *component)
{
    auto **vt = *static_cast<void ***>(component);
    return reinterpret_cast<uint8_t (*)(void *)>(vt[0x268 / 8])(component);
}

void on_career(const SwoftyEvent *ev, void *)
{
    if (!std::strcmp(ev->name, SWOFTY_EVENT_CAREER_UNLOADED)) {
        if (g_1990.exchange(0) == 1)
            swofty::log("[f1-1990] voice: 1990 career unloaded; the agent speaks again");
        std::lock_guard<std::mutex> g(g_lock);
        g_muted_emitters.clear();
        return;
    }
    void *career = swofty::career::entity();
    bool yes = career && f1990::career_data_group_is(career, f1990::kCareerGroup1990, {setup_component, group_index});
    g_1990 = yes ? 1 : 0;
    swofty::log("[f1-1990] voice: career loaded (%s); agent voice %s", yes ? "data group f1_career_1990" : "not a 1990 career",
                yes && agent_voice_mute ? "muted" : "stock");
}

void subscribe(const char *event)
{
    uint32_t token = 0;
    swofty::events::Result r = swofty::events::subscribe(event, on_career, nullptr, &token);
    if (r != swofty::events::Result::ok)
        swofty::log("[f1-1990] voice: subscribe %s: %s; the agent keeps her voice", event, swofty::events::name(r));
}

void mute(void *emitter, const char *speaker)
{
    auto **vt = *static_cast<void ***>(emitter);
    if (!vt || !vt[kRtpcSlot])
        return;
    reinterpret_cast<void (*)(void *, const char *, float, int, float, int)>(vt[kRtpcSlot])(emitter, kSpeechVolume, 0.f, 0,
                                                                                           0.f, 0);
    bool fresh;
    {
        std::lock_guard<std::mutex> g(g_lock);
        fresh = g_muted_emitters.insert(emitter).second;
    }
    if (fresh || g_logged++ < 8)
        swofty::log("[f1-1990] voice: %s line muted (emitter %p, %u muted so far)", speaker, emitter, ++g_muted_lines);
}

}

SWOFTY_WRAP(Game_SpeechSystem_PostLine, void, void *speech, void *line, uint8_t retrigger)
{
    if (speech && line && g_1990.load() == 1 && agent_voice_mute) {
        const char *speaker = f1990::speech_speaker(speech, line);
        void *emitter = at<void *>(line, 0x28);
        if (emitter && f1990::is_agent_speaker(speaker))
            mute(emitter, speaker);
        else if (emitter) {
            bool shared;
            {
                std::lock_guard<std::mutex> g(g_lock);
                shared = g_muted_emitters.count(emitter) != 0;
            }
            if (shared && g_logged++ < 16)
                swofty::log("[f1-1990] voice: emitter %p muted for the agent also speaks %s; that line is silent too",
                            emitter, speaker ? speaker : "(unknown)");
        }
    }
    orig(speech, line, retrigger);
}

SWOFTY_ON_LOAD
{
    swofty::log("[f1-1990] voice: agent mute in 1990 careers %s", agent_voice_mute ? "on" : "off (agent_voice_mute)");
    subscribe(SWOFTY_EVENT_CAREER_LOADED);
    subscribe(SWOFTY_EVENT_CAREER_UNLOADED);
}

SWOFTY_ADDON()
