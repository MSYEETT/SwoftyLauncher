#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <swofty_api.h>

extern "C" __declspec(dllexport) int SwoftyModInit(const SwoftyApi *a)
{
    HMODULE exe = GetModuleHandleW(nullptr);
    auto probe = reinterpret_cast<volatile int64_t *>(GetProcAddress(exe, "probe"));
    auto entry = *reinterpret_cast<uint8_t **>(GetProcAddress(exe, "fake_exports"));
    probe[2] = a->version;
    probe[3] = entry[0] == 0xE9 || entry[0] == 0xFF;
    a->log("[t_v1] API v%u, FeHome_Handle_Command starts with 0x%02x", a->version, entry[0]);
    return a->version == 1;
}
