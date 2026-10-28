// SPDX-License-Identifier: MIT
#pragma once
#include <swofty/events.hpp>
#include <swofty/registry.hpp>
#include <swofty/systems/forward.hpp>

// Sounds: engine sounds (a bundle and where its mixing settings and tuning come from) and each car's
// audio records. Both apply at boot; the bundle file is redirected when the game opens it.

namespace swofty::sounds {

using Result = swofty::Result;

struct EngineSound {
    static constexpr Version version{1, 0};
    Path bundle;
    Ref<EngineSound> settings_from;
    Ref<EngineSound> tuning_from;
    json::Value extra;
};
SWOFTY_RECORD(EngineSound, bundle, settings_from, tuning_from)

struct CarAudio {
    static constexpr Version version{1, 0};
    Ref<cars::Vehicle> vehicle;
    std::string switch_name;
    Ref<EngineSound> engine_sound;
    Ref<CarAudio> from;
    json::Value extra;
};
SWOFTY_RECORD(CarAudio, vehicle, switch_name, engine_sound, from)
SWOFTY_RECORD_REGISTRY(CarAudio, "audio.car")

inline Registry<EngineSound> engine_sounds{"engine_sound", {.applies = Applies::boot}};
inline Registry<CarAudio> car_audio{"audio.car", {.applies = Applies::boot}};

}

namespace swofty::sounds::events {

struct BundleResolve {
    static constexpr const char *point = "audio.bundle.resolve";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    int car;
    Ref<cars::Vehicle> vehicle;
    std::string_view switch_name;
    Ref<EngineSound> engine_sound;
};

}
