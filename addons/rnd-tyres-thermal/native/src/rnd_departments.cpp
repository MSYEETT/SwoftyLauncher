// SPDX-License-Identifier: AGPL-3.0-or-later
//
// R&D: Tyres & Thermal Management - engine plugin for F1 25.
//
// Pairs with the database rows that add a fifth career R&D department.
//
// Hook A - team-department enum store (site_Career_TeamDeptEnumStore). The
// "department" enum resolver already knows the token "Tyres" (value 4) as an
// unused spare; when the token being resolved is "Tyres" the stored enum byte is
// forced to 4. Every resolution is logged.
//
// Hook B - department-table row count (site_Gdb_DeptCountStore, the convergence
// store of Gdb_Init_TableCursor). Only when the cursor's row word is
// "UpgradeDepartmentEntry" and the count is the stock 4, the count becomes 5 so
// the fifth row is read by every consumer.
//
// v0.6.0 - Tyres upgrade instances: the instance init and every state mutator
// special-case department 4; only our instances take the normal path.
// v0.6.1 - the Develop Upgrade screen: the Back route, card art, the vehicle
// performance comparison and a temporary test price.
// See native/HOOKS.md for every site.
//
// Every target is a KB binding resolved at load; one unresolved binding means
// the addon registers nothing. Settings gate behaviour inside the handlers.

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <array>
#include <atomic>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <utility>

#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include "rnd_capacity.hpp"
#include "rnd_dept_tags.hpp"
#include "rnd_fifth_pie.hpp"
#include "rnd_tyres_icon.hpp"
#include "rnd_upgrade_types.hpp"

using namespace swofty::game;

