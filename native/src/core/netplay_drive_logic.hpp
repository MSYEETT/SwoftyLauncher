#pragma once
#include <cstdint>
#include <cstdio>
#include <string>

#include "json.hpp"

// Netplay command logic without the game: parsing a launcher command, deciding whether the game's
// state allows it, and the JSON the reply and status file carry. netplay_drive.cpp feeds it the flow
// states and screens it sees; the host test drives it directly.

namespace runtime::drive {

constexpr uint32_t djb2(const char *s)
{
    uint32_t h = 5381;
    for (; *s; ++s)
        h = h * 33 + static_cast<uint8_t>(*s);
    return h;
}

constexpr uint32_t STATE_FRONTEND = djb2("StateFrontend");
constexpr uint32_t STATE_ONLINE_LOBBY = djb2("StateOnlineLobby");
constexpr uint32_t STATE_READY_MONITOR = djb2("StateReadyMonitor");

static_assert(STATE_FRONTEND == 0x957880c6u, "state name hashing drifted");
static_assert(STATE_ONLINE_LOBBY == 0x5e31bae3u, "state name hashing drifted");

enum class Op { none, status, host, join, leave };
enum class Category { none, f1_2026, f1_2025 };
enum class Screen { main_menu, lan_lobby, other };
enum class Role { none, host, client };

struct Command {
    uint64_t seq = 0;
    Op op = Op::none;
    Category category = Category::none;
    std::string ip;
    std::string error;
};

inline const char *op_name(Op o)
{
    switch (o) {
    case Op::status: return "status";
    case Op::host: return "host";
    case Op::join: return "join";
    case Op::leave: return "leave";
    default: return "none";
    }
}

inline const char *category_name(Category c)
{
    return c == Category::f1_2026 ? "f1-2026" : c == Category::f1_2025 ? "f1-2025" : "none";
}

inline const char *screen_name(Screen s)
{
    return s == Screen::main_menu ? "main_menu" : s == Screen::lan_lobby ? "lan_lobby" : "other";
}

inline const char *role_name(Role r)
{
    return r == Role::host ? "host" : r == Role::client ? "client" : "none";
}

inline Category parse_category(const std::string &s)
{
    if (s == "f1-2026" || s == "f1_2026" || s == "2026")
        return Category::f1_2026;
    if (s == "f1-2025" || s == "f1_2025" || s == "2025")
        return Category::f1_2025;
    return Category::none;
}

// Dotted IPv4 only, four decimal parts 0-255 with no leading zeros, the form inet_addr reads the
// same everywhere. Join via IP (0x14328deb0) passes the text straight to inet_addr.
inline bool valid_ipv4(const std::string &s)
{
    int parts = 0;
    size_t i = 0;
    while (parts < 4) {
        size_t start = i;
        unsigned v = 0;
        while (i < s.size() && s[i] >= '0' && s[i] <= '9' && i - start < 3)
            v = v * 10 + unsigned(s[i++] - '0');
        size_t len = i - start;
        if (!len || v > 255 || (len > 1 && s[start] == '0'))
            return false;
        if (++parts < 4) {
            if (i >= s.size() || s[i] != '.')
                return false;
            ++i;
        }
    }
    return i == s.size() && s != "0.0.0.0" && s != "255.255.255.255";
}

inline Command parse(const std::string &text)
{
    Command c;
    json::Value doc;
    if (!json::parse(text, doc) || doc.type != json::Value::Obj) {
        c.error = "not a JSON object";
        return c;
    }
    const json::Value &seq = doc["seq"];
    if (seq.type == json::Value::Num && seq.n >= 1)
        c.seq = uint64_t(seq.n);
    std::string op = doc["op"].str();
    if (op == "status")
        c.op = Op::status;
    else if (op == "host")
        c.op = Op::host;
    else if (op == "join")
        c.op = Op::join;
    else if (op == "leave")
        c.op = Op::leave;
    else {
        c.error = op.empty() ? "no op" : "unknown op '" + op + "'";
        return c;
    }
    const json::Value &args = doc["args"];
    if (c.op == Op::host) {
        std::string cat = args["category"].str();
        c.category = parse_category(cat);
        if (c.category == Category::none)
            c.error = cat.empty() ? "host needs args.category (f1-2026 or f1-2025)"
                                  : "unknown category '" + cat + "' (f1-2026 or f1-2025)";
    } else if (c.op == Op::join) {
        c.ip = args["ip"].str();
        if (!valid_ipv4(c.ip))
            c.error = c.ip.empty() ? "join needs args.ip (dotted IPv4)" : "'" + c.ip + "' is not a dotted IPv4 address";
    }
    return c;
}

struct State {
    bool configured = false;
    bool frontend = false;
    bool lobby = false;
    bool home_screen = false;
    bool busy = false;
    Role role = Role::none;

    Screen screen() const
    {
        if (lobby)
            return Screen::lan_lobby;
        if (frontend && home_screen && !busy)
            return Screen::main_menu;
        return Screen::other;
    }

    bool in_session() const { return lobby; }
};

// Empty when the command may run now, else the reason it is refused. status always runs.
inline std::string refuse(const Command &c, const State &s)
{
    if (!c.error.empty())
        return c.error;
    if (c.op == Op::status)
        return {};
    if (!s.configured)
        return "netplay is not configured for this game session (SWOFTY_NETPLAY is unset)";
    if (s.busy)
        return "a netplay command is still running";
    switch (c.op) {
    case Op::host:
    case Op::join:
        if (s.in_session())
            return "already in a LAN session; leave it first";
        if (s.screen() != Screen::main_menu)
            return "the game is not on its main menu";
        return {};
    case Op::leave:
        if (!s.in_session())
            return "not in a LAN session";
        return {};
    default:
        return "unknown op";
    }
}

inline void quote(std::string &out, const std::string &s)
{
    out += '"';
    for (unsigned char ch : s) {
        if (ch == '"' || ch == '\\') {
            out += '\\';
            out += char(ch);
        } else if (ch < 0x20) {
            char b[8];
            std::snprintf(b, sizeof b, "\\u%04x", ch);
            out += b;
        } else {
            out += char(ch);
        }
    }
    out += '"';
}

inline std::string status_json(const State &s)
{
    std::string out = "{\"screen\":\"";
    out += screen_name(s.screen());
    out += "\",\"in_session\":";
    out += s.in_session() ? "true" : "false";
    out += ",\"role\":\"";
    out += role_name(s.in_session() ? s.role : Role::none);
    out += "\",\"configured\":";
    out += s.configured ? "true" : "false";
    out += '}';
    return out;
}

inline std::string reply_json(const Command &c, const State &s, const std::string &error, uint64_t t)
{
    std::string out = "{\"seq\":" + std::to_string(c.seq) + ",\"op\":\"" + op_name(c.op) + "\",\"ok\":";
    out += error.empty() ? "true" : "false";
    out += ",\"error\":";
    if (error.empty())
        out += "null";
    else
        quote(out, error);
    out += ",\"status\":" + status_json(s) + ",\"t\":" + std::to_string(t) + '}';
    return out;
}

// Names of inbox files the launcher writes: <digits>.json, written as .tmp and renamed.
inline bool inbox_name(const std::string &name, uint64_t *seq)
{
    if (name.size() < 6 || name.size() > 25 || name.compare(name.size() - 5, 5, ".json") != 0)
        return false;
    uint64_t v = 0;
    for (size_t i = 0; i + 5 < name.size(); i++) {
        if (name[i] < '0' || name[i] > '9')
            return false;
        v = v * 10 + uint64_t(name[i] - '0');
    }
    *seq = v;
    return v != 0;
}

}
