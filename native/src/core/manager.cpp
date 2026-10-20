#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdarg>
#include <cstdio>
#include <mutex>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>
#include "Zydis.h"
#include "json.hpp"
#include "perf.hpp"
#include "safetyhook.hpp"
#include "scan_cache.hpp"
#include "swofty_api.h"

void crash_attach(SwoftyApi *a, const std::wstring &dir);
void settings_attach(SwoftyApi *a, const std::wstring &dir);
int settings_get(const char *id, uint32_t type, SwoftySettingValue *out);
int settings_watch(const char *id, SwoftySettingFn fn, void *user);

namespace runtime {
const SwoftyAssets *assets_table();
const SwoftyEvents *events_table();
const SwoftyOverlay *overlay_table();
const SwoftyTelemetry *telemetry_table();
const SwoftyDb *db_table();
const SwoftyStorage *storage_table();
const SwoftyCareer *career_table();
struct CoreMixins {
    const char *name;
    std::vector<SwoftyMixin> mixins;
};
std::vector<CoreMixins> career_mixins();
void career_setup();
void ready();
void events_drop_module(void *module);
const std::wstring &swofty_root();
void on_stage(void (*fn)(int));
}

namespace {

static_assert(sizeof(SwoftyContext) == sizeof(safetyhook::Context64));
static_assert(offsetof(SwoftyContext, r12) == offsetof(safetyhook::Context64, r12));
static_assert(offsetof(SwoftyContext, rip) == offsetof(safetyhook::Context64, rip));

struct Binding {
    std::string id, kind, status, section, pattern, rkind;
    std::vector<uint8_t> expect;
    long long anchor = 0, insn = 0, len = 0, rva = -1, slots = 0, match = -1;
    bool resolved = false;
    uintptr_t addr = 0;
    std::string sa_string = {};
    long long sa_func_rva = -1, sa_func_off = 0;
    bool sa_durable = false;
};

struct Mixin {
    std::string target;
    uint32_t kind;
    int32_t priority;
    void *handler;
    void **orig;
    uintptr_t addr = 0;
};

struct Addon {
    std::string name;
    std::vector<Mixin> mixins;
    bool active = true;
};

struct Handler {
    size_t addon;
    Mixin *m;
};

struct Group {
    uint32_t kind;
    uintptr_t addr;
    size_t span;
    std::vector<Handler> handlers;
};

struct Held {
    uint32_t kind;
    uintptr_t addr;
    size_t span;
};

struct Mark {
    size_t written, stubs, mids, inlines, sites;
};

constexpr size_t MAX_SITES = 256;

SwoftyApi *api;
uintptr_t base;
bool same_build;
std::vector<Binding> bindings;
std::unordered_map<std::string, size_t> binding_index;
std::string bindings_file = "bindings.json";
std::unordered_map<std::string, uintptr_t> scans;
std::unordered_map<std::string, uintptr_t> anchor_funcs;
bool kb_first = true;
int (*loader_scan)(const char *, const char *, uintptr_t *);
scancache::Cache scan_cache;
std::wstring scan_cache_path;
bool scan_cache_on;
SRWLOCK scan_lock = SRWLOCK_INIT;
json::Value session;
bool have_session;
std::vector<Addon> addons;
Addon *current;
size_t settled;
std::vector<Held> held;
Mark mark;

std::vector<safetyhook::InlineHook> inline_hooks;
std::vector<std::string> inline_names;
std::vector<safetyhook::MidHook> mid_hooks;
std::vector<safetyhook::Allocation> stubs;
std::vector<std::pair<uintptr_t, std::vector<uint8_t>>> written;
struct SiteHandler {
    void (*fn)(SwoftyContext *);
    uint32_t probe;
};

std::vector<SiteHandler> site_chains[MAX_SITES];
size_t sites_used;

template <size_t I> void site_thunk(safetyhook::Context &c)
{
    for (auto &h : site_chains[I]) {
        perf::Scope p(h.probe);
        h.fn(reinterpret_cast<SwoftyContext *>(&c));
    }
}

template <size_t... I> constexpr auto make_thunks(std::index_sequence<I...>)
{
    return std::array<safetyhook::MidHookFn, sizeof...(I)>{site_thunk<I>...};
}
constexpr auto site_thunks = make_thunks(std::make_index_sequence<MAX_SITES>{});

const char *kind_name(uint32_t k)
{
    static const char *names[] = {"?", "wrap", "site", "vtable", "redirect", "patch", "on_load"};
    k = SWOFTY_MIXIN_KIND(k);
    return k < 7 ? names[k] : "?";
}

unsigned long long rva(uintptr_t a)
{
    return a - base;
}

std::string utf8(const wchar_t *w)
{
    char buf[MAX_PATH * 3] = {};
    WideCharToMultiByte(CP_UTF8, 0, w, -1, buf, sizeof buf - 1, nullptr, nullptr);
    return buf;
}

std::string read_file(const std::wstring &path)
{
    std::string out;
    if (FILE *f = _wfopen(path.c_str(), L"rb")) {
        char buf[65536];
        for (size_t n; (n = fread(buf, 1, sizeof buf, f));)
            out.append(buf, n);
        fclose(f);
    }
    return out;
}

std::string select_bindings(const std::wstring &dir, long long stamp, std::string &chosen)
{
    WIN32_FIND_DATAW fd;
    HANDLE h = FindFirstFileW((dir + L"bindings*.json").c_str(), &fd);
    std::string match_text, default_text, first_text;
    std::wstring match_name, first_name;
    int candidates = 0;
    if (h != INVALID_HANDLE_VALUE) {
        do {
            if (fd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY)
                continue;
            std::string text = read_file(dir + fd.cFileName);
            json::Value d;
            if (!json::parse(text, d) || d["build"].type == json::Value::Null)
                continue;
            candidates++;
            if (first_text.empty())
                first_text = text, first_name = fd.cFileName;
            if (!_wcsicmp(fd.cFileName, L"bindings.json"))
                default_text = text;
            if (match_text.empty() && (long long)d["build"]["pe_timestamp"].num() == stamp)
                match_text = text, match_name = fd.cFileName;
        } while (FindNextFileW(h, &fd));
        FindClose(h);
    }
    if (!match_text.empty()) {
        chosen = utf8(match_name.c_str());
        api->log("[core] bindings: %d set(s) present, selected %s by PE timestamp %lld", candidates, chosen.c_str(), stamp);
        return match_text;
    }
    if (!default_text.empty()) {
        chosen = "bindings.json";
        api->log("[core] bindings: %d set(s) present, none match PE timestamp %lld; using bindings.json", candidates,
                 stamp);
        return default_text;
    }
    chosen = first_name.empty() ? "bindings.json" : utf8(first_name.c_str());
    if (candidates)
        api->log("[core] bindings: %d set(s) present, none match PE timestamp %lld and no bindings.json; using %s",
                 candidates, stamp, chosen.c_str());
    return first_text;
}

std::vector<uint8_t> hex_bytes(const std::string &s)
{
    std::vector<uint8_t> out;
    for (const char *p = s.c_str(); *p;) {
        char *end;
        long v = strtol(p, &end, 16);
        if (end == p)
            break;
        out.push_back((uint8_t)v);
        p = end;
    }
    return out;
}

const IMAGE_SECTION_HEADER *section_of(uintptr_t a)
{
    auto nt = reinterpret_cast<const IMAGE_NT_HEADERS *>(base + reinterpret_cast<const IMAGE_DOS_HEADER *>(base)->e_lfanew);
    auto s = IMAGE_FIRST_SECTION(nt);
    for (int i = 0; i < nt->FileHeader.NumberOfSections; i++, s++)
        if (a - (base + s->VirtualAddress) < (s->Misc.VirtualSize ? s->Misc.VirtualSize : s->SizeOfRawData))
            return s;
    return nullptr;
}

bool is_code(uintptr_t a)
{
    auto s = section_of(a);
    return s && (s->Characteristics & IMAGE_SCN_MEM_EXECUTE) && !(s->Characteristics & IMAGE_SCN_MEM_WRITE);
}

bool is_data1_code(uintptr_t a)
{
    auto s = section_of(a);
    return s && !memcmp(s->Name, ".data1", 7) && (s->Characteristics & IMAGE_SCN_MEM_EXECUTE);
}

uint8_t byte_at(uintptr_t a)
{
    return *reinterpret_cast<const uint8_t *>(a);
}

size_t insn_span(uintptr_t a, size_t min)
{
    ZydisDecoder d;
    ZydisDecoderInit(&d, ZYDIS_MACHINE_MODE_LONG_64, ZYDIS_STACK_WIDTH_64);
    size_t n = 0;
    while (n < min) {
        ZydisDecodedInstruction i;
        if (!ZYAN_SUCCESS(ZydisDecoderDecodeInstruction(&d, nullptr, reinterpret_cast<void *>(a + n), 15, &i)))
            return std::max(n, min);
        n += i.length;
    }
    return n;
}

uintptr_t unresolved(const Binding &b, const char *why)
{
    api->log("[core] binding %s: %s", b.id.c_str(), why);
    return 0;
}

const IMAGE_NT_HEADERS *nt_headers()
{
    return reinterpret_cast<const IMAGE_NT_HEADERS *>(base + reinterpret_cast<const IMAGE_DOS_HEADER *>(base)->e_lfanew);
}

bool page_readable(uintptr_t p, size_t n)
{
    MEMORY_BASIC_INFORMATION m{};
    if (!VirtualQuery(reinterpret_cast<void *>(p), &m, sizeof m) || m.State != MEM_COMMIT ||
        !(m.Protect & 0xEE) || (m.Protect & (PAGE_GUARD | PAGE_NOACCESS)))
        return false;
    return p + n <= reinterpret_cast<uintptr_t>(m.BaseAddress) + m.RegionSize;
}

template <class F> void each_committed(uintptr_t lo, uintptr_t hi, F fn)
{
    for (uintptr_t a = lo; a < hi;) {
        MEMORY_BASIC_INFORMATION m{};
        if (!VirtualQuery(reinterpret_cast<void *>(a), &m, sizeof m))
            break;
        uintptr_t next = reinterpret_cast<uintptr_t>(m.BaseAddress) + m.RegionSize;
        if (next > hi)
            next = hi;
        if (next <= a)
            break;
        if (m.State == MEM_COMMIT && (m.Protect & 0xEE) && !(m.Protect & (PAGE_GUARD | PAGE_NOACCESS)))
            fn(a, next - a);
        a = next;
    }
}

uintptr_t section_end(const IMAGE_SECTION_HEADER *s)
{
    return base + s->VirtualAddress + (s->Misc.VirtualSize ? s->Misc.VirtualSize : s->SizeOfRawData);
}

uintptr_t locate_string(const std::string &str)
{
    std::string want = str;
    want.push_back('\0');
    auto nt = nt_headers();
    auto s = IMAGE_FIRST_SECTION(nt);
    uintptr_t hit = 0;
    int hits = 0;
    for (int i = 0; i < nt->FileHeader.NumberOfSections && hits < 2; i++, s++) {
        DWORD ch = s->Characteristics;
        if (!(ch & IMAGE_SCN_MEM_READ) || (ch & IMAGE_SCN_MEM_EXECUTE))
            continue;
        each_committed(base + s->VirtualAddress, section_end(s), [&](uintptr_t p, size_t n) {
            if (n < want.size())
                return;
            const uint8_t *lo = reinterpret_cast<const uint8_t *>(p), *last = lo + n - want.size();
            for (const uint8_t *q = lo; q <= last;) {
                const uint8_t *f = reinterpret_cast<const uint8_t *>(memchr(q, want[0], last - q + 1));
                if (!f)
                    break;
                if (!memcmp(f, want.data(), want.size()) && hits++ == 0)
                    hit = reinterpret_cast<uintptr_t>(f);
                q = f + 1;
            }
        });
    }
    return hits == 1 ? hit : 0;
}

uintptr_t func_base_of(uintptr_t site)
{
    struct RtFunc {
        uint32_t begin, end, unwind;
    };
    auto nt = nt_headers();
    auto &dir = nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION];
    if (!dir.VirtualAddress || dir.Size < sizeof(RtFunc))
        return 0;
    auto *fn = reinterpret_cast<const RtFunc *>(base + dir.VirtualAddress);
    size_t n = dir.Size / sizeof(RtFunc), lo = 0, hi = n;
    uint32_t r = static_cast<uint32_t>(site - base);
    while (lo < hi) {
        size_t mid = (lo + hi) / 2;
        if (fn[mid].begin <= r)
            lo = mid + 1;
        else
            hi = mid;
    }
    if (lo && fn[lo - 1].begin <= r && r < fn[lo - 1].end)
        return base + fn[lo - 1].begin;
    return 0;
}

