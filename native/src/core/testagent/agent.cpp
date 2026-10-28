#include "../runtime.hpp"

#include <safetyhook.hpp>
#include <swofty/game.hpp>
#include <swofty/mixin.hpp>
#include <swofty/test.hpp>

#include <atomic>
#include <cstring>
#include <deque>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

#include "agent_logic.hpp"

// The test agent (docs/design/std-2/test-suite.md, "The agent's commands"). It arms only when the
// game's environment names a run (SWOFTY_TEST) and its token (SWOFTY_TEST_TOKEN), and the run's
// mailbox <game>/swofty/test/<run>/session.json holds the same token: only `swofty test` writes
// those, so normal Play never arms it, never loads a test DLL and never installs its hooks. Once armed,
// a watcher thread does the file IO (commands.jsonl in, results.jsonl out, loader.log tailed for log
// waits) and the flow tick runs the commands, one step per frame, on the thread the menus run on.
// Deleting session.json stops the agent and lets go of the pad.

namespace runtime {
std::string hook_error(const safetyhook::MidHook::Error &e);
bool frame_capture_request(const std::wstring &path);
int frame_capture_poll(std::string *why);
}

namespace {

namespace ta = testagent;

constexpr DWORD POLL_MS = 20;
constexpr uint64_t SESSION_WAIT_MS = 15 * 60 * 1000;
constexpr size_t LOG_RING = 4096;

std::atomic<bool> armed, stopped, started;
ta::Session session;
std::wstring mbox;
std::mutex q_lock;
std::deque<ta::Command> incoming;
std::vector<std::string> outgoing;
std::mutex fire_lock;
std::vector<std::string> fired_ids;
std::atomic<uint64_t> ticks;
std::mutex phase_lock;
std::string screen_name, flow_name;
std::mutex log_lock;
std::deque<std::string> log_ring;
uint64_t log_total;

uint64_t wall_ms()
{
    FILETIME ft;
    GetSystemTimeAsFileTime(&ft);
    uint64_t t = (uint64_t(ft.dwHighDateTime) << 32) | ft.dwLowDateTime;
    return t / 10000 - 11644473600000ull;
}

std::string env(const char *name)
{
    char v[512] = {};
    DWORD n = GetEnvironmentVariableA(name, v, sizeof v);
    return n && n < sizeof v ? std::string(v, n) : std::string();
}

void emit(const std::string &line)
{
    std::lock_guard<std::mutex> g(q_lock);
    outgoing.push_back(line);
}

bool append(const std::wstring &path, const std::string &text)
{
    HANDLE h = CreateFileW(path.c_str(), FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                           nullptr, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (h == INVALID_HANDLE_VALUE)
        return false;
    DWORD w = 0;
    bool ok = WriteFile(h, text.data(), DWORD(text.size()), &w, nullptr) && w == text.size();
    CloseHandle(h);
    return ok;
}

// New bytes of a growing file, from *offset on.
std::string tail(const std::wstring &path, uint64_t *offset)
{
    HANDLE h = CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
                           OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (h == INVALID_HANDLE_VALUE)
        return {};
    LARGE_INTEGER size{};
    std::string out;
    if (GetFileSizeEx(h, &size) && uint64_t(size.QuadPart) > *offset) {
        uint64_t n = std::min<uint64_t>(uint64_t(size.QuadPart) - *offset, 1 << 20);
        LARGE_INTEGER at;
        at.QuadPart = LONGLONG(*offset);
        out.resize(size_t(n));
        DWORD r = 0;
        if (SetFilePointerEx(h, at, nullptr, FILE_BEGIN) && ReadFile(h, out.data(), DWORD(n), &r, nullptr)) {
            out.resize(r);
            *offset += r;
        } else
            out.clear();
    }
    CloseHandle(h);
    return out;
}

// The home view model, from the tile list populate loop: the view model that owns the list in r14
// (its TileList at +0x2c8) is on the stack, and the home one has FeHome_Handle_Command in vtable slot
// 10. Routes fire from it as the home tiles fire them: Vm_Fire_Route(vm->router (+0x28), vm, &name, 0).
const runtime::Sig S_POPULATE_LOOP{"site_FeTile_PopulateLoop", "49 8B 76 78 4D 8B B6 80 00 00 00 4C 39 F6 8B AC 24 00 03 00 00",
                                   0x1745016};
const runtime::Sig S_FIRE_CALL{"Vm_Fire_Route", "48 8B 4E 28 48 89 F2 49 89 F8 45 31 C9 E8 ?? ?? ?? ?? E9 E4 00 00 00",
                               0x42c0e20, runtime::Sig::rel32, 13, 14, 5};
constexpr size_t VM_TILE_LIST = 0x2c8, VM_ROUTER = 0x28, STACK_SCAN = 0x1000;

using Fire_t = void(void *router, void *vm, swofty::game::NeStr **name, uint64_t a4);
using MakeStr_t = uint32_t(swofty::game::NeStr **out, const char *s, uint32_t flags, uintptr_t len);
using Controller_t = void(void *system, uint64_t vehicle, uint32_t ai, int32_t reason);

// The agent's bindings resolve through the core when it arms, never as the core's own wanted bindings:
// normal Play never needs them, and a build without one only makes the commands that need it answer
// unavailable.
uintptr_t a_home_cmd, a_make_str, a_controller;

void make_str(swofty::game::NeStr **s, const char *text)
{
    reinterpret_cast<MakeStr_t *>(a_make_str)(s, text, 0, 0);
}

std::atomic<void *> home_vm{nullptr}, home_vt{nullptr};
uintptr_t a_fire;
safetyhook::MidHook h_populate;
std::string route_why = "not set up";

void set_home(void *vm)
{
    if (home_vm.load() != vm)
        swofty::log("[test] home view model %p", vm);
    home_vt = *static_cast<void **>(vm);
    home_vm = vm;
}

void *live_home()
{
    void *v = home_vm.load();
    return v && runtime::readable(v, VM_TILE_LIST + 8) && *static_cast<void **>(v) == home_vt.load() ? v : nullptr;
}

void populate_mid(safetyhook::Context &c)
{
    auto *list = reinterpret_cast<void *>(c.r14);
    auto *sp = reinterpret_cast<const uint8_t *>(c.rsp);
    if (!list || !sp || !runtime::readable(list, 0x90))
        return;
    for (size_t off = 0; off < STACK_SCAN; off += 8) {
        if ((off == 0 || (reinterpret_cast<uintptr_t>(sp + off) & 0xfff) == 0) && !runtime::readable(sp + off, 8))
            return;
        uint8_t *v;
        std::memcpy(&v, sp + off, sizeof v);
        if (!v || !runtime::readable(v, VM_TILE_LIST + 8) || *reinterpret_cast<void **>(v + VM_TILE_LIST) != list)
            continue;
        auto **vt = *reinterpret_cast<void ***>(v);
        if (runtime::readable(vt, 11 * sizeof(void *)) &&
            reinterpret_cast<uintptr_t>(vt[10]) == a_home_cmd)
            set_home(v);
        return;
    }
}

void routes_setup()
{
    uintptr_t loop = runtime::find(S_POPULATE_LOOP, "test", "test");
    a_fire = runtime::find(S_FIRE_CALL, "test", "test");
    a_home_cmd = swofty::api->resolve("FeHome_Handle_Command");
    a_make_str = swofty::api->resolve("Ne_Make_StrFromCStr");
    if (!a_fire || !a_make_str || !a_home_cmd) {
        route_why = "Vm_Fire_Route, Ne_Make_StrFromCStr or FeHome_Handle_Command did not resolve on this build";
        return;
    }
    if (!loop) {
        route_why = "the tile list populate loop (site_FeTile_PopulateLoop) did not resolve, so the home view model is "
                    "seen only after a home command";
        return;
    }
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(loop), populate_mid);
    if (!h) {
        route_why = "the populate loop hook failed: " + runtime::hook_error(h.error());
        return;
    }
    h_populate = std::move(*h);
    route_why.clear();
}

// The vehicle controller system, from the game's own calls of Game_Set_VehicleController_Thunk
// (session start hands cars to their controllers).
safetyhook::InlineHook h_controller;
std::atomic<void *> controller_system{nullptr};
std::string controller_why = "not set up";

void controller_detour(void *system, uint64_t vehicle, uint32_t ai, int32_t reason)
{
    if (controller_system.exchange(system) != system)
        swofty::log("[test] vehicle controller system %p (vehicle %u, ai %u, reason %d)", system, unsigned(vehicle & 0xff),
                    ai, reason);
    h_controller.call<void>(system, vehicle, ai, reason);
}

void controller_setup()
{
    uintptr_t a = a_controller = swofty::api->resolve("Game_Set_VehicleController_Thunk");
    if (!a) {
        controller_why = "Game_Set_VehicleController_Thunk did not resolve on this build";
        return;
    }
    auto h = safetyhook::InlineHook::create(reinterpret_cast<void *>(a), reinterpret_cast<void *>(controller_detour));
    if (!h) {
        controller_why = "the hook on Game_Set_VehicleController_Thunk failed";
        return;
    }
    h_controller = std::move(*h);
    controller_why.clear();
}

// The virtual pad: XInputGetState and XInputGetCapabilities in xinput1_4.dll (the game imports them
// by ordinal 2 and 4) report a connected pad in slot 0 while the agent is armed. A script's buttons
// replace a real pad's state while they are held; otherwise a real pad passes through.
struct XPad {
    uint16_t buttons;
    uint8_t lt, rt;
    int16_t lx, ly, rx, ry;
};
struct XState {
    DWORD packet;
    XPad pad;
};
struct XCaps {
    uint8_t type, subtype;
    uint16_t flags;
    XPad pad;
    uint16_t vib_l, vib_r;
};

std::atomic<uint32_t> pad_word{0};
std::atomic<bool> pad_held{false};
std::atomic<DWORD> pad_packet{1};
safetyhook::InlineHook h_xget, h_xcaps;
std::string pad_why = "not set up";

DWORD WINAPI x_get_state(DWORD user, XState *s)
{
    DWORD r = h_xget.call<DWORD>(user, s);
    if (user != 0 || !s || !armed.load() || stopped.load())
        return r;
    if (pad_held.load() || r != ERROR_SUCCESS) {
        uint32_t w = pad_word.load();
        std::memset(s, 0, sizeof *s);
        s->packet = pad_packet.load();
        s->pad.buttons = uint16_t(w);
        s->pad.lt = uint8_t(w >> 16);
        s->pad.rt = uint8_t(w >> 24);
    }
    return ERROR_SUCCESS;
}

DWORD WINAPI x_get_caps(DWORD user, DWORD flags, XCaps *c)
{
    DWORD r = h_xcaps.call<DWORD>(user, flags, c);
    if (user != 0 || !c || !armed.load() || stopped.load() || r == ERROR_SUCCESS)
        return r;
    std::memset(c, 0, sizeof *c);
    c->type = 1;
    c->subtype = 1;
    c->pad.buttons = 0xf3ff;
    c->pad.lt = c->pad.rt = 0xff;
    return ERROR_SUCCESS;
}

void pad_setup()
{
    HMODULE x = LoadLibraryW(L"xinput1_4.dll");
    void *get = x ? reinterpret_cast<void *>(GetProcAddress(x, MAKEINTRESOURCEA(2))) : nullptr;
    void *caps = x ? reinterpret_cast<void *>(GetProcAddress(x, MAKEINTRESOURCEA(4))) : nullptr;
    if (!get || !caps) {
        pad_why = "xinput1_4.dll or its XInputGetState/XInputGetCapabilities exports were not found";
        return;
    }
    auto g = safetyhook::InlineHook::create(get, reinterpret_cast<void *>(x_get_state));
    auto c = safetyhook::InlineHook::create(caps, reinterpret_cast<void *>(x_get_caps));
    if (!g || !c) {
        pad_why = "hooking xinput1_4.dll's exports failed";
        return;
    }
    h_xget = std::move(*g);
    h_xcaps = std::move(*c);
    pad_why.clear();
}

// Cars: g_Sess_VehicleTable (0x146ce86e0, L2, no signature) holds a CarLive per vehicle id, so it is
// read only on the build the KB describes. The field table is the KB's L3 rows (re/db/structs/Car*.h);
// "obf" fields are CarObfFloat pairs: the float's bits XOR the key 8 bytes on.
constexpr uintptr_t VEHICLE_TABLE = 0x6ce86e0, VEHICLE_COUNT = 0x6ce86dc;
constexpr size_t CAR_VEHICLE_ID = 0xe8, CAR_IS_AI = 0x124;
enum class Ty { f32, obf, u32, i32, u8 };
struct Field {
    const char *system, *field;
    size_t sys_off, off;
    Ty ty;
};
constexpr Field FIELDS[] = {
    {"chassis", "mass", 0x558, 0x150, Ty::f32},
    {"fuel_tank", "fuel_mass", 0x540, 0x100, Ty::f32},
    {"fuel_tank", "fuel_flow", 0x540, 0x108, Ty::f32},
    {"fuel_tank", "fuel_capacity", 0x540, 0x98, Ty::obf},
    {"engine", "max_torque", 0x530, 0x2518, Ty::obf},
    {"engine", "limiter_rotation_rate", 0x530, 0x2528, Ty::f32},
    {"engine", "rotation_rate", 0x530, 0x2538, Ty::f32},
    {"engine", "throttle", 0x530, 0x2574, Ty::f32},
    {"gearbox", "gear_index", 0x538, 0x680, Ty::u32},
    {"brake_ecu", "brake_bias", 0x550, 0x21d8, Ty::f32},
    {"ers", "energy", 0x5a8, 0x19f0, Ty::f32},
    {"ers", "ers_mode", 0x5a8, 0x1a68, Ty::i32},
    {"fuel_ecu", "fuel_mode", 0x548, 0x128, Ty::i32},
    {"f1_aero", "total_drag", 0x570, 0x19e8, Ty::f32},
};

const char *ty_name(Ty t)
{
    return t == Ty::f32 ? "f32" : t == Ty::obf ? "obf_f32" : t == Ty::u32 ? "u32" : t == Ty::i32 ? "i32" : "u8";
}

std::string car_unbound()
{
    if (!runtime::same_build())
        return "the vehicle table (g_Sess_VehicleTable 0x146ce86e0) has no signature and this is not the build the KB "
               "describes";
    return {};
}

uint8_t *car_at(int index, std::string &why)
{
    uintptr_t base = swofty::api->module_base();
    auto *count = reinterpret_cast<uint32_t *>(base + VEHICLE_COUNT);
    auto **table = reinterpret_cast<uint8_t **>(base + VEHICLE_TABLE);
    if (!runtime::readable(count, 4) || !runtime::readable(table, 8 * 26)) {
        why = "the vehicle table is not readable";
        return nullptr;
    }
    if (index < 0 || index >= 26 || uint32_t(index) >= *count) {
        why = "no car " + std::to_string(index) + " (the session has " + std::to_string(*count) + ")";
        return nullptr;
    }
    uint8_t *car = table[index];
    if (!car || !runtime::readable(car, 0x5b0)) {
        why = "car " + std::to_string(index) + " is not loaded";
        return nullptr;
    }
    return car;
}

int player_index(std::string &why)
{
    uintptr_t base = swofty::api->module_base();
    auto *count = reinterpret_cast<uint32_t *>(base + VEHICLE_COUNT);
    if (!runtime::readable(count, 4) || !*count) {
        why = "no cars in the session";
        return -1;
    }
    for (int i = 0; i < int(*count) && i < 26; i++) {
        std::string w;
        uint8_t *car = car_at(i, w);
        if (car && car[CAR_IS_AI] == 0)
            return i;
    }
    why = "no car is driven by a human (every CarLive.is_ai is set)";
    return -1;
}

uint8_t *field_at(const std::string &path, const Field **out, std::string &why)
{
    ta::CarPath p;
    if (!ta::parse_car_path(path, p, why))
        return nullptr;
    const Field *f = nullptr;
    for (auto &x : FIELDS)
        if (p.system == x.system && p.field == x.field)
            f = &x;
    if (!f) {
        why = "no car field " + p.system + "." + p.field + " in the agent's table";
        return nullptr;
    }
    why = car_unbound();
    if (!why.empty())
        return nullptr;
    int idx = p.player ? player_index(why) : p.index;
    if (idx < 0)
        return nullptr;
    uint8_t *car = car_at(idx, why);
    if (!car)
        return nullptr;
    uint8_t *sys = *reinterpret_cast<uint8_t **>(car + f->sys_off);
    if (!sys || !runtime::readable(sys + f->off, 16)) {
        why = "car " + std::to_string(idx) + " has no " + p.system;
        return nullptr;
    }
    *out = f;
    return sys + f->off;
}

std::string value_json(const uint8_t *at, Ty t)
{
    double v = 0;
    uint32_t bits;
    switch (t) {
    case Ty::f32: {
        float f;
        std::memcpy(&f, at, 4);
        v = f;
        break;
    }
    case Ty::obf: {
        uint32_t key;
        std::memcpy(&bits, at, 4);
        std::memcpy(&key, at + 8, 4);
        bits ^= key;
        float f;
        std::memcpy(&f, &bits, 4);
        v = f;
        break;
    }
    case Ty::u32: std::memcpy(&bits, at, 4); v = bits; break;
    case Ty::i32: {
        int32_t i;
        std::memcpy(&i, at, 4);
        v = i;
        break;
    }
    case Ty::u8: v = at[0]; break;
    }
    return "{\"value\":" + ta::num(v) + ",\"type\":\"" + ty_name(t) + "\"}";
}

// Test DLLs, loaded from the mailbox once armed.
using Names_t = const char *();
using Start_t = void *(const char *, const SwoftyTestHost *);
using Step_t = int(void *);
using End_t = void(void *);
struct Dll {
    std::string name;
    HMODULE m;
    Names_t *names;
    Start_t *start;
    Step_t *step;
    End_t *end;
};
std::vector<Dll> dlls;

void load_dlls()
{
    for (auto &rel : session.dlls) {
        std::wstring path = mbox + L"\\" + runtime::wide(rel);
        for (auto &c : path)
            if (c == L'/')
                c = L'\\';
        HMODULE m = LoadLibraryW(path.c_str());
        Dll d{rel, m, nullptr, nullptr, nullptr, nullptr};
        if (m) {
            d.names = reinterpret_cast<Names_t *>(GetProcAddress(m, "SwoftyTestNames"));
            d.start = reinterpret_cast<Start_t *>(GetProcAddress(m, "SwoftyTestStart"));
            d.step = reinterpret_cast<Step_t *>(GetProcAddress(m, "SwoftyTestStep"));
            d.end = reinterpret_cast<End_t *>(GetProcAddress(m, "SwoftyTestEnd"));
        }
        bool ok = m && d.names && d.start && d.step && d.end;
        swofty::log("[test] test DLL %s: %s", rel.c_str(),
                    !m ? "did not load" : ok ? "loaded" : "has no SWOFTY_TESTS() exports");
        if (ok)
            dlls.push_back(d);
    }
}

struct InGame : ta::Game {
    ta::Runner sub{*this};
    uint64_t sub_seq = 0;
    std::vector<std::pair<uint64_t, std::string>> sub_done;
    Dll *test_dll = nullptr;
    void *test_run = nullptr;
    std::string expects;
    size_t failed = 0;
    std::string capture_file;
    SwoftyTestHost host{sizeof(SwoftyTestHost), SWOFTY_TEST_HOST_VERSION, this, host_start, host_poll, host_expect,
                        host_log, host_read};

