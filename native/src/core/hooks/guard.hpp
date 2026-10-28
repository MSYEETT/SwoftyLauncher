#pragma once
#include <cstdint>

#include "swofty_api.h"

// The handler guard: an SEH frame around one SwoftyHookFn call. isolated_call catches access
// violations, C++ exceptions and the other faults a handler can recover from, and reports the code
// and address in seen. blamed_call lets every fault through to the crash path after calling blame,
// so the log and the black box name the handler first. guard.cpp is the only file built with
// -fms-extensions.

namespace swofty_core::hooks::guard {

constexpr unsigned long MSVC_CXX = 0xE06D7363, GCC_CXX = 0x20474343;

struct Seen {
    unsigned long code;
    uintptr_t at;
};

using BlameFn = void (*)(const void *ctx, unsigned long code, uintptr_t at);

int isolated_call(SwoftyHookFn fn, void *user, SwoftyHookCall *call, Seen *seen);
int blamed_call(SwoftyHookFn fn, void *user, SwoftyHookCall *call, BlameFn blame, const void *ctx);
bool recoverable(unsigned long code);

}