uintptr_t anchor_func(const std::string &str)
{
    auto cached = anchor_funcs.find(str);
    if (cached != anchor_funcs.end())
        return cached->second;
    uintptr_t sVA = locate_string(str), fbase = 0;
    bool conflict = false;
    long long sites = 0;
    if (sVA) {
        auto nt = nt_headers();
        auto s = IMAGE_FIRST_SECTION(nt);
        for (int i = 0; i < nt->FileHeader.NumberOfSections && !conflict; i++, s++) {
            DWORD ch = s->Characteristics;
            if (!(ch & IMAGE_SCN_MEM_EXECUTE) || !(ch & IMAGE_SCN_MEM_READ) || (ch & IMAGE_SCN_MEM_WRITE))
                continue;
            each_committed(base + s->VirtualAddress, section_end(s), [&](uintptr_t p, size_t n) {
                if (n < 4)
                    return;
                for (uintptr_t q = p, last = p + n - 4; q <= last; q++) {
                    if (q + 4 + *reinterpret_cast<const int32_t *>(q) != sVA)
                        continue;
                    uintptr_t f = func_base_of(q);
                    if (!f)
                        continue;
                    sites++;
                    if (!fbase)
                        fbase = f;
                    else if (fbase != f)
                        conflict = true;
                }
            });
        }
    }
    uintptr_t out = (conflict || !sites) ? 0 : fbase;
    anchor_funcs[str] = out;
    return out;
}