    static uint64_t host_start(void *self, const char *line)
    {
        auto *g = static_cast<InGame *>(self);
        std::string l = line ? line : "";
        size_t brace = l.find('{');
        if (brace == std::string::npos)
            return 0;
        uint64_t seq = ++g->sub_seq;
        l.insert(brace + 1, "\"seq\":" + std::to_string(seq) + ",");
        ta::Command c = ta::parse_command(l);
        if (c.cmd == "run_test" || c.cmd == "quit")
            c.error = c.cmd + " is not available inside a test";
        g->sub.submit(std::move(c));
        return seq;
    }

    static int host_poll(void *self, uint64_t op, char *out, size_t cap)
    {
        auto *g = static_cast<InGame *>(self);
        for (auto &[seq, line] : g->sub_done)
            if (seq == op) {
                if (out && cap)
                    snprintf(out, cap, "%s", line.c_str());
                return line.find("\"status\":\"ok\"") != std::string::npos ? 1 : 2;
            }
        return 0;
    }

    static void host_expect(void *self, int ok, const char *what)
    {
        auto *g = static_cast<InGame *>(self);
        std::string w = what ? what : "";
        g->expects += std::string(g->expects.size() > 1 ? "," : "") + "{\"ok\":" + (ok ? "true" : "false") +
                      ",\"what\":" + ta::esc(w) + "}";
        g->failed += !ok;
        emit("{\"event\":\"expect\",\"ok\":" + std::string(ok ? "true" : "false") + ",\"what\":" + ta::esc(w) + "}");
    }

