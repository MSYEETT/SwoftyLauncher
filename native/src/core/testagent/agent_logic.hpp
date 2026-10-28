#pragma once
#include <cstdint>
#include <cstdio>
#include <deque>
#include <map>
#include <memory>
#include <regex>
#include <string>
#include <vector>

#include "../json.hpp"

// The test agent's protocol and command runner, without the game: arming on the run token, parsing
// the CLI's command lines, the results it writes back, the virtual pad's buttons and the runner that
// takes one command at a time and polls the ones that take time (wait, at, pad, capture, run_test).
// agent.cpp gives it a Game that does the work in the game; the host test gives it a fake one.

namespace testagent {

constexpr int PROTOCOL = 1;
constexpr uint64_t SESSION_MAX_AGE_MS = 15 * 60 * 1000;
constexpr uint64_t DEFAULT_WAIT_MS = 30000;
constexpr uint64_t MAX_WAIT_MS = 30 * 60 * 1000;
constexpr size_t MAX_LINE = 64 * 1024;

inline std::string esc(const std::string &s)
{
    std::string o = "\"";
    for (unsigned char c : s) {
        if (c == '"' || c == '\\')
            o += '\\', o += char(c);
        else if (c == '\n')
            o += "\\n";
        else if (c == '\r')
            o += "\\r";
        else if (c == '\t')
            o += "\\t";
        else if (c < 0x20) {
            char b[8];
            snprintf(b, sizeof b, "\\u%04x", c);
            o += b;
        } else
            o += char(c);
    }
    return o + "\"";
}

inline std::string num(double v)
{
    char b[64];
    if (v == double(int64_t(v)) && v > -1e15 && v < 1e15)
        snprintf(b, sizeof b, "%lld", (long long)v);
    else
        snprintf(b, sizeof b, "%.9g", v);
    return b;
}

inline std::string dump(const json::Value &v)
{
    switch (v.type) {
    case json::Value::Null: return "null";
    case json::Value::Bool: return v.b ? "true" : "false";
    case json::Value::Num: return num(v.n);
    case json::Value::Str: return esc(v.s);
    case json::Value::Arr: {
        std::string o = "[";
        for (size_t i = 0; i < v.a.size(); i++)
            o += (i ? "," : "") + dump(v.a[i]);
        return o + "]";
    }
    case json::Value::Obj: {
        std::string o = "{";
        for (size_t i = 0; i < v.o.size(); i++)
            o += (i ? "," : "") + esc(v.o[i].first) + ":" + dump(v.o[i].second);
        return o + "}";
    }
    }
    return "null";
}

inline bool valid_run(const std::string &run)
{
    if (run.empty() || run.size() > 64 || run[0] == '.')
        return false;
    for (char c : run)
        if (!(isalnum((unsigned char)c) || c == '-' || c == '_' || c == '.'))
            return false;
    return true;
}

struct Session {
    std::string run, token;
    uint64_t created_ms = 0;
    std::vector<std::string> dlls;
};

// Two keys arm the agent: the run and token in the game's environment, and session.json in the run's
// mailbox holding the same token, written within the last 15 minutes. Returns "" when armed, else
// why not.
inline std::string arm_check(const std::string &run, const std::string &token, const std::string &session_text,
                             uint64_t now_ms, Session *out)
{
    if (!valid_run(run))
        return "SWOFTY_TEST is not a run name";
    if (token.size() < 16)
        return "SWOFTY_TEST_TOKEN is missing or too short";
    if (session_text.empty())
        return "no session.json in the run's mailbox";
    json::Value v;
    if (!json::parse(session_text, v) || v.type != json::Value::Obj)
        return "session.json is not a JSON object";
    if (v["v"].num() != PROTOCOL)
        return "session.json is protocol " + std::to_string(v["v"].num()) + ", this agent speaks " +
               std::to_string(PROTOCOL);
    if (v["run"].str() != run)
        return "session.json is for run " + v["run"].str();
    if (v["token"].str() != token)
        return "the token in session.json does not match SWOFTY_TEST_TOKEN";
    uint64_t created = uint64_t(v["created_ms"].num());
    if (!created || created > now_ms + 60000 || now_ms - created > SESSION_MAX_AGE_MS)
        return "session.json is older than 15 minutes";
    if (out) {
        out->run = run;
        out->token = token;
        out->created_ms = created;
        out->dlls.clear();
        for (auto &d : v["dlls"].a) {
            std::string s = d.str();
            if (s.empty() || s.find("..") != std::string::npos || s[0] == '/' || s[0] == '\\' || s.find(':') != std::string::npos)
                return "session.json names a test DLL outside the mailbox: " + s;
            out->dlls.push_back(s);
        }
    }
    return {};
}

struct Command {
    uint64_t seq = 0;
    std::string cmd, error;
    json::Value args;
    uint64_t timeout_ms = 0;
};

inline Command parse_command(const std::string &line)
{
    Command c;
    json::Value v;
    if (line.size() > MAX_LINE) {
        c.error = "the command line is longer than 64 KB";
        return c;
    }
    if (!json::parse(line, v) || v.type != json::Value::Obj) {
        c.error = "not a JSON object";
        return c;
    }
    long long seq = v["seq"].num(-1);
    if (seq <= 0) {
        c.error = "missing seq";
        return c;
    }
    c.seq = uint64_t(seq);
    c.cmd = v["cmd"].str();
    if (c.cmd.empty())
        c.error = "missing cmd";
    c.args = v["args"];
    long long t = v["timeout_ms"].num(0);
    c.timeout_ms = t > 0 ? uint64_t(t) : 0;
    if (c.timeout_ms > MAX_WAIT_MS)
        c.timeout_ms = MAX_WAIT_MS;
    return c;
}

enum class Status { ok, error, unavailable, timeout, pending };

inline const char *status_name(Status s)
{
    switch (s) {
    case Status::ok: return "ok";
    case Status::error: return "error";
    case Status::unavailable: return "unavailable";
    case Status::timeout: return "timeout";
    default: return "pending";
    }
}

struct Outcome {
    Status status = Status::ok;
    std::string value = "null", error;