uintptr_t apply_match(const Binding &b, uintptr_t match, const char **why)
{
    uintptr_t at = match + b.anchor, target = at, i = match + b.insn;
    if (b.rkind == "rel32" || b.rkind == "riprel") {
        if (!page_readable(std::min(at, i), (at > i ? at - i : i - at) + 16)) {
            *why = "anchor offset is not mapped on this build";
            return 0;
        }
    }
    if (b.rkind == "rel32") {
        bool branch = (byte_at(i) == 0xE8 || byte_at(i) == 0xE9) && at == i + 1;
        bool jcc = byte_at(i) == 0x0F && (byte_at(i + 1) & 0xF0) == 0x80 && at == i + 2;
        if (!branch && !jcc) {
            *why = "anchor is not the rel32 of a call or jump";
            return 0;
        }
        target = at + 4 + *reinterpret_cast<const int32_t *>(at);
    } else if (b.rkind == "riprel") {
        if (b.anchor <= b.insn || b.anchor + 4 > b.insn + b.len) {
            *why = "anchor is not inside the RIP-relative instruction";
            return 0;
        }
        target = i + b.len + *reinterpret_cast<const int32_t *>(at);
    } else if (b.rkind != "addr") {
        *why = "unknown resolve kind";
        return 0;
    }
    if (!b.expect.empty() &&
        (!page_readable(at, b.expect.size()) || memcmp(reinterpret_cast<const void *>(at), b.expect.data(), b.expect.size()))) {
        *why = "expect bytes differ";
        return 0;
    }
    bool code = b.kind == "func" || b.kind == "site" || b.kind == "call";
    bool data1_site = b.kind == "site" && b.section == ".data1" && is_data1_code(target);
    if (code ? !is_code(target) && !data1_site : !section_of(target)) {
        *why = code ? "target is not in read-only code" : "target is outside the exe";
        return 0;
    }
    if (same_build && b.rva >= 0 && rva(target) != (unsigned long long)b.rva) {
        *why = "target differs from the KB build";
        return 0;
    }
    return target;
}

const std::vector<uint8_t> *hooked_range(uintptr_t a, uintptr_t &start)
{
    for (auto &h : inline_hooks)
        if (a - h.target_address() < h.original_bytes().size())
            return start = h.target_address(), &h.original_bytes();
    for (auto &h : mid_hooks)
        if (a - h.target_address() < h.original_bytes().size())
            return start = h.target_address(), &h.original_bytes();
    for (auto &w : written)
        if (a - w.first < w.second.size())
            return start = w.first, &w.second;
    return nullptr;
}

bool section_named(uintptr_t a, const std::string &name)
{
    auto s = section_of(a);
    return s && name.size() <= 8 && !strncmp(reinterpret_cast<const char *>(s->Name), name.c_str(), 8);
}

bool bytes_at(const Binding &b, uintptr_t at, bool &under)
{
    auto pat = scancache::tokens(b.pattern);
    under = false;
    if (pat.empty() || !page_readable(at, pat.size()))
        return false;
    for (size_t k = 0; k < pat.size(); k++) {
        uintptr_t start = 0;
        auto orig = hooked_range(at + k, start);
        under |= orig != nullptr;
        uint8_t v = orig ? (*orig)[at + k - start] : byte_at(at + k);
        if (pat[k] >= 0 && v != pat[k])
            return false;
    }
    return true;
}

uintptr_t match_under_hooks(const Binding &b)
{
    if (!same_build || b.rva < 0 || b.rkind != "addr")
        return 0;
    uintptr_t at = base + b.rva - b.anchor;
    bool under = false;
    return bytes_at(b, at, under) && under ? at : 0;
}

uintptr_t kb_match(const Binding &b)
{
    if (!kb_first || !same_build || b.match < 0 || b.pattern.empty())
        return 0;
    uintptr_t at = base + b.match;
    bool under = false;
    if (!section_named(at, b.section) || !bytes_at(b, at, under)) {
        api->log("[core] binding %s: the pattern does not match at the KB address 0x%llx; scanning %s", b.id.c_str(),
                 (unsigned long long)at, b.section.c_str());
        return 0;
    }
    if (under && b.rkind != "addr")
        return 0;
    if (under)
        api->log("[core] binding %s: its bytes carry an installed hook; the pattern matches the bytes under it at the KB "
                 "address 0x%llx", b.id.c_str(), (unsigned long long)at);
    return at;
}

uintptr_t resolve_by_pattern(const Binding &b, const char **why, bool *at_kb)
{
    uintptr_t match = kb_match(b);
    *at_kb = match != 0;
    if (match)
        return apply_match(b, match, why);
    auto key = b.section + '|' + b.pattern;
    auto hit = scans.find(key);
    if (hit == scans.end())
        api->scan(b.section.c_str(), b.pattern.c_str(), &match), scans[key] = match;
    else
        match = hit->second;
    if (!match && (match = match_under_hooks(b)))
        api->log("[core] binding %s: its bytes carry an installed hook; the pattern matches the bytes under it at the KB "
                 "address 0x%llx", b.id.c_str(), (unsigned long long)match);
    if (!match) {
        *why = "signature did not match exactly once";
        return 0;
    }
    return apply_match(b, match, why);
}

uintptr_t resolve_by_anchor(const Binding &b, const char **why)
{
    if (b.sa_string.empty()) {
        *why = "no string anchor";
        return 0;
    }
    uintptr_t f = anchor_func(b.sa_string);
    if (!f) {
        *why = "string anchor did not locate a unique function";
        return 0;
    }
    if (same_build && b.sa_func_rva >= 0 && rva(f) != (unsigned long long)b.sa_func_rva) {
        *why = "string anchor function differs from the KB build";
        return 0;
    }
    return apply_match(b, f + b.sa_func_off, why);
}

uintptr_t resolve_binding(const Binding &b)
{
    const char *pw = "signature did not match exactly once", *aw = "no string anchor";
    uintptr_t target = 0;
    const char *method = "";
    bool kb = false;
    /* Resolver primary whenever the baked pattern cannot apply: a different exe than the KB build, or a
       per-build set (e.g. bindings.ea.json) that ships no pattern for this entry. Pattern-first only on
       the native build whose bytes the pattern was derived from. */
    bool anchor_first = !b.sa_string.empty() && (!same_build || b.pattern.empty());
    if (anchor_first) {
        if ((target = resolve_by_anchor(b, &aw)))
            method = "string anchor";
        else if ((target = resolve_by_pattern(b, &pw, &kb)))
            method = kb ? "pattern at the KB address" : "pattern";
    } else {
        if ((target = resolve_by_pattern(b, &pw, &kb)))
            method = kb ? "pattern at the KB address" : "pattern";
        else if (!b.sa_string.empty() && (target = resolve_by_anchor(b, &aw)))
            method = "string anchor (pattern fell back)";
    }
    if (!target) {
        if (b.sa_string.empty())
            return unresolved(b, pw);
        api->log("[core] binding %s: unresolved (pattern: %s; string anchor: %s)", b.id.c_str(), pw, aw);
        return 0;
    }
    api->log("[core] bind %s -> 0x%llx (base+0x%llx), %s %s via %s", b.id.c_str(), (unsigned long long)target,
             rva(target), b.kind.c_str(), b.status.c_str(), method);
    return target;
}

bool ea_diag_on()
{
    char v[8] = {0};
    return GetEnvironmentVariableA("SWOFTY_EA_DIAG", v, sizeof v) && v[0] == '1';
}

