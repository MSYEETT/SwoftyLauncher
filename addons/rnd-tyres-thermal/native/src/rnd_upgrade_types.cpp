// SPDX-License-Identifier: AGPL-3.0-or-later
//
// R&D: Tyres & Thermal Management - upgrade types minted natively.
//
// The stock game knows 19 upgrade types (0..18). An upgrade row's upgrade_type
// string is matched against the 19-entry name table 0x14584f6b8 and anything
// unknown silently becomes 0 (Engine Power). This module mints ids 19+ for the
// Tyres department's own types and extends every place that enumerates or names
// the types, so a Completed upgrade of a new type reaches the car through its own
// <Upgrade name="radUpgrade..."> VTF block, scaled by its own progress:
//
//   resolvers   Career_Load_UpgradeRows 0x143539bc0 (join 0x14353a408) and the
//               upgrade_type_metadata loader 0x14353c4f0 (join 0x14353c8c1): when the
//               stock 19-name loop finds nothing, our names give ids 19+.
//   loops       the per-vehicle fill 0x141238cb0, the department page category rows
//               0x141c7b880 and the research category rows 0x141c73b80 iterate the
//               19-byte order table 0x14566fd16..0x14566fd29 through rip-relative
//               leas; those leas are re-pointed at an extended order table (0..18
//               then our ids) allocated within +-2 GB of the image. The stock table
//               is never written.
//   apply       0x14fa4deb0 applies keys < 11 and 11..17 only; a hook on its second
//               filter sends our keys down the same call. 0x14fa63050 names a block
//               only for type < 0x12; a hook on its "?" path returns our block name.
//   UI labels   16 inlined type switches (cmp type,0x12 ; ja "INVALID UPGRADE")
//               build lng_rnd_<type> and <dept>_<type>; their default case returns
//               our type suffix.
//   telemetry   three ElasticSearch reporters index the camel-case type table
//               0x14564a0d0 with no bound (a read past it is a wild pointer); they get
//               our camel-case name.
//   objectives  the weekend objectives 0x14295c0a0 keep a std::bitset<19> of Available
//               types and throw past 18; our types are left out of it.
//
// The metadata vector (U+0x1a38), the progress function 0x143526260 and the type
// map are dynamic and keyed by type, so they need nothing. Every site is
// byte-verified at its fixed RVA after one exactly-one .xtls scan; the consumers
// are installed all-or-nothing and the resolvers last, so no id 19+ ever exists
// unless every consumer is live. All addresses are for build d873a1c8 (image base
// 0x140000000). Details: native/HOOKS.md v0.9.0.

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <array>
#include <atomic>
#include <cstdint>
#include <cstring>
#include <utility>
#include <vector>

#include <safetyhook.hpp>
#include <swofty/mixin.hpp>

#include "rnd_upgrade_types.hpp"

namespace {

constexpr uintptr_t kImageBase = 0x140000000ull;

constexpr uintptr_t rva(uintptr_t va)
{
    return va - kImageBase;
}

SWOFTY_SETTING(bool, tyres_upgrade_types, true);
SWOFTY_SETTING(bool, log_upgrade_apply, true);

SWOFTY_ON_CHANGE(tyres_upgrade_types, bool on)
{
    swofty::log("[rnd-tyres] tyres_upgrade_types toggled -> %s; install-time, relaunch to apply", on ? "on" : "off");
}

SWOFTY_ON_CHANGE(log_upgrade_apply, bool on)
{
    swofty::log("[rnd-tyres] log_upgrade_apply toggled -> %s; install-time, relaunch to apply", on ? "on" : "off");
}

constexpr bool kShipPuCooling = true;

constexpr uint8_t kStockTypes = 19;
constexpr uint8_t kDeptChassis = 1;
constexpr uint8_t kDeptPowertrain = 3;

struct NewType {
    uint8_t id;
    const char *name;
    const char *camel;
    const char *block;
    const char *suffix;
    uint8_t effect_dept;
};

constexpr NewType kAllTypes[] = {
    {19, "Tyre Thermal", "TyreThermal", "radUpgradeTyreThermal", "tyre_thermal", kDeptChassis},
    {20, "Brake Cooling", "BrakeCooling", "radUpgradeBrakeCooling", "brake_cooling", kDeptChassis},
    {21, "Tyre Life", "TyreLife", "radUpgradeTyreLife", "tyre_life", kDeptChassis},
    {22, "Power Unit Cooling", "PowerUnitCooling", "radUpgradePUCooling", "pu_cooling", kDeptPowertrain},
};
constexpr size_t kNumTypes = kShipPuCooling ? 4 : 3;
constexpr uint8_t kLastId = static_cast<uint8_t>(kStockTypes + kNumTypes - 1);
static_assert(kAllTypes[0].id == kStockTypes && kAllTypes[3].id == 22);

const char kUnknown[] = "unknown";

const NewType *find_id(uint64_t t)
{
    if (t < kStockTypes || t > kLastId)
        return nullptr;
    return &kAllTypes[t - kStockTypes];
}

bool region_readable(const void *p, size_t n)
{
    MEMORY_BASIC_INFORMATION mbi{};
    if (VirtualQuery(p, &mbi, sizeof mbi) != sizeof mbi)
        return false;
    if (mbi.State != MEM_COMMIT || (mbi.Protect & PAGE_GUARD) || (mbi.Protect & PAGE_NOACCESS))
        return false;
    uintptr_t end = reinterpret_cast<uintptr_t>(mbi.BaseAddress) + mbi.RegionSize;
    return reinterpret_cast<uintptr_t>(p) + n <= end;
}

struct RegionCache {
    uintptr_t lo = 0;
    uintptr_t hi = 0;

