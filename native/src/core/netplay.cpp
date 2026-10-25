#include "runtime.hpp"

#include <windows.h>

#include <swofty/mixin.hpp>

#include <cstdint>
#include <cstring>

namespace {

struct Target {
    const char *what, *pattern;
    uintptr_t rva;
    size_t patch_off;
};

// LAN host gate. The custom-session create path (base+0x30ae3f0) asks the privilege provider at
// session+0x4a8 whether hosting is allowed; offline there is no granted session, so it answers no
// and the create reports "CustomCreate failed : Banned". The game already proceeds down the create
// path when that provider is absent, so neutralising the one conditional jump takes that same route.
//   mov rcx,[rsp+0x30]; and ecx,1; cmp rcx,rax; jne <banned>
const Target T_GATE{"host gate (CustomCreate privilege jne)",
                    "48 8B 4C 24 30 83 E1 01 48 39 C1 0F 85 ?? ?? ?? ??", 0x30ae497, 0x0B};

// Transport encryption select, in the NeNetReliableAsyncSocket ctor (base+0x405f260). A byte at
// Security+0x58 (loaded into r15) chooses DTLS or the game's own NeNetNullSSLPeer. DTLS needs key
// material fetched from F1 Online Services, which an offline session never receives. Forcing r15 to
// zero selects the NULL provider the game ships, so there is no key fetch and nothing reaches out.
//   je <...>; movzx r15d, byte [rdi+0x58]; movzx ebp, word [rdi+0x50]
const Target T_SSL{"null-ssl (socket encryption select)",
                   "0F 84 ?? ?? ?? ?? 44 0F B6 7F 58 0F B7 6F 50", 0x405fa67, 0x06};

const uint8_t GATE_PATCH[] = {0x90, 0x90, 0x90, 0x90, 0x90, 0x90};
const uint8_t SSL_PATCH[] = {0x45, 0x31, 0xFF, 0x90, 0x90};

uintptr_t base;

bool off_by_config()
{
    char buf[8] = {};
    if (GetEnvironmentVariableA("SWOFTY_NETPLAY", buf, sizeof buf) && buf[0] == '0')
        return true;
    const json::Value &v = runtime::config()["netplay"];
    return v.type == json::Value::Bool && !v.b;
}

uintptr_t find(const Target &t)
{
    uintptr_t at = 0;
    int hits = swofty::api->scan(".xtls", t.pattern, &at);
    if (hits != 1 || !at) {
        swofty::log("[netplay] %s: signature matched %d time(s); not patched", t.what, hits);
        return 0;
    }
    if (at - base != t.rva)
        swofty::log("[netplay] %s: matched at base+0x%llx, KB build has base+0x%llx", t.what,
                    (unsigned long long)(at - base), (unsigned long long)t.rva);
    return at;
}

bool apply(const Target &t, const uint8_t *bytes, size_t n)
{
    uintptr_t at = find(t);
    if (!at)
        return false;
    void *p = reinterpret_cast<void *>(at + t.patch_off);
    DWORD old = 0;
    if (!VirtualProtect(p, n, PAGE_EXECUTE_READWRITE, &old)) {
        swofty::log("[netplay] %s: VirtualProtect failed (%lu); not patched", t.what, GetLastError());
        return false;
    }
    memcpy(p, bytes, n);
    VirtualProtect(p, n, old, &old);
    FlushInstructionCache(GetCurrentProcess(), p, n);
    swofty::log("[netplay] %s: patched %zu byte(s) at base+0x%llx", t.what, n,
                (unsigned long long)(at + t.patch_off - base));
    return true;
}

void netplay_load()
{
    base = swofty::api->module_base();
    if (!runtime::enabled("netplay", "netplay"))
        return;
    if (off_by_config()) {
        swofty::log("[netplay] turned off by config/env; LAN host stays gated and DTLS is unchanged");
        return;
    }
    bool gate = apply(T_GATE, GATE_PATCH, sizeof GATE_PATCH);
    bool ssl = apply(T_SSL, SSL_PATCH, sizeof SSL_PATCH);
    swofty::log("[netplay] setup: LAN host gate %s, null-ssl %s", gate ? "on" : "off", ssl ? "on" : "off");
}

}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("netplay: LAN host gate + null-ssl", netplay_load);
}