void ea_log(const char *fmt, ...)
{
    char body[2048];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(body, sizeof body, fmt, ap);
    va_end(ap);
    api->log("%s", body);
    static const std::wstring path = runtime::swofty_root() + L"ea-diagnostics.log";
    HANDLE f = CreateFileW(path.c_str(), FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_ALWAYS,
                           FILE_ATTRIBUTE_NORMAL, nullptr);
    if (f == INVALID_HANDLE_VALUE)
        return;
    SYSTEMTIME t;
    GetLocalTime(&t);
    char line[2200];
    int n = snprintf(line, sizeof line, "%04u-%02u-%02u %02u:%02u:%02u.%03u [%5lu] %s\r\n", t.wYear, t.wMonth, t.wDay,
                     t.wHour, t.wMinute, t.wSecond, t.wMilliseconds, GetCurrentThreadId(), body);
    if (n > 0) {
        DWORD w;
        WriteFile(f, line, (DWORD)(n > (int)sizeof line ? (int)sizeof line : n), &w, nullptr);
    }
    CloseHandle(f);
}

const IMAGE_SECTION_HEADER *code_section()
{
    auto nt = nt_headers();
    auto s = IMAGE_FIRST_SECTION(nt);
    const IMAGE_SECTION_HEADER *exec = nullptr;
    for (int i = 0; i < nt->FileHeader.NumberOfSections; i++, s++) {
        DWORD c = s->Characteristics;
        if ((c & IMAGE_SCN_MEM_EXECUTE) && (c & IMAGE_SCN_MEM_READ) && !(c & IMAGE_SCN_MEM_WRITE))
            return s;
        if ((c & IMAGE_SCN_MEM_EXECUTE) && !exec)
            exec = s;
    }
    return exec;
}

double page_entropy(uintptr_t p)
{
    if (!page_readable(p, 0x1000))
        return -1;
    unsigned long cnt[256] = {0};
    auto b = reinterpret_cast<const uint8_t *>(p);
    for (int i = 0; i < 0x1000; i++)
        cnt[b[i]]++;
    double h = 0;
    for (int i = 0; i < 256; i++)
        if (cnt[i]) {
            double pr = cnt[i] / 4096.0;
            h -= pr * std::log2(pr);
        }
    return h;
}

const char *ent_word(double e)
{
    return e < 0 ? "unreadable" : e < 6.5 ? "decrypted" : e > 7.5 ? "encrypted" : "mixed";
}

void ea_sample_entropy(const char *when)
{
    auto s = code_section();
    if (!s) {
        ea_log("[ea] entropy (%s): no EXECUTE|READ code section found", when);
        return;
    }
    uintptr_t start = base + s->VirtualAddress;
    size_t size = s->Misc.VirtualSize ? s->Misc.VirtualSize : s->SizeOfRawData;
    char name[9] = {0};
    memcpy(name, s->Name, 8);
    ea_log("[ea] entropy (%s): section %s base+0x%llx size 0x%llx, 8 pages (entropy <6.5 decrypted, ~8.0 packed)", when,
           name, rva(start), (unsigned long long)size);
    double sum = 0;
    int ok = 0;
    for (int k = 0; k < 8; k++) {
        size_t off = size > 8 * 0x1000 ? (size_t)((double)k / 8 * (double)(size - 0x1000)) & ~(size_t)0xfff
                                       : (size_t)k * 0x1000;
        double e = page_entropy(start + off);
        ea_log("[ea]   page +0x%08llx: entropy %.2f -> %s", (unsigned long long)off, e < 0 ? 0.0 : e, ent_word(e));
        if (e >= 0)
            ok++, sum += e;
    }
    if (ok)
        ea_log("[ea] entropy (%s): %d page(s), avg %.2f -> %s", when, ok, sum / ok,
               (sum / ok) < 6.5 ? "DECRYPTED in bulk" : (sum / ok) > 7.5 ? "STILL PACKED (encrypted)" : "PARTIAL/mixed");
}

void ea_binding_report(const char *when)
{
    ea_log("[ea] per-binding resolution report (%s): %zu binding(s); each shows anchor string, its code xref, the "
           "resolved site, and whether that code is decrypted",
           when, bindings.size());
    size_t resolved = 0, enc = 0, nostr = 0, noxref = 0, noanchor = 0;
    for (auto &b : bindings) {
        if (b.sa_string.empty()) {
            noanchor++;
            ea_log("[ea] binding %-30s VERDICT no-anchor (pattern-only; the EA code section holds no plaintext to "
                   "pattern-scan, needs a decrypted dump to derive an EA string anchor)",
                   b.id.c_str());
            continue;
        }
        uintptr_t sVA = locate_string(b.sa_string);
        if (!sVA) {
            nostr++;
            ea_log("[ea] binding %-30s string NOT found (unique) in the image -> VERDICT string-not-found",
                   b.id.c_str());
            continue;
        }
        uintptr_t f = anchor_func(b.sa_string);
        if (!f) {
            noxref++;
            ea_log("[ea] binding %-30s string @base+0x%llx, code xref NOT found (no unique rel32 to it) -> VERDICT "
                   "xref-not-found (code section is almost certainly still encrypted here)",
                   b.id.c_str(), rva(sVA));
            continue;
        }
        const char *why = "";
        uintptr_t target = resolve_by_anchor(b, &why);
        uintptr_t probe = target ? target : f;
        double e = page_entropy(probe);
        bool dec = e >= 0 && e < 6.5;
        if (target) {
            resolved++;
            ea_log("[ea] binding %-30s string @base+0x%llx, func @base+0x%llx, site @base+0x%llx, site-page entropy "
                   "%.2f (%s) -> VERDICT resolved%s",
                   b.id.c_str(), rva(sVA), rva(f), rva(target), e < 0 ? 0 : e, ent_word(e),
                   dec ? "" : " [WARNING: code reads encrypted, treat the address as unverified]");
        } else {
            enc++;
            /* Show the candidate site so a single run on the target build
               recaptures any in-function offset drift: the bytes at
               anchor-func + func_offset + anchor are what a corrected row
               must describe. */
            char bytes[64] = {0};
            uintptr_t site = f + (uintptr_t)b.sa_func_off + (uintptr_t)b.anchor;
            int o = 0;
            for (int i = 0; i < 12 && o < (int)sizeof bytes - 4; i++) {
                if (!page_readable(site + i, 1))
                    break;
                o += snprintf(bytes + o, sizeof bytes - o, "%02X ", *(uint8_t *)(site + i));
            }
            if (o)
                bytes[o - 1] = 0;
            ea_log("[ea] binding %-30s string @base+0x%llx, func @base+0x%llx, func-page entropy %.2f (%s) -> VERDICT "
                   "%s (%s); candidate site @base+0x%llx bytes [%s]",
                   b.id.c_str(), rva(sVA), rva(f), e < 0 ? 0 : e, ent_word(e),
                   dec ? "resolve-failed" : "code-still-encrypted", why, (unsigned long long)rva(site),
                   bytes[0] ? bytes : "unreadable");
        }
    }
    ea_log("[ea] per-binding summary (%s): %zu resolved, %zu code-still-encrypted, %zu xref-not-found, %zu "
           "string-not-found, %zu no-anchor (of %zu)",
           when, resolved, enc, noxref, nostr, noanchor, bindings.size());
}

void ea_stage_handler(int stage)
{
    if (stage != SWOFTY_STAGE_WINDOW)
        return;
    ea_log("[ea] window stage reached; re-sampling entropy and re-resolving bindings (settles bulk-vs-transient "
           "decryption: if pages/sites are now decrypted, decryption is bulk-on-unlock and the resolver works once "
           "the menu builds)");
    ea_sample_entropy("window");
    ea_binding_report("window");
}

