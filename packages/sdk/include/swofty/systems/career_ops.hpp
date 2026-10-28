// SPDX-License-Identifier: MIT
#pragma once
#include <swofty_api.h>
#include <atomic>
#include <string>
#include <swofty/career.hpp>
#include <swofty/events.hpp>
#include <swofty/registry.hpp>
#include <swofty/systems/forward.hpp>

// The core's career operations table (swofty_api.h SwoftyCareerOps, the core export SwoftyCoreCareerOps) as the
// system headers reach it: the table, the result mapping and a driver reference's stat id. swofty/systems/career.hpp
// and swofty/systems/drivers.hpp both include it.

namespace swofty::career {

namespace detail {

inline std::atomic<const SwoftyCareerOps *> &ops_override()
{
    static std::atomic<const SwoftyCareerOps *> t{nullptr};
    return t;
}

inline void set_ops_table(const SwoftyCareerOps *t) { ops_override() = t; }

inline const SwoftyCareerOps *ops_table()
{
    if (auto t = ops_override().load())
        return t->version >= 1 ? t : nullptr;
#if defined(_WIN32)
    static std::atomic<const SwoftyCareerOps *> found{nullptr};
    if (auto t = found.load())
        return t;
    HMODULE core = GetModuleHandleW(L"swofty_core.dll");
    auto get = core ? reinterpret_cast<SwoftyCoreCareerOpsFn>(GetProcAddress(core, "SwoftyCoreCareerOps")) : nullptr;
    const SwoftyCareerOps *t = get ? get() : nullptr;
    if (t && t->version < 1)
        t = nullptr;
    found = t;
    return t;
#else
    return nullptr;
#endif
}

inline const SwoftyCareerOps *live_ops()
{
    auto t = ops_table();
    return t && t->available && t->available() ? t : nullptr;
}

inline Result ops_result(int r)
{
    return r == SWOFTY_ASSET_OK ? Result::ok
           : r == SWOFTY_ASSET_LATE ? Result::late
           : r == SWOFTY_ASSET_UNAVAILABLE ? Result::unavailable
           : r == SWOFTY_REFUSED ? Result::refused
                                           : Result::invalid;
}

inline bool stat_of(const SwoftyCareerOps *t, const Ref<drivers::Identity> &d, uint32_t &out)
{
    std::string id(d.id());
    return !id.empty() && t->driver_stat(id.c_str(), &out) == SWOFTY_ASSET_OK;
}

}

}
