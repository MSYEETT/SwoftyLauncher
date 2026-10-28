#include <windows.h>

#include "guard.hpp"

namespace swofty_core::hooks::guard {
namespace {

int isolate_filter(EXCEPTION_POINTERS *e, Seen *seen)
{
    DWORD code = e->ExceptionRecord->ExceptionCode;
    if (!recoverable(code))
        return EXCEPTION_CONTINUE_SEARCH;
    seen->code = code;
    seen->at = reinterpret_cast<uintptr_t>(e->ExceptionRecord->ExceptionAddress);
    return EXCEPTION_EXECUTE_HANDLER;
}

int blame_filter(EXCEPTION_POINTERS *e, BlameFn blame, const void *ctx)
{
    DWORD code = e->ExceptionRecord->ExceptionCode;
    if (recoverable(code) && blame)
        blame(ctx, code, reinterpret_cast<uintptr_t>(e->ExceptionRecord->ExceptionAddress));
    return EXCEPTION_CONTINUE_SEARCH;
}

}

bool recoverable(unsigned long code)
{
    switch (code) {
    case EXCEPTION_ACCESS_VIOLATION:
    case EXCEPTION_ILLEGAL_INSTRUCTION:
    case EXCEPTION_INT_DIVIDE_BY_ZERO:
    case EXCEPTION_INT_OVERFLOW:
    case EXCEPTION_PRIV_INSTRUCTION:
    case EXCEPTION_IN_PAGE_ERROR:
    case EXCEPTION_ARRAY_BOUNDS_EXCEEDED:
    case EXCEPTION_DATATYPE_MISALIGNMENT:
    case EXCEPTION_FLT_DIVIDE_BY_ZERO:
    case EXCEPTION_FLT_INVALID_OPERATION:
    case MSVC_CXX:
    case GCC_CXX:
        return true;
    default:
        return false;
    }
}

int isolated_call(SwoftyHookFn fn, void *user, SwoftyHookCall *call, Seen *seen)
{
    __try {
        return fn(user, call);
    } __except (isolate_filter(GetExceptionInformation(), seen)) {
        return 0;
    }
}

int blamed_call(SwoftyHookFn fn, void *user, SwoftyHookCall *call, BlameFn blame, const void *ctx)
{
    __try {
        return fn(user, call);
    } __except (blame_filter(GetExceptionInformation(), blame, ctx)) {
        return 0;
    }
}

}