    static Outcome ok(std::string value = "null") { return {Status::ok, std::move(value), {}}; }
    static Outcome fail(std::string why) { return {Status::error, "null", std::move(why)}; }
    static Outcome unavailable(std::string why) { return {Status::unavailable, "null", std::move(why)}; }
    static Outcome pending() { return {Status::pending, "null", {}}; }
};

inline std::string result_json(uint64_t seq, const Outcome &o, uint64_t ms)
{
    std::string s = "{\"seq\":" + std::to_string(seq) + ",\"status\":\"" + status_name(o.status) + "\"";
    if (o.value != "null")
        s += ",\"value\":" + o.value;
    if (!o.error.empty())
        s += ",\"error\":" + esc(o.error);
    return s + ",\"ms\":" + std::to_string(ms) + "}";
}

// XInput buttons (XINPUT_GAMEPAD_*), and the triggers as two pseudo buttons.
struct Button {
    const char *name;
    uint16_t mask;
};
constexpr Button BUTTONS[] = {
    {"up", 0x0001},    {"down", 0x0002},   {"left", 0x0004}, {"right", 0x0008}, {"start", 0x0010},
    {"menu", 0x0010},  {"back", 0x0020},   {"view", 0x0020}, {"ls", 0x0040},    {"rs", 0x0080},
    {"lb", 0x0100},    {"rb", 0x0200},     {"a", 0x1000},    {"b", 0x2000},     {"x", 0x4000},
    {"y", 0x8000},
};
constexpr uint16_t LT = 0xfffe, RT = 0xffff;

inline bool button(const std::string &name, uint16_t &mask)
{
    if (name == "lt" || name == "rt") {
        mask = name == "lt" ? LT : RT;
        return true;
    }
    for (auto &b : BUTTONS)
        if (name == b.name) {
            mask = b.mask;
            return true;
        }
    return false;
}

struct PadState {
    uint16_t buttons = 0;
    uint8_t lt = 0, rt = 0;
};

inline PadState pad_of(uint16_t mask)
{
    PadState p;
    if (mask == LT)
        p.lt = 255;
    else if (mask == RT)
        p.rt = 255;
    else
        p.buttons = mask;
    return p;
}

// A car path: car[<index>|player].<system>.<field>.
struct CarPath {
    bool player = false;
    int index = -1;
    std::string system, field;
};

inline bool parse_car_path(const std::string &p, CarPath &out, std::string &why)
{
    if (p.rfind("car[", 0) != 0) {
        why = "not a car path";
        return false;
    }
    size_t close = p.find(']');
    if (close == std::string::npos || close + 1 >= p.size() || p[close + 1] != '.') {
        why = "expected car[<index>|player].<system>.<field>";
        return false;
    }
    std::string idx = p.substr(4, close - 4);
    if (idx == "player")
        out.player = true;
    else {
        if (idx.empty() || idx.size() > 2 || idx.find_first_not_of("0123456789") != std::string::npos) {
            why = "the car index is a number from 0 to 25, or player";
            return false;
        }
        out.index = atoi(idx.c_str());
    }
    std::string rest = p.substr(close + 2);
    size_t dot = rest.find('.');
    if (dot == std::string::npos || dot == 0 || dot + 1 >= rest.size()) {
        why = "expected car[...].<system>.<field>";
        return false;
    }
    out.system = rest.substr(0, dot);
    out.field = rest.substr(dot + 1);
    return true;
}

// What the agent asks of the game. Each returns unavailable, with the reason, when its bindings
// didn't resolve; pending from a *_poll means "not yet".
struct Game {
    virtual ~Game() = default;
    virtual Outcome hello() = 0;
    virtual Outcome route(const std::string &name) = 0;
    virtual Outcome skip(int destination, int session) = 0;
    virtual Outcome advance(int days) = 0;
    virtual Outcome ai_drives_player(bool on) = 0;
    virtual Outcome read(const std::string &path) = 0;
    virtual Outcome set(const std::string &path, const json::Value &value) = 0;
    virtual Outcome capture(const std::string &name) = 0;
    virtual Outcome capture_poll() = 0;
    virtual Outcome save_now() = 0;
    virtual Outcome quit() = 0;
    virtual Outcome pad(const PadState &p) = 0;
    virtual std::string phase() = 0;
    virtual Outcome test_start(const std::string &fn) = 0;
    virtual Outcome test_poll() = 0;
    // loader.log lines from line number `from` on; returns the next line number.
    virtual uint64_t log_lines(uint64_t from, std::vector<std::string> &out) = 0;
};

// Fires of each event since arming, so a wait takes the next fire it hasn't seen.
struct Fires {
    std::map<std::string, uint64_t> count, taken;

