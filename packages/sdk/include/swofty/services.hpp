// SPDX-License-Identifier: MIT
#pragma once
#if defined(_WIN32)
#include <windows.h>
#endif
#include <atomic>
#include <functional>
#include <swofty/mixin.hpp>

// Services (standard library 1.6): versioned function tables one addon publishes for others to look up
// by name. T is the table: static constexpr const char *name and uint32_t version, then uint32_t size and
// function pointers, appended per minor. when<T>() passes only a table of T::version or newer and of at
// least sizeof(T); it runs at once when the table is already published, else once it is. On a core
// without the services table both return unavailable.

namespace swofty::services {

enum class Result { ok, late, unavailable, invalid };

namespace detail {

inline const SwoftyServices *table()
{
    if (!api || api->version < 9 || !api->services)
        return nullptr;
#if defined(_WIN32)
    static std::atomic<int> exported{-1};
    if (exported.load() < 0) {
        HMODULE core = GetModuleHandleW(L"swofty_core.dll");
        exported = core && GetProcAddress(core, "SwoftyCoreServices") ? 1 : 0;
    }
    if (!exported.load())
        return nullptr;
#endif
    return api->services->available && api->services->available() ? api->services : nullptr;
}

inline Result result(int r)
{
    return r == SWOFTY_ASSET_OK            ? Result::ok
           : r == SWOFTY_ASSET_LATE        ? Result::late
           : r == SWOFTY_ASSET_UNAVAILABLE ? Result::unavailable
                                           : Result::invalid;
}

template <class T> void when_entry(void *user, const void *table, size_t size, uint32_t version)
{
    if (!table || size < sizeof(T) || version < T::version)
        return;
    (*static_cast<std::function<void(const T &)> *>(user))(*static_cast<const T *>(table));
}

}

inline bool available() { return detail::table() != nullptr; }

template <class T> Result publish(const T &table)
{
    auto t = detail::table();
    if (!t || !t->publish)
        return Result::unavailable;
    return detail::result(t->publish(T::name, T::version, &table, sizeof(T)));
}

template <class T> Result when(std::function<void(const T &)> fn)
{
    auto t = detail::table();
    if (!t || !t->when)
        return Result::unavailable;
    return detail::result(t->when(T::name, T::version, sizeof(T), &detail::when_entry<T>,
                                  new std::function<void(const T &)>(std::move(fn))));
}

inline const char *name(Result r)
{
    return r == Result::ok ? "ok" : r == Result::late ? "late" : r == Result::unavailable ? "unavailable" : "invalid";
}

}
