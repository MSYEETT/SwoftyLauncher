#pragma once
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

namespace swofty::game {
inline constexpr vtable<"wine-test:vt_Fake_Other", 40> vt_Fake_Other{};
inline constexpr call<"wine-test:call_Fake_Callee"> call_Fake_Callee{};
inline constexpr site<"wine-test:site_Fake_Patch"> site_Fake_Patch{};
using Fake_Missing_t = int();
inline constexpr fn<"wine-test:Fake_Missing", Fake_Missing_t> Fake_Missing{};
}

using namespace swofty::game;

inline volatile int64_t *probe()
{
    return reinterpret_cast<volatile int64_t *>(GetProcAddress(GetModuleHandleW(nullptr), "probe"));
}