int core_scan(const char *section, const char *pattern, uintptr_t *out)
{
    if (out)
        *out = 0;
    if (!scan_cache_on || !section || !pattern)
        return loader_scan(section, pattern, out);
    uint64_t known = 0;
    AcquireSRWLockShared(&scan_lock);
    bool have = scan_cache.find(section, pattern, known);
    ReleaseSRWLockShared(&scan_lock);
    if (have) {
        auto pat = scancache::tokens(pattern);
        uintptr_t at = base + known;
        if (section_named(at, section) && page_readable(at, pat.size()) &&
            scancache::matches(reinterpret_cast<const uint8_t *>(at), pat)) {
            api->log("scan %s [%s] -> 1 hit at 0x%llx (base+0x%llx), the address cached for this exe", section, pattern,
                     (unsigned long long)at, (unsigned long long)known);
            if (out)
                *out = at;
            return 1;
        }
    }
    uintptr_t at = 0;
    int hits = loader_scan(section, pattern, &at);
    AcquireSRWLockExclusive(&scan_lock);
    scan_cache.found(section, pattern, hits, hits == 1 ? rva(at) : 0);
    ReleaseSRWLockExclusive(&scan_lock);
    if (out && hits == 1)
        *out = at;
    return hits;
}

void open_scan_cache(SwoftyApi *a, const std::wstring &dir)
{
    char v[16] = {0};
    if (GetEnvironmentVariableA("SWOFTY_RESOLVE", v, sizeof v) && !_stricmp(v, "scan")) {
        kb_first = false;
        api->log("[core] binding resolution: SWOFTY_RESOLVE=scan, every pattern is scanned and the scan cache is off");
        return;
    }
    auto dos = reinterpret_cast<const IMAGE_DOS_HEADER *>(base);
    auto nt = nt_headers();
    size_t span = offsetof(IMAGE_NT_HEADERS, OptionalHeader) + nt->FileHeader.SizeOfOptionalHeader +
                  nt->FileHeader.NumberOfSections * sizeof(IMAGE_SECTION_HEADER);
    uint64_t key = scancache::fnv64(reinterpret_cast<const uint8_t *>(base) + dos->e_lfanew, span);
    CreateDirectoryW((dir + L"..\\..\\cache").c_str(), nullptr);
    scan_cache_path = dir + L"..\\..\\cache\\scan-cache.tsv";
    std::string text = read_file(scan_cache_path);
    if (text.empty()) {
        scan_cache.parse("", key);
        api->log("[core] scan cache: none yet for this exe (key %016llx); patterns found by a scan are kept in "
                 "swofty\\cache\\scan-cache.tsv", (unsigned long long)key);
    } else if (!scan_cache.parse(text, key)) {
        api->log("[core] scan cache: swofty\\cache\\scan-cache.tsv belongs to another exe; starting over (key %016llx)",
                 (unsigned long long)key);
    } else {
        api->log("[core] scan cache: %zu address(es) for this exe (key %016llx), each checked against the pattern before use",
                 scan_cache.rva.size(), (unsigned long long)key);
    }
    loader_scan = a->scan;
    a->scan = core_scan;
    scan_cache_on = true;
}

void save_scan_cache()
{
    AcquireSRWLockExclusive(&scan_lock);
    std::string text;
    size_t n = scan_cache.rva.size();
    if (scan_cache_on && scan_cache.dirty)
        text = scan_cache.serialize(), scan_cache.dirty = false;
    ReleaseSRWLockExclusive(&scan_lock);
    if (text.empty())
        return;
    std::wstring tmp = scan_cache_path + L".tmp";
    HANDLE f = CreateFileW(tmp.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    DWORD w = 0;
    bool ok = f != INVALID_HANDLE_VALUE && WriteFile(f, text.data(), DWORD(text.size()), &w, nullptr) && w == text.size();
    if (f != INVALID_HANDLE_VALUE)
        CloseHandle(f);
    ok = ok && MoveFileExW(tmp.c_str(), scan_cache_path.c_str(), MOVEFILE_REPLACE_EXISTING);
    if (!ok)
        DeleteFileW(tmp.c_str());
    api->log(ok ? "[core] scan cache: %zu address(es) saved to swofty\\cache\\scan-cache.tsv"
                : "[core] scan cache: writing swofty\\cache\\scan-cache.tsv failed (%zu address(es) kept for this session)",
             n);
}

Binding *find_binding(const std::string &id)
{
    auto it = binding_index.find(id);
    return it == binding_index.end() ? nullptr : &bindings[it->second];
}

std::recursive_mutex resolve_lock;

uintptr_t api_resolve(const char *id)
{
    std::lock_guard<std::recursive_mutex> g(resolve_lock);
    const char *name = id ? id : "(null)";
    Binding *b = find_binding(id ? id : "");
    if (!b) {
        if (current)
            api->log("[core] addon %s needs %s, which %s does not list: this loader is older than the addon, or %s is "
                     "not an exported binding",
                     current->name.c_str(), name, bindings_file.c_str(), name);
        else
            api->log("[core] no binding %s in %s", name, bindings_file.c_str());
        return 0;
    }
    if (!b->resolved)
        b->addr = resolve_binding(*b), b->resolved = true;
    if (!b->addr && current)
        api->log("[core] addon %s needs %s, which does not resolve on this build", current->name.c_str(), name);
    return b->addr;
}

bool same_dll(const std::string &decl, const std::string &dll)
{
    auto slash = decl.find_last_of("/\\");
    std::string a = slash == std::string::npos ? decl : decl.substr(slash + 1);
    return a.size() == dll.size() && !_stricmp(a.c_str(), dll.c_str());
}

bool declared(const Addon &addon, const SwoftyMixin &m)
{
    for (auto &e : session["native"].a) {
        if (!same_dll(e["dll"].str(), addon.name))
            continue;
        for (auto &d : e["mixins"].a) {
            bool vt = SWOFTY_MIXIN_KIND(m.kind) == SWOFTY_MIXIN_VTABLE;
            if (d["target"].str() == m.target && d["kind"].str() == kind_name(m.kind) &&
                (!vt || d["slot"].num(-1) == SWOFTY_MIXIN_ARG(m.kind)) &&
                (d["priority"].type == json::Value::Null || d["priority"].num() == m.priority) &&
                d["overwrite"].b == bool(m.kind & SWOFTY_MIXIN_OVERWRITE))
                return true;
        }
    }
    return false;
}

int api_register(const SwoftyMixin *m, size_t n)
{
    if (!current) {
        api->log("[core] register_mixins outside SwoftyAddonInit; refused");
        return 0;
    }
    std::vector<Mixin> add;
    for (size_t i = 0; i < n; i++) {
        uint32_t k = SWOFTY_MIXIN_KIND(m[i].kind);
        bool on_load = k == SWOFTY_MIXIN_ON_LOAD;
        if (k < SWOFTY_MIXIN_WRAP || k > SWOFTY_MIXIN_ON_LOAD || !m[i].handler || (!on_load && !m[i].target) ||
            (k == SWOFTY_MIXIN_PATCH && !SWOFTY_MIXIN_ARG(m[i].kind))) {
            api->log("[core] addon %s: mixin %zu is malformed (kind 0x%x); registration refused", current->name.c_str(), i,
                     m[i].kind);
            return 0;
        }
        if (!on_load && have_session && !declared(*current, m[i])) {
            api->log("[core] addon %s: %s on %s is not declared in session.json; registration refused",
                     current->name.c_str(), kind_name(k), m[i].target);
            return 0;
        }
        add.push_back({on_load ? "" : m[i].target, m[i].kind, m[i].priority, m[i].handler, m[i].orig});
    }
    current->mixins.insert(current->mixins.end(), add.begin(), add.end());
    api->log("[core] addon %s: registered %zu mixin(s)", current->name.c_str(), n);
    return 1;
}

void *api_fields(const void *obj, size_t size)
{
    struct Key {
        const void *obj;
        HMODULE mod;
        bool operator==(const Key &) const = default;
    };
    struct Hash {
        size_t operator()(const Key &k) const { return std::hash<const void *>()(k.obj) ^ (uintptr_t)k.mod; }
    };
    static std::unordered_map<Key, std::pair<size_t, void *>, Hash> table;
    static SRWLOCK lock = SRWLOCK_INIT;
    HMODULE mod = nullptr;
    GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                       reinterpret_cast<LPCWSTR>(__builtin_return_address(0)), &mod);
    void *out = nullptr;
    size_t held = 0;
    AcquireSRWLockExclusive(&lock);
    auto it = table.find({obj, mod});
    if (!size) {
        if (it != table.end())
            free(it->second.second), table.erase(it);
    } else if (it == table.end()) {
        out = calloc(1, size);
        table[{obj, mod}] = {size, out};
    } else if (it->second.first == size) {
        out = it->second.second;
    } else {
        held = it->second.first;
    }
    ReleaseSRWLockExclusive(&lock);
    if (held)
        api->log("[core] fields(%p, %zu): this addon already keeps %zu bytes on that object", obj, size, held);
    return out;
}

