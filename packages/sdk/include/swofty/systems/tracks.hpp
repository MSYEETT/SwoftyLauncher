// SPDX-License-Identifier: MIT
#pragma once
#include <cstdint>
#include <swofty/events.hpp>
#include <swofty/systems/forward.hpp>

// Tracks: the hook points 1.6.0 ships ahead of the tracks system (tracks::tracks, venues, data and
// per_package come in 1.6.x).

namespace swofty::tracks::events {

struct FuelLoad {
    static constexpr const char *point = "session.fuel.load";
    static constexpr uint32_t version = 1;
    enum Kind : uint8_t { practice = 0, qualifying = 1, race = 2, other = 3 };
    uint32_t size, payload_version;
    const char *track_id;
    uint32_t session_type;
    float kg;
    uint8_t kind;

    Ref<Track> track() const { return swofty::detail::payload_ref<Track>(track_id); }
};

}