    static void host_log(void *, const char *text)
    {
        swofty::log("[test] %s", text ? text : "");
        emit("{\"event\":\"log\",\"text\":" + ta::esc(text ? text : "") + "}");
    }

    static int host_read(void *self, const char *path, char *out, size_t cap)
    {
        ta::Outcome o = static_cast<InGame *>(self)->read(path ? path : "");
        bool ok = o.status == ta::Status::ok;
        if (out && cap)
            snprintf(out, cap, "%s", ok ? o.value.c_str() : o.error.c_str());
        return ok;
    }

    ta::Outcome hello() override
    {
        std::string c = "{\"route\":" + ta::esc(route_why.empty() ? "ok" : route_why) +
                        ",\"pad\":" + ta::esc(pad_why.empty() ? "ok" : pad_why) +
                        ",\"ai_drives_player\":" + ta::esc(controller_why.empty() ? "ok" : controller_why) +
                        ",\"car\":" + ta::esc(car_unbound().empty() ? "ok" : car_unbound()) + "}";
        std::string names = "[";
        for (auto &d : dlls)
            for (const char *p = d.names(); p && *p;) {
                const char *nl = std::strchr(p, '\n');
                std::string n = nl ? std::string(p, nl - p) : std::string(p);
                names += (names.size() > 1 ? "," : "") + ta::esc(n);
                p = nl ? nl + 1 : nullptr;
            }
        return ta::Outcome::ok("{\"v\":" + std::to_string(ta::PROTOCOL) + ",\"pid\":" + std::to_string(GetCurrentProcessId()) +
                               ",\"run\":" + ta::esc(session.run) + ",\"ticks\":" + std::to_string(ticks.load()) +
                               ",\"phase\":" + ta::esc(phase()) + ",\"bindings\":" + c + ",\"tests\":" + names + "]}");
    }