void drop(size_t i, const char *fmt, const char *a = "", const char *b = "", const char *c = "")
{
    if (!addons[i].active)
        return;
    char why[512];
    snprintf(why, sizeof why, fmt, a, b, c);
    addons[i].active = false;
    api->log("[core] addon %s dropped: %s; none of its mixins install", addons[i].name.c_str(), why);
}

void check_targets(size_t i)
{
    for (auto &m : addons[i].mixins) {
        uint32_t k = SWOFTY_MIXIN_KIND(m.kind), arg = SWOFTY_MIXIN_ARG(m.kind);
        if (k == SWOFTY_MIXIN_ON_LOAD)
            continue;
        Binding *b = find_binding(m.target);
        if (!b)
            return drop(i, "needs %s, which %s does not list", m.target.c_str(), bindings_file.c_str());
        m.addr = api_resolve(m.target.c_str());
        if (!m.addr)
            return drop(i, "needs %s, which does not resolve on this build", m.target.c_str());
        const std::string &bk = b->kind;
        bool ok = k == SWOFTY_MIXIN_WRAP     ? bk == "func"
                  : k == SWOFTY_MIXIN_VTABLE ? bk == "vtable"
                  : k == SWOFTY_MIXIN_REDIRECT ? bk == "call"
                                               : bk == "site" || bk == "call";
        if (!ok)
            return drop(i, "%s on %s needs another kind of binding, it is a %s", kind_name(k), m.target.c_str(), bk.c_str());
        if (k == SWOFTY_MIXIN_VTABLE && b->slots && arg >= (uint32_t)b->slots)
            return drop(i, "vtable slot out of range on %s", m.target.c_str());
        if (k == SWOFTY_MIXIN_VTABLE)
            m.addr += 8 * arg;
        if (k == SWOFTY_MIXIN_PATCH && arg > b->expect.size())
            return drop(i, "patch on %s is longer than the binding's expect bytes", m.target.c_str());
        if (k == SWOFTY_MIXIN_REDIRECT && byte_at(m.addr) != 0xE8)
            return drop(i, "redirect target %s is not a call rel32", m.target.c_str());
    }
}

std::vector<Group> plan(size_t from)
{
    std::vector<Group> groups;
    for (size_t i = from; i < addons.size(); i++) {
        if (!addons[i].active)
            continue;
        for (auto &m : addons[i].mixins) {
            uint32_t k = SWOFTY_MIXIN_KIND(m.kind);
            if (k == SWOFTY_MIXIN_ON_LOAD)
                continue;
            bool chains = k == SWOFTY_MIXIN_WRAP || k == SWOFTY_MIXIN_SITE || k == SWOFTY_MIXIN_VTABLE;
            auto g = std::find_if(groups.begin(), groups.end(),
                                  [&](const Group &x) { return chains && x.kind == k && x.addr == m.addr; });
            if (g == groups.end()) {
                size_t span = k == SWOFTY_MIXIN_VTABLE     ? 8
                              : k == SWOFTY_MIXIN_REDIRECT ? 5
                              : k == SWOFTY_MIXIN_PATCH    ? SWOFTY_MIXIN_ARG(m.kind)
                                                           : insn_span(m.addr, 5);
                groups.push_back({k, m.addr, span, {}});
                g = groups.end() - 1;
            }
            g->handlers.push_back({i, &m});
        }
    }
    for (auto &g : groups)
        std::stable_sort(g.handlers.begin(), g.handlers.end(),
                         [](const Handler &a, const Handler &b) { return a.m->priority > b.m->priority; });
    std::stable_sort(groups.begin(), groups.end(), [](const Group &a, const Group &b) { return a.kind < b.kind; });
    return groups;
}

std::string owners(const Group &g)
{
    std::string s;
    for (auto &h : g.handlers)
        s += (s.empty() ? "" : " > ") + addons[h.addon].name + "(" + std::to_string(h.m->priority) +
             (h.m->kind & SWOFTY_MIXIN_OVERWRITE ? ", overwrite" : "") + ")";
    return s;
}

bool drop_conflicts(const std::vector<Group> &groups, bool warn)
{
    std::vector<size_t> bad;
    for (size_t x = 0; x < groups.size(); x++) {
        auto &g = groups[x];
        auto &target = g.handlers[0].m->target;
        std::string over;
        for (auto &h : g.handlers)
            if (h.m->kind & SWOFTY_MIXIN_OVERWRITE)
                over += (over.empty() ? "" : ", ") + addons[h.addon].name, bad.push_back(h.addon);
        if (std::count(over.begin(), over.end(), ',')) {
            api->log("[core] conflict: two overwrites on %s: %s", target.c_str(), over.c_str());
        } else if (!over.empty()) {
            bad.pop_back();
            if (warn && !(g.handlers.back().m->kind & SWOFTY_MIXIN_OVERWRITE))
                api->log("[core] warning: an overwrite on %s hides the handlers below it: %s", target.c_str(),
                         owners(g).c_str());
        }
        for (size_t y = x + 1; y < groups.size(); y++) {
            auto &o = groups[y];
            bool data = g.kind == SWOFTY_MIXIN_VTABLE, odata = o.kind == SWOFTY_MIXIN_VTABLE;
            if (data != odata || g.addr >= o.addr + o.span || o.addr >= g.addr + g.span)
                continue;
            api->log("[core] conflict: %s %s at 0x%llx overlaps %s %s at 0x%llx: %s vs %s", kind_name(g.kind),
                     target.c_str(), (unsigned long long)g.addr, kind_name(o.kind), o.handlers[0].m->target.c_str(),
                     (unsigned long long)o.addr, owners(g).c_str(), owners(o).c_str());
            for (auto *z : {&g, &o})
                for (auto &h : z->handlers)
                    bad.push_back(h.addon);
        }
        for (auto &h : held) {
            bool data = g.kind == SWOFTY_MIXIN_VTABLE, hdata = h.kind == SWOFTY_MIXIN_VTABLE;
            if (data != hdata || g.addr >= h.addr + h.span || h.addr >= g.addr + g.span)
                continue;
            api->log("[core] conflict: %s %s at 0x%llx overlaps a %s installed at the early stage: %s", kind_name(g.kind),
                     target.c_str(), (unsigned long long)g.addr, kind_name(h.kind), owners(g).c_str());
            for (auto &x : g.handlers)
                bad.push_back(x.addon);
        }
    }
    for (size_t i : bad)
        drop(i, "it conflicts with another addon (see the conflict line above)");
    return !bad.empty();
}

