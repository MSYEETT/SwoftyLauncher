// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Career Mode Editor: the bridge between the launcher page and the running game.
//
// Mailbox in the addon's storage folder, <game>/swofty/addons/swofty.career-editor/
// (protocol/bridge.md has the formats):
//   inbox/<seq>.json   edit batches from the launcher (it writes a temp file, then renames)
//   outbox/<seq>.json  one ack per batch: per-op ok / before / after / error
//   snapshot.json      status plus every registered section, rewritten when it changes
//   journal.jsonl      one line per applied op
//
// Everything runs on the flow thread from the events library's frame callback. Batches run in
// seq order, one per frame, and only while the gate is open: ce::safe() for CE_OP ops,
// ce::in_career() for CE_OP_ANYTIME ops. A blocked batch stays queued and the snapshot says why.
// Every op and section call is wrapped in an SEH guard: a fault is logged, answered as an error
// and the op or section is switched off for the rest of the game session.

#include <algorithm>
#include <cmath>
#include <cstdarg>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <map>
#include <set>
#include <string>
#include <string_view>
#include <vector>

#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#else
#include <ctime>
#include <unistd.h>
#endif

#include <swofty/mixin.hpp>
#include <swofty/storage.hpp>

#include "bridge.hpp"

namespace st = swofty::storage;