    ta::Outcome route(const std::string &name) override
    {
        if (!route_why.empty())
            return ta::Outcome::unavailable(route_why);
        void *vm = live_home();
        if (!vm)
            return ta::Outcome::unavailable("no home view model: the home screen has not been built since the agent armed");
        void *router = *reinterpret_cast<void **>(static_cast<uint8_t *>(vm) + VM_ROUTER);
        if (!router)
            return ta::Outcome::unavailable("the home view model has no router");
        swofty::game::NeStr *s = nullptr;
        make_str(&s, name.c_str());
        reinterpret_cast<Fire_t *>(a_fire)(router, vm, &s, 0);
        make_str(&s, nullptr);
        swofty::log("[test] route %s fired from the home view model", name.c_str());
        return ta::Outcome::ok();
    }

    ta::Outcome skip(int destination, int session_type) override
    {
        if (!runtime::same_build())
            return ta::Outcome::unavailable("MessageSetAdvanceDestination's vtable (0x1457548b8) has no signature and this "
                                            "is not the build the KB describes");
        const SwoftyCareer *career = runtime::career_table();
        if (!career || !career->post)
            return ta::Outcome::unavailable("the career API's message post is not available");
        struct alignas(8) Message {
            void *vt;
            uint64_t zero;
            uint32_t destination, session;
        } m{reinterpret_cast<void *>(swofty::api->module_base() + 0x57548b8), 0, uint32_t(destination),
            uint32_t(session_type)};
        int r = career->post(&m);
        swofty::log("[test] skip: MessageSetAdvanceDestination destination %d session %d posted -> %d", destination,
                    session_type, r);
        if (r == SWOFTY_ASSET_LATE)
            return ta::Outcome::unavailable("the message bus is not active yet");
        if (r != SWOFTY_ASSET_OK)
            return ta::Outcome::fail("the message bus refused the message (" + std::to_string(r) + ")");
        return ta::Outcome::ok();
    }

