#include "runtime.hpp"

#include <safetyhook.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <cstring>
#include <deque>
#include <mutex>
#include <set>
#include <string>
#include <thread>

#include "netplay_drive_logic.hpp"
#include "netplay_session.hpp"

// Launcher commands for LAN netplay, over the same kind of file mailbox the Career Editor uses
// (protocol/bridge.md there): <game>/swofty/netplay/inbox/<seq>.json from the launcher (written as
// .tmp and renamed), one reply per command in outbox/<seq>.json, and status.json rewritten when it
// changes and every 5 s. A watcher thread does all file IO; commands run on the flow thread from the
// flow tick (runtime::netplay_tick), the thread the menus' own handlers run on.
//
// State comes from what core already sees: flow states (StateFrontend for the front end,
// StateOnlineLobby for a LAN lobby) and the screens the UI opens (fe_home for the main menu).
// status and clear always answer. configure (the room: member number and bridge base port, shared
// with the bridge hooks through netplay_session.hpp), host, join and leave need SWOFTY_NETPLAY in
// the game's environment, host and join also a configured room, and each a game state that allows
// it. host, join and leave report unavailable until their game entry points are bound
// (research/multiplayer/lan-netplay-design.md, "Driving host/join").

namespace runtime {
std::string hook_error(const safetyhook::MidHook::Error &e);
}

namespace {

namespace drive = runtime::drive;

// The systems the LAN tile's getAction switches with (base+0x172c840: game mode "online_custom",
// then NetworkSubsystem::changeMPMode to eLan) come from its init (base+0x172c5a0), which looks
// them up in the tile builder's context: the game mode manager into item+0x148, the network
// subsystem into item+0x150. With the LAN tile no longer built, a probe at the builder's populate
// loop (site_FeTile_PopulateLoop, where rbx and rdi are the init arguments of every tile) runs that
// init on a private LAN item while armed, so the mode switch can later run on it without the tile.
// The probe only reads; it logs what it reached.
const runtime::Sig S_LAN_VT{"vt_FeTile_LanItem",
                            "48 8D 05 ?? ?? ?? ?? 49 89 04 24 C4 C1 78 11 B4 24 48 01 00 00 41 C6 84 24 58 01 00 00 00",
                            0x5704740, runtime::Sig::riprel, 0, 3, 7};
const runtime::Sig S_POPULATE_LOOP{"site_FeTile_PopulateLoop",
                                   "49 8B 76 78 4D 8B B6 80 00 00 00 4C 39 F6 8B AC 24 00 03 00 00", 0x1745016};
constexpr size_t LAN_ITEM_SIZE = 0x160, LAN_GAME_MODES = 0x148, LAN_NETWORK = 0x150, LAN_ACTIVE = 0x158;
constexpr unsigned MAX_PROBES = 8;

using LanInit_t = uint64_t(void *item, void *registry, void *ctx);

alignas(16) uint8_t lan_item[LAN_ITEM_SIZE];
void **lan_vt;
safetyhook::MidHook h_populate;
unsigned probes;
std::atomic<bool> lan_systems;

template <class T> T lan_field(size_t off)
{
    T v;
    std::memcpy(&v, lan_item + off, sizeof v);
    return v;
}

void populate_mid(safetyhook::Context &c)
{
    if (lan_systems.load() || probes >= MAX_PROBES)
        return;
    auto *registry = reinterpret_cast<void *>(c.rbx), *ctx = reinterpret_cast<void *>(c.rdi);
    if (!runtime::readable(registry, 8) || !runtime::readable(ctx, 0x28))
        return;
    ++probes;
    std::memset(lan_item, 0, sizeof lan_item);
    std::memcpy(lan_item, &lan_vt, sizeof lan_vt);
    uint64_t rc = reinterpret_cast<LanInit_t *>(lan_vt[3])(lan_item, registry, ctx);
    auto *modes = lan_field<void *>(LAN_GAME_MODES), *network = lan_field<void *>(LAN_NETWORK);
    bool ok = rc == 0 && modes && network;
    lan_systems = ok;
    swofty::log("[netplay] LAN systems probe %u: init returned 0x%llx, game modes %p, network %p, Network.LAN.active %u%s",
                probes, (unsigned long long)rc, modes, network, unsigned(lan_item[LAN_ACTIVE]),
                ok ? "; the mode switch can run without the LAN tile" : "");
}

void probe_setup()
{
    uintptr_t vt = runtime::find(S_LAN_VT, "netplay", "netplay"), loop = runtime::find(S_POPULATE_LOOP, "netplay", "netplay");
    if (!vt || !loop || !runtime::readable(reinterpret_cast<void *>(vt), 6 * sizeof(void *))) {
        swofty::log("[netplay] LAN systems probe not installed (LAN item vtable %s, populate loop %s)",
                    vt ? "found" : "missing", loop ? "found" : "missing");
        return;
    }
    lan_vt = reinterpret_cast<void **>(vt);
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(loop), populate_mid);
    if (!h) {
        swofty::log("[netplay] LAN systems probe: the hook failed: %s", runtime::hook_error(h.error()).c_str());
        return;
    }
    h_populate = std::move(*h);
    swofty::log("[netplay] LAN systems probe at base+0x%llx (LAN item init base+0x%llx, getAction base+0x%llx)",
                (unsigned long long)(loop - swofty::api->module_base()),
                (unsigned long long)(reinterpret_cast<uintptr_t>(lan_vt[3]) - swofty::api->module_base()),
                (unsigned long long)(reinterpret_cast<uintptr_t>(lan_vt[5]) - swofty::api->module_base()));
}