bool write_bytes(uintptr_t at, const void *data, size_t n)
{
    DWORD old;
    auto p = reinterpret_cast<void *>(at);
    if (!VirtualProtect(p, n, is_code(at) ? PAGE_EXECUTE_READWRITE : PAGE_READWRITE, &old))
        return false;
    auto b = reinterpret_cast<const uint8_t *>(p);
    written.push_back({at, std::vector<uint8_t>(b, b + n)});
    memcpy(p, data, n);
    VirtualProtect(p, n, old, &old);
    FlushInstructionCache(GetCurrentProcess(), p, n);
    return true;
}

void uninstall_all()
{
    for (size_t i = written.size(); i-- > mark.written;) {
        auto &w = written[i];
        DWORD old;
        auto p = reinterpret_cast<void *>(w.first);
        VirtualProtect(p, w.second.size(), PAGE_EXECUTE_READWRITE, &old);
        memcpy(p, w.second.data(), w.second.size());
        VirtualProtect(p, w.second.size(), old, &old);
        FlushInstructionCache(GetCurrentProcess(), p, w.second.size());
    }
    written.resize(mark.written);
    stubs.erase(stubs.begin() + mark.stubs, stubs.end());
    mid_hooks.erase(mid_hooks.begin() + mark.mids, mid_hooks.end());
    inline_hooks.erase(inline_hooks.begin() + mark.inlines, inline_hooks.end());
    inline_names.erase(inline_names.begin() + mark.inlines, inline_names.end());
    for (size_t i = mark.sites; i < MAX_SITES; i++)
        site_chains[i].clear();
    sites_used = mark.sites;
}

void chain(const Group &g, void *last)
{
    for (size_t i = 0; i < g.handlers.size(); i++)
        if (g.handlers[i].m->orig)
            *g.handlers[i].m->orig = i + 1 < g.handlers.size() ? g.handlers[i + 1].m->handler : last;
}

const char *install(const Group &g)
{
    void *first = g.handlers[0].m->handler;
    switch (g.kind) {
    case SWOFTY_MIXIN_WRAP: {
        auto h = safetyhook::InlineHook::create(reinterpret_cast<void *>(g.addr), first, safetyhook::InlineHook::StartDisabled);
        if (!h)
            return "InlineHook failed";
        chain(g, h->original<void *>());
        if (!h->enable())
            return "InlineHook enable failed";
        inline_hooks.push_back(std::move(*h));
        inline_names.push_back(g.handlers[0].m->target);
        return nullptr;
    }
    case SWOFTY_MIXIN_SITE: {
        if (sites_used == MAX_SITES)
            return "too many sites";
        size_t s = sites_used++;
        for (auto &h : g.handlers)
            site_chains[s].push_back({reinterpret_cast<void (*)(SwoftyContext *)>(h.m->handler),
                                      perf::probe(perf::owner_of(h.m->handler), "site " + h.m->target)});
        auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(g.addr), site_thunks[s]);
        if (!h)
            return "MidHook failed";
        mid_hooks.push_back(std::move(*h));
        return nullptr;
    }
    case SWOFTY_MIXIN_VTABLE:
        chain(g, *reinterpret_cast<void **>(g.addr));
        return write_bytes(g.addr, &first, sizeof first) ? nullptr : "VirtualProtect failed";
    case SWOFTY_MIXIN_REDIRECT: {
        auto stub = safetyhook::Allocator::global()->allocate_near({reinterpret_cast<uint8_t *>(g.addr)}, 14);
        if (!stub)
            return "no memory within 2 GB of the call";
        uint8_t jmp[14] = {0xFF, 0x25};
        memcpy(jmp + 6, &first, 8);
        memcpy(stub->data(), jmp, sizeof jmp);
        chain(g, reinterpret_cast<void *>(g.addr + 5 + *reinterpret_cast<const int32_t *>(g.addr + 1)));
        int32_t rel = (int32_t)(stub->address() - (g.addr + 5));
        stubs.push_back(std::move(*stub));
        return write_bytes(g.addr + 1, &rel, 4) ? nullptr : "VirtualProtect failed";
    }
    case SWOFTY_MIXIN_PATCH: {
        auto *b = find_binding(g.handlers[0].m->target);
        if (memcmp(reinterpret_cast<const void *>(g.addr), b->expect.data(), b->expect.size()))
            return "bytes changed since resolve";
        return write_bytes(g.addr, first, g.span) ? nullptr : "VirtualProtect failed";
    }
    }
    return "unknown kind";
}

bool install_all(const std::vector<Group> &groups)
{
    for (auto &g : groups) {
        const char *err = install(g);
        auto &target = g.handlers[0].m->target;
        if (err) {
            api->log("[core] %s on %s at 0x%llx: %s", kind_name(g.kind), target.c_str(), (unsigned long long)g.addr, err);
            uninstall_all();
            for (auto &h : g.handlers)
                drop(h.addon, "installing its %s on %s failed", kind_name(g.kind), target.c_str());
            return false;
        }
        api->log("[core] %s %s at 0x%llx (base+0x%llx) installed: %s", kind_name(g.kind), target.c_str(),
                 (unsigned long long)g.addr, rva(g.addr), owners(g).c_str());
    }
    return true;
}

}

namespace runtime {

bool core_mixins_active(const char *name)
{
    for (auto &a : addons)
        if (a.name == name)
            return a.active && !a.mixins.empty();
    return false;
}

}

std::string core_code_owner(uintptr_t addr)
{
    for (auto &m : addons)
        for (auto &x : m.mixins)
            if (reinterpret_cast<uintptr_t>(x.handler) == addr && SWOFTY_MIXIN_KIND(x.kind) != SWOFTY_MIXIN_PATCH)
                return m.name + " " + kind_name(x.kind) + (x.target.empty() ? "" : " " + x.target) + " handler";
    for (size_t i = 0; i < inline_hooks.size(); i++)
        if (addr - inline_hooks[i].trampoline().address() < inline_hooks[i].trampoline().size())
            return "trampoline of the wrap on " + inline_names[i];
    return {};
}

