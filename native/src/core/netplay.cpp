#include "runtime.hpp"

#include <swofty/mixin.hpp>

namespace {

const runtime::Sig S_HOST_GATE{"Game_SessionCreate_HostGate",
                               "48 8B 4C 24 30 83 E1 01 48 39 C1 0F 85 ?? ?? ?? ??", 0x30ae4a2,
                               runtime::Sig::addr, 0, 0xB, 0};

const runtime::Sig S_ENCRYPT_SELECT{"Net_Socket_EncryptionSelect",
                                    "48 85 C0 0F 84 ?? ?? ?? ?? 44 0F B6 7F 58", 0x4405fa6d,
                                    runtime::Sig::addr, 0, 9, 0};

bool patch(uintptr_t at, const uint8_t *bytes, size_t n)
{
    if (!at)
        return false;
    DWORD old;
    auto p = reinterpret_cast<void *>(at);
    if (!VirtualProtect(p, n, PAGE_EXECUTE_READWRITE, &old))
        return false;
    memcpy(p, bytes, n);
    VirtualProtect(p, n, old, &old);
    FlushInstructionCache(GetCurrentProcess(), p, n);
    return true;
}

void setup()
{
    if (!runtime::enabled("netplay", "netplay"))
        return;

    static const uint8_t open_gate[] = {0x90, 0x90, 0x90, 0x90, 0x90, 0x90};
    static const uint8_t force_null_ssl[] = {0x45, 0x31, 0xFF, 0x90, 0x90};

    bool gate = patch(runtime::find(S_HOST_GATE, "netplay"), open_gate, sizeof open_gate);
    swofty::log("[netplay] host-session gate %s", gate ? "opened" : "not patched (signature did not resolve)");

    bool enc = patch(runtime::find(S_ENCRYPT_SELECT, "netplay"), force_null_ssl, sizeof force_null_ssl);
    swofty::log("[netplay] transport encryption %s",
                enc ? "forced to the null-SSL provider" : "left unchanged (signature did not resolve)");
}

}

namespace runtime {

void netplay_setup()
{
    setup();
}

}