    ta::Outcome advance(int) override
    {
        return ta::Outcome::unavailable("the Team HQ advance view model is not reachable: Career_TeamHqAdvance_Start "
                                        "(0x1422bb0f0, L1) takes vm+0x730, and no core hook sees the "
                                        "TeamHQAdvanceViewModel yet");
    }

    ta::Outcome ai_drives_player(bool on) override
    {
        if (!controller_why.empty())
            return ta::Outcome::unavailable(controller_why);
        void *sys = controller_system.load();
        if (!sys)
            return ta::Outcome::unavailable("the game has not handed a car to a controller since the agent armed, so the "
                                            "vehicle controller system is not known yet (start a session first)");
        std::string why = car_unbound();
        int idx = why.empty() ? player_index(why) : -1;
        if (idx < 0)
            return ta::Outcome::unavailable("the player's car: " + why);
        uint8_t *car = car_at(idx, why);
        if (!car)
            return ta::Outcome::unavailable("the player's car: " + why);
        uint8_t vid = car[CAR_VEHICLE_ID];
        reinterpret_cast<Controller_t *>(a_controller)(sys, vid, on ? 1 : 0, 6);
        swofty::log("[test] ai_drives_player %s: vehicle %u, controller system %p, reason 6 (dev)", on ? "on" : "off",
                    unsigned(vid), sys);
        return ta::Outcome::ok("{\"vehicle\":" + std::to_string(vid) + "}");
    }