constexpr DWORD POLL_MS = 250;
constexpr uint64_t HEARTBEAT_MS = 5000;

std::atomic<bool> armed, frontend, lobby, home_screen;
std::atomic<int> role{int(drive::Role::none)};

std::mutex queue_lock;
std::deque<drive::Command> incoming;
std::deque<std::pair<uint64_t, std::string>> outgoing;
std::atomic<bool> started;

drive::State state()
{
    drive::State s;
    s.armed = armed.load();
    s.configured = runtime::netplay_session::configured();
    s.self = runtime::netplay_session::self_member.load();
    s.base = runtime::netplay_session::base_port.load();
    s.frontend = frontend.load();
    s.lobby = lobby.load();
    s.home_screen = home_screen.load();
    s.role = drive::Role(role.load());
    return s;
}

uint64_t wall_ms()
{
    FILETIME ft;
    GetSystemTimeAsFileTime(&ft);
    uint64_t t = (uint64_t(ft.dwHighDateTime) << 32) | ft.dwLowDateTime;
    return t / 10000 - 11644473600000ull;
}

std::wstring dir(const char *sub)
{
    return runtime::under_swofty(std::string("netplay\\") + sub);
}

std::string run_host(const drive::Command &)
{
    return "unavailable: hosting is not bound in this build (needs the LAN mode switch and Host LAN game entry points)";
}

std::string run_join(const drive::Command &)
{
    return "unavailable: joining is not bound in this build (needs the LAN mode switch and Join via IP entry points)";
}

std::string run_leave(const drive::Command &)
{
    return "unavailable: leaving is not bound in this build";
}