namespace {

constexpr uint64_t ci_fnv1a64(const char *s)
{
    uint64_t h = 0xcbf29ce484222325ull;
    for (; *s; ++s) {
        uint8_t c = static_cast<uint8_t>(*s);
        if (c >= 'A' && c <= 'Z')
            c |= 0x20;
        h = (h ^ c) * 0x100000001b3ull;
    }
    return h;
}

constexpr uint64_t kTyres = ci_fnv1a64("Tyres");
constexpr uint8_t kTyresValue = 4;

static_assert(kTyres == 0xac9965efa31b61d0ull, "Tyres token hash drifted");
static_assert(ci_fnv1a64("Powertrain") == 0xeacc53611edf399cull, "enum hashing drifted");
static_assert(ci_fnv1a64("Chassis") == 0x184f58717a1a6a3full, "enum hashing drifted");
static_assert(ci_fnv1a64("Aero") == 0xd0fae783d5a9fb7cull, "enum hashing drifted");

const char *token_name(uint64_t h)
{
    switch (h) {
    case 0xeacc53611edf399cull: return "Powertrain";
    case 0x184f58717a1a6a3full: return "Chassis";
    case 0xd0fae783d5a9fb7cull: return "Aero";
    case 0xac9965efa31b61d0ull: return "Tyres";
    default: return "<other>";
    }
}

std::atomic<int> g_log_count{0};

// rax = ci-FNV-1a-64 of the resolved "department" token, cl = the enum the game resolved.
SWOFTY_SITE(site_Career_TeamDeptEnumStore, swofty::Context &c)
{
    uint64_t h = c.rax;
    unsigned resolved = static_cast<unsigned>(c.rcx & 0xffu);

    if (h == kTyres) {
        c.rcx = (c.rcx & ~0xffull) | kTyresValue;
        swofty::log("[rnd-tyres] dept store: token=Tyres(0x%016llx) game-resolved=%u -> forced=%u",
                    (unsigned long long)h, resolved, (unsigned)kTyresValue);
        return;
    }

    int n = g_log_count.fetch_add(1);
    if (n < 64)
        swofty::log("[rnd-tyres] dept store: token=%s(0x%016llx) value=%u", token_name(h),
                    (unsigned long long)h, resolved);
    else if (n == 64)
        swofty::log("[rnd-tyres] further non-Tyres dept stores suppressed (>64)");
}

constexpr char kDeptTable[] = "UpgradeDepartmentEntry";
constexpr uint32_t kStockDeptCount = 4;
constexpr uint32_t kNewDeptCount = 5;

bool cursor_table_is(uintptr_t cursor, const char *lit)
{
    const volatile char *p = reinterpret_cast<const volatile char *>(cursor + 0x100);
    for (size_t i = 0; i < 48; ++i) {
        char a = static_cast<char>(p[i]);
        char b = lit[i];
        if (a != b)
            return false;
        if (b == '\0')
            return true;
    }
    return false;
}

std::atomic<int> g_count_log{0};

// rsi = GdbTableCursor; ecx = row count just resolved from the instancecount key.
SWOFTY_SITE(site_Gdb_DeptCountStore, swofty::Context &c)
{
    if (!cursor_table_is(c.rsi, kDeptTable))
        return;
    uint32_t cnt = static_cast<uint32_t>(c.rcx & 0xffffffffu);
    if (cnt == kStockDeptCount) {
        c.rcx = (c.rcx & ~0xffffffffull) | kNewDeptCount;
        swofty::log("[rnd-tyres] dept-count: UpgradeDepartmentEntry row_count %u -> forced %u (cursor=0x%llx)", cnt,
                    kNewDeptCount, (unsigned long long)c.rsi);
    } else {
        int n = g_count_log.fetch_add(1);
        if (n < 16)
            swofty::log("[rnd-tyres] dept-count: UpgradeDepartmentEntry row_count=%u (not the stock 4; left unchanged)",
                        cnt);
    }
}

// ---------------------------------------------------------------------------
// R&D > Performance UI groundwork: k_Career_RnDDeptColourTable [4] (the white
// spare the fifth card reads) becomes the Tyres orange, and the box-grid and pie
// builders are instrumented at their entries (site_Career_RnDDeptBoxGridEntry,
// site_Career_RnDPotentialPieEntry). See native/HOOKS.md.

constexpr uint32_t kColAero    = 0xffeead00;
constexpr uint32_t kColChassis = 0xff3ec58d;
constexpr uint32_t kColDura    = 0xff16deff;
constexpr uint32_t kColPower   = 0xff7a2aed;
constexpr uint32_t kColSpare   = 0xffffffff;
constexpr uint32_t kColTyres   = 0xff0078ff;

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

uint32_t sel_index()
{
    return *g_Career_RnDOverviewSelectedDept;
}

bool write_colour4(uint32_t *tbl)
{
    DWORD old = 0;
    if (!VirtualProtect(&tbl[4], sizeof(uint32_t), PAGE_READWRITE, &old))
        return false;
    *reinterpret_cast<volatile uint32_t *>(&tbl[4]) = kColTyres;
    DWORD tmp = 0;
    VirtualProtect(&tbl[4], sizeof(uint32_t), old, &tmp);
    return true;
}

void ui_colour_init()
{
    uint32_t *tbl = *k_Career_RnDDeptColourTable;
    uint32_t c[6];
    for (int i = 0; i < 6; ++i)
        c[i] = tbl[i];
    swofty::log("[rnd-tyres] colour table: [0]=%08x [1]=%08x [2]=%08x [3]=%08x [4]=%08x [5]=%08x", c[0], c[1], c[2],
                c[3], c[4], c[5]);
    if (c[4] == kColTyres) {
        swofty::log("[rnd-tyres] colour[4] already = Tyres 0x%08x", kColTyres);
        return;
    }
    bool layout_ok = c[0] == kColAero && c[1] == kColChassis && c[2] == kColDura && c[3] == kColPower
                     && c[4] == kColSpare && c[5] == kColSpare;
    if (!layout_ok) {
        swofty::log("[rnd-tyres] colour table layout differs from the decompile; colour[4] left stock (SAFE)");
        return;
    }
    if (!write_colour4(tbl)) {
        swofty::log("[rnd-tyres] VirtualProtect failed for colour[4] @0x%llx; NOT writing (SAFE)",
                    (unsigned long long)(uintptr_t)&tbl[4]);
        return;
    }
    swofty::log("[rnd-tyres] colour[4] white spare -> Tyres 0x%08x (@0x%llx); a future 5th card uses this colour",
                kColTyres, (unsigned long long)(uintptr_t)&tbl[4]);
}

std::atomic<int> g_box_seen{0};
std::atomic<int> g_pie_seen{0};

SWOFTY_SITE(site_Career_RnDDeptBoxGridEntry, swofty::Context &c)
{
    if (g_box_seen.fetch_add(1) != 0)
        return;
    uintptr_t self = c.rcx;
    unsigned long long begin = 0, end = 0, cap = 0;
    if (region_readable(reinterpret_cast<void *>(self + 0x280), 0x18)) {
        begin = *reinterpret_cast<uint64_t *>(self + 0x280);
        end = *reinterpret_cast<uint64_t *>(self + 0x288);
        cap = *reinterpret_cast<uint64_t *>(self + 0x290);
    }
    swofty::log("[rnd-tyres] box-grid builder reached: this=0x%llx pre-build card vec{begin=0x%llx end=0x%llx cap=%llu}"
                " selection=%u (4 cards hardcoded; 5th box deferred)",
                (unsigned long long)self, begin, end, cap, sel_index());
}

SWOFTY_SITE(site_Career_RnDPotentialPieEntry, swofty::Context &c)
{
    if (g_pie_seen.fetch_add(1) != 0)
        return;
    swofty::log("[rnd-tyres] pie builder reached: this=0x%llx selection=%u "
                "(sector buffer 0x900 = 4*0x220 hardcoded; 5th slice deferred)",
                (unsigned long long)c.rcx, sel_index());
}

// ---------------------------------------------------------------------------
// R&D > Performance: the LIVE fifth department box (Tyres & Thermal Management).
//
// Career_Build_RnDDeptBoxGrid builds four department cards plus two panels into a
// pointer vector at builder+0x280..+0x290 and attaches every slot to the container.
// At site_Career_RnDDeptBoxGridInject (rsi = builder, rdi = icon-prefix object,
// all four cards stored) the vector grows to seven and a real Tyres card is built
// with Career_Construct_RnDDeptCard: name "lng_rnd_dept_tyres", a borrowed icon
// frame, enum 4 (colour [4]). The stock attach loop binds it. The selection clamp
// stays stock, so the fifth box only renders.

SWOFTY_SETTING(bool, render_fifth_box, true);
std::atomic<bool> g_fifth_enabled{true};

SWOFTY_ON_CHANGE(render_fifth_box, bool on)
{
    g_fifth_enabled.store(on);
    swofty::log("[rnd-tyres] render_fifth_box toggled -> %s", on ? "on" : "off");
}

constexpr uint8_t kTyresDeptEnum = 4;
constexpr size_t  kDeptCardSize  = 0x950;
constexpr size_t  kNeStrChars    = 9;
constexpr size_t  kNeStrRefcount = 4;

using VtAllocFn = void *(*)(void *allocator, size_t bytes, size_t align);
using VtFreeFn  = void (*)(void *allocator, void *ptr);

bool region_writable(const void *p, size_t n)
{
    MEMORY_BASIC_INFORMATION mbi{};
    if (VirtualQuery(p, &mbi, sizeof mbi) != sizeof mbi)
        return false;
    if (mbi.State != MEM_COMMIT)
        return false;
    const DWORD writable = PAGE_READWRITE | PAGE_WRITECOPY | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY;
    if ((mbi.Protect & PAGE_GUARD) || (mbi.Protect & PAGE_NOACCESS) || !(mbi.Protect & writable))
        return false;
    uintptr_t endp = reinterpret_cast<uintptr_t>(mbi.BaseAddress) + mbi.RegionSize;
    return reinterpret_cast<uintptr_t>(p) + n <= endp;
}

void *vt_alloc(void *allocator, size_t bytes, size_t align)
{
    if (!region_readable(allocator, 8))
        return nullptr;
    uintptr_t vt = *reinterpret_cast<uintptr_t *>(allocator);
    if (!region_readable(reinterpret_cast<void *>(vt + 0x18), 8))
        return nullptr;
    auto fn = *reinterpret_cast<VtAllocFn *>(vt + 0x18);
    return fn(allocator, bytes, align);
}

void vt_free(void *allocator, void *ptr)
{
    if (!region_readable(allocator, 8))
        return;
    uintptr_t vt = *reinterpret_cast<uintptr_t *>(allocator);
    if (!region_readable(reinterpret_cast<void *>(vt + 0x38), 8))
        return;
    auto fn = *reinterpret_cast<VtFreeFn *>(vt + 0x38);
    fn(allocator, ptr);
}

// Drop our reference on a temporary NeStr exactly as the game does: atomic
// refcount--, and if it hit zero free it through g_Ne_DefaultAllocator (vt+0x38).
void release_nestr(void *s)
{
    if (!s || !region_writable(reinterpret_cast<uint8_t *>(s) + kNeStrRefcount, sizeof(uint16_t)))
        return;
    auto *rc = reinterpret_cast<volatile uint16_t *>(reinterpret_cast<uint8_t *>(s) + kNeStrRefcount);
    uint16_t after = __atomic_sub_fetch(rc, 1, __ATOMIC_SEQ_CST);
    if (after != 0)
        return;
    uint16_t expected = 0;
    if (!__atomic_compare_exchange_n(rc, &expected, static_cast<uint16_t>(0xffff), false, __ATOMIC_SEQ_CST,
                                     __ATOMIC_SEQ_CST))
        return;
    void *alloc = *g_Ne_DefaultAllocator;
    if (alloc)
        vt_free(alloc, s);
}

std::atomic<int> g_fifth_log{0};

SWOFTY_SITE(site_Career_RnDDeptBoxGridInject, swofty::Context &c)
{
    if (!g_fifth_enabled.load())
        return;
    bool first = g_fifth_log.fetch_add(1) == 0;
    uintptr_t self = c.rsi;
    uintptr_t lvar8 = c.rdi;

    auto bail = [&](const char *why) {
        if (first)
            swofty::log("[rnd-tyres] 5th box aborted: %s (self=0x%llx)", why, (unsigned long long)self);
    };

    if (!region_readable(reinterpret_cast<void *>(self + 0x278), 0x28)) { bail("builder vector fields unreadable"); return; }

    void *vec_alloc  = *reinterpret_cast<void **>(self + 0x278);
    uintptr_t begin  = *reinterpret_cast<uintptr_t *>(self + 0x280);
    uintptr_t endp   = *reinterpret_cast<uintptr_t *>(self + 0x288);
    uintptr_t cap    = *reinterpret_cast<uintptr_t *>(self + 0x290);
    void *card_alloc = *reinterpret_cast<void **>(self + 0x298);
    void *ctx28      = *reinterpret_cast<void **>(self + 0x28);

    if (begin == 0) { bail("null card vector"); return; }
    uintptr_t used = (endp - begin) / 8;
    if (used != 6) {
        if (first)
            swofty::log("[rnd-tyres] 5th box aborted: card vector has %llu slots, expected 6 "
                        "(begin=0x%llx end=0x%llx cap=%llu)",
                        (unsigned long long)used, (unsigned long long)begin, (unsigned long long)endp,
                        (unsigned long long)cap);
        return;
    }
    if (!region_readable(reinterpret_cast<void *>(lvar8 + 0x660), 8)) { bail("icon-prefix slot unreadable"); return; }
    char *prefix = *reinterpret_cast<char **>(lvar8 + 0x660);
    if (!region_readable(prefix, 1)) { bail("icon-prefix string unreadable"); return; }

    if (first)
        swofty::log("[rnd-tyres] 5th box: page opened; pre-grow card vec{begin=0x%llx end=0x%llx cap=%llu} "
                    "prefix=\"%s\"",
                    (unsigned long long)begin, (unsigned long long)endp, (unsigned long long)cap, prefix);

    uintptr_t newbegin = begin;
    if (cap < 7) {
        void *nb = vt_alloc(vec_alloc, 8 * sizeof(void *), sizeof(void *));
        if (!nb || !region_writable(nb, 8 * sizeof(void *))) { bail("vector realloc failed"); return; }
        uintptr_t *dst = reinterpret_cast<uintptr_t *>(nb);
        const uintptr_t *src = reinterpret_cast<const uintptr_t *>(begin);
        for (int i = 0; i < 6; ++i)
            dst[i] = src[i];
        dst[6] = 0;
        dst[7] = 0;
        vt_free(vec_alloc, reinterpret_cast<void *>(begin));
        newbegin = reinterpret_cast<uintptr_t>(nb);
        *reinterpret_cast<uintptr_t *>(self + 0x280) = newbegin;
        *reinterpret_cast<uintptr_t *>(self + 0x290) = 8;
    } else if (!region_writable(reinterpret_cast<void *>(begin + 0x30), 8)) {
        bail("slot[6] not writable");
        return;
    }
    *reinterpret_cast<uintptr_t *>(self + 0x288) = newbegin + 0x38;

    void *card = vt_alloc(card_alloc, kDeptCardSize, 8);
    if (!card) {
        *reinterpret_cast<uintptr_t *>(self + 0x288) = newbegin + 0x30;
        bail("card alloc failed");
        return;
    }

    NeStr *s = nullptr;
    const char *icon = "";
    Ne_Make_StrFromCStr(&s, "", 0, 0);
    Ne_Format_String(&s, "%s.%s.%s", prefix, "rnd_dept_icons_small", rnd_tyres_icon::dept("aero"));
    if (s && region_readable(reinterpret_cast<uint8_t *>(s) + kNeStrChars, 1))
        icon = reinterpret_cast<const char *>(reinterpret_cast<uint8_t *>(s) + kNeStrChars);

    Career_Construct_RnDDeptCard(card, ctx28, reinterpret_cast<void *>(self), card_alloc, "lng_rnd_dept_tyres", icon,
                                 kTyresDeptEnum);

    *reinterpret_cast<uintptr_t *>(newbegin + 0x30) = reinterpret_cast<uintptr_t>(card);
    release_nestr(s);

    if (first)
        swofty::log("[rnd-tyres] 5th box INJECTED: card=0x%llx slot[6]@0x%llx begin=0x%llx end=0x%llx cap=8 "
                    "name=lng_rnd_dept_tyres icon=\"%s.rnd_dept_icons_small.%s\" enum=%u colour[4]=0x%08x",
                    (unsigned long long)reinterpret_cast<uintptr_t>(card), (unsigned long long)(newbegin + 0x30),
                    (unsigned long long)newbegin, (unsigned long long)(newbegin + 0x38), prefix,
                    rnd_tyres_icon::dept("aero"), (unsigned)kTyresDeptEnum, (*k_Career_RnDDeptColourTable)[kTyresDeptEnum]);
}

// ---------------------------------------------------------------------------
// R&D > Performance: the development-potential pie 5th slice (Tyres).
//
// Career_Build_RnDPieOverview (class B) allocates the pie object, constructs
// exactly four 0x220 slots with Career_Construct_RnDPieObject and move-assigns
// it into this+0x668. v0.7.1, with render_fifth_pie on (latched at load):
//   site_Career_RnDPieBufSize        edx 0x900 -> 0xb40 (0x80 header + 5 slots + 0x20 footer)
//   site_Career_RnDPieCtorRet        r14 = new object: construct slot 4 like the ctor loop
//                                    (17 handles {vtable, 0, 0, 0}, vtables copied from the
//                                    stock slots), write the footer, register the object
//   site_Career_RnDPie*SlotDtor      the first slot destroy of each destroy path
//                                    (move-assign, teardown, class-B dtor, moved-from):
//                                    a registered object's slot 4 is destroyed first
//   site_Career_RnDPiePopulateDeptGuard  cmp r14d,3 -> 4 (codes 0..4)
//   site_Career_RnDPiePopulateGate   slot index 4 runs only on a registered object with
//                                    its footer and slot-4 vtables intact; otherwise
//                                    resume at site_Career_RnDPiePopulateNextCode

SWOFTY_SETTING(bool, render_fifth_pie, true);
std::atomic<bool> g_pie_on{false};

SWOFTY_ON_CHANGE(render_fifth_pie, bool on)
{
    swofty::log("[rnd-tyres] render_fifth_pie toggled -> %s; pie code patches are install-time, relaunch to apply",
                on ? "on" : "off");
}

constexpr size_t   kPieHeader      = 0x80;
constexpr size_t   kPieSlotStride  = 0x220;
constexpr size_t   kPieHandleSize  = 0x20;
constexpr size_t   kPieHandles     = kPieSlotStride / kPieHandleSize;
constexpr size_t   kPieSlot4Off    = kPieHeader + 4 * kPieSlotStride;
constexpr size_t   kPieFooterOff   = kPieHeader + 5 * kPieSlotStride;
constexpr uint32_t kPieBufStock    = 0x900;
constexpr uint32_t kPieBufNew      = static_cast<uint32_t>(kPieFooterOff + 0x20);
constexpr size_t   kPieFrameThis   = 0x40;
constexpr size_t   kPieObjField    = 0x670;
constexpr uint64_t kPieFooterMagic = 0x3545525954444e52ull;
constexpr uint64_t kPieFooterKey   = 0x9e3779b97f4a7c15ull;
static_assert(kPieHandles == 17 && kPieSlot4Off == 0x900 && kPieFooterOff == 0xb20 && kPieBufNew == 0xb40);

struct PieFooter {
    uint64_t magic;
    uint64_t self;
    uint64_t size;
    uint64_t gen;
};
static_assert(sizeof(PieFooter) == 0x20);

struct PieReg {
    uintptr_t obj;
    uint64_t gen;
};

constexpr size_t kPieRegCap = 16;
SRWLOCK g_pie_reg_lock = SRWLOCK_INIT;
PieReg g_pie_reg[kPieRegCap];
std::atomic<int> g_pie_live{0};
std::atomic<uint64_t> g_pie_gen{0};
uintptr_t g_pie_slot_vt[kPieHandles];
std::atomic<bool> g_pie_vt_known{false};
thread_local bool t_pie_grown = false;

std::atomic<int> g_pie_ctor_log{0};
std::atomic<int> g_pie_dtor_log{0};
std::atomic<int> g_pie_gate_log{0};
std::atomic<int> g_pie_skip_log{0};

bool pie_reg_add(uintptr_t obj, uint64_t gen, int &live)
{
    AcquireSRWLockExclusive(&g_pie_reg_lock);
    PieReg *slot = nullptr;
    for (auto &e : g_pie_reg) {
        if (e.obj == obj) {
            slot = &e;
            break;
        }
        if (!slot && e.obj == 0)
            slot = &e;
    }
    bool ok = slot != nullptr;
    if (ok) {
        if (slot->obj == 0)
            g_pie_live.fetch_add(1);
        slot->obj = obj;
        slot->gen = gen;
    }
    live = g_pie_live.load();
    ReleaseSRWLockExclusive(&g_pie_reg_lock);
    return ok;
}

bool pie_reg_find(uintptr_t obj, uint64_t &gen)
{
    bool found = false;
    AcquireSRWLockShared(&g_pie_reg_lock);
    for (const auto &e : g_pie_reg) {
        if (e.obj == obj) {
            gen = e.gen;
            found = true;
            break;
        }
    }
    ReleaseSRWLockShared(&g_pie_reg_lock);
    return found;
}

bool pie_reg_take(uintptr_t obj, uint64_t &gen)
{
    bool found = false;
    AcquireSRWLockExclusive(&g_pie_reg_lock);
    for (auto &e : g_pie_reg) {
        if (e.obj == obj) {
            gen = e.gen;
            e.obj = 0;
            e.gen = 0;
            g_pie_live.fetch_sub(1);
            found = true;
            break;
        }
    }
    ReleaseSRWLockExclusive(&g_pie_reg_lock);
    return found;
}

bool pie_footer_ok(uintptr_t obj, uint64_t gen)
{
    const auto *f = reinterpret_cast<const PieFooter *>(obj + kPieFooterOff);
    return f->magic == kPieFooterMagic && f->self == (obj ^ kPieFooterKey) && f->size == kPieBufNew && f->gen == gen;
}

uintptr_t pie_handle_vt(uintptr_t obj, size_t slot, size_t handle)
{
    return *reinterpret_cast<const uintptr_t *>(obj + kPieHeader + slot * kPieSlotStride + handle * kPieHandleSize);
}

int pie_slot4_bad_handle(uintptr_t obj)
{
    for (size_t i = 0; i < kPieHandles; ++i)
        if (pie_handle_vt(obj, 4, i) != g_pie_slot_vt[i])
            return static_cast<int>(i);
    return -1;
}

int pie_stock_slots_bad_handle(uintptr_t obj)
{
    bool known = g_pie_vt_known.load(std::memory_order_acquire);
    for (size_t i = 0; i < kPieHandles; ++i) {
        uintptr_t vt = pie_handle_vt(obj, 0, i);
        if (!vt || (known && vt != g_pie_slot_vt[i]))
            return static_cast<int>(i);
        for (size_t k = 1; k < 4; ++k)
            if (pie_handle_vt(obj, k, i) != vt)
                return static_cast<int>(k * kPieHandles + i);
    }
    return -1;
}

void pie_skip_log(const char *fmt, unsigned long long a, unsigned long long b)
{
    int n = g_pie_skip_log.fetch_add(1);
    if (n >= 8 && (n & 63) != 0)
        return;
    char reason[200];
    snprintf(reason, sizeof reason, fmt, a, b);
    swofty::log("[rnd-tyres] pie5 slice SKIPPED: %s; slot 4 left untouched, the stock four populate normally", reason);
}

SWOFTY_SITE(site_Career_RnDPieBufSize, swofty::Context &c)
{
    t_pie_grown = g_pie_on.load();
    if (t_pie_grown)
        c.rdx = kPieBufNew;
}

SWOFTY_SITE(site_Career_RnDPieCtorRet, swofty::Context &c)
{
    bool grown = t_pie_grown;
    t_pie_grown = false;
    uintptr_t obj = c.r14;
    if (!g_pie_on.load())
        return;
    bool first = g_pie_ctor_log.fetch_add(1) < 8;
    if (!obj) {
        if (first)
            swofty::log("[rnd-tyres] pie: pie object alloc returned null; slot 4 not constructed (SAFE)");
        return;
    }
    if (!grown) {
        if (first)
            swofty::log("[rnd-tyres] pie: object 0x%llx was allocated with 0x%x bytes (not 0x%x); slot 4 not "
                        "constructed (SAFE)",
                        (unsigned long long)obj, kPieBufStock, kPieBufNew);
        return;
    }
    if (!region_writable(reinterpret_cast<void *>(obj), kPieBufNew)) {
        if (first)
            swofty::log("[rnd-tyres] pie: object 0x%llx not writable to +0x%x; slot 4 not constructed (SAFE)",
                        (unsigned long long)obj, kPieBufNew);
        return;
    }
    int bad = pie_stock_slots_bad_handle(obj);
    if (bad >= 0) {
        if (first)
            swofty::log("[rnd-tyres] pie: object 0x%llx slots 0..3 not as the ctor builds them (vt 0x%llx/0x%llx); "
                        "slot 4 not constructed (SAFE)",
                        (unsigned long long)obj, (unsigned long long)pie_handle_vt(obj, 0, 0),
                        (unsigned long long)pie_handle_vt(obj, 3, kPieHandles - 1));
        return;
    }
    if (!g_pie_vt_known.load(std::memory_order_acquire)) {
        for (size_t i = 0; i < kPieHandles; ++i)
            g_pie_slot_vt[i] = pie_handle_vt(obj, 0, i);
        g_pie_vt_known.store(true, std::memory_order_release);
    }
    for (size_t i = 0; i < kPieHandles; ++i) {
        auto *h = reinterpret_cast<uintptr_t *>(obj + kPieSlot4Off + i * kPieHandleSize);
        h[0] = g_pie_slot_vt[i];
        h[1] = 0;
        h[2] = 0;
        h[3] = 0;
    }
    uint64_t gen = g_pie_gen.fetch_add(1) + 1;
    auto *f = reinterpret_cast<PieFooter *>(obj + kPieFooterOff);
    f->magic = kPieFooterMagic;
    f->self = obj ^ kPieFooterKey;
    f->size = kPieBufNew;
    f->gen = gen;
    int live = 0;
    if (!pie_reg_add(obj, gen, live)) {
        f->magic = 0;
        if (first)
            swofty::log("[rnd-tyres] pie: registry full (%d live); object 0x%llx slot 4 constructed empty but NOT "
                        "registered, populate will skip it (SAFE)",
                        live, (unsigned long long)obj);
        return;
    }
    if (first)
        swofty::log("[rnd-tyres] pie: constructed slot 4 @0x%llx in pie object 0x%llx (alloc 0x%x, 17 handles, "
                    "footer gen %llu, %d live)",
                    (unsigned long long)(obj + kPieSlot4Off), (unsigned long long)obj, kPieBufNew,
                    (unsigned long long)gen, live);
}

void pie_destroy_slot4_first(uintptr_t s, void (*dtor)(void *))
{
    if (g_pie_live.load(std::memory_order_relaxed) == 0)
        return;
    for (size_t k = 0; k < 4; ++k) {
        uintptr_t off = kPieHeader + (3 - k) * kPieSlotStride;
        if (s < off)
            continue;
        uintptr_t obj = s - off;
        uint64_t gen = 0;
        if (!pie_reg_take(obj, gen))
            continue;
        bool first = g_pie_dtor_log.fetch_add(1) < 8;
        if (!region_writable(reinterpret_cast<void *>(obj), kPieBufNew) || !pie_footer_ok(obj, gen)) {
            if (first)
                swofty::log("[rnd-tyres] pie: object 0x%llx footer lost before destroy; slot 4 NOT destroyed (SAFE)",
                            (unsigned long long)obj);
            return;
        }
        int bad = pie_slot4_bad_handle(obj);
        reinterpret_cast<PieFooter *>(obj + kPieFooterOff)->magic = 0;
        if (bad >= 0) {
            if (first)
                swofty::log("[rnd-tyres] pie: object 0x%llx slot 4 handle %d vtable changed; slot 4 NOT destroyed "
                            "(SAFE)",
                            (unsigned long long)obj, bad);
            return;
        }
        dtor(reinterpret_cast<void *>(obj + kPieSlot4Off));
        if (first)
            swofty::log("[rnd-tyres] pie: destroyed slot 4 of pie object 0x%llx (gen %llu) before the stock slot "
                        "destructors (%d live)",
                        (unsigned long long)obj, (unsigned long long)gen, g_pie_live.load());
        return;
    }
}

SWOFTY_REDIRECT(site_Career_RnDPieMoveAssignSlotDtor, void, void *slot)
{
    pie_destroy_slot4_first(reinterpret_cast<uintptr_t>(slot), orig);
    orig(slot);
}

SWOFTY_REDIRECT(site_Career_RnDPieTeardownSlotDtor, void, void *slot)
{
    pie_destroy_slot4_first(reinterpret_cast<uintptr_t>(slot), orig);
    orig(slot);
}

SWOFTY_REDIRECT(site_Career_RnDPerfViewModelSlotDtor, void, void *slot)
{
    pie_destroy_slot4_first(reinterpret_cast<uintptr_t>(slot), orig);
    orig(slot);
}

SWOFTY_REDIRECT(site_Career_RnDPieMovedFromSlotDtor, void, void *slot)
{
    pie_destroy_slot4_first(reinterpret_cast<uintptr_t>(slot), orig);
    orig(slot);
}

SWOFTY_SITE(site_Career_RnDPiePopulateDeptGuard, swofty::Context &c)
{
    if (g_pie_on.load() && static_cast<uint32_t>(c.r14) == 4)
        c.rip = site_Career_RnDPiePopulateSlotCap.address();
}

SWOFTY_SITE(site_Career_RnDPiePopulateGate, swofty::Context &c)
{
    uint32_t index = static_cast<uint32_t>(c.r13);
    if (index < 4 || !g_pie_on.load())
        return;
    auto skip = [&](const char *fmt, unsigned long long a, unsigned long long b) {
        pie_skip_log(fmt, a, b);
        c.rip = site_Career_RnDPiePopulateNextCode.address();
    };
    if (index != 4) {
        skip("slot index %llu (code %llu) is past slot 4", index, static_cast<uint32_t>(c.r14));
        return;
    }
    uintptr_t self = *reinterpret_cast<const uintptr_t *>(c.rsp + kPieFrameThis);
    if (!self || !region_readable(reinterpret_cast<void *>(self + kPieObjField), sizeof(uintptr_t))) {
        skip("view model 0x%llx unreadable (code %llu)", self, static_cast<uint32_t>(c.r14));
        return;
    }
    uintptr_t obj = *reinterpret_cast<const uintptr_t *>(self + kPieObjField);
    if (!obj) {
        skip("view model 0x%llx has no pie object (code %llu)", self, static_cast<uint32_t>(c.r14));
        return;
    }
    uint64_t gen = 0;
    if (!pie_reg_find(obj, gen)) {
        skip("pie object 0x%llx was not built with the 0x%llx-byte buffer by this addon (no slot 4)", obj, kPieBufNew);
        return;
    }
    if (!region_readable(reinterpret_cast<void *>(obj), kPieBufNew)) {
        skip("pie object 0x%llx not readable to +0x%llx", obj, kPieBufNew);
        return;
    }
    if (!pie_footer_ok(obj, gen)) {
        skip("pie object 0x%llx footer at +0xb20 missing or stale (gen %llu); capacity not proven", obj, gen);
        return;
    }
    int bad = pie_slot4_bad_handle(obj);
    if (bad >= 0) {
        skip("pie object 0x%llx slot 4 handle %llu is not the constructed vtable", obj, static_cast<unsigned>(bad));
        return;
    }
    if (g_pie_gate_log.fetch_add(1) < 4)
        swofty::log("[rnd-tyres] pie5 slice slot 4 verified: object 0x%llx gen %llu, capacity 0x%x (footer), 17 "
                    "constructed handles; populating code %u",
                    (unsigned long long)obj, (unsigned long long)gen, kPieBufNew, static_cast<uint32_t>(c.r14));
}

// ---------------------------------------------------------------------------
// v0.7.2 - the Tyres tile's fitted/total count and progress ring (pie_tyres_counts).
//
// Career_Write_RnDPieTileCounts (this = rdi, query = rsi) counts codes 0..3 into
// slots 0..3. With the 5th pie slot live:
//   site_Career_RnDTileCountGuards      cmp cl,3 -> 4 (code 4 is counted)
//   site_Career_RnDTileCountIndexGuard  cmp ebx,3 -> 4 plus the gate: index 4 runs only
//                                       on a pie object this addon built with slot 4;
//                                       otherwise resume at site_Career_RnDTileCountExit,
//                                       which is what the stock guard does for index 4.

SWOFTY_SETTING(bool, pie_tyres_counts, true);
std::atomic<bool> g_counts_on{false};

SWOFTY_ON_CHANGE(pie_tyres_counts, bool on)
{
    swofty::log("[rnd-tyres] pie_tyres_counts toggled -> %s; install-time, relaunch to apply", on ? "on" : "off");
}

constexpr size_t kQueryCount    = 0xc00;
constexpr size_t kQueryCap      = 0x180;
constexpr size_t kCountInstSize = 0xe0;

std::atomic<int> g_counts_log{0};
std::atomic<int> g_counts_skip_log{0};

SWOFTY_SITE(site_Career_RnDTileCountGuards, swofty::Context &c)
{
    if (g_counts_on.load() && (c.rcx & 0xff) == 4)
        c.rip = site_Career_RnDTileCountIndexGuard.address();
}

SWOFTY_SITE(site_Career_RnDTileCountIndexGuard, swofty::Context &c)
{
    uint32_t index = static_cast<uint32_t>(c.rbx);
    if (index != 4 || !g_counts_on.load())
        return;
    auto skip = [&](const char *fmt, unsigned long long a, unsigned long long b) {
        int n = g_counts_skip_log.fetch_add(1);
        if (n < 8 || (n & 63) == 0) {
            char why[200];
            snprintf(why, sizeof why, fmt, a, b);
            swofty::log("[rnd-tyres] pie5 counts SKIPPED: %s; the Tyres tile keeps 0/0, the stock four are counted", why);
        }
        c.rip = site_Career_RnDTileCountExit.address();
    };
    uintptr_t self = c.rdi;
    if (!self || !region_readable(reinterpret_cast<void *>(self + kPieObjField), sizeof(uintptr_t))) {
        skip("view model 0x%llx unreadable (index %llu)", self, index);
        return;
    }
    uintptr_t obj = *reinterpret_cast<const uintptr_t *>(self + kPieObjField);
    uint64_t gen = 0;
    if (!obj || !pie_reg_find(obj, gen)) {
        skip("pie object 0x%llx was not built with the 0x%llx-byte buffer by this addon (no slot 4)", obj, kPieBufNew);
        return;
    }
    if (!region_readable(reinterpret_cast<void *>(obj), kPieBufNew) || !pie_footer_ok(obj, gen)) {
        skip("pie object 0x%llx footer at +0xb20 missing or stale (gen %llu)", obj, gen);
        return;
    }
    int bad = pie_slot4_bad_handle(obj);
    if (bad >= 0) {
        skip("pie object 0x%llx slot 4 handle %llu is not the constructed vtable", obj, static_cast<unsigned>(bad));
        return;
    }
    c.rip = site_Career_RnDTileCountBody.address();
    if (g_counts_log.fetch_add(1) >= 6)
        return;
    uintptr_t q = c.rsi;
    unsigned total = 0, fitted = 0;
    if (q && region_readable(reinterpret_cast<void *>(q), kQueryCount + 8)) {
        uint64_t n = *reinterpret_cast<const uint64_t *>(q + kQueryCount);
        for (uint64_t i = 0; i < n && i < kQueryCap; ++i) {
            uintptr_t u = *reinterpret_cast<const uintptr_t *>(q + i * 8);
            if (!u || !region_readable(reinterpret_cast<void *>(u), kCountInstSize))
                continue;
            if (*reinterpret_cast<const uint8_t *>(u + 0x72) != 4)
                continue;
            ++total;
            uintptr_t se = *reinterpret_cast<const uintptr_t *>(u + 0xc8);
            if (se && region_readable(reinterpret_cast<void *>(se - 8), 1) && *reinterpret_cast<const uint8_t *>(se - 8) == 3)
                ++fitted;
        }
    }
    swofty::log("[rnd-tyres] pie5 counts: slot 4 verified (object 0x%llx gen %llu); Tyres tile upgrades_fitted/"
                "total_upgrades = %u/%u",
                (unsigned long long)obj, (unsigned long long)gen, fitted, total);
}

// ---------------------------------------------------------------------------
// R&D > Performance: 5th-box LAYOUT diagnostic (why the injected box is invisible).
//
// The grid is a UITC tab container at builder+0x2d8 whose authored slot vector
// (builder+0x548..+0x550) holds a fixed four cells; the per-card attach never adds
// one, so the injected card has no visible cell. Read-only, one-shot, at
// site_Career_RnDDeptBoxLayoutProbe (rsi = builder, after the selection loop). See
// native/HOOKS.md.

SWOFTY_SETTING(bool, render_fifth_box_layout, true);
std::atomic<bool> g_layout_enabled{true};
std::atomic<int> g_layout_log{0};

SWOFTY_ON_CHANGE(render_fifth_box_layout, bool on)
{
    g_layout_enabled.store(on);
    swofty::log("[rnd-tyres] render_fifth_box_layout toggled -> %s", on ? "on" : "off");
}

uintptr_t rd_ptr(uintptr_t a)
{
    return region_readable(reinterpret_cast<void *>(a), 8) ? *reinterpret_cast<uintptr_t *>(a) : 0;
}

SWOFTY_SITE(site_Career_RnDDeptBoxLayoutProbe, swofty::Context &c)
{
    if (!g_layout_enabled.load())
        return;
    if (g_layout_log.fetch_add(1) != 0)
        return;
    uintptr_t self = c.rsi;
    uintptr_t cbeg = rd_ptr(self + 0x280), cend = rd_ptr(self + 0x288);
    uintptr_t sbeg = rd_ptr(self + 0x548), send = rd_ptr(self + 0x550);
    unsigned long long cards = (cbeg && cend >= cbeg) ? (cend - cbeg) / 8 : 0;
    unsigned long long slots = (sbeg && send >= sbeg) ? (send - sbeg) / 0x10 : 0;
    swofty::log("[rnd-tyres] box-layout: card-vec=%llu slot-vec(container+0x270)=%llu "
                "(builder=0x%llx cards[0x280]=0x%llx..0x%llx slots[0x548]=0x%llx..0x%llx); a 5th VISIBLE cell needs "
                "a 5th authored/cloned slot + scene rect (see HOOKS.md)",
                cards, slots, (unsigned long long)self, (unsigned long long)cbeg, (unsigned long long)cend,
                (unsigned long long)sbeg, (unsigned long long)send);
    for (unsigned long long i = 0; i < slots && i < 8; ++i) {
        uintptr_t slotobj = rd_ptr(sbeg + i * 0x10 + 8);
        uintptr_t bound = slotobj ? rd_ptr(slotobj + 0x20) : 0;
        uintptr_t inner = slotobj ? rd_ptr(slotobj + 8) : 0;
        swofty::log("[rnd-tyres] box-layout slot[%llu]: slotObj=0x%llx bound-node=0x%llx inner-node=0x%llx", i,
                    (unsigned long long)slotobj, (unsigned long long)bound, (unsigned long long)inner);
    }
    for (unsigned long long i = 0; i < cards && i < 8; ++i) {
        uintptr_t card = rd_ptr(cbeg + i * 8);
        unsigned en = (card && region_readable(reinterpret_cast<void *>(card + 0x6d8), 1))
                          ? *reinterpret_cast<uint8_t *>(card + 0x6d8)
                          : 0xff;
        swofty::log("[rnd-tyres] box-layout card[%llu]: card=0x%llx node(+0x268)=0x%llx enum(+0x6d8)=%u", i,
                    (unsigned long long)card, (unsigned long long)(card ? card + 0x268 : 0), en);
    }
}

// ---------------------------------------------------------------------------
// Upgrade-tree attachment (piece 2): read-only confirmation of the code-4 ("fake")
// container path. Career_Load_UpgradeRows loops k_Career_UpgradeDeptCodeTable and
// formats <team_link>.<deptname>; at site_Career_LoadUpgradeRows_DeptPath (r8 =
// team_link, r9 = dept name, dil = code) this logs each path and its FNV-1a-64
// container hash. db/tyres_tree.json writes our rows under upgrade_department =
// "Fake" (ENUM A = 4). See native/HOOKS.md.

SWOFTY_SETTING(bool, attach_tyres_tree, true);
std::atomic<bool> g_tree_enabled{true};
std::atomic<int> g_tree_log{0};

SWOFTY_ON_CHANGE(attach_tyres_tree, bool on)
{
    g_tree_enabled.store(on);
    swofty::log("[rnd-tyres] attach_tyres_tree toggled -> %s", on ? "on" : "off");
}

constexpr uint64_t kFnvBasis = 0xcbf29ce484222325ull;
constexpr uint64_t kFnvPrime = 0x100000001b3ull;

uint64_t fnv_add(uint64_t h, const char *s, size_t n)
{
    for (size_t i = 0; i < n; ++i)
        h = (h ^ static_cast<uint8_t>(s[i])) * kFnvPrime;
    return h;
}

size_t copy_cstr(const char *src, char *dst, size_t cap)
{
    size_t i = 0;
    if (!src || !region_readable(src, 1)) {
        dst[0] = 0;
        return 0;
    }
    for (; i < cap - 1 && region_readable(src + i, 1) && src[i]; ++i)
        dst[i] = src[i];
    dst[i] = 0;
    return i;
}

SWOFTY_SITE(site_Career_LoadUpgradeRows_DeptPath, swofty::Context &c)
{
    if (!g_tree_enabled.load())
        return;
    if (g_tree_log.fetch_add(1) >= 10)
        return;
    unsigned code = static_cast<unsigned>(c.rdi & 0xff);
    char team[192], dept[48];
    size_t tlen = copy_cstr(reinterpret_cast<const char *>(c.r8), team, sizeof team);
    size_t dlen = copy_cstr(reinterpret_cast<const char *>(c.r9), dept, sizeof dept);
    uint64_t h = fnv_add(fnv_add(fnv_add(kFnvBasis, team, tlen), ".", 1), dept, dlen);
    if (code == 4) {
        uint64_t meta0 = fnv_add(h, ".upgrade_0._metadata_", 21);
        uint64_t cnt = fnv_add(h, "._upgrade_instancecount_", 24);
        swofty::log("[rnd-tyres] upgrade-tree code=4 FAKE/TYRES: path=\"%s.%s\" container FNV={%016llx} <<< place "
                    "db/tyres_tree.json rows under this: select \"{%016llx}.upgrade_N\" + \"{%016llx}\" "
                    "_upgrade_instancecount_ (ex: <team_link>.fake.upgrade_0._metadata_ key=%016llx, "
                    "_upgrade_instancecount_ key=%016llx)",
                    team, dept, (unsigned long long)h, (unsigned long long)h, (unsigned long long)h,
                    (unsigned long long)meta0, (unsigned long long)cnt);
    } else {
        swofty::log("[rnd-tyres] upgrade-tree code=%u dept=\"%s\" team_link=\"%s\" container FNV={%016llx}", code,
                    dept, team, (unsigned long long)h);
    }
}

// ---------------------------------------------------------------------------
// R&D upgrade tiles: tile-source-count diagnostic. At site_Career_RnDTreeRowCountProbe
// in Career_Load_UpgradeTreeNodes, [rsp+0x378] = the container's row_count and
// [rsp+0x58] = &dept code byte: row_count 4 for code 4 means our nodes are in the
// tree model; 1 means the instancecount bump is not honoured. Read-only.

SWOFTY_SETTING(bool, diag_tree_rowcount, true);
std::atomic<bool> g_trc_enabled{true};
std::atomic<int> g_trc_log{0};

SWOFTY_ON_CHANGE(diag_tree_rowcount, bool on)
{
    g_trc_enabled.store(on);
    swofty::log("[rnd-tyres] diag_tree_rowcount toggled -> %s", on ? "on" : "off");
}

SWOFTY_SITE(site_Career_RnDTreeRowCountProbe, swofty::Context &c)
{
    if (!g_trc_enabled.load())
        return;
    uintptr_t rsp = c.rsp;
    const void *code_pp = reinterpret_cast<const void *>(rsp + 0x58);
    const void *rc_p = reinterpret_cast<const void *>(rsp + 0x378);
    if (!region_readable(code_pp, 8) || !region_readable(rc_p, 4))
        return;
    uintptr_t code_ptr = *reinterpret_cast<const uintptr_t *>(code_pp);
    if (!region_readable(reinterpret_cast<const void *>(code_ptr), 1))
        return;
    unsigned code = *reinterpret_cast<const uint8_t *>(code_ptr);
    uint32_t row_count = *reinterpret_cast<const uint32_t *>(rc_p);
    int n = g_trc_log.fetch_add(1);
    if (code == 4) {
        if (n < 48)
            swofty::log("[rnd-tyres] tree-rows code=4 FAKE/TYRES: tree-view data source row_count=%u "
                        "(tiles the data layer yields for the Tyres branch; 4 = hub+our 3 loaded -> blocker is "
                        "the DISPLAY/active-dept filter; 1 = _upgrade_instancecount_ bump NOT honoured -> data)",
                        row_count);
    } else if (n < 16) {
        swofty::log("[rnd-tyres] tree-rows code=%u: tree-view data source row_count=%u", code, row_count);
    }
}

// ---------------------------------------------------------------------------
// R&D overview box-grid selection (v0.5.0 leftover, default OFF since v0.6.0).
//
// site_Career_RnDBoxGridNameRead is always live: a no-op for idx 0..3, it keeps
// k_Career_RnDDeptNameTable[4] (= colour[0]) from ever being dereferenced.
// site_Career_RnDBoxGridNameClamp relaxes `cmp al,4 ; cmovae` to admit idx 4 only
// when select_fifth_dept was on at launch (resume at site_Career_RnDBoxGridNameIndex).

SWOFTY_SETTING(bool, select_fifth_dept, false);
std::atomic<bool> g_select_on{false};
std::atomic<int> g_name_log{0};

SWOFTY_ON_CHANGE(select_fifth_dept, bool on)
{
    swofty::log("[rnd-tyres] select_fifth_dept toggled -> %s; the box-grid clamp byte-patch is install-time, "
                "relaunch to apply",
                on ? "on" : "off");
}

const char kTyresDeptName[] = "Tyres & Thermal Management";

SWOFTY_SITE(site_Career_RnDBoxGridNameRead, swofty::Context &c)
{
    if ((c.rax & 0xffull) == 4ull) {
        c.rdx = reinterpret_cast<uintptr_t>(kTyresDeptName);
        int n = g_name_log.fetch_add(1);
        if (n < 16)
            swofty::log("[rnd-tyres] dept-name[4] overrun intercepted -> \"%s\" (nameTable[4]=colour[0] char* avoided)",
                        kTyresDeptName);
    }
}

SWOFTY_SITE(site_Career_RnDBoxGridNameClamp, swofty::Context &c)
{
    if (g_select_on.load() && (c.rax & 0xffull) == 4ull)
        c.rip = site_Career_RnDBoxGridNameIndex.address();
}

// ---------------------------------------------------------------------------
// v0.6.0: instantiate the Tyres upgrades (why the Tyres department page is empty).
//
// Every loaded UpgradeRecord becomes a 0xe0-byte instance in its team's trees; the
// R&D page query keeps an instance only if its last state entry is enabled. Three
// layers skip department 4 (+0x72), the tree hub's department:
//  1. site_Career_UpgradeInitFakeDeptBranch: dept 4 gets one Invalid, disabled
//     sentinel instead of the normal init (site_Career_UpgradeInitNormal).
//  2. site_Career_Upgrade{PushState,SetEnabled,SetLocked,SetFitted,AutoFit}DeptGate:
//     every state mutator returns on dept > 3.
//  3. site_Career_UpgradeGraphHubTest: the availability search treats dept 4 as the hub.
// For OUR instances only (dept 4, team row +0x7c == 0xff, node hash +0x58 or
// upgrade_name), each site resumes on the normal path; the stock hub keeps the
// sentinel. A sanitizer re-runs Car_Reset_LastFittedUpgrade on any of ours whose
// last entry is still disabled (a persisted v0.4/v0.5 sentinel) at
// site_Career_DeserialiseUpgradesEnd, at the Career_Refresh_UpgradeAvailability
// entry and at site_Career_RnDDeptPagePreQuery, then invalidates the owner cache
// (mode 0) or refreshes availability (modes 1/2, load only). Install-time:
// instantiate_tyres_upgrades is latched at launch.

SWOFTY_SETTING(bool, instantiate_tyres_upgrades, true);
std::atomic<bool> g_inst_live{false};

SWOFTY_ON_CHANGE(instantiate_tyres_upgrades, bool on)
{
    swofty::log("[rnd-tyres] instantiate_tyres_upgrades toggled -> %s; the instance hooks are install-time, relaunch "
                "to apply",
                on ? "on" : "off");
}

constexpr uint32_t djb2(const char *s)
{
    uint32_t h = 0x1505;
    for (; *s; ++s)
        h = h * 33u + static_cast<uint8_t>(*s);
    return h;
}

constexpr int kNumRows = 15;

constexpr uint32_t row_hash(int n)
{
    uint32_t h = 0x1505;
    for (const char *p = "tyres_thermal_"; *p; ++p)
        h = h * 33u + static_cast<uint8_t>(*p);
    if (n >= 10)
        h = h * 33u + static_cast<uint8_t>('0' + n / 10);
    h = h * 33u + static_cast<uint8_t>('0' + n % 10);
    for (const char *p = "_255"; *p; ++p)
        h = h * 33u + static_cast<uint8_t>(*p);
    return h;
}

struct OurRow {
    const char *name;
    const char *art;
};

constexpr OurRow kOurRows[kNumRows] = {
    {"tt_thermal_model", "tyre_blankets"},      {"tt_rim_heat_shield", "tyre_blankets"},
    {"tt_rim_cooling", "tyre_blankets"},        {"tt_carcass_heat", "tyre_blankets"},
    {"bc_duct_inlet", "calipers"},              {"bc_disc_ventilation", "calipers"},
    {"bc_caliper_fins", "calipers"},            {"bc_drum_package", "calipers"},
    {"tl_sidewall_load", "tyre_blankets"},      {"tl_pressure_telemetry", "tyre_blankets"},
    {"tl_camber_wear", "tyre_blankets"},        {"tl_low_deg_package", "tyre_blankets"},
    {"pc_radiator_core", "turbo_charger_wear_1"}, {"pc_cooling_louvres", "turbo_charger_wear_1"},
    {"pc_integrated_thermal", "turbo_charger_wear_1"},
};
constexpr const char *kLegacyNames[3] = {"tyres_wear_reduction", "tyres_brake_cooling", "tyres_thermal_reliability"};

constexpr auto make_row_hashes()
{
    std::array<uint32_t, kNumRows> h{};
    for (int i = 0; i < kNumRows; ++i)
        h[static_cast<size_t>(i)] = row_hash(i + 1);
    return h;
}
constexpr auto kOurHash = make_row_hashes();
static_assert(kOurHash[0] == 0x27e41c13u && kOurHash[1] == 0x27f63494u && kOurHash[2] == 0x28084d15u,
              "node hash drifted from the live instances");
static_assert(kOurHash[14] == djb2("tyres_thermal_15_255") && kOurHash[9] == djb2("tyres_thermal_10_255"),
              "row hash builder drifted from djb2");
static_assert(djb2("default_255") == 0xbd057d45u, "hub hash drifted from the live instance");

constexpr size_t   kInstSize       = 0xe0;
constexpr size_t   kInstName       = 0x10;
constexpr size_t   kInstHash       = 0x58;
constexpr size_t   kInstDept       = 0x72;
constexpr size_t   kInstRowFlag    = 0x7c;
constexpr size_t   kInstStatesBeg  = 0xc0;
constexpr size_t   kInstStatesEnd  = 0xc8;
constexpr uint8_t  kDeptFake       = 4;
constexpr uint8_t  kTeamRow        = 0xff;
constexpr size_t   kUpSize         = 0x1b10;
constexpr size_t   kUpMode         = 0x8;
constexpr size_t   kUpTeam         = 0xc;
constexpr size_t   kUpTrees        = 0xe0;
constexpr size_t   kUpTreeStride   = 0xc40;
constexpr size_t   kUpNumTrees     = 0x1960;
constexpr size_t   kUpOwnerCache   = 0x1aa8;
constexpr uint32_t kOwnerNone      = 0x3fff0000;

constexpr const char *kGuardNames[] = {"state push guard", "enabled flag guard", "facility lock guard",
                                       "fit flag guard", "auto-fit pass guard"};
constexpr size_t kNumGuards = sizeof(kGuardNames) / sizeof(kGuardNames[0]);

std::atomic<int> g_init_log{0};
std::atomic<int> g_hub_log{0};
std::atomic<int> g_guard_log[kNumGuards];
std::atomic<int> g_bfs_log{0};
std::atomic<int> g_sanitize_log{0};
std::atomic<int> g_sanitize_depth{0};
std::atomic<int> g_sanitize_fail{0};
std::atomic<bool> g_sanitize_off{false};
std::atomic<int> g_push_log{0};

const char *state_name(unsigned s)
{
    static const char *const kNames[] = {"Unavailable", "Available",         "InProgress",
                                         "Completed",   "Failed",            "Invalid",
                                         "PendingDevelopment", "InDevelopment", "PendingFitting"};
    return s < 9 ? kNames[s] : "?";
}

bool cstr_eq(const char *a, const char *b, size_t cap)
{
    for (size_t i = 0; i < cap; ++i) {
        if (a[i] != b[i])
            return false;
        if (b[i] == '\0')
            return true;
    }
    return false;
}

const char *inst_name(uintptr_t u)
{
    const char *s = *reinterpret_cast<const char *const *>(u + kInstName);
    return (s && region_readable(s, 32)) ? s : "?";
}

int our_index(uintptr_t u)
{
    const uint8_t *p = reinterpret_cast<const uint8_t *>(u);
    if (p[kInstDept] != kDeptFake || p[kInstRowFlag] != kTeamRow)
        return -1;
    uint32_t h = *reinterpret_cast<const uint32_t *>(u + kInstHash);
    for (int i = 0; i < kNumRows; ++i)
        if (h == kOurHash[static_cast<size_t>(i)])
            return i;
    const char *s = *reinterpret_cast<const char *const *>(u + kInstName);
    if (!s || !region_readable(s, 32))
        return -1;
    for (int i = 0; i < kNumRows; ++i)
        if (cstr_eq(s, kOurRows[i].name, 32))
            return i;
    for (int i = 0; i < 3; ++i)
        if (cstr_eq(s, kLegacyNames[i], 32))
            return i;
    return -1;
}

SWOFTY_SITE(site_Career_UpgradeInitFakeDeptBranch, swofty::Context &c)
{
    uintptr_t u = c.rcx;
    if (!g_inst_live.load() || *reinterpret_cast<const uint8_t *>(u + kInstDept) != kDeptFake)
        return;
    int idx = our_index(u);
    if (idx < 0) {
        if (g_hub_log.fetch_add(1) < 3)
            swofty::log("[rnd-tyres] instance init: dept-4 \"%s\" hash=0x%08x is not ours -> stock Invalid sentinel "
                        "(hub stays hidden)",
                        inst_name(u), *reinterpret_cast<const uint32_t *>(u + kInstHash));
        return;
    }
    c.rip = site_Career_UpgradeInitNormal.address();
    if (g_init_log.fetch_add(1) < 12)
        swofty::log("[rnd-tyres] instance init: tyres_thermal_%d \"%s\" -> normal init (Unavailable, enabled) instead "
                    "of the dept-4 sentinel",
                    idx + 1, inst_name(u));
}

int admit_guard(swofty::Context &c, size_t g)
{
    uintptr_t u = c.rcx;
    if (!g_inst_live.load() || *reinterpret_cast<const uint8_t *>(u + kInstDept) != kDeptFake)
        return -1;
    int idx = our_index(u);
    if (idx < 0)
        return -1;
    if (g_guard_log[g].fetch_add(1) < 3)
        swofty::log("[rnd-tyres] %s admitted tyres_thermal_%d \"%s\" (dept 4 treated as a real department)",
                    kGuardNames[g], idx + 1, inst_name(u));
    return idx;
}

SWOFTY_SITE(site_Career_UpgradePushStateDeptGate, swofty::Context &c)
{
    int idx = admit_guard(c, 0);
    if (idx < 0)
        return;
    c.rip = site_Career_UpgradePushStateBody.address();
    unsigned to = static_cast<unsigned>(c.r8 & 0xffu);
    if (to != 0 && to != 1 && g_push_log.fetch_add(1) < 160)
        swofty::log("[rnd-tyres] state push: tyres_thermal_%d \"%s\" owner=0x%x -> %u (%s) arg=0x%x", idx + 1,
                    inst_name(c.rcx), static_cast<unsigned>(c.rdx & 0xffffffffu), to, state_name(to),
                    static_cast<unsigned>(c.r9 & 0xffffffffu));
}

SWOFTY_SITE(site_Career_UpgradeSetEnabledDeptGate, swofty::Context &c)
{
    if (admit_guard(c, 1) >= 0)
        c.rip = site_Career_UpgradeSetEnabledBody.address();
}

SWOFTY_SITE(site_Career_UpgradeSetLockedDeptGate, swofty::Context &c)
{
    if (admit_guard(c, 2) >= 0)
        c.rip = site_Career_UpgradeSetLockedBody.address();
}

SWOFTY_SITE(site_Career_UpgradeSetFittedDeptGate, swofty::Context &c)
{
    if (admit_guard(c, 3) >= 0)
        c.rip = site_Career_UpgradeSetFittedBody.address();
}

SWOFTY_SITE(site_Career_UpgradeAutoFitDeptGate, swofty::Context &c)
{
    if (admit_guard(c, 4) >= 0)
        c.rip = site_Career_UpgradeAutoFitBody.address();
}

SWOFTY_SITE(site_Career_UpgradeGraphHubTest, swofty::Context &c)
{
    if (!g_inst_live.load())
        return;
    int idx = our_index(c.rax);
    if (idx < 0)
        return;
    c.rip = site_Career_UpgradeGraphRegularNode.address();
    if (g_bfs_log.fetch_add(1) < 3)
        swofty::log("[rnd-tyres] availability BFS: tyres_thermal_%d is a regular node, not the hub", idx + 1);
}

bool states_of(uintptr_t u, uintptr_t &sb, uintptr_t &se)
{
    sb = *reinterpret_cast<const uint64_t *>(u + kInstStatesBeg);
    se = *reinterpret_cast<const uint64_t *>(u + kInstStatesEnd);
    if (sb == se)
        return true;
    if (se < sb || (se - sb) % 0x10 || (se - sb) > 0x10000)
        return false;
    return region_readable(reinterpret_cast<void *>(se - 0x10), 0x10);
}

bool needs_init(uintptr_t u)
{
    uintptr_t sb = 0, se = 0;
    if (!states_of(u, sb, se))
        return false;
    if (sb == se)
        return true;
    return *reinterpret_cast<const uint8_t *>(se - 4) == 0;
}

int sanitize_upgrades(uintptr_t U, const char *site, bool may_refresh)
{
    if (!g_inst_live.load() || g_sanitize_off.load() || g_sanitize_depth.load() != 0)
        return 0;
    if (!U || !region_readable(reinterpret_cast<void *>(U), kUpSize))
        return 0;
    uint64_t nt = *reinterpret_cast<const uint64_t *>(U + kUpNumTrees);
    if (nt == 0 || nt > 2)
        return 0;
    g_sanitize_depth.fetch_add(1);
    int reinit = 0;
    for (uint64_t k = 0; k < nt; ++k) {
        uintptr_t T = U + kUpTrees + k * kUpTreeStride;
        uintptr_t b = *reinterpret_cast<const uint64_t *>(T + 0x10);
        uintptr_t e = *reinterpret_cast<const uint64_t *>(T + 0x18);
        if (!b || e < b || (e - b) % kInstSize || (e - b) / kInstSize > 0x180)
            continue;
        if (!region_readable(reinterpret_cast<void *>(b), e - b))
            continue;
        for (uintptr_t u = b; u < e; u += kInstSize) {
            if (*reinterpret_cast<const uint8_t *>(u + kInstDept) != kDeptFake)
                continue;
            int idx = our_index(u);
            if (idx < 0 || !needs_init(u))
                continue;
            Car_Reset_LastFittedUpgrade(reinterpret_cast<void *>(u));
            ++reinit;
            uintptr_t sb = 0, se = 0;
            bool ok = states_of(u, sb, se) && se != sb;
            if (needs_init(u) && g_sanitize_fail.fetch_add(1) + 1 >= 8) {
                g_sanitize_off.store(true);
                swofty::log("[rnd-tyres] sanitize(%s): the game's init left \"%s\" disabled 8 times; sanitizer switched "
                            "OFF (SAFE)",
                            site, inst_name(u));
            }
            if (g_sanitize_log.fetch_add(1) < 48)
                swofty::log("[rnd-tyres] sanitize(%s): team 0x%x tree%llu tyres_thermal_%d \"%s\" had no enabled state "
                            "(persisted dept-4 sentinel) -> re-initialised: states=%llu last state=%u enabled=%u",
                            site, *reinterpret_cast<const uint32_t *>(U + kUpTeam), (unsigned long long)k, idx + 1,
                            inst_name(u), ok ? (unsigned long long)((se - sb) / 0x10) : 0ull,
                            ok ? (unsigned)*reinterpret_cast<const uint8_t *>(se - 8) : 0xffu,
                            ok ? (unsigned)*reinterpret_cast<const uint8_t *>(se - 4) : 0xffu);
        }
    }
    if (reinit) {
        uint8_t mode = *reinterpret_cast<const uint8_t *>(U + kUpMode);
        if (mode == 0) {
            *reinterpret_cast<volatile uint32_t *>(U + kUpOwnerCache) = kOwnerNone;
            swofty::log("[rnd-tyres] sanitize(%s): mode 0 -> owner cache invalidated so the next R&D query recomputes "
                        "availability",
                        site);
        } else if ((mode == 1 || mode == 2) && may_refresh) {
            Career_Refresh_UpgradeAvailability(reinterpret_cast<void *>(U), 0);
            swofty::log("[rnd-tyres] sanitize(%s): mode %u -> availability refreshed (day 0, as the game's reset does)",
                        site, (unsigned)mode);
        }
    }
    g_sanitize_depth.fetch_sub(1);
    return reinit;
}

SWOFTY_SITE(site_Career_DeserialiseUpgradesEnd, swofty::Context &c)
{
    sanitize_upgrades(c.rdi, "load", true);
}

SWOFTY_WRAP(Career_Refresh_UpgradeAvailability, void, void *upgrades, uint16_t day)
{
    sanitize_upgrades(reinterpret_cast<uintptr_t>(upgrades), "refresh", false);
    orig(upgrades, day);
}

SWOFTY_SITE(site_Career_RnDDeptPagePreQuery, swofty::Context &c)
{
    sanitize_upgrades(c.rcx, "page", false);
}

// ---------------------------------------------------------------------------
// Diagnostic: the R&D department page's instance query, per department. At
// site_Career_RnDDeptPageQueryDone (right after Career_Query_EnabledUpgrades;
// rsi = card, results at rsp+0xdf0, count at +0xc00) this logs the result by
// department plus every dept-4 instance of the team. Read-only.

SWOFTY_SETTING(bool, diag_tyres_instances, true);
std::atomic<bool> g_diag_inst_enabled{true};
std::atomic<int> g_page_log_tyres{0};
std::atomic<uint32_t> g_page_log_mask{0};

SWOFTY_ON_CHANGE(diag_tyres_instances, bool on)
{
    g_diag_inst_enabled.store(on);
    swofty::log("[rnd-tyres] diag_tyres_instances toggled -> %s", on ? "on" : "off");
}

void log_dept4_instances(uintptr_t U)
{
    if (!U || !region_readable(reinterpret_cast<void *>(U), kUpSize))
        return;
    uint64_t nt = *reinterpret_cast<const uint64_t *>(U + kUpNumTrees);
    if (nt == 0 || nt > 2)
        return;
    for (uint64_t k = 0; k < nt; ++k) {
        uintptr_t T = U + kUpTrees + k * kUpTreeStride;
        uintptr_t b = *reinterpret_cast<const uint64_t *>(T + 0x10);
        uintptr_t e = *reinterpret_cast<const uint64_t *>(T + 0x18);
        if (!b || e < b || (e - b) % kInstSize || (e - b) / kInstSize > 0x180
            || !region_readable(reinterpret_cast<void *>(b), e - b))
            continue;
        for (uintptr_t u = b; u < e; u += kInstSize) {
            if (*reinterpret_cast<const uint8_t *>(u + kInstDept) != kDeptFake)
                continue;
            uintptr_t sb = 0, se = 0;
            bool ok = states_of(u, sb, se);
            bool has = ok && se != sb;
            int idx = our_index(u);
            swofty::log("[rnd-tyres]   tree%llu dept-4 \"%s\" hash=0x%08x %s states=%llu last: state=%u enabled=%u "
                        "facility-locked=%u day=0x%04x",
                        (unsigned long long)k, inst_name(u), *reinterpret_cast<const uint32_t *>(u + kInstHash),
                        idx >= 0 ? "OURS" : "hub/other", has ? (unsigned long long)((se - sb) / 0x10) : 0ull,
                        has ? (unsigned)*reinterpret_cast<const uint8_t *>(se - 8) : 0xffu,
                        has ? (unsigned)*reinterpret_cast<const uint8_t *>(se - 4) : 0xffu,
                        has ? (unsigned)*reinterpret_cast<const uint8_t *>(se - 3) : 0xffu,
                        has ? (unsigned)*reinterpret_cast<const uint16_t *>(se - 0xc) : 0xffffu);
        }
    }
}

SWOFTY_SITE(site_Career_RnDDeptPageQueryDone, swofty::Context &c)
{
    if (!g_diag_inst_enabled.load())
        return;
    uintptr_t card = c.rsi;
    if (!region_readable(reinterpret_cast<void *>(card + 0x688), 0x58))
        return;
    unsigned dept = *reinterpret_cast<const uint8_t *>(card + 0x6d8);
    if (dept == kDeptFake) {
        if (g_page_log_tyres.fetch_add(1) >= 24)
            return;
    } else {
        uint32_t bit = 1u << (dept & 31);
        if (g_page_log_mask.fetch_or(bit) & bit)
            return;
    }
    uintptr_t out = c.rsp + 0xdf0;
    if (!region_readable(reinterpret_cast<void *>(out), 0xc08))
        return;
    uint64_t n = *reinterpret_cast<const uint64_t *>(out + 0xc00);
    if (n > 0x180)
        n = 0x180;
    unsigned by[6] = {0, 0, 0, 0, 0, 0};
    for (uint64_t i = 0; i < n; ++i) {
        uintptr_t u = *reinterpret_cast<const uint64_t *>(out + i * 8);
        if (!u || !region_readable(reinterpret_cast<void *>(u + kInstDept), 1)) {
            ++by[5];
            continue;
        }
        uint8_t d = *reinterpret_cast<const uint8_t *>(u + kInstDept);
        ++by[d < 5 ? d : 5];
    }
    uintptr_t U = *reinterpret_cast<const uint64_t *>(card + 0x688);
    uint32_t team = region_readable(reinterpret_cast<void *>(c.rsp + 0x8c), 4)
                        ? *reinterpret_cast<const uint32_t *>(c.rsp + 0x8c)
                        : 0;
    unsigned mode = 0xff, cache = 0;
    if (U && region_readable(reinterpret_cast<void *>(U), kUpSize)) {
        mode = *reinterpret_cast<const uint8_t *>(U + kUpMode);
        cache = *reinterpret_cast<const uint32_t *>(U + kUpOwnerCache);
    }
    swofty::log("[rnd-tyres] instances: R&D page dept=%u team=0x%x query=%llu by dept 0:%u 1:%u 2:%u 3:%u 4:%u "
                "other:%u (Upgrades=0x%llx mode=%u owner=0x%x)%s",
                dept, team, (unsigned long long)n, by[0], by[1], by[2], by[3], by[4], by[5], (unsigned long long)U, mode,
                cache, dept == kDeptFake ? (by[4] >= 12 ? " <<< Tyres page FIXED (4:12 or 4:15)"
                                                        : " <<< Tyres page: expect 4:12 or 4:15")
                                         : "");
    if (dept != kDeptFake)
        return;
    for (uint64_t i = 0; i < n; ++i) {
        uintptr_t u = *reinterpret_cast<const uint64_t *>(out + i * 8);
        if (u && region_readable(reinterpret_cast<void *>(u), kInstSize)
            && *reinterpret_cast<const uint8_t *>(u + kInstDept) == kDeptFake)
            swofty::log("[rnd-tyres]   Tyres page lists \"%s\"", inst_name(u));
    }
    log_dept4_instances(U);
}

// ---------------------------------------------------------------------------
// v0.6.1: the DEVELOP UPGRADE screen for the Tyres upgrades.
//
// Back trap: the purchase panel's back_pressed (and the perks back and the
// cross-department jump) send the container "open <name>" with "error_department"
// for department >= 4, and the open case of Career_Dispatch_RnDContainerCommand has
// no such name. At site_Career_RnDContainerOpenMaint the names "tyres" and
// "error_department" select the Tyres card's tab the stock way
// (Career_Refresh_RnDDeptMode department mode, Vm_Set_TabEnabled, resume at
// site_Career_RnDContainerOpenTail), or route to Durability when no Tyres tab owns
// a slot. Back can no longer dead-end. Install-time.
//
// v0.7.2 Tyres tile -> Tyres page (pie_tyres_open): site_Career_RnDBoxGridFirstTab
// picks the Tyres card's element while the Performance page's last focused tile is
// Tyres (rnd_fifth_pie::tyres_focus_pending) and that card owns a slot.
//
// Card art: the six site_Career_RnDCardArt{Image,Mask}N name the list's own frame
// for the state ([rsp+0x20]); the three site_Career_RnDCardDeptIconN give
// "error_department" the Tyres frame. Comparison: site_Career_RnDCompareDept aliases
// the comparison's department byte to the effect's stock department. Card colour:
// site_Career_RnDCardColourPick0..4 read k_Career_RnDCardColourTable[4] as the Tyres
// orange when the screen fixes or the pie publisher (render_fifth_pie +
// pie_tyres_performance) are on, which is when the table write used to happen.
// Test price: Career_Calc_UpgradeCost returns kTestPrice for ours (tyres_test_price,
// temporary).

SWOFTY_SETTING(bool, tyres_upgrade_screen, true);
SWOFTY_SETTING(bool, tyres_test_price, false);
std::atomic<bool> g_route_on{false};
std::atomic<bool> g_screen_enabled{true};
std::atomic<bool> g_price_enabled{false};

SWOFTY_ON_CHANGE(tyres_upgrade_screen, bool on)
{
    g_screen_enabled.store(on);
    swofty::log("[rnd-tyres] tyres_upgrade_screen toggled -> %s (art/comparison follow at once; the Back route stays "
                "installed until relaunch)",
                on ? "on" : "off");
}

SWOFTY_ON_CHANGE(tyres_test_price, bool on)
{
    g_price_enabled.store(on);
    swofty::log("[rnd-tyres] tyres_test_price toggled -> %s", on ? "on" : "off");
}

constexpr uint32_t kHashTyresTab = djb2("tyres");
constexpr uint32_t kHashErrDept  = djb2("error_department");
constexpr uint32_t kHashMaint    = djb2("maintenance");
static_assert(kHashTyresTab == 0x107666dcu && kHashErrDept == 0x7e5c1342u && kHashMaint == 0x5c31ec68u,
              "djb2 tab-name hashes drifted");
static_assert(djb2("back_pressed") == 0x493fe92bu, "back_pressed hash drifted");

constexpr uint32_t kTestPrice = 5;
constexpr uint8_t kCompareChassis = 1;
constexpr uint8_t kComparePowertrain = 3;

uint8_t compare_dept(uintptr_t inst, int idx)
{
    int d = rnd_upgrade_types::effect_dept(*reinterpret_cast<const uint8_t *>(inst + 0x70));
    if (d >= 0)
        return static_cast<uint8_t>(d);
    return idx >= 12 ? kComparePowertrain : kCompareChassis;
}

const char *compare_name(uint8_t d)
{
    static const char *const kNames[] = {"Aerodynamics", "Chassis", "Durability", "Powertrain"};
    return d < 4 ? kNames[d] : "?";
}
const char kTyresIconFrame[] = "maintenance";

bool screen_live()
{
    return g_route_on.load() && g_screen_enabled.load();
}

std::atomic<int> g_open_log{0};
std::atomic<int> g_compare_log{0};
std::atomic<int> g_art_log{0};
std::atomic<int> g_cost_log{0};
std::atomic<int> g_back_log{0};
std::atomic<int> g_option_log{0};
std::atomic<int> g_purchase_log{0};
std::atomic<int> g_box_tab_log{0};

bool find_tyres_tab(uintptr_t C, uintptr_t &elem, uintptr_t &card)
{
    if (!region_readable(reinterpret_cast<void *>(C + 0x280), 0x10)
        || !region_readable(reinterpret_cast<void *>(C + 0x548), 0x10))
        return false;
    uintptr_t cb = rd_ptr(C + 0x280), ce = rd_ptr(C + 0x288);
    uintptr_t sb = rd_ptr(C + 0x548), se = rd_ptr(C + 0x550);
    if (!cb || ce < cb || (ce - cb) % 8 || (ce - cb) / 8 > 16 || !sb || se < sb || (se - sb) % 0x10
        || (se - sb) / 0x10 > 32)
        return false;
    for (uintptr_t p = cb; p < ce; p += 8) {
        uintptr_t k = rd_ptr(p);
        if (!k || !region_readable(reinterpret_cast<void *>(k + 0x268), 0x478))
            continue;
        if (rd_ptr(k) != vt_Career_RnDDeptCard.address() || *reinterpret_cast<const uint8_t *>(k + 0x6d8) != kDeptFake)
            continue;
        for (uintptr_t s = sb; s < se; s += 0x10) {
            uintptr_t obj = rd_ptr(s + 8);
            if (!obj || !region_readable(reinterpret_cast<void *>(obj), 0x28))
                continue;
            if (rd_ptr(obj + 0x20) == k + 0x268 && rd_ptr(obj + 8) != 0) {
                elem = p;
                card = k;
                return true;
            }
        }
    }
    return false;
}

SWOFTY_SITE(site_Career_RnDContainerOpenMaint, swofty::Context &c)
{
    if (!g_route_on.load())
        return;
    uint32_t h = static_cast<uint32_t>(c.rax & 0xffffffffu);
    if (h != kHashTyresTab && h != kHashErrDept)
        return;
    const char *tok = h == kHashTyresTab ? "tyres" : "error_department";
    uintptr_t C = c.rsi;
    uintptr_t elem = 0, card = 0;
    bool first = g_open_log.fetch_add(1) < 64;
    if (find_tyres_tab(C, elem, card)) {
        uintptr_t holder = C;
        Career_Refresh_RnDDeptMode(&holder, 0);
        Vm_Set_TabEnabled(C + 0x2d8, card + 0x268, 1);
        c.rax = elem;
        c.rip = site_Career_RnDContainerOpenTail.address();
        if (first)
            swofty::log("[rnd-tyres] develop-upgrade back/open \"%s\" -> Tyres tab (card=0x%llx slot elem=0x%llx); "
                        "department mode restored",
                        tok, (unsigned long long)card, (unsigned long long)elem);
        return;
    }
    c.rax = (c.rax & ~0xffffffffull) | kHashMaint;
    if (first)
        swofty::log("[rnd-tyres] develop-upgrade back/open \"%s\": no Tyres tab with a slot (container=0x%llx) -> "
                    "routed to Durability so Back never dead-ends",
                    tok, (unsigned long long)C);
}

SWOFTY_SITE(site_Career_RnDBoxGridFirstTab, swofty::Context &c)
{
    if (!g_route_on.load() || !rnd_fifth_pie::open_tracking_live() || !rnd_fifth_pie::tyres_focus_pending())
        return;
    uintptr_t C = c.rsi;
    uintptr_t elem = 0, card = 0;
    bool first = g_box_tab_log.fetch_add(1) < 32;
    if (find_tyres_tab(C, elem, card)) {
        uintptr_t was = c.rax;
        c.rax = elem;
        if (first)
            swofty::log("[rnd-tyres] departments first tab: Tyres was the focused tile -> Tyres tab (card=0x%llx slot "
                        "elem=0x%llx, stock element 0x%llx)",
                        (unsigned long long)card, (unsigned long long)elem, (unsigned long long)was);
        return;
    }
    if (first)
        swofty::log("[rnd-tyres] departments first tab: Tyres was the focused tile but no Tyres tab owns a slot "
                    "(container=0x%llx); stock first tab kept",
                    (unsigned long long)C);
}

SWOFTY_SITE(site_Career_RnDCompareDept, swofty::Context &c)
{
    if (!screen_live())
        return;
    uint8_t *d = reinterpret_cast<uint8_t *>(c.rsp + 0x108);
    if (*d != kDeptFake || !region_readable(reinterpret_cast<void *>(c.rsi + 0x6a8), 8))
        return;
    uintptr_t inst = rd_ptr(c.rsi + 0x6a8);
    if (!inst || !region_readable(reinterpret_cast<void *>(inst), kInstSize))
        return;
    int idx = our_index(inst);
    if (idx < 0)
        return;
    uint8_t to = compare_dept(inst, idx);
    *d = to;
    if (g_compare_log.fetch_add(1) < 12)
        swofty::log("[rnd-tyres] vehicle performance comparison: tyres_thermal_%d \"%s\" department 4 -> %u (%s) for "
                    "the team bars",
                    idx + 1, inst_name(inst), static_cast<unsigned>(to), compare_name(to));
}

int match_one(const char *s, const char *n, const char *&suffix)
{
    size_t k = 0;
    while (n[k] && s[k] == n[k])
        ++k;
    if (n[k] == '\0' && (s[k] == '\0' || s[k] == '_')) {
        suffix = s + k;
        return 1;
    }
    return 0;
}

int match_our_name(const char *s, const char *&suffix)
{
    int best = -1;
    size_t best_len = 0;
    for (int i = 0; i < kNumRows; ++i) {
        const char *t = nullptr;
        size_t len = std::strlen(kOurRows[i].name);
        if (len > best_len && match_one(s, kOurRows[i].name, t)) {
            best = i;
            best_len = len;
            suffix = t;
        }
    }
    if (best >= 0)
        return best;
    for (int i = 0; i < 3; ++i)
        if (match_one(s, kLegacyNames[i], suffix))
            return i;
    return -1;
}

const char *frame_suffix(const char *state, bool omxm, bool mask)
{
    if (mask)
        return omxm ? "_m" : "";
    if (omxm && cstr_eq(state, "_in_progress", 13))
        return "_progress";
    return state;
}

char *next_art_buf()
{
    static char bufs[8][96];
    static std::atomic<unsigned> slot{0};
    return bufs[slot.fetch_add(1) & 7];
}

void part_art(swofty::Context &c, unsigned site, bool mask)
{
    if (!screen_live())
        return;
    const char **arg = reinterpret_cast<const char **>(c.rsp + 0x20);
    const char *name = *arg;
    if (!name || !region_readable(name, 1))
        return;
    if (cstr_eq(name, "error_department", 17)) {
        *arg = rnd_tyres_icon::dept(kTyresIconFrame);
        if (g_art_log.fetch_add(1) < 24)
            swofty::log("[rnd-tyres] card art: site %u department icon \"error_department\" -> \"%s\"", site,
                        rnd_tyres_icon::dept(kTyresIconFrame));
        return;
    }
    const char *suffix = nullptr;
    int idx = match_our_name(name, suffix);
    if (idx < 0)
        return;
    const char *pkg = reinterpret_cast<const char *>(c.r8);
    bool omxm = false;
    if (pkg && region_readable(pkg, 1)) {
        for (size_t i = 0; i < 160 && pkg[i]; ++i)
            if (pkg[i] == 'o' && pkg[i + 1] == 'm' && pkg[i + 2] == '_' && pkg[i + 3] == 'x' && pkg[i + 4] == 'm') {
                omxm = true;
                break;
            }
    }
    char *buf = next_art_buf();
    size_t o = 0;
    auto put = [&](const char *t) {
        for (; *t && o + 1 < 96; ++t)
            buf[o++] = *t;
    };
    bool own = mask ? rnd_tyres_icon::part_mask_live() : rnd_tyres_icon::part_art_live();
    if (omxm)
        put("om_xm_");
    put(own ? kOurRows[idx].name : kOurRows[idx].art);
    put(frame_suffix(suffix, omxm, mask));
    buf[o] = '\0';
    *arg = buf;
    if (g_art_log.fetch_add(1) < 32)
        swofty::log("[rnd-tyres] card art: site %u %s \"%s\" -> %s \"%s\"", site, mask ? "mask" : "image", name,
                    own ? "own" : "borrowed", buf);
}

SWOFTY_SITE(site_Career_RnDCardArtImage0, swofty::Context &c) { part_art(c, 0, false); }
SWOFTY_SITE(site_Career_RnDCardArtMask0, swofty::Context &c) { part_art(c, 1, true); }
SWOFTY_SITE(site_Career_RnDCardArtImage1, swofty::Context &c) { part_art(c, 2, false); }
SWOFTY_SITE(site_Career_RnDCardArtMask1, swofty::Context &c) { part_art(c, 3, true); }
SWOFTY_SITE(site_Career_RnDCardArtImage2, swofty::Context &c) { part_art(c, 4, false); }
SWOFTY_SITE(site_Career_RnDCardArtMask2, swofty::Context &c) { part_art(c, 5, true); }
SWOFTY_SITE(site_Career_RnDCardDeptIcon0, swofty::Context &c) { part_art(c, 6, false); }
SWOFTY_SITE(site_Career_RnDCardDeptIcon1, swofty::Context &c) { part_art(c, 7, false); }
SWOFTY_SITE(site_Career_RnDCardDeptIcon2, swofty::Context &c) { part_art(c, 8, false); }

SWOFTY_WRAP(Career_Calc_UpgradeCost, int32_t, void *list, void *inst, void *mods, uint32_t option)
{
    uintptr_t u = reinterpret_cast<uintptr_t>(inst);
    if (g_price_enabled.load() && u && *reinterpret_cast<const uint8_t *>(u + kInstDept) == kDeptFake) {
        int idx = our_index(u);
        if (idx >= 0) {
            if (g_cost_log.fetch_add(1) < 16)
                swofty::log("[rnd-tyres] test price: tyres_thermal_%d \"%s\" option=0x%x -> %u research points",
                            idx + 1, inst_name(u), option, kTestPrice);
            return kTestPrice;
        }
    }
    return orig(list, inst, mods, option);
}

SWOFTY_SITE(site_Career_RnDPurchaseBackOpen, swofty::Context &c)
{
    if (!g_route_on.load())
        return;
    uintptr_t panel = c.rsi;
    if (!region_readable(reinterpret_cast<void *>(panel + 0x6a8), 0x48))
        return;
    unsigned dept = *reinterpret_cast<const uint8_t *>(panel + 0x6e8);
    int n = g_back_log.fetch_add(1);
    if (dept < 4 && n >= 4)
        return;
    if (n >= 48)
        return;
    uintptr_t inst = rd_ptr(panel + 0x6a8);
    const char *nm = (inst && region_readable(reinterpret_cast<void *>(inst), kInstSize)) ? inst_name(inst) : "?";
    const char *to = reinterpret_cast<const char *>(c.rdx);
    char tob[48];
    copy_cstr(to, tob, sizeof tob);
    swofty::log("[rnd-tyres] develop-upgrade back_pressed: upgrade \"%s\" dept=%u -> open \"%s\"%s", nm, dept, tob,
                dept >= 4 ? " (handled by the Tyres-tab route)" : "");
}

SWOFTY_SITE(site_Career_RnDPurchaseOptionsEntry, swofty::Context &c)
{
    if (!g_route_on.load())
        return;
    uintptr_t panel = c.rcx;
    if (!region_readable(reinterpret_cast<void *>(panel + 0x6a8), 0x48))
        return;
    uintptr_t inst = rd_ptr(panel + 0x6a8);
    if (!inst || !region_readable(reinterpret_cast<void *>(inst), kInstSize))
        return;
    int idx = our_index(inst);
    if (idx < 0 || g_option_log.fetch_add(1) >= 40)
        return;
    uintptr_t sb = 0, se = 0;
    bool ok = states_of(inst, sb, se) && se != sb;
    const uint8_t *p = reinterpret_cast<const uint8_t *>(panel);
    swofty::log("[rnd-tyres] develop-upgrade options: tyres_thermal_%d \"%s\" arg=0x%x state=%s blocked: "
                "cannot-develop=%u insufficient-points=%u research-limit=%u",
                idx + 1, inst_name(inst), static_cast<unsigned>(c.rdx & 0xffffffffu),
                ok ? state_name(*reinterpret_cast<const uint8_t *>(se - 8)) : "?", p[0x6d0], p[0x6d1], p[0x6d2]);
}

SWOFTY_SITE(site_Career_RnDPurchaseRequestEntry, swofty::Context &c)
{
    if (!g_route_on.load())
        return;
    uintptr_t inst = c.r8;
    if (!inst || !region_readable(reinterpret_cast<void *>(inst), kInstSize))
        return;
    int idx = our_index(inst);
    if (idx < 0 || g_purchase_log.fetch_add(1) >= 40)
        return;
    uintptr_t sb = 0, se = 0;
    bool ok = states_of(inst, sb, se) && se != sb;
    swofty::log("[rnd-tyres] purchase request: tyres_thermal_%d \"%s\" team=0x%x option=%u state=%s", idx + 1,
                inst_name(inst), static_cast<unsigned>(c.rdx & 0xffffffffu), static_cast<unsigned>(c.r9 & 0xffu),
                ok ? state_name(*reinterpret_cast<const uint8_t *>(se - 8)) : "?");
}

std::atomic<bool> g_card_orange{false};

void card_colour(uintptr_t &reg, uintptr_t index)
{
    if (g_card_orange.load() && index == kDeptFake)
        reg = kColTyres;
}

SWOFTY_SITE(site_Career_RnDCardColourPick0, swofty::Context &c)
{
    if ((c.r12 & 0xff) <= 5)
        card_colour(c.rax, c.r14);
}
SWOFTY_SITE(site_Career_RnDCardColourPick1, swofty::Context &c) { card_colour(c.rcx, c.rax); }
SWOFTY_SITE(site_Career_RnDCardColourPick2, swofty::Context &c) { card_colour(c.rcx, c.rax); }
SWOFTY_SITE(site_Career_RnDCardColourPick3, swofty::Context &c) { card_colour(c.rcx, c.rax); }
SWOFTY_SITE(site_Career_RnDCardColourPick4, swofty::Context &c) { card_colour(c.rax, c.rcx); }

bool addon_setting_on(const char *id, bool fallback)
{
    SwoftySettingValue v{};
    if (swofty::detail::settings_api(swofty::api) && swofty::api->setting(id, SWOFTY_SETTING_TOGGLE, &v))
        return v.toggle != 0;
    return fallback;
}

} // namespace