extern "C" __declspec(dllexport) int SwoftyCoreAttach(SwoftyApi *a)
{
    api = a;
    base = a->module_base();
    wchar_t path[MAX_PATH] = {};
    HMODULE self = nullptr;
    GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                       reinterpret_cast<LPCWSTR>(&SwoftyCoreAttach), &self);
    GetModuleFileNameW(self, path, MAX_PATH);
    std::wstring dir(path);
    dir.resize(dir.find_last_of(L'\\') + 1);
    crash_attach(a, dir);
    settings_attach(a, dir);
    perf::attach();
    auto nt = reinterpret_cast<const IMAGE_NT_HEADERS *>(base + reinterpret_cast<const IMAGE_DOS_HEADER *>(base)->e_lfanew);
    long long running_stamp = (long long)nt->FileHeader.TimeDateStamp;
    std::string chosen = "bindings.json";
    json::Value doc;
    if (!json::parse(select_bindings(dir, running_stamp, chosen), doc))
        api->log("[core] %ls%s missing or unreadable; no binding will resolve", dir.c_str(), chosen.c_str());
    bindings_file = chosen;
    for (auto &j : doc["bindings"].a) {
        Binding b{j["id"].str(), j["kind"].str(), j["status"].str(), j["section"].str(), j["pattern"].str(),
                  j["resolve"]["kind"].str("addr"), hex_bytes(j["expect"].str()), j["anchor"].num(),
                  j["resolve"]["insn"].num(), j["resolve"]["len"].num(), j["rva"].num(-1), j["slots"].num(),
                  j["match"].num(-1)};
        const auto &sa = j["string_anchor"];
        if (sa.type == json::Value::Obj) {
            b.sa_string = sa["string"].str();
            b.sa_func_rva = sa["func_rva"].num(-1);
            b.sa_func_off = sa["func_offset"].num();
            b.sa_durable = sa["durable"].b;
        }
        binding_index[b.id] = bindings.size();
        bindings.push_back(std::move(b));
    }
    same_build = doc["build"]["pe_timestamp"].num() == running_stamp;
    size_t anchored = 0, durable = 0, placed = 0;
    for (auto &b : bindings) {
        if (!b.sa_string.empty())
            anchored++, durable += b.sa_durable;
        placed += b.match >= 0 && !b.pattern.empty();
    }
    api->log("[core] %s: %zu binding(s) for build %.16s, %s; %zu string-anchored (%zu build-agnostic), %s first",
             chosen.c_str(), bindings.size(), doc["build"]["sha256"].str("?").c_str(),
             same_build ? "this exe (rva checked)" : "a different exe (string anchors decide)", anchored, durable,
             same_build ? "pattern" : "string anchor");
    open_scan_cache(a, dir);
    long long kb_size = doc["build"]["size"].num(-1), exe_size = -1;
    wchar_t exe[MAX_PATH] = {};
    WIN32_FILE_ATTRIBUTE_DATA fa{};
    if (GetModuleFileNameW(nullptr, exe, MAX_PATH) && GetFileAttributesExW(exe, GetFileExInfoStandard, &fa))
        exe_size = (long long)(((unsigned long long)fa.nFileSizeHigh << 32) | fa.nFileSizeLow);
    if (kb_first && same_build && kb_size > 0 && exe_size != kb_size) {
        kb_first = false;
        api->log("[core] binding resolution: the exe is %lld bytes and the KB build %lld; the KB addresses are not used, "
                 "every pattern is scanned", exe_size, kb_size);
    }
    if (kb_first && same_build)
        api->log("[core] binding resolution: %zu of %zu pattern(s) carry their KB address; each is checked there first and "
                 "scanned only when the bytes differ", placed, bindings.size());
    std::wstring session_path = dir + L"..\\..\\session.json";
    std::string text = read_file(session_path);
    have_session = !text.empty();
    if (have_session && !json::parse(text, session)) {
        api->log("[core] session.json unreadable; every mixin registration is refused");
        session = {};
    }
    api->log(have_session ? "[core] session.json: mixins not declared there are refused"
                          : "[core] no session.json; mixins are not checked against a manifest");
    a->resolve = api_resolve;
    a->register_mixins = api_register;
    a->fields = api_fields;
    a->setting = settings_get;
    a->watch_setting = settings_watch;
    a->assets = runtime::assets_table();
    a->events = runtime::events_table();
    a->overlay = runtime::overlay_table();
    a->telemetry = runtime::telemetry_table();
    a->db = runtime::db_table();
    a->storage = runtime::storage_table();
    a->career = runtime::career_table();
    a->version = SWOFTY_API_VERSION;
    for (auto &c : runtime::career_mixins()) {
        Addon own{c.name, {}, true};
        for (auto &m : c.mixins)
            own.mixins.push_back({m.target, m.kind, m.priority, m.handler, m.orig});
        api->log("[core] core mixin set '%s': %zu mixin(s), installed or dropped on its own", c.name, own.mixins.size());
        addons.push_back(std::move(own));
    }
    if (ea_diag_on()) {
        ea_log("[ea] core attached on a build the loader flagged as NON-MATCHING (bindings %s, build %.16s, "
               "same_build=%d); running EA decryption + per-binding resolution diagnostics (report-only, no mixin is "
               "installed when resolution fails)",
               chosen.c_str(), doc["build"]["sha256"].str("?").c_str(), (int)same_build);
        ea_sample_entropy("core-init");
        ea_binding_report("core-init");
        runtime::on_stage(ea_stage_handler);
    }
    return 1;
}

extern "C" __declspec(dllexport) int SwoftyCoreInit(const wchar_t *name, SwoftyAddonInitFn init)
{
    addons.push_back({utf8(name), {}, true});
    current = &addons.back();
    int r = init(api);
    if (!r) {
        current->active = false, current->mixins.clear();
        HMODULE m = nullptr;
        GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                           reinterpret_cast<LPCWSTR>(init), &m);
        runtime::events_drop_module(m);
    }
    current = nullptr;
    return r;
}

extern "C" __declspec(dllexport) int SwoftyCoreCommit(void)
{
    size_t from = settled;
    settled = addons.size();
    for (size_t i = from; i < addons.size(); i++)
        if (addons[i].active)
            check_targets(i);
    std::vector<Group> groups;
    do
        groups = plan(from);
    while (drop_conflicts(groups, false) || !install_all(groups));
    drop_conflicts(groups, true);
    size_t active = 0, count[7] = {};
    for (auto &g : groups) {
        count[SWOFTY_MIXIN_KIND(g.kind)]++;
        held.push_back({g.kind, g.addr, g.span});
    }
    mark = {written.size(), stubs.size(), mid_hooks.size(), inline_hooks.size(), sites_used};
    if (!from)
        runtime::career_setup();
    for (size_t i = from; i < addons.size(); i++) {
        auto &m = addons[i];
        if (!m.active)
            continue;
        active++;
        for (auto &x : m.mixins)
            if (SWOFTY_MIXIN_KIND(x.kind) == SWOFTY_MIXIN_ON_LOAD)
                reinterpret_cast<void (*)()>(x.handler)();
    }
    if (!from)
        runtime::ready();
    api->log("[core] commit: %zu of %zu mixin addon(s) active; %zu wrap(s), %zu site(s), %zu vtable slot(s), %zu "
             "redirect(s), %zu patch(es)",
             active, addons.size() - from, count[SWOFTY_MIXIN_WRAP], count[SWOFTY_MIXIN_SITE], count[SWOFTY_MIXIN_VTABLE],
             count[SWOFTY_MIXIN_REDIRECT], count[SWOFTY_MIXIN_PATCH]);
    save_scan_cache();
    return (int)active;
}