    ta::Outcome read(const std::string &path) override
    {
        if (path == "agent.ticks")
            return ta::Outcome::ok("{\"value\":" + std::to_string(ticks.load()) + ",\"type\":\"u64\"}");
        if (path == "agent.run")
            return ta::Outcome::ok("{\"value\":" + ta::esc(session.run) + ",\"type\":\"str\"}");
        if (path == "ui.screen" || path == "flow.state") {
            std::lock_guard<std::mutex> g(phase_lock);
            return ta::Outcome::ok("{\"value\":" + ta::esc(path == "ui.screen" ? screen_name : flow_name) +
                                   ",\"type\":\"str\"}");
        }
        if (path.rfind("car[", 0) == 0) {
            const Field *f = nullptr;
            std::string why;
            uint8_t *at = field_at(path, &f, why);
            if (!at)
                return ta::Outcome::unavailable(why);
            return ta::Outcome::ok(value_json(at, f->ty));
        }
        return ta::Outcome::unavailable("no reader for " + path +
                                        " (this agent reads agent.ticks, agent.run, ui.screen, flow.state and "
                                        "car[<index>|player].<system>.<field>)");
    }

    ta::Outcome set(const std::string &path, const json::Value &value) override
    {
        if (path.rfind("car[", 0) != 0)
            return ta::Outcome::unavailable("only car fields can be set (car[<index>|player].<system>.<field>)");
        if (value.type != json::Value::Num)
            return ta::Outcome::fail("a car field takes a number");
        const Field *f = nullptr;
        std::string why;
        uint8_t *at = field_at(path, &f, why);
        if (!at)
            return ta::Outcome::unavailable(why);
        std::string before = value_json(at, f->ty);
        float fl = float(value.n);
        uint32_t bits;
        switch (f->ty) {
        case Ty::f32: std::memcpy(at, &fl, 4); break;
        case Ty::obf: {
            uint32_t key;
            std::memcpy(&key, at + 8, 4);
            std::memcpy(&bits, &fl, 4);
            bits ^= key;
            std::memcpy(at, &bits, 4);
            break;
        }
        case Ty::u32:
        case Ty::i32: {
            int32_t i = int32_t(value.n);
            std::memcpy(at, &i, 4);
            break;
        }
        case Ty::u8: at[0] = uint8_t(value.n); break;
        }
        std::string after = value_json(at, f->ty);
        swofty::log("[test] set %s: %s -> %s", path.c_str(), before.c_str(), after.c_str());
        return ta::Outcome::ok("{\"before\":" + before + ",\"after\":" + after + "}");
    }

    ta::Outcome capture(const std::string &name) override
    {
        std::wstring dir = mbox + L"\\frames";
        CreateDirectoryW(dir.c_str(), nullptr);
        capture_file = "frames/" + name + ".png";
        if (!runtime::frame_capture_request(dir + L"\\" + runtime::wide(name) + L".png"))
            return ta::Outcome::unavailable("a frame capture is already waiting for a frame");
        return ta::Outcome::ok();
    }

