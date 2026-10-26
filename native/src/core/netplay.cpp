#include "runtime.hpp"

#include <swofty/mixin.hpp>

namespace {

const runtime::Sig S_HOST_GATE{"Game_SessionCreate_HostGate",
                               "48 8B 4C 24 30 83 E1 01 48 39 C1 0F 85 ?? ?? ?? ??", 0x30ae4a2,
                               runtime::Sig::addr, 0, 0xB, 0};

const runtime::Sig S_ENCRYPT_SELECT{"Net_Socket_EncryptionSelect",
                                    "48 85 C0 0F 84 ?? ?? ?? ?? 44 0F B6 7F 58", 0x405fa6d,
                                    runtime::Sig::addr, 0, 9, 0};

// Lobby-enter privilege re-check. While a session is active (session object +0x1a8 set, i.e. the
// lobby has been entered), the session-update tick (base+0x30e0560) re-queries the same privilege
// provider as the host gate above (session+0x4a8, vtbl+0x130); offline it answers "not granted",
// so the tick hands reason 10 to the session error setter (base+0x30670c0), which surfaces as
// network_error_kicked_for_cheating and leaves the lobby. The anchored je is the provider-granted
// skip; making it an unconditional jmp turns the re-check off while the provider query itself, the
// remote-kick reasons and the content reconciliation stay untouched.
const runtime::Sig S_LOBBY_KICK{"Net_SessionTick_KickForCheating",
                                 "FF 90 30 01 00 00 0F B6 84 24 E0 02 00 00 48 8B 8C 24 E8 02 00 00"
                                 " 83 E1 01 48 39 C1 74 ??",
                                 0x30e6122, runtime::Sig::addr, 0, 0x1C, 0};

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
    static const uint8_t open_gate[] = {0x90, 0x90, 0x90, 0x90, 0x90, 0x90};
    static const uint8_t force_null_ssl[] = {0x45, 0x31, 0xFF, 0x90, 0x90};

    bool gate = patch(runtime::find(S_HOST_GATE, "netplay", "netplay"), open_gate, sizeof open_gate);
    swofty::log("[netplay] host-session gate %s", gate ? "opened" : "not patched (signature did not resolve)");

    bool enc = patch(runtime::find(S_ENCRYPT_SELECT, "netplay", "netplay"), force_null_ssl, sizeof force_null_ssl);
    swofty::log("[netplay] transport encryption %s",
                enc ? "forced to the null-SSL provider" : "left unchanged (signature did not resolve)");

    static const uint8_t skip_lobby_kick[] = {0xEB};
    bool lobby = patch(runtime::find(S_LOBBY_KICK, "netplay", "netplay"), skip_lobby_kick, sizeof skip_lobby_kick);
    swofty::log("[netplay] lobby privilege re-check %s",
                lobby ? "neutralised (no kicked-for-cheating at lobby enter)"
                      : "left unchanged (signature did not resolve)");
}

}

namespace runtime {

void netplay_setup()
{
    setup();
}

}