SWOFTY_ON_LOAD
{
    const unsigned long long base = swofty::api ? (unsigned long long)swofty::api->module_base() : 0;
    swofty::log("[rnd-tyres] loaded; API v%u; game base 0x%llx; Tyres token=0x%016llx value=%u",
                swofty::api ? swofty::api->version : 0u, base, (unsigned long long)kTyres, (unsigned)kTyresValue);

    // v0.8.0: the Tyres icon frames, before any hook names a frame (rnd_tyres_icon.cpp).
    rnd_tyres_icon::probe();
    rnd_tyres_icon::probe_art();

    ui_colour_init();
    swofty::log("[rnd-tyres] instrumented box-grid builder (site_Career_RnDDeptBoxGridEntry) and pie builder "
                "(site_Career_RnDPotentialPieEntry)");

    g_fifth_enabled.store(render_fifth_box.get());
    swofty::log("[rnd-tyres] 5th-box inject hook installed (site_Career_RnDDeptBoxGridInject); render_fifth_box=%s",
                g_fifth_enabled.load() ? "on" : "off");

    g_pie_on.store(render_fifth_pie.get());
    if (g_pie_on.load())
        swofty::log("[rnd-tyres] pie 5th slice INSTALLED: buffer 0x%x->0x%x (5 slots + footer), slot 4 constructed "
                    "after the ctor and destroyed with the stock four, populate gate site_Career_RnDPiePopulateGate, "
                    "guard cmp r14d,3->4 at site_Career_RnDPiePopulateDeptGuard",
                    kPieBufStock, kPieBufNew);
    else
        swofty::log("[rnd-tyres] render_fifth_pie off; pie 5th slice NOT installed (SAFE)");

    // The Tyres slice names/colour/icon, the "tyres" focus path and the
    // department_performance.tyres data for the 5-sector pie (rnd_fifth_pie.cpp).
    rnd_fifth_pie::install(base, g_pie_on.load());

    // v0.7.2: the Tyres tile's fitted/total and progress ring, on the slot-4 machinery.
    if (!pie_tyres_counts.get())
        swofty::log("[rnd-tyres] pie_tyres_counts off; the Tyres tile count stays 0/0");
    else if (!g_pie_on.load())
        swofty::log("[rnd-tyres] pie5 counts: the 5th pie slot is not installed; Tyres tile count NOT installed (SAFE)");
    else {
        g_counts_on.store(true);
        swofty::log("[rnd-tyres] pie5 counts INSTALLED: Career_Write_RnDPieTileCounts counts code 4 into slot 4 (cmp "
                    "cl,3->4 at site_Career_RnDTileCountGuards, cmp ebx,3->4 and the slot-4 gate at "
                    "site_Career_RnDTileCountIndexGuard)");
    }

    // v0.7.2: department 4's icon and colour in the R&D lists (rnd_dept_tags.cpp).
    rnd_dept_tags::install(base);

    // v0.7.1: Durability-equivalent research capacity and R&D modifiers (rnd_capacity.cpp).
    rnd_capacity::install(base);

    // v0.9.0: the Tyres upgrade types 19-22 (rnd_upgrade_types.cpp).
    rnd_upgrade_types::install(base);

    g_layout_enabled.store(render_fifth_box_layout.get());
    swofty::log("[rnd-tyres] box-layout diagnostic installed (site_Career_RnDDeptBoxLayoutProbe); "
                "render_fifth_box_layout=%s",
                g_layout_enabled.load() ? "on" : "off");

    g_tree_enabled.store(attach_tyres_tree.get());
    swofty::log("[rnd-tyres] upgrade-tree instrument installed (site_Career_LoadUpgradeRows_DeptPath); "
                "attach_tyres_tree=%s; db rows use upgrade_department=Fake (ENUM A=4, no Aero collision)",
                g_tree_enabled.load() ? "on" : "off");

    g_trc_enabled.store(diag_tree_rowcount.get());
    swofty::log("[rnd-tyres] tree-rows diagnostic installed (site_Career_RnDTreeRowCountProbe); diag_tree_rowcount=%s",
                g_trc_enabled.load() ? "on" : "off");

    swofty::log("[rnd-tyres] box-grid name guard installed (site_Career_RnDBoxGridNameRead); idx 4 -> \"%s\"",
                kTyresDeptName);
    g_select_on.store(select_fifth_dept.get());
    if (g_select_on.load())
        swofty::log("[rnd-tyres] box-grid 5th-dept selection ENABLED (name-guarded; overview only, not the tile page): "
                    "cmp al,4 -> cmp al,5 at site_Career_RnDBoxGridNameClamp");
    else
        swofty::log("[rnd-tyres] select_fifth_dept off (default); box-grid clamp left stock. The v0.5.0 TeamHQ clamp "
                    "relax is removed.");

    g_diag_inst_enabled.store(diag_tyres_instances.get());
    swofty::log("[rnd-tyres] R&D page instance diagnostic installed (site_Career_RnDDeptPageQueryDone); "
                "diag_tyres_instances=%s",
                g_diag_inst_enabled.load() ? "on" : "off");

    g_inst_live.store(instantiate_tyres_upgrades.get());
    if (g_inst_live.load())
        swofty::log("[rnd-tyres] Tyres upgrade instances ENABLED: tyres_thermal_1..%d (hash 0x%08x..0x%08x) take the "
                    "normal init + state machine; the dept-4 hub \"default\"/central_fake_upgrade keeps the stock "
                    "sentinel; sanitizers at load (deserialize end), refresh entry and R&D page pre-query",
                    kNumRows, kOurHash[0], kOurHash[kNumRows - 1]);
    else
        swofty::log("[rnd-tyres] instantiate_tyres_upgrades off; Tyres upgrades keep the stock dept-4 sentinel "
                    "(page stays empty)");

    g_screen_enabled.store(tyres_upgrade_screen.get());
    g_route_on.store(g_screen_enabled.load());
    if (g_route_on.load()) {
        swofty::log("[rnd-tyres] Back route LIVE: purchase back_pressed, perks back and cross-department jumps for "
                    "department 4 now open the Tyres tab (Durability if it has no slot)");
        if (rnd_fifth_pie::open_tracking_live())
            swofty::log("[rnd-tyres] Tyres tile route LIVE: pressing the Tyres tile on R&D > Performance opens the "
                        "Tyres department page");
        else
            swofty::log("[rnd-tyres] Tyres tile -> Tyres page route NOT installed: pie_tyres_open tracking is off or "
                        "not installed (the departments screen opens the stock-focused department)");
    } else {
        swofty::log("[rnd-tyres] tyres_upgrade_screen off; Develop Upgrade screen fixes NOT installed (Back from a "
                    "Tyres upgrade dead-ends, as in v0.6.0)");
    }

    g_card_orange.store(g_route_on.load() || (g_pie_on.load() && addon_setting_on("pie_tyres_performance", true)));
    if (g_card_orange.load())
        swofty::log("[rnd-tyres] upgrade card colour[4] (dept 4) white -> Tyres 0x%08x at the k_Career_RnDCardColourTable "
                    "readers (site_Career_RnDCardColourPick0..4)",
                    kColTyres);

    g_price_enabled.store(tyres_test_price.get());
    if (g_price_enabled.load())
        swofty::log("[rnd-tyres] TEST PRICE LIVE: tyres_thermal_1..%d cost %u research points for research, "
                    "development, standard and rush (temporary; turn tyres_test_price off to restore the database "
                    "cost)",
                    kNumRows, kTestPrice);
    else
        swofty::log("[rnd-tyres] tyres_test_price off; the Tyres upgrades use the database cost");

    swofty::log("[rnd-tyres] hooked dept-enum store (site_Career_TeamDeptEnumStore); Tyres resolutions forced to %u",
                (unsigned)kTyresValue);
    swofty::log("[rnd-tyres] hooked dept-count store (site_Gdb_DeptCountStore); UpgradeDepartmentEntry count %u -> %u",
                (unsigned)kStockDeptCount, (unsigned)kNewDeptCount);
}

SWOFTY_ADDON()
