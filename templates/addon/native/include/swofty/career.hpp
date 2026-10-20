// SPDX-License-Identifier: MIT
#pragma once
#include <cstdint>
#include <string>
#include <string_view>
#include <swofty/mixin.hpp>

// The running career, as the core sees it: front-end flow states, whether a career is loaded and
// in which mode, whether Team HQ is idle, the championship entity and its components, and the
// game's message bus. The core publishes the same facts as events (flow.state, career.loaded,
// career.unloaded, career.hub; see swofty/events.hpp); the getters here answer a late subscriber.
// entity(), component() and post() call into the game: use them on the game's flow thread (an
// events or on_frame callback), never from a thread of your own.

namespace swofty::career {

enum class Result { ok, late, unavailable, invalid };

enum class Mode : uint32_t { none = SWOFTY_CAREER_NONE, my_team = SWOFTY_CAREER_MY_TEAM, driver_career = SWOFTY_CAREER_DRIVER };

enum class Status : uint32_t {
    none = SWOFTY_FLOW_NONE,
    entering = SWOFTY_FLOW_ENTERING,
    updating = SWOFTY_FLOW_UPDATING,
    exiting = SWOFTY_FLOW_EXITING,
};

using State = SwoftyCareerState;
using Flow = SwoftyFlowState;

namespace detail {

inline const SwoftyCareer *table()
{
    return api && api->version >= 6 ? api->career : nullptr;
}

inline Result result(int r)
{
    return r == SWOFTY_ASSET_OK ? Result::ok
           : r == SWOFTY_ASSET_LATE ? Result::late
           : r == SWOFTY_ASSET_UNAVAILABLE ? Result::unavailable
                                           : Result::invalid;
}

}

// djb2 of a flow state name, the hash the game keys its states by.
constexpr uint32_t state_hash(std::string_view name)
{
    uint32_t h = 5381;
    for (char c : name)
        h = h * 33 + static_cast<uint8_t>(c);
    return h;
}

// The career library is usable: the core tracks the flow states on this build.
inline bool available()
{
    auto t = detail::table();
    return t && t->available();
}

// Current career facts. ok also when no career is loaded (out->loaded is 0 then).
inline Result state(State *out)
{
    auto t = detail::table();
    if (!t)
        return Result::unavailable;
    return detail::result(t->state(out));
}

// The last flow-state transition; late before the first one.
inline Result flow(Flow *out)
{
    auto t = detail::table();
    if (!t)
        return Result::unavailable;
    return detail::result(t->flow(out));
}

// Current status of a flow state and how long it has held it. invalid when the core has not seen
// that state yet (its status is then none).
inline Result flow_status(uint32_t hash, Status *status, uint64_t *age_ms = nullptr)
{
    auto t = detail::table();
    if (!t)
        return Result::unavailable;
    uint32_t s = 0;
    uint64_t age = 0;
    int r = t->flow_status(hash, &s, &age);
    if (status)
        *status = static_cast<Status>(s);
    if (age_ms)
        *age_ms = age;
    return detail::result(r);
}

inline Result flow_status(std::string_view name, Status *status, uint64_t *age_ms = nullptr)
{
    return flow_status(state_hash(name), status, age_ms);
}

// The championship entity (the career entity in a career), or nullptr.
inline void *entity()
{
    auto t = detail::table();
    return t ? t->entity() : nullptr;
}

// A component of the championship entity through the game's own registry get, by class name
// ("CareerBankComponent") or by the descriptor's KB binding id ("k_Career_CareerBankComponentDesc").
// nullptr when no career is loaded, the descriptor has no binding, or the entity lacks it.
inline void *component(std::string_view type)
{
    auto t = detail::table();
    return t ? t->component(std::string(type).c_str()) : nullptr;
}

// A component by the address of its static type descriptor.
inline void *component_by_desc(const void *desc)
{
    auto t = detail::table();
    return t ? t->component_by_desc(desc) : nullptr;
}

// Publish a message on the game's message bus (synchronous: every subscriber has run when it
// returns). late when the bus is missing or inactive.
inline Result post(void *msg)
{
    auto t = detail::table();
    if (!t)
        return Result::unavailable;
    return detail::result(t->post(msg));
}

inline const char *name(Mode m)
{
    return m == Mode::my_team ? "my_team" : m == Mode::driver_career ? "driver_career" : "none";
}

inline const char *name(Status s)
{
    return s == Status::entering ? "Entering" : s == Status::updating ? "Updating" : s == Status::exiting ? "Exiting" : "None";
}

inline const char *name(Result r)
{
    return r == Result::ok ? "ok" : r == Result::late ? "late" : r == Result::unavailable ? "unavailable" : "invalid";
}

inline Mode mode(const State &s)
{
    return static_cast<Mode>(s.mode);
}

}