namespace ce {

// ---- time ---------------------------------------------------------------------------------------

uint64_t now_ms()
{
#if defined(_WIN32)
    return GetTickCount64();
#else
    timespec t{};
    clock_gettime(CLOCK_MONOTONIC, &t);
    return uint64_t(t.tv_sec) * 1000 + uint64_t(t.tv_nsec / 1000000);
#endif
}

uint64_t wall_ms()
{
#if defined(_WIN32)
    FILETIME ft;
    GetSystemTimeAsFileTime(&ft);
    uint64_t t = (uint64_t(ft.dwHighDateTime) << 32) | ft.dwLowDateTime;
    return t / 10000 - 11644473600000ull;
#else
    timespec t{};
    clock_gettime(CLOCK_REALTIME, &t);
    return uint64_t(t.tv_sec) * 1000 + uint64_t(t.tv_nsec / 1000000);
#endif
}

namespace {

unsigned long pid()
{
#if defined(_WIN32)
    return GetCurrentProcessId();
#else
    return (unsigned long)getpid();
#endif
}

} // namespace

// ---- Writer -------------------------------------------------------------------------------------

namespace {

size_t utf8_len(const unsigned char *p, size_t n)
{
    unsigned char c = p[0];
    size_t len = c >= 0xf0 && c <= 0xf4 ? 4 : c >= 0xe0 ? 3 : c >= 0xc2 && c < 0xe0 ? 2 : 0;
    if (!len || len > n)
        return 0;
    for (size_t i = 1; i < len; ++i)
        if ((p[i] & 0xc0) != 0x80)
            return 0;
    if (len == 3 && ((c == 0xe0 && p[1] < 0xa0) || (c == 0xed && p[1] >= 0xa0)))
        return 0;
    if (len == 4 && ((c == 0xf0 && p[1] < 0x90) || (c == 0xf4 && p[1] >= 0x90)))
        return 0;
    return len;
}

void quote(std::string &out, std::string_view s)
{
    out += '"';
    const auto *p = reinterpret_cast<const unsigned char *>(s.data());
    size_t n = s.size();
    for (size_t i = 0; i < n;) {
        unsigned char c = p[i];
        if (c >= 0x80) {
            size_t len = utf8_len(p + i, n - i);
            if (len) {
                out.append(reinterpret_cast<const char *>(p + i), len);
                i += len;
            } else {
                out += "\xef\xbf\xbd";
                ++i;
            }
            continue;
        }
        switch (c) {
        case '"': out += "\\\""; break;
        case '\\': out += "\\\\"; break;
        case '\n': out += "\\n"; break;
        case '\r': out += "\\r"; break;
        case '\t': out += "\\t"; break;
        default:
            if (c < 0x20 || c == 0x7f) {
                char b[8];
                std::snprintf(b, sizeof b, "\\u%04x", c);
                out += b;
            } else {
                out += char(c);
            }
        }
        ++i;
    }
    out += '"';
}

} // namespace

void Writer::sep()
{
    if (after_key_) {
        after_key_ = false;
        return;
    }
    if (!first_.empty()) {
        if (!first_.back())
            out_ += ',';
        first_.back() = false;
    }
}

void Writer::begin_object()
{
    sep();
    out_ += '{';
    first_.push_back(true);
}

void Writer::end_object()
{
    out_ += '}';
    if (!first_.empty())
        first_.pop_back();
}

void Writer::begin_array()
{
    sep();
    out_ += '[';
    first_.push_back(true);
}

void Writer::end_array()
{
    out_ += ']';
    if (!first_.empty())
        first_.pop_back();
}

void Writer::key(const char *k)
{
    sep();
    quote(out_, k ? k : "");
    out_ += ':';
    after_key_ = true;
}

void Writer::str(const std::string &v)
{
    sep();
    quote(out_, v);
}

void Writer::num(double v)
{
    sep();
    if (!std::isfinite(v)) {
        out_ += "null";
        return;
    }
    char b[40];
    if (v == std::floor(v) && std::fabs(v) < 9007199254740992.0)
        std::snprintf(b, sizeof b, "%lld", (long long)v);
    else
        std::snprintf(b, sizeof b, "%.17g", v);
    if (!std::strchr(b, 'e') && std::strchr(b, '.')) {
        char t[40];
        for (int p = 6; p <= 17; ++p) {
            std::snprintf(t, sizeof t, "%.*g", p, v);
            if (std::strtod(t, nullptr) == v) {
                std::memcpy(b, t, sizeof t);
                break;
            }
        }
    }
    out_ += b;
}

void Writer::integer(int64_t v)
{
    sep();
    out_ += std::to_string(v);
}

void Writer::boolean(bool v)
{
    sep();
    out_ += v ? "true" : "false";
}

void Writer::null()
{
    sep();
    out_ += "null";
}

// ---- logging ------------------------------------------------------------------------------------

void log(const char *fmt, ...)
{
    char buf[1536];
    va_list ap;
    va_start(ap, fmt);
    std::vsnprintf(buf, sizeof buf, fmt, ap);
    va_end(ap);
    swofty::log("[career-editor] %s", buf);
}

// ---- registries ---------------------------------------------------------------------------------

namespace {

struct OpEntry {
    std::string name;
    OpFn fn;
    bool anytime;
    std::string quarantined;
};

struct SectionEntry {
    std::string name;
    SectionFn fn;
    std::string quarantined;
    std::string text;
    Json value;
    bool rendered = false;
    uint64_t slow_logged = 0;
    bool big_logged = false;
};

std::vector<OpEntry> &op_table()
{
    static std::vector<OpEntry> v;
    return v;
}

std::vector<SectionEntry> &section_table()
{
    static std::vector<SectionEntry> v;
    return v;
}

std::vector<std::string> &duplicates()
{
    static std::vector<std::string> v;
    return v;
}

void add_op(const char *name, OpFn fn, bool anytime)
{
    if (!name || !fn)
        return;
    for (auto &e : op_table())
        if (e.name == name) {
            duplicates().push_back(std::string("op ") + name);
            return;
        }
    op_table().push_back({name, fn, anytime, {}});
}

OpEntry *find_op(const std::string &name)
{
    for (auto &e : op_table())
        if (e.name == name)
            return &e;
    return nullptr;
}

} // namespace

void register_op(const char *name, OpFn fn)
{
    add_op(name, fn, false);
}

void register_op_anytime(const char *name, OpFn fn)
{
    add_op(name, fn, true);
}

void register_section(const char *name, SectionFn fn)
{
    if (!name || !fn)
        return;
    for (auto &e : section_table())
        if (e.name == name) {
            duplicates().push_back(std::string("section ") + name);
            return;
        }
    SectionEntry e;
    e.name = name;
    e.fn = fn;
    section_table().push_back(std::move(e));
}

// ---- JSON helpers -------------------------------------------------------------------------------

namespace {

void emit(Writer &w, const Json &v, int depth = 0)
{
    if (depth > 32) {
        w.null();
        return;
    }
    switch (v.type) {
    case Json::Null: w.null(); break;
    case Json::Bool: w.boolean(v.b); break;
    case Json::Num: w.num(v.n); break;
    case Json::Str: w.str(v.s); break;
    case Json::Arr:
        w.begin_array();
        for (auto &x : v.a)
            emit(w, x, depth + 1);
        w.end_array();
        break;
    case Json::Obj:
        w.begin_object();
        for (auto &[k, x] : v.o) {
            w.key(k.c_str());
            emit(w, x, depth + 1);
        }
        w.end_object();
        break;
    }
}

void emit_raw(Writer &w, const std::string &raw)
{
    if (raw.empty()) {
        w.null();
        return;
    }
    Json v;
    if (json::parse(raw, v))
        emit(w, v);
    else
        w.str(raw);
}

std::string clip(std::string s, size_t n = 300)
{
    if (s.size() > n) {
        s.resize(n);
        s += "...";
    }
    return s;
}

std::string to_text(const Json &v)
{
    Writer w;
    emit(w, v);
    return w.text();
}

// ---- fault guard --------------------------------------------------------------------------------

#if defined(_WIN32)
int fault_filter(unsigned long code)
{
    switch (code) {
    case EXCEPTION_ACCESS_VIOLATION:
    case EXCEPTION_IN_PAGE_ERROR:
    case EXCEPTION_INT_DIVIDE_BY_ZERO:
    case EXCEPTION_ILLEGAL_INSTRUCTION:
    case EXCEPTION_PRIV_INSTRUCTION:
    case EXCEPTION_DATATYPE_MISALIGNMENT:
    case EXCEPTION_ARRAY_BOUNDS_EXCEEDED:
        return EXCEPTION_EXECUTE_HANDLER;
    default:
        return EXCEPTION_CONTINUE_SEARCH;
    }
}

bool guarded(void (*fn)(void *), void *ctx, unsigned long *code)
{
    __try {
        fn(ctx);
        return true;
    } __except (fault_filter(*code = GetExceptionCode())) {
        return false;
    }
}
#else
bool guarded(void (*fn)(void *), void *ctx, unsigned long *code)
{
    *code = 0;
    fn(ctx);
    return true;
}
#endif

struct OpCall {
    OpFn fn;
    const Json *args;
    OpResult out;
};

void run_op_body(void *p)
{
    auto *c = static_cast<OpCall *>(p);
    try {
        c->out = c->fn(*c->args);
    } catch (const std::exception &e) {
        c->out = OpResult{};
        c->out.error = std::string("exception: ") + e.what();
    } catch (...) {
        c->out = OpResult{};
        c->out.error = "exception";
    }
}

struct SectionCall {
    SectionFn fn;
    Writer w;
    std::string error;
};

void run_section_body(void *p)
{
    auto *c = static_cast<SectionCall *>(p);
    try {
        c->fn(c->w);
    } catch (const std::exception &e) {
        c->error = std::string("exception: ") + e.what();
    } catch (...) {
        c->error = "exception";
    }
}

// ---- mailbox ------------------------------------------------------------------------------------

constexpr uint64_t kStatePollMs = 250;
constexpr uint64_t kInboxPollMs = 500;
constexpr uint64_t kInboxRelistMs = 10000;
constexpr uint64_t kSnapshotMs = 1000;
constexpr uint64_t kHeartbeatMs = 5000;
constexpr size_t kMaxOps = 256;

struct Batch {
    uint64_t seq = 0;
    bool parsed = false;
    bool bad = false;
    std::string error;
    Json doc;
    bool anytime = true;
};

struct Bridge {
    bool up = false;
    bool storage = false;
    uint64_t boot = 0;
    uint64_t last_state = 0;
    uint64_t last_inbox = 0;
    uint64_t last_list = 0;
    uint64_t last_snapshot = 0;
    uint64_t last_write = 0;
    uint64_t sections_at = 0;
    std::string sections_text;
    bool sections_live = false;
    bool force_render = false;
    std::map<uint64_t, Batch> pending;
    std::set<uint64_t> done;
    uint64_t blocked_seq = 0;
    std::string blocked_why;
    uint64_t blocked_logged_seq = 0;
    std::string blocked_logged_why;
    uint64_t last_ack = 0;
    uint64_t applied = 0;
    uint64_t failed = 0;
    std::string last_body;
#if defined(_WIN32)
    std::wstring inbox_dir;
    FILETIME inbox_mtime{};
    bool doorbell = false;
#endif
};

Bridge g;

#if defined(_WIN32)
void init_doorbell()
{
    wchar_t path[MAX_PATH * 2];
    DWORD n = GetModuleFileNameW(nullptr, path, DWORD(sizeof path / sizeof path[0]));
    if (!n || n >= sizeof path / sizeof path[0])
        return;
    std::wstring dir(path, n);
    size_t cut = dir.find_last_of(L"\\/");
    if (cut == std::wstring::npos)
        return;
    g.inbox_dir = dir.substr(0, cut) + L"\\swofty\\addons\\swofty.career-editor\\inbox";
    WIN32_FILE_ATTRIBUTE_DATA a{};
    g.doorbell = GetFileAttributesExW(g.inbox_dir.c_str(), GetFileExInfoStandard, &a) &&
                 (a.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY);
}

bool doorbell_rang()
{
    if (!g.doorbell)
        return false;
    WIN32_FILE_ATTRIBUTE_DATA a{};
    if (!GetFileAttributesExW(g.inbox_dir.c_str(), GetFileExInfoStandard, &a))
        return true;
    bool changed = CompareFileTime(&a.ftLastWriteTime, &g.inbox_mtime) != 0;
    g.inbox_mtime = a.ftLastWriteTime;
    return changed;
}
#else
void init_doorbell() {}
#endif

std::string inbox_path(uint64_t seq)
{
    return "inbox/" + std::to_string(seq) + ".json";
}

std::string outbox_path(uint64_t seq)
{
    return "outbox/" + std::to_string(seq) + ".json";
}

bool parse_seq(std::string_view name, uint64_t &seq)
{
    if (name.size() < 6 || name.substr(name.size() - 5) != ".json")
        return false;
    std::string_view digits = name.substr(0, name.size() - 5);
    if (digits.empty() || digits.size() > 19)
        return false;
    uint64_t v = 0;
    for (char c : digits) {
        if (c < '0' || c > '9')
            return false;
        v = v * 10 + uint64_t(c - '0');
    }
    seq = v;
    return true;
}

void list_inbox()
{
    std::vector<char> buf(64 * 1024);
    size_t len = 0;
    st::Result r = st::list("inbox", buf.data(), buf.size(), &len);
    if (r == st::Result::invalid && len > buf.size()) {
        buf.resize(len + 1024);
        r = st::list("inbox", buf.data(), buf.size(), &len);
    }
    if (r != st::Result::ok)
        return;
    std::set<uint64_t> seen;
    std::string_view all(buf.data(), std::min(len, buf.size()));
    size_t at = 0;
    while (at < all.size()) {
        size_t nl = all.find('\n', at);
        std::string_view name = all.substr(at, nl == std::string_view::npos ? std::string_view::npos : nl - at);
        at = nl == std::string_view::npos ? all.size() : nl + 1;
        uint64_t seq;
        if (!parse_seq(name, seq))
            continue;
        seen.insert(seq);
        if (g.done.count(seq) || g.pending.count(seq))
            continue;
        uint64_t sz = 0;
        if (st::size(outbox_path(seq), &sz) == st::Result::ok) {
            log("batch %llu was already acked; dropping its inbox file", (unsigned long long)seq);
            st::remove(inbox_path(seq));
            g.done.insert(seq);
            continue;
        }
        Batch b;
        b.seq = seq;
        g.pending.emplace(seq, std::move(b));
        g.force_render = true;
        log("batch %llu queued", (unsigned long long)seq);
    }
    for (auto it = g.pending.begin(); it != g.pending.end();) {
        if (!seen.count(it->first)) {
            log("batch %llu was withdrawn by the launcher before it ran", (unsigned long long)it->first);
            it = g.pending.erase(it);
        } else {
            ++it;
        }
    }
}

void parse_batch(Batch &b)
{
    b.parsed = true;
    std::string text;
    if (st::read(inbox_path(b.seq), text) != st::Result::ok) {
        b.bad = true;
        b.error = "the batch file could not be read";
        return;
    }
    if (!json::parse(text, b.doc) || b.doc.type != Json::Obj) {
        b.bad = true;
        b.error = "the batch is not a JSON object";
        return;
    }
    const Json &ops = b.doc["ops"];
    if (ops.type != Json::Arr) {
        b.bad = true;
        b.error = "the batch has no ops array";
        return;
    }
    if (ops.a.size() > kMaxOps) {
        b.bad = true;
        b.error = "the batch has more than 256 ops";
        return;
    }
    b.anytime = true;
    for (auto &o : ops.a) {
        std::string name = o["op"].str();
        if (name == "bridge.ping")
            continue;
        OpEntry *e = find_op(name);
        if (e && !e->anytime)
            b.anytime = false;
    }
}

Json args_with_expect(const Json &op)
{
    Json args = op["args"];
    if (args.type == Json::Null)
        args.type = Json::Obj;
    const Json &expect = op["expect"];
    if (args.type == Json::Obj && expect.type != Json::Null && args["expect"].type == Json::Null)
        args.o.emplace_back("expect", expect);
    return args;
}

OpResult run_one(const std::string &name, const Json &args, bool &ran)
{
    ran = false;
    OpResult r;
    if (name == "bridge.ping") {
        Writer w;
        w.begin_object();
        w.key("boot");
        w.integer(int64_t(g.boot));
        w.key("mode");
        w.str(state::mode_name(mode()));
        w.key("safe");
        w.boolean(safe());
        w.end_object();
        r.ok = true;
        r.before = w.text();
        r.after = w.text();
        ran = true;
        return r;
    }
    OpEntry *e = find_op(name);
    if (!e) {
        r.error = "unknown op '" + name + "' (not in this build)";
        return r;
    }
    if (!e->quarantined.empty()) {
        r.error = "op switched off after a fault this session: " + e->quarantined;
        return r;
    }
    std::string why = state::blocked(e->anytime);
    if (!why.empty()) {
        r.error = "blocked: " + why;
        return r;
    }
    OpCall call{e->fn, &args, {}};
    unsigned long code = 0;
    if (!guarded(run_op_body, &call, &code)) {
        char b[96];
        std::snprintf(b, sizeof b, "fault 0x%08lx inside the op", code);
        e->quarantined = b;
        log("op %s FAULTED (0x%08lx); it is switched off until the game restarts", name.c_str(), code);
        r.error = b;
        ran = true;
        return r;
    }
    ran = true;
    return call.out;
}

void run_batch(Batch &b)
{
    uint64_t t0 = now_ms();
    const Json &ops = b.doc["ops"];
    bool stop_on_error = b.doc["stop_on_error"].type == Json::Bool && b.doc["stop_on_error"].b;
    std::string session = b.doc["session"].str();
    std::string label = b.doc["label"].str();

    Writer ack;
    ack.begin_object();
    ack.key("seq");
    ack.integer(int64_t(b.seq));
    ack.key("boot");
    ack.integer(int64_t(g.boot));
    ack.key("session");
    ack.str(session);
    ack.key("t");
    ack.integer(int64_t(wall_ms()));
    ack.key("mode");
    ack.str(state::mode_name(mode()));
    ack.key("results");
    ack.begin_array();

    bool all_ok = true;
    size_t n_ok = 0;
    for (size_t i = 0; i < ops.a.size(); ++i) {
        const Json &o = ops.a[i];
        std::string name = o["op"].str();
        Json args = args_with_expect(o);
        bool ran = false;
        OpResult r;
        if (name.empty())
            r.error = "op name missing";
        else if (stop_on_error && !all_ok)
            r.error = "skipped: an earlier op in the batch failed";
        else
            r = run_one(name, args, ran);
        if (r.ok && r.error.empty())
            ++n_ok;
        else
            all_ok = false;

        std::string args_text = to_text(args);
        log("op %llu.%zu %s args=%s -> %s%s%s before=%s after=%s", (unsigned long long)b.seq, i, name.c_str(),
            clip(args_text, 200).c_str(), r.ok ? "ok" : "FAILED", r.error.empty() ? "" : ": ", r.error.c_str(),
            clip(r.before, 160).c_str(), clip(r.after, 160).c_str());

        ack.begin_object();
        ack.key("i");
        ack.integer(int64_t(i));
        ack.key("op");
        ack.str(name);
        ack.key("ok");
        ack.boolean(r.ok && r.error.empty());
        ack.key("ran");
        ack.boolean(ran);
        ack.key("args");
        emit(ack, args);
        ack.key("error");
        if (r.error.empty())
            ack.null();
        else
            ack.str(r.error);
        ack.key("before");
        emit_raw(ack, r.before);
        ack.key("after");
        emit_raw(ack, r.after);
        ack.end_object();

        if (r.ok && r.error.empty() && name != "bridge.ping") {
            Writer j;
            j.begin_object();
            j.key("t");
            j.integer(int64_t(wall_ms()));
            j.key("seq");
            j.integer(int64_t(b.seq));
            j.key("i");
            j.integer(int64_t(i));
            j.key("session");
            j.str(session);
            j.key("label");
            j.str(label);
            j.key("mode");
            j.str(state::mode_name(mode()));
            j.key("op");
            j.str(name);
            j.key("args");
            emit(j, args);
            j.key("before");
            emit_raw(j, r.before);
            j.key("after");
            emit_raw(j, r.after);
            j.end_object();
            st::append("journal.jsonl", j.text() + "\n");
            ++g.applied;
        } else if (!(r.ok && r.error.empty())) {
            ++g.failed;
        }
    }
    ack.end_array();
    ack.key("ok");
    ack.boolean(all_ok);
    ack.key("ms");
    ack.integer(int64_t(now_ms() - t0));
    ack.end_object();

    st::write(outbox_path(b.seq), ack.text());
    st::remove(inbox_path(b.seq));
    g.last_ack = b.seq;
    g.force_render = true;
    log("batch %llu done: %zu/%zu ops ok in %llu ms", (unsigned long long)b.seq, n_ok, ops.a.size(),
        (unsigned long long)(now_ms() - t0));
}

void reject_batch(Batch &b)
{
    Writer ack;
    ack.begin_object();
    ack.key("seq");
    ack.integer(int64_t(b.seq));
    ack.key("boot");
    ack.integer(int64_t(g.boot));
    ack.key("t");
    ack.integer(int64_t(wall_ms()));
    ack.key("ok");
    ack.boolean(false);
    ack.key("error");
    ack.str(b.error);
    ack.key("results");
    ack.begin_array();
    ack.end_array();
    ack.end_object();
    st::write(outbox_path(b.seq), ack.text());
    st::remove(inbox_path(b.seq));
    g.last_ack = b.seq;
    ++g.failed;
    log("batch %llu rejected: %s", (unsigned long long)b.seq, b.error.c_str());
}

void drain_one()
{
    g.blocked_seq = 0;
    g.blocked_why.clear();
    if (g.pending.empty())
        return;
    auto it = g.pending.begin();
    Batch &b = it->second;
    if (!b.parsed)
        parse_batch(b);
    if (b.bad) {
        reject_batch(b);
        g.done.insert(b.seq);
        g.pending.erase(it);
        return;
    }
    std::string why = state::blocked(b.anytime);
    if (!why.empty()) {
        g.blocked_seq = b.seq;
        g.blocked_why = why;
        if (g.blocked_logged_seq != b.seq || g.blocked_logged_why != why) {
            log("batch %llu waits: %s", (unsigned long long)b.seq, why.c_str());
            g.blocked_logged_seq = b.seq;
            g.blocked_logged_why = why;
            g.force_render = true;
        }
        return;
    }
    uint64_t sz = 0;
    if (st::size(inbox_path(b.seq), &sz) != st::Result::ok) {
        log("batch %llu vanished before it ran", (unsigned long long)b.seq);
        g.pending.erase(it);
        return;
    }
    run_batch(b);
    g.done.insert(b.seq);
    g.pending.erase(it);
}

// ---- snapshot -----------------------------------------------------------------------------------

void render_sections(uint64_t now)
{
    for (auto &s : section_table()) {
        if (!s.quarantined.empty())
            continue;
        SectionCall call{s.fn, {}, {}};
        uint64_t t0 = now_ms();
        unsigned long code = 0;
        bool ok = guarded(run_section_body, &call, &code);
        uint64_t took = now_ms() - t0;
        if (!ok) {
            char b[96];
            std::snprintf(b, sizeof b, "fault 0x%08lx while rendering", code);
            s.quarantined = b;
            s.rendered = false;
            log("section %s FAULTED (0x%08lx); it is switched off until the game restarts", s.name.c_str(), code);
            continue;
        }
        if (took > 25 && now - s.slow_logged > 60000) {
            s.slow_logged = now;
            log("section %s took %llu ms to render", s.name.c_str(), (unsigned long long)took);
        }
        Json v;
        if (!call.error.empty()) {
            Writer w;
            w.begin_object();
            w.key("error");
            w.str(call.error);
            w.end_object();
            s.text = w.text();
            json::parse(s.text, s.value);
        } else if (call.w.text().empty() || !json::parse(call.w.text(), v)) {
            Writer w;
            w.begin_object();
            w.key("error");
            w.str("the section wrote invalid JSON");
            w.end_object();
            s.text = w.text();
            json::parse(s.text, s.value);
            log("section %s wrote invalid JSON: %s", s.name.c_str(), clip(call.w.text(), 200).c_str());
        } else {
            s.text = call.w.text();
            s.value = std::move(v);
        }
        s.rendered = true;
        if (s.text.size() > 64 * 1024 && !s.big_logged) {
            s.big_logged = true;
            log("section %s is %zu KB; the snapshot is rewritten whole, so keep sections under 64 KB", s.name.c_str(),
                s.text.size() / 1024);
        }
    }
    std::string all;
    for (auto &s : section_table())
        if (s.rendered)
            all += s.name + '=' + s.text + '\n';
    if (all != g.sections_text) {
        g.sections_text = std::move(all);
        g.sections_at = wall_ms();
    }
}

std::string snapshot_body(uint64_t now)
{
    Writer w;
    w.begin_object();
    w.key("boot");
    w.integer(int64_t(g.boot));
    w.key("pid");
    w.integer(int64_t(pid()));
    w.key("addon");
    w.begin_object();
    w.key("id");
    w.str("swofty.career-editor");
    w.key("version");
    w.str(kVersion);
    w.key("api");
    w.integer(swofty::api ? swofty::api->version : 0);
    w.end_object();

    w.key("status");
    state::write_status(w, now);

    w.key("bridge");
    w.begin_object();
    w.key("pending");
    w.begin_array();
    for (auto &[seq, b] : g.pending)
        w.integer(int64_t(seq));
    w.end_array();
    w.key("blocked");
    if (g.blocked_seq) {
        w.begin_object();
        w.key("seq");
        w.integer(int64_t(g.blocked_seq));
        w.key("why");
        w.str(g.blocked_why);
        w.end_object();
    } else {
        w.null();
    }
    w.key("last_ack");
    if (g.last_ack)
        w.integer(int64_t(g.last_ack));
    else
        w.null();
    w.key("applied");
    w.integer(int64_t(g.applied));
    w.key("failed");
    w.integer(int64_t(g.failed));
    w.key("ops");
    w.begin_array();
    for (auto &e : op_table()) {
        w.begin_object();
        w.key("name");
        w.str(e.name);
        w.key("anytime");
        w.boolean(e.anytime);
        w.key("off");
        if (e.quarantined.empty())
            w.null();
        else
            w.str(e.quarantined);
        w.end_object();
    }
    w.end_array();
    w.key("sections");
    w.begin_array();
    for (auto &s : section_table()) {
        w.begin_object();
        w.key("name");
        w.str(s.name);
        w.key("off");
        if (s.quarantined.empty())
            w.null();
        else
            w.str(s.quarantined);
        w.end_object();
    }
    w.end_array();
    w.key("duplicates");
    w.begin_array();
    for (auto &d : duplicates())
        w.str(d);
    w.end_array();
    w.end_object();

    w.key("sections_live");
    w.boolean(g.sections_live);
    w.key("sections_at");
    if (g.sections_at)
        w.integer(int64_t(g.sections_at));
    else
        w.null();
    w.key("sections");
    w.begin_object();
    for (auto &s : section_table()) {
        if (!s.rendered)
            continue;
        w.key(s.name.c_str());
        emit(w, s.value);
    }
    w.end_object();
    w.end_object();
    return w.text();
}

void snapshot(uint64_t now)
{
    g.sections_live = state::sections_ok();
    if (g.sections_live)
        render_sections(now);
    else if (!in_career())
        for (auto &s : section_table())
            s.rendered = false;
    std::string body = snapshot_body(now);
    if (body == g.last_body && now - g.last_write < kHeartbeatMs)
        return;
    std::string out = "{\"v\":1,\"written\":" + std::to_string(wall_ms()) + "," + body.substr(1);
    if (st::write("snapshot.json", out) == st::Result::ok) {
        g.last_body = std::move(body);
        g.last_write = now;
    }
}

} // namespace

namespace bridge {

const Json *section(const char *name)
{
    for (auto &s : section_table())
        if (s.rendered && s.name == name)
            return &s.value;
    return nullptr;
}

void start()
{
    g.boot = wall_ms();
    g.storage = st::available();
    std::string ops, anytime, sections;
    size_t n_anytime = 0;
    for (auto &e : op_table()) {
        std::string &list = e.anytime ? anytime : ops;
        list += (list.empty() ? "" : " ") + e.name;
        n_anytime += e.anytime;
    }
    for (auto &s : section_table())
        sections += (sections.empty() ? "" : " ") + s.name;
    log("bridge v%s: %zu Team HQ op(s) [%s], %zu anytime op(s) [%s], %zu section(s) [%s]", kVersion,
        op_table().size() - n_anytime, ops.c_str(), n_anytime, anytime.c_str(), section_table().size(),
        sections.c_str());
    for (auto &d : duplicates())
        log("duplicate registration ignored: %s", d.c_str());
    if (!g.storage) {
        log("storage library unavailable: the mailbox is off (enable the storage feature)");
        return;
    }
    st::write("inbox/.keep", std::string_view());
    st::write("outbox/.keep", std::string_view());
    init_doorbell();
#if defined(_WIN32)
    log("mailbox ready in swofty/addons/swofty.career-editor/ (inbox doorbell %s)",
        g.doorbell ? "on" : "off, listing every 0.5 s");
#endif
    list_inbox();
    g.last_list = now_ms();
    g.up = true;
}

void frame()
{
    if (!g.up)
        return;
    uint64_t now = now_ms();
    if (now - g.last_state >= kStatePollMs) {
        g.last_state = now;
        state::poll(now);
    }
    if (now - g.last_inbox >= kInboxPollMs) {
        g.last_inbox = now;
#if defined(_WIN32)
        bool relist = !g.doorbell || doorbell_rang() || now - g.last_list >= kInboxRelistMs;
#else
        bool relist = true;
#endif
        if (relist) {
            g.last_list = now;
            list_inbox();
        }
    }
    drain_one();
    if (g.force_render || now - g.last_snapshot >= kSnapshotMs) {
        g.force_render = false;
        g.last_snapshot = now;
        snapshot(now);
    }
}

} // namespace bridge

} // namespace ce