    bool ok(uintptr_t p, size_t n)
    {
        if (!p || p + n < p)
            return false;
        if (p >= lo && p + n <= hi)
            return true;
        MEMORY_BASIC_INFORMATION mbi{};
        if (VirtualQuery(reinterpret_cast<void *>(p), &mbi, sizeof mbi) != sizeof mbi)
            return false;
        if (mbi.State != MEM_COMMIT || (mbi.Protect & PAGE_GUARD) || (mbi.Protect & PAGE_NOACCESS))
            return false;
        uintptr_t b = reinterpret_cast<uintptr_t>(mbi.BaseAddress);
        uintptr_t e = b + mbi.RegionSize;
        if (p + n > e)
            return false;
        lo = b;
        hi = e;
        return true;
    }
};

bool bytes_are(uintptr_t addr, const uint8_t *want, size_t n)
{
    if (!region_readable(reinterpret_cast<void *>(addr), n))
        return false;
    const volatile uint8_t *p = reinterpret_cast<const volatile uint8_t *>(addr);
    for (size_t i = 0; i < n; ++i)
        if (p[i] != want[i])
            return false;
    return true;
}

template <class T> T rd(uintptr_t a)
{
    T v;
    std::memcpy(&v, reinterpret_cast<const void *>(a), sizeof v);
    return v;
}

char lower(char ch)
{
    return (ch >= 'A' && ch <= 'Z') ? static_cast<char>(ch | 0x20) : ch;
}

const NewType *match_name(const char *s)
{
    if (!s || !region_readable(s, 1))
        return nullptr;
    for (size_t i = 0; i < kNumTypes; ++i) {
        const char *n = kAllTypes[i].name;
        size_t k = 0;
        for (; n[k]; ++k) {
            if (((reinterpret_cast<uintptr_t>(s + k) & 0xfff) == 0) && !region_readable(s + k, 1))
                return nullptr;
            if (lower(s[k]) != lower(n[k]))
                break;
        }
        if (n[k] == '\0' && region_readable(s + k, 1) && s[k] == '\0')
            return &kAllTypes[i];
    }
    return nullptr;
}

std::atomic<bool> g_live{false};
std::atomic<int> g_resolve_log{0};
std::atomic<int> g_ui_log{0};
std::atomic<int> g_tele_log{0};
std::atomic<int> g_name_log{0};

// ---------------------------------------------------------------------------
// Resolvers.
//   0x14353a3d0..0x14353a405  for i in 0..18: if (!strcmp(names[order[i]], s)) break
//                             r14 = i, r15 = id, r13 = decoded upgrade_type string
//   0x14353a408               mov [rsp+0xc8],r15b   (join: matched, or r14 == 0x13 and r15 = 0)
//   0x14353c890..0x14353c8be  same loop in the metadata loader: rsi = i, r14 = id, r12 = string
//   0x14353c8c1               lea rsi,[rsp+0x248]   (join)

constexpr uintptr_t kVaRowsScan = 0x14353a3d7ull;
constexpr uintptr_t kVaRowsWin = 0x14353a3d0ull;
constexpr uintptr_t kVaRowsJoin = 0x14353a408ull;
constexpr uintptr_t kVaMetaWin = 0x14353c890ull;
constexpr uintptr_t kVaMetaJoin = 0x14353c8c1ull;

const char kRowsSig[] = "4D 0F BE 3C 06 48 89 E9 49 83 FF 12 77 0B 48 8D 05 CC 52 31 02 4A 8B 0C F8 4C 89 EA E8 20 35 "
                        "E1 01 85 C0 74 0C 49 FF C6 49 83 FE 13 75 CB 45 31 FF 44 88 BC 24 C8 00 00 00";

const uint8_t kRowsBytes[] = {0x48, 0x8d, 0x05, 0x3f, 0x59, 0x13, 0x02, 0x4d, 0x0f, 0xbe, 0x3c, 0x06, 0x48,
                              0x89, 0xe9, 0x49, 0x83, 0xff, 0x12, 0x77, 0x0b, 0x48, 0x8d, 0x05, 0xcc, 0x52,
                              0x31, 0x02, 0x4a, 0x8b, 0x0c, 0xf8, 0x4c, 0x89, 0xea, 0xe8, 0x20, 0x35, 0xe1,
                              0x01, 0x85, 0xc0, 0x74, 0x0c, 0x49, 0xff, 0xc6, 0x49, 0x83, 0xfe, 0x13, 0x75,
                              0xcb, 0x45, 0x31, 0xff, 0x44, 0x88, 0xbc, 0x24, 0xc8, 0x00, 0x00, 0x00};
const uint8_t kMetaBytes[] = {0x4e, 0x0f, 0xbe, 0x34, 0x3e, 0x48, 0x89, 0xd9, 0x49, 0x83, 0xfe, 0x12,
                              0x77, 0x0b, 0x48, 0x8d, 0x05, 0x13, 0x2e, 0x31, 0x02, 0x4a, 0x8b, 0x0c,
                              0xf0, 0x4c, 0x89, 0xe2, 0xe8, 0x67, 0x10, 0xe1, 0x01, 0x85, 0xc0, 0x74,
                              0x0c, 0x48, 0xff, 0xc6, 0x48, 0x83, 0xfe, 0x13, 0x75, 0xd2, 0x45, 0x31,
                              0xf6, 0x48, 0x8d, 0xb4, 0x24, 0x48, 0x02, 0x00, 0x00};

void log_resolve(const char *where, const NewType *t)
{
    if (g_resolve_log.fetch_add(1) < 48)
        swofty::log("[rnd-tyres] upgrade types: %s \"%s\" -> type %u (%s)", where, t->name, (unsigned)t->id,
                    t->block);
}

void on_rows_join(safetyhook::Context &c)
{
    if (c.r14 != kStockTypes)
        return;
    const NewType *t = match_name(reinterpret_cast<const char *>(c.r13));
    if (!t)
        return;
    c.r15 = t->id;
    log_resolve("upgrade row", t);
}

void on_meta_join(safetyhook::Context &c)
{
    if (c.rsi != kStockTypes)
        return;
    const NewType *t = match_name(reinterpret_cast<const char *>(c.r12));
    if (!t)
        return;
    c.r14 = t->id;
    log_resolve("upgrade_type_metadata row", t);
}

// ---------------------------------------------------------------------------
// Loops over the order table 0x14566fd16 (19 bytes, end 0x14566fd29).

constexpr uintptr_t kVaOrder = 0x14566fd16ull;
constexpr uintptr_t kVaOrderEnd = 0x14566fd29ull;

struct LeaSite {
    uintptr_t va;
    uint8_t rex;
    uint8_t modrm;
    bool end;
    const char *what;
};

const LeaSite kLeaSites[] = {
    {0x141238f0aull, 0x4c, 0x3d, false, "vehicle fill 0x141238cb0 order start (lea r15)"},
    {0x141238f84ull, 0x48, 0x05, true, "vehicle fill 0x141238cb0 order end (lea rax)"},
    {0x141c7e4c8ull, 0x4c, 0x35, false, "department page 0x141c7b880 category order start (lea r14)"},
    {0x141c7e4dfull, 0x4c, 0x3d, true, "department page 0x141c7b880 category order end (lea r15)"},
    {0x141c75733ull, 0x4c, 0x25, false, "research rows 0x141c73b80 category order start (lea r12)"},
    {0x141c7574aull, 0x4c, 0x2d, true, "research rows 0x141c73b80 category order end (lea r13)"},
};
constexpr size_t kNumLeas = sizeof(kLeaSites) / sizeof(kLeaSites[0]);

// Loop bodies, verified around each lea: fill (0x141238f84 cmp r15,rax ; je), page
// (0x141c7e4cf/0x141c7e4d7 leas), research (0x141c7573a/0x141c75742 leas).
const uint8_t kFillEndTail[] = {0x49, 0x39, 0xc7, 0x74, 0x67};
const uint8_t kPageMid[] = {0x48, 0x8d, 0xbc, 0x24, 0x48, 0x01, 0x00, 0x00, 0x48, 0x8d, 0x9c, 0x24, 0xe0, 0x01, 0x00, 0x00};
const uint8_t kResearchMid[] = {0x48, 0x8d, 0xbc, 0x24, 0x08, 0x01, 0x00, 0x00,
                                0x48, 0x8d, 0x9c, 0x24, 0xc0, 0x0d, 0x00, 0x00};

safetyhook::Allocation g_order_mem;
uintptr_t g_order = 0;
size_t g_order_len = 0;
std::array<int32_t, kNumLeas> g_old_disp{};
bool g_leas_patched = false;

bool lea_ok(uintptr_t base, const LeaSite &s)
{
    uintptr_t at = base + rva(s.va);
    const uint8_t head[3] = {s.rex, 0x8d, s.modrm};
    if (!bytes_are(at, head, 3))
        return false;
    int32_t disp = rd<int32_t>(at + 3);
    uintptr_t tgt = at + 7 + static_cast<intptr_t>(disp);
    return tgt == base + rva(s.end ? kVaOrderEnd : kVaOrder);
}

bool write_disp(uintptr_t at, int32_t disp)
{
    DWORD old = 0;
    if (!VirtualProtect(reinterpret_cast<void *>(at), 4, PAGE_EXECUTE_READWRITE, &old))
        return false;
    std::memcpy(reinterpret_cast<void *>(at), &disp, 4);
    DWORD tmp = 0;
    VirtualProtect(reinterpret_cast<void *>(at), 4, old, &tmp);
    FlushInstructionCache(GetCurrentProcess(), reinterpret_cast<void *>(at), 4);
    return true;
}

void restore_leas(uintptr_t base, size_t upto)
{
    for (size_t i = 0; i < upto; ++i)
        write_disp(base + rva(kLeaSites[i].va) + 3, g_old_disp[i]);
}

bool patch_leas(uintptr_t base)
{
    for (size_t i = 0; i < kNumLeas; ++i) {
        uintptr_t at = base + rva(kLeaSites[i].va);
        uintptr_t tgt = g_order + (kLeaSites[i].end ? g_order_len : 0);
        intptr_t d = static_cast<intptr_t>(tgt) - static_cast<intptr_t>(at + 7);
        if (d < INT32_MIN || d > INT32_MAX) {
            swofty::log("[rnd-tyres] upgrade types: extended order table 0x%llx out of rel32 reach of %s; NOT "
                        "installed (SAFE)",
                        (unsigned long long)g_order, kLeaSites[i].what);
            restore_leas(base, i);
            return false;
        }
        g_old_disp[i] = rd<int32_t>(at + 3);
        if (!write_disp(at + 3, static_cast<int32_t>(d))) {
            swofty::log("[rnd-tyres] upgrade types: VirtualProtect failed on %s; NOT installed (SAFE)",
                        kLeaSites[i].what);
            restore_leas(base, i);
            return false;
        }
    }
    g_leas_patched = true;
    for (const auto &s : kLeaSites)
        swofty::log("[rnd-tyres] upgrade types: %s @0x%llx -> extended order table 0x%llx%s", s.what,
                    (unsigned long long)(base + rva(s.va)), (unsigned long long)g_order, s.end ? " + len" : "");
    return true;
}

// ---------------------------------------------------------------------------
// Apply.
//   0x14fa4e018 movzx edx,byte [rbx+0x20] ; lea eax,[rdx-0xb] ; cmp al,6 ; ja 0x14fa4e02b
//   0x14fa4e023 mov rcx,rsi ; call 0x142f64860 (-> 0x14fa63050(component, type))
//   0x14fa630d1 cmp dl,0x11 ; ja 0x14fa6315f ; ... name = 0x145803ce4 + rel32[type]
//   0x14fa6315f lea rdx,["?"] ; 0x14fa63166 join (rdx = block name)

constexpr uintptr_t kVaFilterWin = 0x14fa4e018ull;
constexpr uintptr_t kVaFilter = 0x14fa4e01cull;
constexpr uintptr_t kVaFilterCall = 0x14fa4e023ull;
constexpr uintptr_t kVaNameWin = 0x14fa630d1ull;
constexpr uintptr_t kVaNameQ = 0x14fa6315full;
constexpr uintptr_t kVaNameJoin = 0x14fa63166ull;
constexpr uintptr_t kVaApply = 0x14fa63050ull;
constexpr uintptr_t kVaBlockNames = 0x145803ce4ull;

const uint8_t kFilterBytes[] = {0x0f, 0xb6, 0x53, 0x20, 0x8d, 0x42, 0xf5, 0x3c, 0x06, 0x77,
                                0x08, 0x48, 0x89, 0xf1, 0xe8, 0x35, 0x68, 0x51, 0xf3};
const uint8_t kNameBytes[] = {0x80, 0xfa, 0x11, 0x0f, 0x87, 0x85, 0x00, 0x00, 0x00, 0x48, 0x0f, 0xbe, 0xc2, 0x4c,
                              0x8d, 0x05, 0xff, 0x0b, 0xda, 0xf5, 0x49, 0x63, 0x14, 0x80, 0x4c, 0x01, 0xc2};
const uint8_t kNameQBytes[] = {0x48, 0x8d, 0x15, 0xfa, 0xf5, 0xba, 0xf5, 0x4c, 0x8b, 0x41, 0x18, 0xb8};
const uint8_t kApplyBytes[] = {0x4c, 0x8b, 0x41, 0x28, 0x4d, 0x8b, 0x48, 0x08,
                               0x41, 0x80, 0x79, 0x19, 0x00, 0x4c, 0x89, 0xc0};

uintptr_t g_filter_call = 0;
uintptr_t g_name_join = 0;
uintptr_t g_block_names = 0;

void on_apply_filter(safetyhook::Context &c)
{
    if (find_id(c.rdx & 0xff) && (c.rdx >> 8) == 0)
        c.rip = g_filter_call;
}

void on_block_name(safetyhook::Context &c)
{
    const NewType *t = find_id(c.rdx & 0xff);
    if (!t)
        return;
    c.rdx = reinterpret_cast<uintptr_t>(t->block);
    c.rip = g_name_join;
    if (g_name_log.fetch_add(1) < 8)
        swofty::log("[rnd-tyres] upgrade types: apply 0x14fa63050 type %u names block %s", (unsigned)t->id,
                    t->block);
}

// ---------------------------------------------------------------------------
// Apply-entry progress log (read-only). rcx = vehicle upgrade component, dl = type.
// Component +0x28 = std::map<u8, std::map<u8, {progress, mass, bhp}>> header; outer
// node key +0x20, value (inner header) +0x28; inner node value +0x20 = progress;
// node +0x19 = is-nil. perc = clamp(sum of inner progress, 0, 1), exactly as
// 0x14fa63050 computes it; an absent key makes the game return without applying.

const char *block_name(uint8_t type)
{
    if (const NewType *t = find_id(type))
        return t->block;
    if (type < 0x12 && g_block_names) {
        int32_t rel = rd<int32_t>(g_block_names + type * 4u);
        return reinterpret_cast<const char *>(g_block_names + static_cast<intptr_t>(rel));
    }
    return "?";
}

bool map_progress(uintptr_t comp, uint8_t type, float &perc)
{
    RegionCache rc;
    if (!rc.ok(comp + 0x28, 8))
        return false;
    uintptr_t head = rd<uintptr_t>(comp + 0x28);
    if (!rc.ok(head, 0x30))
        return false;
    uintptr_t node = rd<uintptr_t>(head + 8);
    uintptr_t found = head;
    for (int guard = 0; guard < 64; ++guard) {
        if (!rc.ok(node, 0x30))
            return false;
        if (rd<uint8_t>(node + 0x19))
            break;
        if (rd<uint8_t>(node + 0x20) >= type) {
            found = node;
            node = rd<uintptr_t>(node);
        } else {
            node = rd<uintptr_t>(node + 0x10);
        }
    }
    if (found == head || rd<uint8_t>(found + 0x20) != type)
        return false;
    uintptr_t inner = rd<uintptr_t>(found + 0x28);
    if (!rc.ok(inner, 0x30))
        return false;
    uintptr_t stack[64];
    int sp = 0;
    stack[sp++] = rd<uintptr_t>(inner + 8);
    float sum = 0.0f;
    int visited = 0;
    while (sp > 0 && visited < 64) {
        uintptr_t n = stack[--sp];
        if (!rc.ok(n, 0x30) || rd<uint8_t>(n + 0x19))
            continue;
        ++visited;
        sum += rd<float>(n + 0x20);
        if (sp + 2 <= 64) {
            stack[sp++] = rd<uintptr_t>(n);
            stack[sp++] = rd<uintptr_t>(n + 0x10);
        }
    }
    perc = sum < 0.0f ? 0.0f : (sum > 1.0f ? 1.0f : sum);
    return true;
}

struct Seen {
    uintptr_t comp;
    uint32_t type;
    uint32_t bits;
};

SRWLOCK g_seen_lock = SRWLOCK_INIT;
Seen g_seen[128];
unsigned g_seen_next = 0;
std::atomic<int> g_apply_lines{0};

bool first_time(uintptr_t comp, uint8_t type, float perc)
{
    uint32_t bits = 0;
    std::memcpy(&bits, &perc, 4);
    AcquireSRWLockExclusive(&g_seen_lock);
    for (auto &s : g_seen)
        if (s.comp == comp && s.type == type) {
            bool fresh = s.bits != bits;
            s.bits = bits;
            ReleaseSRWLockExclusive(&g_seen_lock);
            return fresh;
        }
    g_seen[g_seen_next++ % 128] = {comp, type, bits};
    ReleaseSRWLockExclusive(&g_seen_lock);
    return true;
}

void on_apply_entry(safetyhook::Context &c)
{
    uint8_t type = static_cast<uint8_t>(c.rdx);
    float perc = 0.0f;
    if (!map_progress(c.rcx, type, perc))
        return;
    if (!first_time(c.rcx, type, perc) || g_apply_lines.fetch_add(1) >= 600)
        return;
    swofty::log("[rnd-tyres] upgrade apply: type %u %s perc %.4f (vehicle component 0x%llx)", (unsigned)type,
                block_name(type), perc, (unsigned long long)c.rcx);
}

// ---------------------------------------------------------------------------
// UI type switches. Each is `cmp <type>,0x12 ; ja <default>` -> jump table, and the
// default is `lea r8|r9,["INVALID UPGRADE"] ; jmp <join>`; the join formats
// lng_rnd_%s (r8) or the <dept>_<type> category frame (r9).

enum class Reg { RAX, RCX, RSI, R9, R14, R8 };

uintptr_t &reg_ref(safetyhook::Context &c, Reg r)
{
    switch (r) {
    case Reg::RAX: return c.rax;
    case Reg::RCX: return c.rcx;
    case Reg::RSI: return c.rsi;
    case Reg::R9: return c.r9;
    case Reg::R14: return c.r14;
    case Reg::R8: break;
    }
    return c.r8;
}

struct UiSite {
    uintptr_t dflt;
    uintptr_t join;
    uintptr_t head;
    Reg type_reg;
    uint8_t width;
    Reg dst;
    uint8_t head_len;
    uint8_t head_bytes[10];
    uint8_t dflt_bytes[7];
    const char *fn;
};

const UiSite kUiSites[] = {
    {0x1417f831dull, 0x1417f83d2ull, 0x1417f82fbull, Reg::R14, 1, Reg::R8, 6, {0x41, 0x80, 0xfe, 0x12, 0x77, 0x1c}, {0x4c, 0x8d, 0x05, 0x7b, 0x5c, 0x08, 0x04}, "upgrade card 0x1417f8090"},
    {0x14181465aull, 0x14181470full, 0x141814638ull, Reg::RAX, 8, Reg::R8, 6, {0x48, 0x83, 0xf8, 0x12, 0x77, 0x1c}, {0x4c, 0x8d, 0x05, 0x3e, 0x99, 0x06, 0x04}, "0x141814320"},
    {0x141816e11ull, 0x141816ec6ull, 0x141816defull, Reg::RAX, 8, Reg::R8, 6, {0x48, 0x83, 0xf8, 0x12, 0x77, 0x1c}, {0x4c, 0x8d, 0x05, 0x87, 0x71, 0x06, 0x04}, "0x1418169f0"},
    {0x1418b0b11ull, 0x1418b0bc0ull, 0x1418b0aa2ull, Reg::RAX, 8, Reg::R8, 6, {0x48, 0x83, 0xf8, 0x12, 0x77, 0x69}, {0x4c, 0x8d, 0x05, 0x87, 0xd4, 0xfc, 0x03}, "0x1418b02b0 label"},
    {0x1418b0d32ull, 0x1418b0de0ull, 0x1418b0cf3ull, Reg::RCX, 8, Reg::R9, 6, {0x48, 0x83, 0xf9, 0x12, 0x77, 0x39}, {0x4c, 0x8d, 0x0d, 0x66, 0xd2, 0xfc, 0x03}, "0x1418b02b0 icon"},
    {0x1419bf4aeull, 0x1419bf563ull, 0x1419bf48cull, Reg::RCX, 8, Reg::R9, 6, {0x48, 0x83, 0xf9, 0x12, 0x77, 0x1c}, {0x4c, 0x8d, 0x0d, 0xea, 0xea, 0xeb, 0x03}, "Team HQ R&D popup 0x1419be1e0 icon"},
    {0x141c514c6ull, 0x141c5157bull, 0x141c514a4ull, Reg::RAX, 8, Reg::R8, 6, {0x48, 0x83, 0xf8, 0x12, 0x77, 0x1c}, {0x4c, 0x8d, 0x05, 0xd2, 0xca, 0xc2, 0x03}, "0x141c4fde0"},
    {0x141c7007cull, 0x141c70131ull, 0x141c7005aull, Reg::RAX, 8, Reg::R8, 6, {0x48, 0x83, 0xf8, 0x12, 0x77, 0x1c}, {0x4c, 0x8d, 0x05, 0x1c, 0xdf, 0xc0, 0x03}, "0x141c6e1b0"},
    {0x141c76dafull, 0x141c76e64ull, 0x141c76d8dull, Reg::R9, 4, Reg::R8, 6, {0x41, 0x83, 0xf9, 0x12, 0x77, 0x1c}, {0x4c, 0x8d, 0x05, 0xe9, 0x71, 0xc0, 0x03}, "research category row 0x141c76b60 label"},
    {0x141c77df8ull, 0x141c77ea1ull, 0x141c77dc7ull, Reg::RSI, 1, Reg::R9, 6, {0x40, 0x80, 0xfe, 0x12, 0x77, 0x2b}, {0x4c, 0x8d, 0x0d, 0xa0, 0x61, 0xc0, 0x03}, "research category row 0x141c76b60 icon"},
    {0x141c80c06ull, 0x141c80cbbull, 0x141c80be4ull, Reg::RAX, 8, Reg::R8, 6, {0x48, 0x83, 0xf8, 0x12, 0x77, 0x1c}, {0x4c, 0x8d, 0x05, 0x92, 0xd3, 0xbf, 0x03}, "0x141c80600"},
    {0x141c826b6ull, 0x141c8276bull, 0x141c82695ull, Reg::RSI, 4, Reg::R8, 5, {0x83, 0xfe, 0x12, 0x77, 0x1c}, {0x4c, 0x8d, 0x05, 0xe2, 0xb8, 0xbf, 0x03}, "department category row 0x141c82470 label"},
    {0x141c836f1ull, 0x141c8379aull, 0x141c836c0ull, Reg::RSI, 1, Reg::R9, 6, {0x40, 0x80, 0xfe, 0x12, 0x77, 0x2b}, {0x4c, 0x8d, 0x0d, 0xa7, 0xa8, 0xbf, 0x03}, "department category row 0x141c82470 icon"},
    {0x141c8e93bull, 0x141c8e9f0ull, 0x141c8e919ull, Reg::RAX, 8, Reg::R8, 6, {0x48, 0x83, 0xf8, 0x12, 0x77, 0x1c}, {0x4c, 0x8d, 0x05, 0x5d, 0xf6, 0xbe, 0x03}, "develop-upgrade panel 0x141c8d540"},
    {0x141f3d185ull, 0x141f3d1d0ull, 0x141f3d0d5ull, Reg::RAX, 8, Reg::R8, 10, {0x48, 0x83, 0xf8, 0x12, 0x0f, 0x87, 0xa6, 0x00, 0x00, 0x00}, {0x4c, 0x8d, 0x05, 0x13, 0x0e, 0x94, 0x03}, "in-progress list 0x141f3bec0 label"},
    {0x141f3d960ull, 0x141f3d9b0ull, 0x141f3d8caull, Reg::RCX, 8, Reg::R9, 10, {0x48, 0x83, 0xf9, 0x12, 0x0f, 0x87, 0x8c, 0x00, 0x00, 0x00}, {0x4c, 0x8d, 0x0d, 0x38, 0x06, 0x94, 0x03}, "in-progress list 0x141f3bec0 icon"},
};
constexpr size_t kNumUi = sizeof(kUiSites) / sizeof(kUiSites[0]);

uintptr_t g_base = 0;

template <size_t I> void on_ui(safetyhook::Context &c)
{
    const UiSite &s = kUiSites[I];
    uint64_t v = reg_ref(c, s.type_reg);
    if (s.width == 1)
        v &= 0xff;
    else if (s.width == 4)
        v &= 0xffffffffull;
    const NewType *t = find_id(v);
    if (!t)
        return;
    reg_ref(c, s.dst) = reinterpret_cast<uintptr_t>(t->suffix);
    c.rip = g_base + rva(s.join);
    if (g_ui_log.fetch_add(1) < 24)
        swofty::log("[rnd-tyres] upgrade types: %s type %u -> \"%s\"", s.fn, (unsigned)t->id, t->suffix);
}

using UiFn = void (*)(safetyhook::Context &);
template <size_t... I> constexpr auto make_ui_fns(std::index_sequence<I...>)
{
    return std::array<UiFn, sizeof...(I)>{&on_ui<I>...};
}
constexpr auto kUiFns = make_ui_fns(std::make_index_sequence<kNumUi>{});

// ---------------------------------------------------------------------------
// Telemetry: camel-case type name table 0x14564a0d0 (19 entries, then string bytes).
//   0x14071e6f6 movzx eax,[rax+0x70] ; 0x14071e6fa lea rcx,[table] ; mov rdx,[rcx+rax*8] ;
//               mov rcx,rbx ; 0x14071e708 call 0x14058eed0           (research purchased)
//   0x14071e989 movzx eax,[rax+0x70] ; 0x14071e98d mov rdx,[rbp+rax*8] ;
//               0x14071e992 mov rcx,rbx ; call 0x14058eed0             (upgrade failed)
//   0x140777efb movzx eax,[rbx+0x70] ; 0x140777eff lea rdx,[table] ; mov rdx,[rdx+rax*8] ;
//               0x140777f0a call 0x140590390                         (upgrade fitted record)

const uint8_t kTeleResearch[] = {0x48, 0x8b, 0x47, 0x10, 0x0f, 0xb6, 0x40, 0x70, 0x48, 0x8d, 0x0d, 0xcf, 0xb9, 0xf2,
                                 0x04, 0x48, 0x8b, 0x14, 0xc1, 0x48, 0x89, 0xd9, 0xe8, 0xc3, 0x07, 0xe7, 0xff};
const uint8_t kTeleWriter[] = {0x48, 0x8b, 0x0e, 0x0f, 0xb6, 0x43, 0x70, 0x48, 0x8d, 0x15, 0xca, 0x21,
                               0xed, 0x04, 0x48, 0x8b, 0x14, 0xc2, 0xe8, 0x81, 0x84, 0xe1, 0xff};
const uint8_t kTeleFailed[] = {0x48, 0x8b, 0x06, 0x0f, 0xb6, 0x40, 0x70, 0x48, 0x8b, 0x54,
                               0xc5, 0x00, 0x48, 0x89, 0xd9, 0xe8, 0x36, 0x05, 0xe7, 0xff};

const char *camel_of(uint64_t t)
{
    const NewType *n = find_id(t);
    return n ? n->camel : kUnknown;
}

void tele_log(const char *what, uint64_t t)
{
    if (g_tele_log.fetch_add(1) < 8)
        swofty::log("[rnd-tyres] upgrade types: telemetry %s type %u -> \"%s\"", what, (unsigned)t, camel_of(t));
}

void on_tele_research(safetyhook::Context &c)
{
    uint64_t t = c.rax & 0xffffffffull;
    if (t < kStockTypes)
        return;
    c.rdx = reinterpret_cast<uintptr_t>(camel_of(t));
    c.rcx = c.rbx;
    c.rip = g_base + rva(0x14071e708ull);
    tele_log("research purchased 0x14071e470", t);
}

void on_tele_failed(safetyhook::Context &c)
{
    uint64_t t = c.rax & 0xffffffffull;
    if (t < kStockTypes)
        return;
    c.rdx = reinterpret_cast<uintptr_t>(camel_of(t));
    c.rip = g_base + rva(0x14071e992ull);
    tele_log("upgrade failed 0x14071e810", t);
}

void on_tele_writer(safetyhook::Context &c)
{
    uint64_t t = c.rax & 0xffffffffull;
    if (t < kStockTypes)
        return;
    c.rdx = reinterpret_cast<uintptr_t>(camel_of(t));
    c.rip = g_base + rva(0x140777f0aull);
    tele_log("upgrade record 0x140777d50", t);
}

// ---------------------------------------------------------------------------
// Weekend objectives 0x14295c0a0 (run on the Team HQ -> session flow) put the types of
// the team's Available upgrades into a std::bitset<19> that filters the upgrade-discount
// rewards; a type past 18 reaches the bitset's throw ("invalid bitset position"):
//   0x14295c6ac cmp byte [r8-8],1 ; jne 0x14295c694      (last state != Available)
//   0x14295c6b3 movzx edx,byte [rdx+0x70] ; cmp edx,0x13 ; jb 0x14295c690 (bts r15d,edx)
//   0x14295c6bc jmp 0x14295cf89                          (-> throw)
// The rewards name stock types only (camel-case resolver 0x1409ea830), so our types are
// left out of the set, as if those upgrades were not Available.

constexpr uintptr_t kVaObjWin = 0x14295c690ull;
constexpr uintptr_t kVaObjType = 0x14295c6b3ull;
constexpr uintptr_t kVaObjNext = 0x14295c694ull;

const uint8_t kObjBytes[] = {0x41, 0x0f, 0xab, 0xd7, 0x48, 0x83, 0xc1, 0x08, 0x48, 0x39, 0xc8, 0x74, 0x2f,
                             0x48, 0x8b, 0x94, 0x0c, 0xd0, 0x00, 0x00, 0x00, 0x4c, 0x8b, 0x82, 0xc8, 0x00,
                             0x00, 0x00, 0x41, 0x80, 0x78, 0xf8, 0x01, 0x75, 0xe1, 0x0f, 0xb6, 0x52, 0x70,
                             0x83, 0xfa, 0x13, 0x72, 0xd4, 0xe9, 0xc8, 0x08, 0x00, 0x00};

std::atomic<int> g_obj_log{0};

void on_objective_type(safetyhook::Context &c)
{
    const NewType *t = find_id(rd<uint8_t>(c.rdx + 0x70));
    if (!t)
        return;
    c.rip = g_base + rva(kVaObjNext);
    if (g_obj_log.fetch_add(1) < 8)
        swofty::log("[rnd-tyres] upgrade types: weekend objectives 0x14295c0a0 leaves Available type %u (%s) out "
                    "of the discount type bitset",
                    (unsigned)t->id, t->camel);
}

// ---------------------------------------------------------------------------

struct HookSpec {
    uintptr_t va;
    void (*fn)(safetyhook::Context &);
    const char *what;
};

std::vector<safetyhook::MidHook> g_hooks;
std::array<safetyhook::MidHook, 2> g_resolvers;
safetyhook::MidHook g_apply_log_hook;

bool create_into(std::vector<safetyhook::MidHook> &out, uintptr_t base, const HookSpec &h)
{
    uintptr_t at = base + rva(h.va);
    auto r = safetyhook::MidHook::create(reinterpret_cast<void *>(at), h.fn);
    if (!r) {
        swofty::log("[rnd-tyres] upgrade types: MidHook::create failed for %s @0x%llx (err %d); NOT installed (SAFE)",
                    h.what, (unsigned long long)at, (int)r.error().type);
        return false;
    }
    out.push_back(std::move(*r));
    swofty::log("[rnd-tyres] upgrade types: hooked %s @0x%llx (base+0x%llx)", h.what, (unsigned long long)at,
                (unsigned long long)rva(h.va));
    return true;
}

bool verify_all(uintptr_t base)
{
    struct Win {
        uintptr_t va;
        const uint8_t *b;
        size_t n;
        const char *what;
    };
    const Win wins[] = {
        {kVaRowsWin, kRowsBytes, sizeof kRowsBytes, "rows resolver"},
        {kVaMetaWin, kMetaBytes, sizeof kMetaBytes, "metadata resolver"},
        {0x141238f8bull, kFillEndTail, sizeof kFillEndTail, "vehicle fill loop end"},
        {0x141c7e4cfull, kPageMid, sizeof kPageMid, "department page loop"},
        {0x141c7573aull, kResearchMid, sizeof kResearchMid, "research rows loop"},
        {kVaFilterWin, kFilterBytes, sizeof kFilterBytes, "apply key filter"},
        {kVaNameWin, kNameBytes, sizeof kNameBytes, "apply block-name bound"},
        {kVaNameQ, kNameQBytes, sizeof kNameQBytes, "apply block-name default"},
        {0x14071e6f2ull, kTeleResearch, sizeof kTeleResearch, "telemetry research purchased"},
        {0x140777ef8ull, kTeleWriter, sizeof kTeleWriter, "telemetry upgrade record"},
        {0x14071e986ull, kTeleFailed, sizeof kTeleFailed, "telemetry upgrade failed"},
        {kVaObjWin, kObjBytes, sizeof kObjBytes, "weekend objectives type bitset"},
    };
    for (const auto &w : wins)
        if (!bytes_are(base + rva(w.va), w.b, w.n)) {
            swofty::log("[rnd-tyres] upgrade types: %s bytes @0x%llx differ from the disassembly; NOT installed "
                        "(SAFE)",
                        w.what, (unsigned long long)(base + rva(w.va)));
            return false;
        }
    for (const auto &s : kLeaSites)
        if (!lea_ok(base, s)) {
            swofty::log("[rnd-tyres] upgrade types: %s @0x%llx does not address the order table; NOT installed (SAFE)",
                        s.what, (unsigned long long)(base + rva(s.va)));
            return false;
        }
    for (const auto &s : kUiSites)
        if (!bytes_are(base + rva(s.head), s.head_bytes, s.head_len) ||
            !bytes_are(base + rva(s.dflt), s.dflt_bytes, sizeof s.dflt_bytes)) {
            swofty::log("[rnd-tyres] upgrade types: UI type switch %s @0x%llx differs from the disassembly; NOT "
                        "installed (SAFE)",
                        s.fn, (unsigned long long)(base + rva(s.dflt)));
            return false;
        }
    return true;
}

void install_apply_log(uintptr_t base)
{
    if (!log_upgrade_apply.get()) {
        swofty::log("[rnd-tyres] upgrade apply log: log_upgrade_apply off (SAFE)");
        return;
    }
    if (!bytes_are(base + rva(kVaApply), kApplyBytes, sizeof kApplyBytes) ||
        !bytes_are(base + rva(kVaNameWin), kNameBytes, sizeof kNameBytes)) {
        swofty::log("[rnd-tyres] upgrade apply log: 0x14fa63050 bytes differ from the disassembly; NOT installed "
                    "(SAFE)");
        return;
    }
    g_block_names = base + rva(kVaBlockNames);
    auto r = safetyhook::MidHook::create(reinterpret_cast<void *>(base + rva(kVaApply)), &on_apply_entry);
    if (!r) {
        swofty::log("[rnd-tyres] upgrade apply log: MidHook::create failed @0x%llx (err %d); NOT installed (SAFE)",
                    (unsigned long long)(base + rva(kVaApply)), (int)r.error().type);
        return;
    }
    g_apply_log_hook = std::move(*r);
    swofty::log("[rnd-tyres] upgrade apply log INSTALLED: read-only hook @0x%llx logs each type's perc once per "
                "vehicle component and value",
                (unsigned long long)(base + rva(kVaApply)));
}

} // namespace

