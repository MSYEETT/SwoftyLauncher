#include "runtime.hpp"

#include <swofty/mixin.hpp>

#include <atomic>
#include <deque>
#include <mutex>
#include <set>
#include <string>
#include <thread>

#include "netplay_drive_logic.hpp"

// Launcher commands for LAN netplay, over the same kind of file mailbox the Career Editor uses
// (protocol/bridge.md there): <game>/swofty/netplay/inbox/<seq>.json from the launcher (written as
// .tmp and renamed), one reply per command in outbox/<seq>.json, and status.json rewritten when it
// changes and every 5 s. A watcher thread does all file IO; commands run on the flow thread from the
// flow tick (runtime::netplay_tick), the thread the menus' own handlers run on.
//
// State comes from what core already sees: flow states (StateFrontend for the front end,
// StateOnlineLobby for a LAN lobby) and the screens the UI opens (fe_home for the main menu).
// status always answers; host, join and leave need SWOFTY_NETPLAY in the game's environment and a
// game state that allows them, and report unavailable until their game entry points are bound
// (research/multiplayer/lan-netplay-design.md, "Driving host/join").

namespace {

namespace drive = runtime::drive;

constexpr DWORD POLL_MS = 250;
constexpr uint64_t HEARTBEAT_MS = 5000;

std::atomic<bool> configured, frontend, lobby, home_screen;
std::atomic<int> role{int(drive::Role::none)};

std::mutex queue_lock;
std::deque<drive::Command> incoming;
std::deque<std::pair<uint64_t, std::string>> outgoing;
std::atomic<bool> started;

drive::State state()
{
    drive::State s;
    s.configured = configured.load();
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
    configured = n > 0 && n < sizeof v;
    swofty::log("[netplay] launcher commands on (<game>/swofty/netplay); host, join and leave %s",
                configured ? "accepted (SWOFTY_NETPLAY set)" : "refused (SWOFTY_NETPLAY unset), status only");
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