    ta::Outcome capture_poll() override
    {
        std::string why;
        int r = runtime::frame_capture_poll(&why);
        if (r == 0)
            return ta::Outcome::pending();
        if (r < 0)
            return ta::Outcome::unavailable(why);
        return ta::Outcome::ok("{\"file\":" + ta::esc(capture_file) + "}");
    }

    ta::Outcome save_now() override
    {
        return ta::Outcome::unavailable("no save trigger is bound: the SaveProfile request thunk (0x1436595b0) and "
                                        "Game_TOPLevelUI_TriggerProfileSave (0x140ff7cf0, L1) need RE first");
    }

    static BOOL CALLBACK close_main(HWND h, LPARAM count)
    {
        DWORD pid = 0;
        GetWindowThreadProcessId(h, &pid);
        if (pid == GetCurrentProcessId() && IsWindowVisible(h) && !GetWindow(h, GW_OWNER) && PostMessageW(h, WM_CLOSE, 0, 0))
            ++*reinterpret_cast<int *>(count);
        return TRUE;
    }

    ta::Outcome quit() override
    {
        int closed = 0;
        EnumWindows(close_main, reinterpret_cast<LPARAM>(&closed));
        swofty::log("[test] quit: WM_CLOSE posted to %d main window(s)", closed);
        if (!closed)
            return ta::Outcome::unavailable("no visible main window to close; the CLI closes the game by its pid");
        return ta::Outcome::ok("{\"windows\":" + std::to_string(closed) + "}");
    }

    ta::Outcome pad(const ta::PadState &p) override
    {
        if (!pad_why.empty())
            return ta::Outcome::unavailable(pad_why);
        pad_word = uint32_t(p.buttons) | uint32_t(p.lt) << 16 | uint32_t(p.rt) << 24;
        pad_held = p.buttons || p.lt || p.rt;
        pad_packet.fetch_add(1);
        return ta::Outcome::ok();
    }

    std::string phase() override
    {
        std::lock_guard<std::mutex> g(phase_lock);
        return "|" + screen_name + "|" + flow_name + "|";
    }

    ta::Outcome test_start(const std::string &fn) override
    {
        for (auto &d : dlls)
            for (const char *p = d.names(); p && *p;) {
                const char *nl = std::strchr(p, '\n');
                std::string n = nl ? std::string(p, nl - p) : std::string(p);
                p = nl ? nl + 1 : nullptr;
                if (n != fn)
                    continue;
                expects.clear();
                failed = 0;
                sub_done.clear();
                test_run = d.start(fn.c_str(), &host);
                if (!test_run)
                    return ta::Outcome::fail("the test DLL did not start " + fn);
                test_dll = &d;
                swofty::log("[test] run_test %s (%s)", fn.c_str(), d.name.c_str());
                return ta::Outcome::ok();
            }
        return ta::Outcome::unavailable(dlls.empty() ? "no test DLL is loaded" : "no test DLL registers " + fn);
    }

    ta::Outcome test_poll() override
    {
        if (!test_run)
            return ta::Outcome::fail("no test is running");
        std::vector<std::string> out;
        sub.tick(GetTickCount64(), out);
        for (auto &l : out) {
            json::Value v;
            if (json::parse(l, v))
                sub_done.emplace_back(uint64_t(v["seq"].num()), l);
        }
        int r = test_dll->step(test_run);
        if (r == 0)
            return ta::Outcome::pending();
        test_dll->end(test_run);
        test_run = nullptr;
        std::string value = "{\"expects\":[" + expects + "]}";
        swofty::log("[test] run_test finished: %s (%zu failed expectation(s))",
                    r == 1 ? "passed" : r == 2 ? "failed" : "the test threw", failed);
        if (r == 1)
            return ta::Outcome::ok(value);
        ta::Outcome o = ta::Outcome::fail(r == 3 ? "the test threw an exception"
                                                 : std::to_string(failed) + " expectation(s) failed");
        o.value = value;
        return o;
    }

