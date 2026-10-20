// SPDX-License-Identifier: AGPL-3.0-or-later
//
// R&D: Tyres & Thermal Management - research capacity for department code 4.
//
// The develop-upgrade panel update (Career_Update_RnDPurchasePanel) computes the
// research-limit flag (panel+0x6d2, read by the option rows to block the buy):
//
//   list   = Career_Get_TeamModifiers(career+0xc90, team)
//   slots  = (int) sum of entry.value over entries with
//            kind == 0, dept == panel dept, active != 0
//   in-use = enabled upgrades of the panel dept whose last
//            state is InProgress (2) for this team
//   limit  = gate([rsp+0x57]) && slots <= in-use   (site_Career_RnDPanelResearchLimit)
//
// A team modifier list is a vector of 8-byte entries {u8 kind, u8 dept, u8 source,
// u8 active, f32 value}. Every entry is created by the per-team init functions,
// which are fully unrolled over departments 0..3 and never emit department 4:
// Career_Init_TeamModifiers (232 Career_Add_TeamModifier calls: 23 kinds x depts
// 0..3, then a {0,1,2,3} loop that fills the base values) and
// Career_Init_SharedTeamModifiers (80 calls, the other list layout).
// The DB-driven listeners and the save loader only UPDATE or RESTORE existing
// entries. The v0.6.1 department row (department = Fake) gives the kind maps a
// key 4, but no list ever holds a dept-4 entry, so slots = 0 and 0 <= 0 sets the
// flag.
//
// Fix: when the panel (or the R&D department page) reads the team list for
// department 4, mirror every Durability (dept 2) entry as a dept-4 entry: the
// missing ones are added with the game's own Career_Add_TeamModifier
// (add-if-absent, then stable passes by kind and by department, list allocator;
// the mirror re-reads the list after each add), present ones get
// Durability's value. The stock departments' entries are never touched, and only
// department 4 reads the new ones. Once added the entries live in the game's list,
// so the listener callbacks keep the attribute-driven ones current and the stock
// save writer persists them.
//
// Every pointer is region-checked and everything acts ONLY for department 4. Every
// handler is a no-op until install() runs.

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <atomic>
#include <cstdint>
#include <cstring>

#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include "rnd_capacity.hpp"

using namespace swofty::game;

namespace {

constexpr uint8_t kDeptDurability = 2;
constexpr uint8_t kDeptTyres = 4;
constexpr uint8_t kKindResearchSlots = 0;
constexpr uint8_t kStateInProgress = 2;

constexpr size_t kMaxEntries = 4096;
constexpr size_t kMaxMirror = 512;
constexpr size_t kInUseCap = (0x27a0 - 0x1ba0) / 8;

constexpr uintptr_t kPanelCareer = 0x680;
constexpr uintptr_t kPanelTeam   = 0x6cc;
constexpr uintptr_t kPanelDept   = 0x6e8;
constexpr uintptr_t kCardCareer  = 0x678;
constexpr uintptr_t kCardTeam    = 0x6bc;

constexpr uintptr_t kInstSize   = 0xe0;
constexpr uintptr_t kInstDept   = 0x72;
constexpr uintptr_t kInstStates = 0xc0;
constexpr uintptr_t kStateSize  = 0x10;
constexpr uintptr_t kStateCode  = 0x8;

std::atomic<bool> g_live{false};
std::atomic<uintptr_t> g_last_panel{0};
std::atomic<int> g_last_tyres_slots{-1};
std::atomic<int> g_last_dur_slots{-1};
std::atomic<int> g_sync_log{0};
std::atomic<int> g_page_log{0};
std::atomic<int> g_check_log{0};
std::atomic<uint64_t> g_check_last{~0ull};

bool region_readable(const void *p, size_t n)
{
    MEMORY_BASIC_INFORMATION mbi{};
    if (VirtualQuery(p, &mbi, sizeof mbi) != sizeof mbi)
        return false;
    if (mbi.State != MEM_COMMIT)
        return false;
    if ((mbi.Protect & PAGE_GUARD) || (mbi.Protect & PAGE_NOACCESS))
        return false;
    uintptr_t end = reinterpret_cast<uintptr_t>(mbi.BaseAddress) + mbi.RegionSize;
    return reinterpret_cast<uintptr_t>(p) + n <= end;
}

struct RegionCache {
    uintptr_t lo = 0;
    uintptr_t hi = 0;