    void fired(const std::string &id) { ++count[id]; }

    bool take(const std::string &id)
    {
        auto c = count.find(id);
        uint64_t &t = taken[id];
        if (c == count.end() || c->second <= t)
            return false;
        t = c->second;
        return true;
    }
};

inline int skip_destination(const std::string &to)
{
    if (to == "next")
        return 2;
    if (to == "simulate" || to == "session_end")
        return 3;
    if (to == "session")
        return 4;
    if (to == "quick_practice")
        return 5;
    if (to == "quick_practice_skip")
        return 6;
    return -1;
}

constexpr int SESSION_RACE = 15;

class Runner {
  public:
    explicit Runner(Game &g) : g_(g) {}

    void submit(Command c) { queue_.push_back(std::move(c)); }
    Fires &fires() { return fires_; }
    bool busy() const { return active_ || !queue_.empty(); }
    size_t queued() const { return queue_.size(); }

    // One step on the game's flow thread: poll the running command, or start the next one. Lines
    // to append to results.jsonl go to out.
    void tick(uint64_t now, std::vector<std::string> &out)
    {
        if (active_) {
            Outcome o = poll(now);
            if (o.status == Status::pending)
                return;
            finish(o, now, out);
            return;
        }
        if (queue_.empty())
            return;
        cur_ = std::move(queue_.front());
        queue_.pop_front();
        started_ = now;
        stage_ = 0;
        Outcome o = cur_.error.empty() ? start(now) : Outcome::fail(cur_.error);
        if (o.status == Status::pending) {
            active_ = true;
            return;
        }
        finish(o, now, out);
    }

