#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <algorithm>
#include <array>
#include <cstdio>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>
#include "Zydis.h"
#include "json.hpp"
#include "safetyhook.hpp"
#include "swofty_api.h"

void crash_attach(SwoftyApi *a, const std::wstring &dir);
void settings_attach(SwoftyApi *a, const std::wstring &dir);
int settings_get(const char *id, uint32_t type, SwoftySettingValue *out);
int settings_watch(const char *id, SwoftySettingFn fn, void *user);

namespace {

static_assert(sizeof(SwoftyContext) == sizeof(safetyhook::Context64));
static_assert(offsetof(SwoftyContext, r12) == offsetof(safetyhook::Context64, r12));
static_assert(offsetof(SwoftyContext, rip) == offsetof(safetyhook::Context64, rip));

struct Binding {
    std::string id, kind, status, section, pattern, rkind;
    std::vector<uint8_t> expect;
    long long anchor = 0, insn = 0, len = 0, rva = -1, slots = 0;
    bool resolved = false;
    uintptr_t addr = 0;
};

struct Mixin {
    std::string target;
    uint32_t kind;
    int32_t priority;
    void *handler;
    void **orig;
    uintptr_t addr = 0;
};

struct Mod {
    std::string name;
    std::vector<Mixin> mixins;
    bool active = true;
};

struct Handler {
    size_t mod;
    Mixin *m;
};

struct Group {
    uint32_t kind;
    uintptr_t addr;
    size_t span;
    std::vector<Handler> handlers;
};

constexpr size_t MAX_SITES = 64;

SwoftyApi *api;
uintptr_t base;
bool same_build;
std::vector<Binding> bindings;
std::unordered_map<std::string, size_t> binding_index;
std::unordered_map<std::string, uintptr_t> scans;
json::Value session;
bool have_session, committed;
std::vector<Mod> mods;
Mod *current;

std::vector<safetyhook::InlineHook> inline_hooks;
std::vector<std::string> inline_names;
std::vector<safetyhook::MidHook> mid_hooks;
std::vector<safetyhook::Allocation> stubs;
std::vector<std::pair<uintptr_t, std::vector<uint8_t>>> written;
std::vector<void (*)(SwoftyContext *)> site_chains[MAX_SITES];
size_t sites_used;

template <size_t I> void site_thunk(safetyhook::Context &c)
{
    for (auto f : site_chains[I])
        f(reinterpret_cast<SwoftyContext *>(&c));
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

uintptr_t resolve_binding(const Binding &b)
{
    auto key = b.section + '|' + b.pattern;
    auto hit = scans.find(key);
    uintptr_t match = 0;
    if (hit == scans.end())
        api->scan(b.section.c_str(), b.pattern.c_str(), &match), scans[key] = match;
    else
        match = hit->second;
    if (!match)
        return unresolved(b, "signature did not match exactly once");
    uintptr_t at = match + b.anchor, target = at, i = match + b.insn;
    if (b.rkind == "rel32") {
        bool branch = (byte_at(i) == 0xE8 || byte_at(i) == 0xE9) && at == i + 1;
        bool jcc = byte_at(i) == 0x0F && (byte_at(i + 1) & 0xF0) == 0x80 && at == i + 2;
        if (!branch && !jcc)
            return unresolved(b, "anchor is not the rel32 of a call or jump");
        target = at + 4 + *reinterpret_cast<const int32_t *>(at);
    } else if (b.rkind == "riprel") {
        if (b.anchor <= b.insn || b.anchor + 4 > b.insn + b.len)
            return unresolved(b, "anchor is not inside the RIP-relative instruction");
        target = i + b.len + *reinterpret_cast<const int32_t *>(at);
    } else if (b.rkind != "addr") {
        return unresolved(b, "unknown resolve kind");
    }
    if (!b.expect.empty() && memcmp(reinterpret_cast<const void *>(at), b.expect.data(), b.expect.size()))
        return unresolved(b, "expect bytes differ");
    bool code = b.kind == "func" || b.kind == "site" || b.kind == "call";
    if (code ? !is_code(target) : !section_of(target))
        return unresolved(b, code ? "target is not in read-only code" : "target is outside the exe");
    if (same_build && b.rva >= 0 && rva(target) != (unsigned long long)b.rva)
        return unresolved(b, "target differs from the KB build");
    api->log("[core] bind %s -> 0x%llx (base+0x%llx), %s %s", b.id.c_str(), (unsigned long long)target, rva(target),
             b.kind.c_str(), b.status.c_str());
    return target;
}

Binding *find_binding(const std::string &id)
{
    auto it = binding_index.find(id);
    return it == binding_index.end() ? nullptr : &bindings[it->second];
}

uintptr_t api_resolve(const char *id)
{
    Binding *b = find_binding(id ? id : "");
    if (!b)
        api->log("[core] %s%sno binding %s in bindings.json", current ? "mod " : "", current ? (current->name + ": ").c_str() : "",
                 id ? id : "(null)");
    else if (!b->resolved)
        b->addr = resolve_binding(*b), b->resolved = true;
    uintptr_t a = b ? b->addr : 0;
    if (!a && current)
        api->log("[core] mod %s needs %s, which does not resolve on this build", current->name.c_str(), id ? id : "(null)");
    return a;
}

bool same_dll(const std::string &decl, const std::string &mod)
{
    auto slash = decl.find_last_of("/\\");
    std::string a = slash == std::string::npos ? decl : decl.substr(slash + 1);
    return a.size() == mod.size() && !_stricmp(a.c_str(), mod.c_str());
}

bool declared(const Mod &mod, const SwoftyMixin &m)
{
    for (auto &e : session["native"].a) {
        if (!same_dll(e["dll"].str(), mod.name))
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
    if (!current || committed) {
        api->log("[core] register_mixins outside SwoftyModInit; refused");
        return 0;
    }
    std::vector<Mixin> add;
    for (size_t i = 0; i < n; i++) {
        uint32_t k = SWOFTY_MIXIN_KIND(m[i].kind);
        bool on_load = k == SWOFTY_MIXIN_ON_LOAD;
        if (k < SWOFTY_MIXIN_WRAP || k > SWOFTY_MIXIN_ON_LOAD || !m[i].handler || (!on_load && !m[i].target) ||
            (k == SWOFTY_MIXIN_PATCH && !SWOFTY_MIXIN_ARG(m[i].kind))) {
            api->log("[core] mod %s: mixin %zu is malformed (kind 0x%x); registration refused", current->name.c_str(), i,
                     m[i].kind);
            return 0;
        }
        if (!on_load && have_session && !declared(*current, m[i])) {
            api->log("[core] mod %s: %s on %s is not declared in session.json; registration refused",
                     current->name.c_str(), kind_name(k), m[i].target);
            return 0;
        }
        add.push_back({on_load ? "" : m[i].target, m[i].kind, m[i].priority, m[i].handler, m[i].orig});
    }
    current->mixins.insert(current->mixins.end(), add.begin(), add.end());
    api->log("[core] mod %s: registered %zu mixin(s)", current->name.c_str(), n);
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
        api->log("[core] fields(%p, %zu): this mod already keeps %zu bytes on that object", obj, size, held);
    return out;
}

void drop(size_t i, const char *fmt, const char *a = "", const char *b = "", const char *c = "")
{
    if (!mods[i].active)
        return;
    char why[512];
    snprintf(why, sizeof why, fmt, a, b, c);
    mods[i].active = false;
    api->log("[core] mod %s dropped: %s; none of its mixins install", mods[i].name.c_str(), why);
}

void check_targets(size_t i)
{
    for (auto &m : mods[i].mixins) {
        uint32_t k = SWOFTY_MIXIN_KIND(m.kind), arg = SWOFTY_MIXIN_ARG(m.kind);
        if (k == SWOFTY_MIXIN_ON_LOAD)
            continue;
        Binding *b = find_binding(m.target);
        m.addr = b ? api_resolve(m.target.c_str()) : 0;
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

std::vector<Group> plan()
{
    std::vector<Group> groups;
    for (size_t i = 0; i < mods.size(); i++) {
        if (!mods[i].active)
            continue;
        for (auto &m : mods[i].mixins) {
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
        s += (s.empty() ? "" : " > ") + mods[h.mod].name + "(" + std::to_string(h.m->priority) +
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
                over += (over.empty() ? "" : ", ") + mods[h.mod].name, bad.push_back(h.mod);
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
                    bad.push_back(h.mod);
        }
    }
    for (size_t i : bad)
        drop(i, "it conflicts with another mod (see the conflict line above)");
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
    for (auto it = written.rbegin(); it != written.rend(); ++it) {
        DWORD old;
        auto p = reinterpret_cast<void *>(it->first);
        VirtualProtect(p, it->second.size(), PAGE_EXECUTE_READWRITE, &old);
        memcpy(p, it->second.data(), it->second.size());
        VirtualProtect(p, it->second.size(), old, &old);
        FlushInstructionCache(GetCurrentProcess(), p, it->second.size());
    }
    written.clear();
    stubs.clear();
    mid_hooks.clear();
    inline_hooks.clear();
    inline_names.clear();
    for (auto &c : site_chains)
        c.clear();
    sites_used = 0;
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
            site_chains[s].push_back(reinterpret_cast<void (*)(SwoftyContext *)>(h.m->handler));
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
                drop(h.mod, "installing its %s on %s failed", kind_name(g.kind), target.c_str());
            return false;
        }
        api->log("[core] %s %s at 0x%llx (base+0x%llx) installed: %s", kind_name(g.kind), target.c_str(),
                 (unsigned long long)g.addr, rva(g.addr), owners(g).c_str());
    }
    return true;
}

}

std::string core_code_owner(uintptr_t addr)
{
    for (auto &m : mods)
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
    json::Value doc;
    if (!json::parse(read_file(dir + L"bindings.json"), doc))
        api->log("[core] %lsbindings.json missing or unreadable; no binding will resolve", dir.c_str());
    for (auto &j : doc["bindings"].a) {
        Binding b{j["id"].str(), j["kind"].str(), j["status"].str(), j["section"].str(), j["pattern"].str(),
                  j["resolve"]["kind"].str("addr"), hex_bytes(j["expect"].str()), j["anchor"].num(),
                  j["resolve"]["insn"].num(), j["resolve"]["len"].num(), j["rva"].num(-1), j["slots"].num()};
        binding_index[b.id] = bindings.size();
        bindings.push_back(std::move(b));
    }
    auto nt = reinterpret_cast<const IMAGE_NT_HEADERS *>(base + reinterpret_cast<const IMAGE_DOS_HEADER *>(base)->e_lfanew);
    same_build = doc["build"]["pe_timestamp"].num() == (long long)nt->FileHeader.TimeDateStamp;
    api->log("[core] bindings.json: %zu binding(s) for build %.16s, %s", bindings.size(),
             doc["build"]["sha256"].str("?").c_str(),
             same_build ? "this exe (rva checked)" : "a different exe (signatures decide)");
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
    a->version = SWOFTY_API_VERSION;
    return 1;
}

extern "C" __declspec(dllexport) int SwoftyCoreInit(const wchar_t *name, SwoftyModInitFn init)
{
    if (committed) {
        api->log("[core] mod %ls: arrived after the commit; not initialised", name);
        return 0;
    }
    mods.push_back({utf8(name), {}, true});
    current = &mods.back();
    int r = init(api);
    if (!r)
        current->active = false, current->mixins.clear();
    current = nullptr;
    return r;
}

extern "C" __declspec(dllexport) int SwoftyCoreCommit(void)
{
    committed = true;
    for (size_t i = 0; i < mods.size(); i++)
        if (mods[i].active)
            check_targets(i);
    std::vector<Group> groups;
    do
        groups = plan();
    while (drop_conflicts(groups, false) || !install_all(groups));
    drop_conflicts(groups, true);
    size_t active = 0, count[7] = {};
    for (auto &g : groups)
        count[SWOFTY_MIXIN_KIND(g.kind)]++;
    for (auto &m : mods) {
        if (!m.active)
            continue;
        active++;
        for (auto &x : m.mixins)
            if (SWOFTY_MIXIN_KIND(x.kind) == SWOFTY_MIXIN_ON_LOAD)
                reinterpret_cast<void (*)()>(x.handler)();
    }
    api->log("[core] commit: %zu of %zu mixin mod(s) active; %zu wrap(s), %zu site(s), %zu vtable slot(s), %zu "
             "redirect(s), %zu patch(es)",
             active, mods.size(), count[SWOFTY_MIXIN_WRAP], count[SWOFTY_MIXIN_SITE], count[SWOFTY_MIXIN_VTABLE],
             count[SWOFTY_MIXIN_REDIRECT], count[SWOFTY_MIXIN_PATCH]);
    return (int)active;
}