    uint64_t log_lines(uint64_t from, std::vector<std::string> &out) override
    {
        std::lock_guard<std::mutex> g(log_lock);
        uint64_t first = log_total - log_ring.size();
        if (from == UINT64_MAX)
            return log_total;
        for (uint64_t i = std::max(from, first); i < log_total; i++)
            out.push_back(log_ring[size_t(i - first)]);
        return log_total;
    }
};

InGame *game;
ta::Runner *runner;

void refuse(const std::string &why)
{
    swofty::log("[test] agent refused: %s", why.c_str());
    DWORD a = GetFileAttributesW(mbox.c_str());
    if (a != INVALID_FILE_ATTRIBUTES && (a & FILE_ATTRIBUTE_DIRECTORY))
        append(mbox + L"\\results.jsonl", "{\"event\":\"refused\",\"why\":" + ta::esc(why) + "}\n");
}

void watch()
{
    std::wstring session_path = mbox + L"\\session.json";
    uint64_t start = GetTickCount64();
    std::string text;
    for (;;) {
        text = runtime::read_file(session_path, 64 << 10);
        if (!text.empty() || GetTickCount64() - start > SESSION_WAIT_MS)
            break;
        Sleep(250);
    }
    std::string why = ta::arm_check(session.run, env("SWOFTY_TEST_TOKEN"), text, wall_ms(), &session);
    if (!why.empty()) {
        refuse(why);
        return;
    }
    routes_setup();
    pad_setup();
    controller_setup();
    load_dlls();
    game = new InGame;
    runner = new ta::Runner(*game);
    armed = true;
    swofty::log("[test] agent armed for run %s: %zu test DLL(s); route %s, pad %s, ai_drives_player %s",
                session.run.c_str(), dlls.size(), route_why.empty() ? "ok" : route_why.c_str(),
                pad_why.empty() ? "ok" : pad_why.c_str(), controller_why.empty() ? "ok" : controller_why.c_str());
    ta::Outcome h = game->hello();
    std::string hello = h.value;
    hello.insert(1, "\"event\":\"hello\",");
    std::wstring results = mbox + L"\\results.jsonl", commands = mbox + L"\\commands.jsonl";
    std::wstring log_path = runtime::under_swofty("loader.log");
    append(results, hello + "\n");
    uint64_t cmd_off = 0, log_off = 0;
    std::string cmd_part, log_part;
    {
        std::string skip = tail(log_path, &log_off);
        (void)skip;
    }
    for (;;) {
        if (GetFileAttributesW(session_path.c_str()) == INVALID_FILE_ATTRIBUTES) {
            stopped = true;
            pad_word = 0;
            pad_held = false;
            swofty::log("[test] session.json removed: the agent stops and lets go of the pad");
            return;
        }
        cmd_part += tail(commands, &cmd_off);
        for (size_t nl; (nl = cmd_part.find('\n')) != std::string::npos;) {
            std::string line = cmd_part.substr(0, nl);
            cmd_part.erase(0, nl + 1);
            if (line.find_first_not_of(" \r\t") == std::string::npos)
                continue;
            ta::Command c = ta::parse_command(line);
            if (!c.seq) {
                append(results, "{\"event\":\"bad_command\",\"why\":" + ta::esc(c.error) + "}\n");
                continue;
            }
            std::lock_guard<std::mutex> g(q_lock);
            incoming.push_back(std::move(c));
        }
        log_part += tail(log_path, &log_off);
        {
            std::lock_guard<std::mutex> g(log_lock);
            for (size_t nl; (nl = log_part.find('\n')) != std::string::npos;) {
                log_ring.push_back(log_part.substr(0, nl));
                log_part.erase(0, nl + 1);
                ++log_total;
                if (log_ring.size() > LOG_RING)
                    log_ring.pop_front();
            }
        }
        std::vector<std::string> out;
        {
            std::lock_guard<std::mutex> g(q_lock);
            out.swap(outgoing);
        }
        std::string body;
        for (auto &l : out)
            body += l + "\n";
        if (!body.empty() && !append(results, body))
            swofty::log("[test] could not append to results.jsonl");
        Sleep(POLL_MS);
    }
}

}

namespace runtime {

void testagent_setup()
{
    std::string run = env("SWOFTY_TEST");
    if (run.empty() || started.exchange(true))
        return;
    if (!testagent::valid_run(run)) {
        swofty::log("[test] agent refused: SWOFTY_TEST is not a run name");
        return;
    }
    session.run = run;
    mbox = runtime::under_swofty("test\\" + run);
    swofty::log("[test] SWOFTY_TEST=%s: waiting for the run's session.json to arm the test agent", run.c_str());
    std::thread(watch).detach();
}

void testagent_tick()
{
    if (!armed.load() || stopped.load())
        return;
    ++ticks;
    {
        std::lock_guard<std::mutex> g(fire_lock);
        for (auto &id : fired_ids)
            runner->fires().fired(id);
        fired_ids.clear();
    }
    {
        std::lock_guard<std::mutex> g(q_lock);
        while (!incoming.empty()) {
            runner->submit(std::move(incoming.front()));
            incoming.pop_front();
        }
    }
    std::vector<std::string> out;
    runner->tick(GetTickCount64(), out);
    for (auto &l : out)
        emit(l);
}

void testagent_fired(const std::string &id, bool hot)
{
    if (!armed.load())
        return;
    {
        std::lock_guard<std::mutex> g(fire_lock);
        fired_ids.push_back(id);
    }
    if (!hot)
        emit("{\"event\":\"fired\",\"id\":" + testagent::esc(id) + "}");
}

void testagent_screen(const char *name)
{
    if (!armed.load() || !name)
        return;
    std::lock_guard<std::mutex> g(phase_lock);
    screen_name = name;
}

void testagent_flow(const std::string &name, uint32_t to)
{
    if (!armed.load() || (to != 1 && to != 2))
        return;
    std::lock_guard<std::mutex> g(phase_lock);
    flow_name = name;
}

void testagent_home_vm(void *vm)
{
    if (armed.load() && vm && runtime::readable(vm, VM_TILE_LIST + 8))
        set_home(vm);
}

}
