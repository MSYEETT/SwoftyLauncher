#include "runtime.hpp"

#include <safetyhook.hpp>
#include <swofty/mixin.hpp>

#include "events_dispatch.hpp"

std::string settings_addon(const void *from, std::string &dll);

namespace {

const runtime::Sig S_FLOW{"Vm_Update_FlowStateMachine",
                          "41 57 41 56 41 55 41 54 56 57 55 53 48 81 EC 58 02 00 00 C5 F8 29 B4 24 40 02 00 00 C5 F9 28 F2",
                          0x4423ce0};
const runtime::Sig S_SESSION_ENTER{"Game_GameModule_OnSessionEnter",
                                   "41 57 41 56 41 54 56 57 53 48 81 EC 38 02 00 00 48 89 CE 80 3D ?? ?? ?? ?? 01",
                                   0x80c300};
const runtime::Sig S_RACE_STARTED{"Game_ChampionshipFormationLapComponent_OnRaceStarted",
                                  "41 57 41 56 41 55 41 54 56 57 55 53 48 83 EC 28 48 89 CE 80 3D ?? ?? ?? ?? 01 75 53",
                                  0xb808a0};

runtime::ev::Dispatcher disp;
safetyhook::MidHook h_flow, h_session_enter, h_race_started;
bool armed;

double now_seconds()
{
    static LARGE_INTEGER freq = [] {
        LARGE_INTEGER f;
        QueryPerformanceFrequency(&f);
        return f;
    }();
    LARGE_INTEGER t;
    QueryPerformanceCounter(&t);
    return freq.QuadPart ? double(t.QuadPart) / double(freq.QuadPart) : 0.0;
}

uint64_t caller_module(const void *from)
{
    HMODULE m = nullptr;
    GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                       reinterpret_cast<LPCWSTR>(from), &m);
    return reinterpret_cast<uint64_t>(m);
}

std::string who(const void *from)
{
    std::string dll;
    std::string id = settings_addon(from, dll);
    return id.empty() ? dll : id;
}

void flow_mid(safetyhook::Context &)
{
    double dt = disp.tick(now_seconds());
    static int logged;
    if (logged < 2)
        swofty::log("[events] flow tick %d observed, dt %f; onFrame is live", ++logged, dt);
}

void session_enter_mid(safetyhook::Context &)
{
    disp.fire(SWOFTY_EVENT_SESSION_START, 0, 0, nullptr, now_seconds());
}

void race_started_mid(safetyhook::Context &)
{
    disp.fire(SWOFTY_EVENT_RACE_START, 0, 0, nullptr, now_seconds());
}

int ev_available()
{
    return armed && runtime::enabled("events", nullptr);
}

int ev_subscribe(const char *event, SwoftyEventFn fn, void *user, uint32_t *token)
{
    const void *from = __builtin_return_address(0);
    if (!runtime::enabled("events", nullptr))
        return SWOFTY_ASSET_UNAVAILABLE;
    int r = disp.subscribe(caller_module(from), event, fn, user, token);
    swofty::log("[events] subscribe %s by %s -> %s%s", event ? event : "(null)", who(from).c_str(),
                r == SWOFTY_ASSET_OK ? "ok token " : "invalid",
                r == SWOFTY_ASSET_OK && token ? std::to_string(*token).c_str() : "");
    return r;
}

int ev_on_frame(SwoftyFrameFn fn, void *user, uint32_t *token)
{
    const void *from = __builtin_return_address(0);
    if (!runtime::enabled("events", nullptr))
        return SWOFTY_ASSET_UNAVAILABLE;
    int r = disp.on_frame(caller_module(from), fn, user, token);
    swofty::log("[events] on_frame by %s -> %s%s", who(from).c_str(), r == SWOFTY_ASSET_OK ? "ok token " : "invalid",
                r == SWOFTY_ASSET_OK && token ? std::to_string(*token).c_str() : "");
    return r;
}

int ev_unsubscribe(uint32_t token)
{
    if (!runtime::enabled("events", nullptr))
        return SWOFTY_ASSET_UNAVAILABLE;
    int r = disp.unsubscribe(token);
    swofty::log("[events] unsubscribe token %u -> %s", token, r == SWOFTY_ASSET_OK ? "ok" : "invalid");
    return r;
}

const SwoftyEvents table = {ev_available, ev_subscribe, ev_on_frame, ev_unsubscribe};

uintptr_t resolve(const runtime::Sig &s)
{
    uintptr_t at = 0, base = swofty::api->module_base();
    int hits = swofty::api->scan(".xtls", s.pattern, &at);
    if (hits != 1 || !at) {
        swofty::log("[events] %s signature matched %d time(s); hook not installed", s.what, hits);
        return 0;
    }
    if (at - base != s.rva)
        swofty::log("[events] %s resolved to base+0x%llx, KB build has base+0x%llx", s.what,
                    (unsigned long long)(at - base), (unsigned long long)s.rva);
    return at;
}

safetyhook::MidHook mid(const runtime::Sig &s, void (*fn)(safetyhook::Context &))
{
    uintptr_t at = resolve(s);
    if (!at)
        return {};
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), fn);
    if (!h) {
        swofty::log("[events] %s at base+0x%llx: MidHook failed", s.what, (unsigned long long)s.rva);
        return {};
    }
    swofty::log("[events] %s hooked at base+0x%llx", s.what, (unsigned long long)s.rva);
    return std::move(*h);
}

void setup()
{
    if (!runtime::enabled("events", "events")) {
        swofty::log("[events] feature off; subscribe/on_frame will report unavailable");
        return;
    }
    h_flow = mid(S_FLOW, flow_mid);
    h_session_enter = mid(S_SESSION_ENTER, session_enter_mid);
    h_race_started = mid(S_RACE_STARTED, race_started_mid);
    armed = bool(h_flow);
    swofty::log("[events] setup: %s; onFrame %s, session.start %s, race.start %s",
                armed ? "live" : "flow tick did not resolve (onFrame will not fire on this build)",
                h_flow ? "armed" : "off", h_session_enter ? "armed" : "off", h_race_started ? "armed" : "off");
}

}

namespace runtime {

const SwoftyEvents *events_table()
{
    return &table;
}

void events_setup()
{
    setup();
}

void events_drop_module(void *module)
{
    size_t n = disp.drop_owner(reinterpret_cast<uint64_t>(module));
    if (n)
        swofty::log("[events] dropped %zu subscription(s) from a module that unloaded or failed to init", n);
}

}