  private:
    Game &g_;
    std::deque<Command> queue_;
    Command cur_;
    bool active_ = false;
    uint64_t started_ = 0, deadline_ = 0, stage_at_ = 0;
    int stage_ = 0;
    PadState first_, then_;
    bool has_then_ = false;
    uint64_t hold_ms_ = 0;
    std::string want_;
    std::regex re_;
    uint64_t log_at_ = 0;
    Fires fires_;

    uint64_t limit(uint64_t def) const { return cur_.timeout_ms ? cur_.timeout_ms : def; }

    void finish(const Outcome &o, uint64_t now, std::vector<std::string> &out)
    {
        active_ = false;
        out.push_back(result_json(cur_.seq, o, now - started_));
    }

    Outcome start(uint64_t now)
    {
        const std::string &c = cur_.cmd;
        const json::Value &a = cur_.args;
        if (c == "hello" || c == "status")
            return g_.hello();
        if (c == "route") {
            std::string r = a["route"].str();
            if (r.empty())
                return Outcome::fail("route needs {\"route\": <name>}");
            return g_.route(r);
        }
        if (c == "skip") {
            std::string to = a["to"].str();
            int d = skip_destination(to);
            if (d < 0)
                return Outcome::fail("skip to next, simulate, session, quick_practice or quick_practice_skip, not '" + to + "'");
            int session = int(a["session"].num(SESSION_RACE));
            return g_.skip(d, session);
        }
        if (c == "advance") {
            long long d = a["days"].num(0);
            if (d <= 0 || d > 366)
                return Outcome::fail("advance needs {\"days\": 1 to 366}");
            return g_.advance(int(d));
        }
        if (c == "ai_drives_player") {
            if (a["on"].type != json::Value::Bool)
                return Outcome::fail("ai_drives_player needs {\"on\": true|false}");
            return g_.ai_drives_player(a["on"].b);
        }
        if (c == "read") {
            std::string p = a["path"].str();
            if (p.empty())
                return Outcome::fail("read needs {\"path\": ...}");
            return g_.read(p);
        }
        if (c == "set") {
            std::string p = a["path"].str();
            if (p.empty() || a["value"].type == json::Value::Null)
                return Outcome::fail("set needs {\"path\": ..., \"value\": ...}");
            return g_.set(p, a["value"]);
        }
        if (c == "capture") {
            std::string n = a["name"].str("frame");
            for (char ch : n)
                if (!(isalnum((unsigned char)ch) || ch == '-' || ch == '_'))
                    return Outcome::fail("a capture name is letters, digits, '-' and '_'");
            Outcome o = g_.capture(n);
            if (o.status != Status::ok)
                return o;
            deadline_ = now + limit(10000);
            return Outcome::pending();
        }
        if (c == "wait") {
            want_ = a["event"].str();
            if (want_.empty())
                return Outcome::fail("wait needs {\"event\": <id>}");
            deadline_ = now + limit(DEFAULT_WAIT_MS);
            return poll(now);
        }
        if (c == "at") {
            want_ = a["phase"].str();
            if (want_.empty())
                return Outcome::fail("at needs {\"phase\": <screen or flow state>}");
            deadline_ = now + limit(DEFAULT_WAIT_MS);
            return poll(now);
        }
        if (c == "log") {
            want_ = a["pattern"].str();
            try {
                re_ = std::regex(want_);
            } catch (...) {
                return Outcome::fail("not a regular expression: " + want_);
            }
            std::vector<std::string> none;
            log_at_ = g_.log_lines(UINT64_MAX, none);
            deadline_ = now + limit(DEFAULT_WAIT_MS);
            return Outcome::pending();
        }
        if (c == "pad") {
            uint16_t m;
            std::string press = a["press"].str();
            if (a["release"].type == json::Value::Bool && a["release"].b)
                return g_.pad(PadState{});
            if (!button(press, m))
                return Outcome::fail("unknown button '" + press +
                                     "' (a, b, x, y, lb, rb, lt, rt, ls, rs, start, back, up, down, left, right)");
            first_ = pad_of(m);
            has_then_ = false;
            std::string then = a["then"].str();
            if (!then.empty()) {
                if (!button(then, m))
                    return Outcome::fail("unknown button '" + then + "'");
                then_ = pad_of(m);
                has_then_ = true;
            }
            long long ms = a["ms"].num(120);
            hold_ms_ = uint64_t(ms < 16 ? 16 : ms > 10000 ? 10000 : ms);
            Outcome o = g_.pad(first_);
            if (o.status != Status::ok)
                return o;
            stage_ = 1;
            stage_at_ = now;
            return Outcome::pending();
        }
        if (c == "save") {
            std::string what = a["what"].str("now");
            if (what != "now")
                return Outcome::fail("save takes \"now\"");
            return g_.save_now();
        }
        if (c == "quit")
            return g_.quit();
        if (c == "run_test") {
            std::string fn = a["fn"].str();
            if (fn.empty())
                return Outcome::fail("run_test needs {\"fn\": <name>}");
            Outcome o = g_.test_start(fn);
            if (o.status != Status::ok)
                return o;
            deadline_ = now + limit(300000);
            return Outcome::pending();
        }
        return Outcome::fail("unknown command '" + c + "'");
    }