void watch()
{
    std::wstring inbox = dir("inbox"), outbox = dir("outbox");
    CreateDirectoryW(runtime::under_swofty("netplay").c_str(), nullptr);
    CreateDirectoryW(inbox.c_str(), nullptr);
    CreateDirectoryW(outbox.c_str(), nullptr);
    std::set<uint64_t> seen;
    std::string last_status;
    uint64_t last_written = 0;
    for (;;) {
        WIN32_FIND_DATAW fd;
        HANDLE h = FindFirstFileW((inbox + L"\\*.json").c_str(), &fd);
        if (h != INVALID_HANDLE_VALUE) {
            do {
                uint64_t seq = 0;
                std::string name = runtime::narrow(fd.cFileName);
                if (!drive::inbox_name(name, &seq) || seen.count(seq))
                    continue;
                seen.insert(seq);
                drive::Command c = drive::parse(runtime::read_file(inbox + L"\\" + fd.cFileName, 64 << 10));
                if (!c.seq || c.seq != seq) {
                    if (c.error.empty())
                        c.error = "seq missing or different from the file name";
                    c.seq = seq;
                }
                swofty::log("[netplay] command %llu: %s%s%s", (unsigned long long)seq, drive::op_name(c.op),
                            c.error.empty() ? "" : " (", c.error.empty() ? "" : (c.error + ")").c_str());
                std::lock_guard<std::mutex> g(queue_lock);
                incoming.push_back(std::move(c));
            } while (FindNextFileW(h, &fd));
            FindClose(h);
        }
        std::deque<std::pair<uint64_t, std::string>> done;
        {
            std::lock_guard<std::mutex> g(queue_lock);
            done.swap(outgoing);
        }
        for (auto &[seq, body] : done) {
            std::wstring n = std::to_wstring(seq) + L".json";
            if (!runtime::write_file(outbox + L"\\" + n, body))
                swofty::log("[netplay] could not write the reply to command %llu", (unsigned long long)seq);
            DeleteFileW((inbox + L"\\" + n).c_str());
        }
        std::string status = drive::status_json(state());
        uint64_t now = GetTickCount64();
        if (status != last_status || now - last_written >= HEARTBEAT_MS) {
            std::string body = "{\"v\":1,\"pid\":" + std::to_string(GetCurrentProcessId()) +
                               ",\"written\":" + std::to_string(wall_ms()) + ",\"status\":" + status + "}";
            if (runtime::write_file(runtime::under_swofty("netplay\\status.json"), body)) {
                if (status != last_status)
                    swofty::log("[netplay] status %s", status.c_str());
                last_status = status;
                last_written = now;
            }
        }
        Sleep(POLL_MS);
    }
}

}

namespace runtime {

void netplay_drive_setup()
{
    char v[256] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_NETPLAY", v, sizeof v);
    armed = n > 0 && n < sizeof v;
    swofty::log("[netplay] launcher commands on (<game>/swofty/netplay); configure, host, join and leave %s",
                armed ? "accepted (SWOFTY_NETPLAY set)" : "refused (SWOFTY_NETPLAY unset), status and clear only");
    if (armed)
        probe_setup();
    if (!started.exchange(true))
        std::thread(watch).detach();
}

void netplay_flow(uint32_t hash, uint32_t to)
{
    bool in = to == 1 || to == 2;
    if (hash == drive::STATE_FRONTEND) {
        frontend = in;
        if (!in)
            home_screen = false;
    } else if (hash == drive::STATE_ONLINE_LOBBY) {
        lobby = in;
        if (!in)
            role = int(drive::Role::none);
    }
}

void netplay_screen(const char *name)
{
    static const char *const overlays[] = {"fe_planes", "persistent", "playercard", "render_badge",
                                           "theming_overlay_frontend", "standard_wipe", "frontend_transition",
                                           "slice_bg_content_underlay", "f1_life_driver_tags", "audio_themes"};
    if (!name || std::strncmp(name, "fe_", 3) != 0)
        return;
    for (const char *o : overlays)
        if (!std::strcmp(name, o))
            return;
    home_screen = !std::strcmp(name, "fe_home");
}

void netplay_tick()
{
    if (!started.load())
        return;
    drive::Command c;
    {
        std::lock_guard<std::mutex> g(queue_lock);
        if (incoming.empty())
            return;
        c = std::move(incoming.front());
        incoming.pop_front();
    }
    drive::State s = state();
    std::string error = drive::refuse(c, s);
    if (error.empty()) {
        switch (c.op) {
        case drive::Op::configure:
            runtime::netplay_session::configure(c.self, c.base);
            runtime::netplay_bridge_status_changed();
            break;
        case drive::Op::clear:
            runtime::netplay_session::clear();
            runtime::netplay_bridge_status_changed();
            break;
        case drive::Op::host: error = run_host(c); break;
        case drive::Op::join: error = run_join(c); break;
        case drive::Op::leave: error = run_leave(c); break;
        default: break;
        }
    }
    if (c.op != drive::Op::status || !error.empty())
        swofty::log("[netplay] command %llu %s: %s", (unsigned long long)c.seq, drive::op_name(c.op),
                    error.empty() ? "ok" : error.c_str());
    std::string body = drive::reply_json(c, state(), error, wall_ms());
    std::lock_guard<std::mutex> g(queue_lock);
    outgoing.emplace_back(c.seq, std::move(body));
}

}