namespace rnd_upgrade_types {

bool live()
{
    return g_live.load();
}

int effect_dept(uint8_t type)
{
    const NewType *t = find_id(type);
    return (t && g_live.load()) ? t->effect_dept : -1;
}

void install(uintptr_t base)
{
    if (!swofty::api || !base) {
        swofty::log("[rnd-tyres] upgrade types: no API/module base; NOT installed (SAFE)");
        return;
    }
    g_base = base;
    install_apply_log(base);

    bool on = tyres_upgrade_types.get();
    swofty::log("[rnd-tyres] upgrade types: tyres_upgrade_types=%s, %u new types (Power Unit Cooling %s)",
                on ? "on" : "off", (unsigned)kNumTypes, kShipPuCooling ? "shipped" : "not shipped");
    if (!on) {
        swofty::log("[rnd-tyres] upgrade types: tyres_upgrade_types off; the 19 stock types only, and a Tyres row "
                    "naming a new type resolves to Engine Power (SAFE for stock rows)");
        return;
    }
    uintptr_t hit = 0;
    int matches = swofty::api->scan(".xtls", kRowsSig, &hit);
    if (matches != 1 || hit != base + rva(kVaRowsScan)) {
        swofty::log("[rnd-tyres] upgrade types: resolver signature matched %d time(s) (hit 0x%llx); NOT installed "
                    "(SAFE)",
                    matches, (unsigned long long)hit);
        return;
    }
    if (!verify_all(base))
        return;

    g_order_len = kStockTypes + kNumTypes;
    auto mem = safetyhook::Allocator::global()->allocate_near(
        {reinterpret_cast<uint8_t *>(base + rva(kVaOrder))}, 64);
    if (!mem) {
        swofty::log("[rnd-tyres] upgrade types: no memory within rel32 reach of the image; NOT installed (SAFE)");
        return;
    }
    g_order_mem = std::move(*mem);
    g_order = g_order_mem.address();
    std::memcpy(g_order_mem.data(), reinterpret_cast<const void *>(base + rva(kVaOrder)), kStockTypes);
    for (size_t i = 0; i < kNumTypes; ++i)
        g_order_mem.data()[kStockTypes + i] = kAllTypes[i].id;
    g_filter_call = base + rva(kVaFilterCall);
    g_name_join = base + rva(kVaNameJoin);

    std::vector<HookSpec> specs = {
        {0x14071e6faull, &on_tele_research, "telemetry research purchased camel type name (0x14071e470)"},
        {0x14071e98dull, &on_tele_failed, "telemetry upgrade failed camel type name (0x14071e810)"},
        {0x140777effull, &on_tele_writer, "telemetry upgrade record camel type name (0x140777d50)"},
        {kVaObjType, &on_objective_type, "weekend objectives 0x14295c0a0 Available type bitset<19>"},
        {kVaFilter, &on_apply_filter, "apply 0x14fa4deb0 key filter 11..17 (+ new types)"},
        {kVaNameQ, &on_block_name, "apply 0x14fa63050 block name past 0x11"},
    };
    for (size_t i = 0; i < kNumUi; ++i)
        specs.push_back({kUiSites[i].dflt, kUiFns[i], kUiSites[i].fn});
    std::vector<safetyhook::MidHook> hooks;
    hooks.reserve(specs.size());
    for (const auto &h : specs)
        if (!create_into(hooks, base, h)) {
            swofty::log("[rnd-tyres] upgrade types: consumer hooks rolled back; no type id 19+ is minted (SAFE)");
            return;
        }
    if (!patch_leas(base))
        return;

    std::vector<safetyhook::MidHook> res;
    if (!create_into(res, base, {kVaRowsJoin, &on_rows_join, "rows resolver join (Career_Load_UpgradeRows)"}) ||
        !create_into(res, base, {kVaMetaJoin, &on_meta_join, "upgrade_type_metadata resolver join (0x14353c4f0)"})) {
        restore_leas(base, kNumLeas);
        g_leas_patched = false;
        swofty::log("[rnd-tyres] upgrade types: resolvers not hooked; loops restored, no type id 19+ is minted (SAFE)");
        return;
    }
    g_hooks = std::move(hooks);
    g_resolvers[0] = std::move(res[0]);
    g_resolvers[1] = std::move(res[1]);
    g_live.store(true);
    char ids[160];
    size_t o = 0;
    for (size_t i = 0; i < kNumTypes && o + 48 < sizeof ids; ++i)
        o += static_cast<size_t>(std::snprintf(ids + o, sizeof ids - o, "%s%u %s", i ? ", " : "",
                                               (unsigned)kAllTypes[i].id, kAllTypes[i].block));
    swofty::log("[rnd-tyres] upgrade types INSTALLED: %s; %zu consumer hooks, 6 order-table leas -> 0x%llx (%zu "
                "types), resolvers last",
                ids, g_hooks.size(), (unsigned long long)g_order, g_order_len);
}

} // namespace rnd_upgrade_types