    bool ok(uintptr_t p, size_t n)
    {
        if (p >= lo && p + n <= hi && p + n >= p)
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

bool region_writable(const void *p, size_t n)
{
    MEMORY_BASIC_INFORMATION mbi{};
    if (VirtualQuery(p, &mbi, sizeof mbi) != sizeof mbi)
        return false;
    if (mbi.State != MEM_COMMIT || (mbi.Protect & PAGE_GUARD))
        return false;
    DWORD w = PAGE_READWRITE | PAGE_WRITECOPY | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY;
    if (!(mbi.Protect & w))
        return false;
    uintptr_t end = reinterpret_cast<uintptr_t>(mbi.BaseAddress) + mbi.RegionSize;
    return reinterpret_cast<uintptr_t>(p) + n <= end;
}

template <class T> T rd(uintptr_t a)
{
    T v;
    std::memcpy(&v, reinterpret_cast<const void *>(a), sizeof v);
    return v;
}

// ---------------------------------------------------------------------------
// Team modifier list: {allocator*, begin, end, capacity}, 8-byte entries.

struct ListView {
    uintptr_t begin = 0;
    size_t count = 0;
};

bool list_view(uintptr_t list, ListView &v, RegionCache &rc)
{
    if (!list || !rc.ok(list, 0x20))
        return false;
    uintptr_t b = rd<uintptr_t>(list + 0x08);
    uintptr_t e = rd<uintptr_t>(list + 0x10);
    if (!b || e < b || ((e - b) & 7) || (e - b) / 8 > kMaxEntries)
        return false;
    if (e != b && !rc.ok(b, e - b))
        return false;
    v.begin = b;
    v.count = (e - b) / 8;
    return true;
}

uint8_t key_dept(uint32_t key)
{
    return static_cast<uint8_t>(key >> 8);
}

uintptr_t find_key(const ListView &v, uint32_t key)
{
    for (size_t i = 0; i < v.count; ++i)
        if (rd<uint32_t>(v.begin + i * 8) == key)
            return v.begin + i * 8;
    return 0;
}

int slots_of(uintptr_t list, uint8_t dept)
{
    RegionCache rc;
    ListView v;
    if (!list_view(list, v, rc))
        return -1;
    float sum = 0.0f;
    for (size_t i = 0; i < v.count; ++i) {
        uint32_t key = rd<uint32_t>(v.begin + i * 8);
        if ((key & 0xff) == kKindResearchSlots && key_dept(key) == dept && (key >> 24) != 0)
            sum += rd<float>(v.begin + i * 8 + 4);
    }
    return static_cast<int>(sum);
}

bool allocator_ok(uintptr_t list, RegionCache &rc)
{
    uintptr_t alloc = rd<uintptr_t>(list);
    if (!alloc || !rc.ok(alloc, 8))
        return false;
    uintptr_t vt = rd<uintptr_t>(alloc);
    return vt && region_readable(reinterpret_cast<void *>(vt), 0x40);
}

struct Mirror {
    bool ok = false;
    int src = 0;
    int inserted = 0;
    int updated = 0;
};

Mirror mirror_durability(uintptr_t list)
{
    Mirror m;
    RegionCache rc;
    ListView v;
    if (!list_view(list, v, rc) || !allocator_ok(list, rc))
        return m;
    uint32_t keys[kMaxMirror];
    uint32_t vals[kMaxMirror];
    size_t n = 0;
    for (size_t i = 0; i < v.count && n < kMaxMirror; ++i) {
        uint32_t key = rd<uint32_t>(v.begin + i * 8);
        if (key_dept(key) != kDeptDurability)
            continue;
        keys[n] = key;
        vals[n] = rd<uint32_t>(v.begin + i * 8 + 4);
        ++n;
    }
    m.src = static_cast<int>(n);
    for (size_t i = 0; i < n; ++i) {
        uint32_t key4 = (keys[i] & 0xffff00ffu) | (static_cast<uint32_t>(kDeptTyres) << 8);
        uintptr_t at = find_key(v, key4);
        if (at) {
            if (rd<uint32_t>(at + 4) != vals[i]) {
                if (!region_writable(reinterpret_cast<void *>(at + 4), 4))
                    return m;
                std::memcpy(reinterpret_cast<void *>(at + 4), &vals[i], 4);
                ++m.updated;
            }
            continue;
        }
        float f;
        std::memcpy(&f, &vals[i], 4);
        Career_Add_TeamModifier(reinterpret_cast<void *>(list), key4, f);
        RegionCache fresh;
        if (!list_view(list, v, fresh) || !find_key(v, key4))
            return m;
        ++m.inserted;
    }
    m.ok = n != 0;
    return m;
}

int durability_in_use(uintptr_t rsp)
{
    RegionCache rc;
    if (!rc.ok(rsp + 0x1ba0, 0x27a8 - 0x1ba0))
        return -1;
    uint64_t n = rd<uint64_t>(rsp + 0x27a0);
    if (n > kInUseCap)
        return -1;
    int busy = 0;
    for (uint64_t i = 0; i < n; ++i) {
        uintptr_t inst = rd<uintptr_t>(rsp + 0x1ba0 + i * 8);
        if (!inst || !rc.ok(inst, kInstSize))
            continue;
        if (rd<uint8_t>(inst + kInstDept) != kDeptDurability)
            continue;
        uintptr_t sb = rd<uintptr_t>(inst + kInstStates);
        uintptr_t se = rd<uintptr_t>(inst + kInstStates + 8);
        if (!sb || se <= sb || ((se - sb) % kStateSize))
            continue;
        if (!rc.ok(se - kStateSize, kStateSize))
            continue;
        if (rd<uint8_t>(se - kStateSize + kStateCode) == kStateInProgress)
            ++busy;
    }
    return busy;
}

} // namespace

// ---------------------------------------------------------------------------
// Develop-upgrade panel update Career_Update_RnDPurchasePanel.
//   site_Career_RnDPanelModifierList   after Career_Get_TeamModifiers returned
//                                      non-null; rax = list, bl = panel dept
//                                      ([rsi+0x6e8]), rsi = panel. The slot sum
//                                      loop reads the list right after it, so a
//                                      reallocation by Career_Add_TeamModifier is
//                                      invisible to the game.
//   site_Career_RnDPanelResearchLimit  `cmp ebp,eax`: ebp = research slots, eax =
//                                      InProgress count of the panel dept, r12b =
//                                      the stock gate; the flag stored to
//                                      [rsi+0x6d2] is gate && slots <= in-use. The
//                                      owner-filtered enabled upgrades are at
//                                      [rsp+0x1ba0], count [rsp+0x27a0].

SWOFTY_SITE(site_Career_RnDPanelModifierList, swofty::Context &c)
{
    if (!g_live.load() || static_cast<uint8_t>(c.rbx) != kDeptTyres)
        return;
    uintptr_t panel = c.rsi;
    uintptr_t list = c.rax;
    if (!region_readable(reinterpret_cast<void *>(panel + kPanelCareer), kPanelDept + 1 - kPanelCareer))
        return;
    uint32_t team = rd<uint32_t>(panel + kPanelTeam);
    Mirror m = mirror_durability(list);
    g_last_panel.store(panel);
    g_last_tyres_slots.store(slots_of(list, kDeptTyres));
    g_last_dur_slots.store(slots_of(list, kDeptDurability));
    if (!m.ok) {
        if (g_sync_log.fetch_add(1) < 8)
            swofty::log("[rnd-tyres] capacity: panel team 0x%x list 0x%llx has no Durability entries to mirror "
                        "(src %d, inserted %d); dept 4 left as is (SAFE)",
                        team, (unsigned long long)list, m.src, m.inserted);
        return;
    }
    if ((m.inserted || m.updated) && g_sync_log.fetch_add(1) < 32)
        swofty::log("[rnd-tyres] capacity: panel team 0x%x list 0x%llx: dept 4 mirrored from Durability (src %d, "
                    "inserted %d, updated %d) -> research slots dept4=%d durability=%d",
                    team, (unsigned long long)list, m.src, m.inserted, m.updated, g_last_tyres_slots.load(),
                    g_last_dur_slots.load());
}

SWOFTY_SITE(site_Career_RnDPanelResearchLimit, swofty::Context &c)
{
    if (!g_live.load())
        return;
    uintptr_t panel = c.rsi;
    if (!region_readable(reinterpret_cast<void *>(panel + kPanelCareer), kPanelDept + 1 - kPanelCareer))
        return;
    if (rd<uint8_t>(panel + kPanelDept) != kDeptTyres || g_check_log.load() >= 48)
        return;
    int slots = static_cast<int>(static_cast<uint32_t>(c.rbp));
    int busy = static_cast<int>(static_cast<uint32_t>(c.rax));
    unsigned gate = static_cast<unsigned>(c.r12 & 0xff);
    unsigned limit = (gate & (slots <= busy ? 1u : 0u)) & 1u;
    uint32_t team = rd<uint32_t>(panel + kPanelTeam);
    bool same = g_last_panel.load() == panel;
    int dur_slots = same ? g_last_dur_slots.load() : -1;
    int dur_busy = durability_in_use(c.rsp);
    uint64_t sig = (static_cast<uint64_t>(static_cast<uint8_t>(slots)) << 0) |
                   (static_cast<uint64_t>(static_cast<uint8_t>(busy)) << 8) |
                   (static_cast<uint64_t>(static_cast<uint8_t>(dur_slots)) << 16) |
                   (static_cast<uint64_t>(static_cast<uint8_t>(dur_busy)) << 24) |
                   (static_cast<uint64_t>(gate & 1u) << 32) | (static_cast<uint64_t>(team) << 33);
    if (g_check_last.exchange(sig) == sig)
        return;
    if (g_check_log.fetch_add(1) >= 48)
        return;
    swofty::log("[rnd-tyres] capacity check: team 0x%x dept 4 slots=%d in-progress=%d gate=%u -> research-limit=%u "
                "| Durability slots=%d in-progress=%d",
                team, slots, busy, gate, limit, dur_slots, dur_busy);
}

// ---------------------------------------------------------------------------
// R&D department page Career_Build_RnDDeptPage (the Tyres tab shows its own slot
// count).
//   site_Career_RnDPageModifierList  after Career_Get_TeamModifiers returned
//                                    non-null; rax = list, bl = card dept
//                                    ([rsi+0x6d8]), rsi = the department card.

SWOFTY_SITE(site_Career_RnDPageModifierList, swofty::Context &c)
{
    if (!g_live.load() || static_cast<uint8_t>(c.rbx) != kDeptTyres)
        return;
    uintptr_t card = c.rsi;
    if (!region_readable(reinterpret_cast<void *>(card + kCardCareer), kCardTeam + 4 - kCardCareer))
        return;
    uint32_t team = rd<uint32_t>(card + kCardTeam);
    Mirror m = mirror_durability(c.rax);
    if (!m.ok) {
        if (g_page_log.fetch_add(1) < 8)
            swofty::log("[rnd-tyres] capacity: Tyres page team 0x%x list 0x%llx has no Durability entries to mirror; "
                        "dept 4 left as is (SAFE)",
                        team, (unsigned long long)c.rax);
        return;
    }
    if ((m.inserted || m.updated) && g_page_log.fetch_add(1) < 32)
        swofty::log("[rnd-tyres] capacity: Tyres page team 0x%x list 0x%llx: dept 4 mirrored from Durability (src "
                    "%d, inserted %d, updated %d) -> research slots dept4=%d durability=%d",
                    team, (unsigned long long)c.rax, m.src, m.inserted, m.updated, slots_of(c.rax, kDeptTyres),
                    slots_of(c.rax, kDeptDurability));
}

namespace rnd_capacity {

void install(uintptr_t base)
{
    auto hooked = [base](const char *piece, uintptr_t at, const char *what) {
        swofty::log("[rnd-tyres] capacity %s: hooked %s @0x%llx (base+0x%llx)", piece, what, (unsigned long long)at,
                    (unsigned long long)(at - base));
    };
    hooked("panel", site_Career_RnDPanelModifierList.address(), "panel modifier list (rax)");
    hooked("panel", site_Career_RnDPanelResearchLimit.address(), "panel research-limit check (ebp/eax)");
    g_live.store(true);
    swofty::log("[rnd-tyres] capacity INSTALLED: dept 4 mirrors Durability's team modifiers (research slots, "
                "scales, rush) when the develop-upgrade panel reads them; TeamModifiers_Add @0x%llx",
                (unsigned long long)Career_Add_TeamModifier.address());
    hooked("page", site_Career_RnDPageModifierList.address(), "R&D page modifier list (rax)");
}

} // namespace rnd_capacity