    Outcome poll(uint64_t now)
    {
        const std::string &c = cur_.cmd;
        if (c == "wait") {
            if (fires_.take(want_))
                return Outcome::ok("{\"event\":" + esc(want_) + ",\"fires\":" + std::to_string(fires_.count[want_]) + "}");
        } else if (c == "at") {
            std::string p = g_.phase();
            if (p == want_ || p == "fe_" + want_ || p.find("|" + want_ + "|") != std::string::npos ||
                p.find("|fe_" + want_ + "|") != std::string::npos)
                return Outcome::ok("{\"phase\":" + esc(want_) + "}");
        } else if (c == "log") {
            std::vector<std::string> lines;
            log_at_ = g_.log_lines(log_at_, lines);
            for (auto &l : lines)
                if (std::regex_search(l, re_))
                    return Outcome::ok("{\"line\":" + esc(l) + "}");
        } else if (c == "capture") {
            Outcome o = g_.capture_poll();
            if (o.status != Status::pending)
                return o;
        } else if (c == "run_test") {
            Outcome o = g_.test_poll();
            if (o.status != Status::pending)
                return o;
        } else if (c == "pad") {
            if (now - stage_at_ < hold_ms_)
                return Outcome::pending();
            if (stage_ == 1) {
                g_.pad(PadState{});
                stage_ = 2;
                stage_at_ = now;
                return Outcome::pending();
            }
            if (stage_ == 2 && has_then_) {
                g_.pad(then_);
                stage_ = 3;
                stage_at_ = now;
                return Outcome::pending();
            }
            if (stage_ == 3) {
                g_.pad(PadState{});
                stage_ = 4;
                stage_at_ = now;
                return Outcome::pending();
            }
            return Outcome::ok();
        }
        if (now >= deadline_) {
            Outcome o{Status::timeout, "null", c + " " + want_ + ": not within " + std::to_string((deadline_ - started_) / 1000) + " s"};
            if (c == "capture" || c == "run_test")
                o.error = c + ": not finished within " + std::to_string((deadline_ - started_) / 1000) + " s";
            return o;
        }
        return Outcome::pending();
    }
};

}
