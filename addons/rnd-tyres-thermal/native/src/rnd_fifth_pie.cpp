// SPDX-License-Identifier: AGPL-3.0-or-later
//
// R&D: Tyres & Thermal Management - the development-potential pie's 5th slice.
//
// The addon's pie piece (install_fifth_pie, rnd_departments.cpp) makes
// Career_Populate_RnDPieSlices process department code 4: the pie object grows to
// five slots, slot 4 is constructed after the ctor and gated at populate, and the
// `code < 4` guard is relaxed. On its own that code-4 pass takes the jump-table
// DEFAULT case: name "error_department" (k_Career_RnDErrorDeptName), description
// key lng_rnd_dept_error_department_overview_description, colour white, and an
// icon path overrides.career.rnd_wheel_icon.error_department that does not exist.
// The pie wedges themselves are bound in laptop_r_and_d.ui to a SECOND data source,
// focused_team_perf_root.department_performance.<dept>, published by the per-team
// performance publisher, which only ever lists departments that have native stat
// entries (codes 0..3 in the stock data) and names code 4 "error_department" too.
//
// This translation unit finishes the job in four pieces, all always on:
//
//  slice            - populate: code 4 becomes "tyres" (VM child `tyres`,
//                     department_name "tyres", lng_rnd_dept_tyres,
//                     lng_rnd_dept_tyres_overview_description), colour
//                     0xff0078ff, and the wheel icon borrows the `maintenance`
//                     frame (overrides.career.rnd_wheel_icon has no tyres frame).
//  focus            - the class-B department_focused handler: the data string
//                     "tyres" now focuses the `tyres` child and raises
//                     highlight_pie_sector like the stock four, WITHOUT writing
//                     the box-grid selection index g_Career_RnDOverviewSelectedDept.
//  open             - the Tyres tile opens the Tyres department page: a "Tyres is
//                     the focused department" flag replaces the stale stock
//                     selection index for the departments screen's first tab, and
//                     the Performance page restores its focus to the Tyres tile.
//  performance      - publisher: department code 4 is added to the department
//                     loop that publishes department_performance (and only that
//                     loop), published as `tyres` with lng_rnd_dept_tyres,
//                     rnd_goto_tyres_department, colour 0xff0078ff, and a real
//                     perf / potential fraction derived from the car attributes
//                     the Tyres upgrades develop.
//
// Every handler acts ONLY on department code 4 - the stock four never change -
// and every pointer it dereferences is region-checked. The slice and focus pieces
// rely on install_fifth_pie's populate changes in rnd_departments.cpp. Every handler
// is a no-op until install() runs. See PIE_SITES.md for the disassembly that
// proves each site.

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <atomic>
#include <cmath>
#include <cstdint>

#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include "rnd_fifth_pie.hpp"
#include "rnd_tyres_icon.hpp"

using namespace swofty::game;

namespace {

constexpr uint32_t djb2(const char *s)
{
    uint32_t h = 0x1505;
    for (; *s; ++s)
        h = h * 33u + static_cast<uint8_t>(*s);
    return h;
}

constexpr uint8_t kDeptTyres = 4;
constexpr uint32_t kColTyres = 0xff0078ff; // ABGR = RGB(255,120,0) orange
constexpr uint32_t kColWhite = 0xffffffff;

// The short name every code-4 lookup resolves to. The VM child it creates is
// djb2("tyres") under the same parent the stock aero/chassis/... children use.
const char kTyresName[] = "tyres";
// overrides.career.rnd_wheel_icon frames: aero chassis maintenance engine
// regulation shutdown (overrides.uidb). No `tyres` frame, so code 4 borrows one.
const char kIconBorrow[] = "maintenance";
static_assert(djb2("tyres") == 0x107666dcu, "tyres child hash drifted");

// The data strings the focus handlers compare, in index order (0..3).
const char *const kStockDepts[4] = {"aero", "chassis", "maintenance", "engine"};

std::atomic<bool> g_live{false};

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

// Reads stop at the first mismatch or at the literal's terminator, so a valid
// engine C-string is never read past its own NUL.
bool cstr_is(const char *s, const char *lit)
{
    for (size_t i = 0;; ++i) {
        if (s[i] != lit[i])
            return false;
        if (lit[i] == '\0')
            return true;
    }
}

int stock_index(const char *s)
{
    for (int i = 0; i < 4; ++i)
        if (cstr_is(s, kStockDepts[i]))
            return i;
    return -1;
}

uintptr_t err_name()
{
    return reinterpret_cast<uintptr_t>(k_Career_RnDErrorDeptName.get());
}

bool code4(const swofty::Context &c)
{
    return static_cast<uint32_t>(c.r14) == kDeptTyres;
}

void log_hooked(uintptr_t base, const char *piece, uintptr_t at, const char *what)
{
    swofty::log("[rnd-tyres] pie5 %s: hooked %s @0x%llx (base+0x%llx)", piece, what, (unsigned long long)at,
                (unsigned long long)(at - base));
}

std::atomic<int> g_slice_log[4];
std::atomic<int> g_focus_log{0};
std::atomic<bool> g_tyres_focus{false};
std::atomic<int> g_open_log[2];
std::atomic<int> g_perf_log[6];

void clear_tyres_focus(int index, const char *page)
{
    if (!g_tyres_focus.exchange(false))
        return;
    if (g_open_log[0].fetch_add(1) < 16)
        swofty::log("[rnd-tyres] pie5 open: %s focused stock department index %u; Tyres focus cleared", page,
                    static_cast<unsigned>(index));
}

// Upgrades (0x1b10, one per team) - layout per native/HOOKS.md v0.6.0 and the
// publisher's stat-vector readers (PIE_SITES.md, Piece 3).
constexpr size_t   kUpSize       = 0x1b10;
constexpr size_t   kUpTeam       = 0xc;
constexpr size_t   kUpTrees      = 0xe0;
constexpr size_t   kUpTreeStride = 0xc40;
constexpr size_t   kUpNumTrees   = 0x1960;
constexpr size_t   kUpStatsBeg   = 0x1a38;
constexpr size_t   kUpStatsEnd   = 0x1a40;
constexpr size_t   kStatSize     = 0x20;
constexpr size_t   kStatType     = 0x0;
constexpr size_t   kStatDept     = 0x1;
constexpr size_t   kStatCur      = 0xc;
constexpr size_t   kStatMax      = 0x10;
constexpr size_t   kInstSize     = 0xe0;
constexpr size_t   kInstHash     = 0x58;
constexpr size_t   kInstType     = 0x70;
constexpr size_t   kInstDept     = 0x72;
constexpr size_t   kInstRowFlag  = 0x7c;
constexpr uint8_t  kTeamRow      = 0xff;
constexpr uint32_t kHubHash      = 0xbd057d45u;
static_assert(djb2("default_255") == kHubHash, "hub hash drifted");

// Publisher frame slots (rsp-relative, this build).
constexpr size_t kFrmList  = 0xa0;
constexpr size_t kFrmCount = 0xa8;
constexpr size_t kFrmPot   = 0x154;
constexpr size_t kFrmPerf  = 0x158;
constexpr size_t kFrmUp    = 0x160;

bool perf_code4(const swofty::Context &c)
{
    if (c.rbp < c.rsp + kFrmList || c.rbp >= c.rsp + kFrmList + 8)
        return false;
    return *reinterpret_cast<const uint8_t *>(c.rbp) == kDeptTyres;
}

struct TypeSet {
    uint8_t t[16];
    int n = 0;
    void add(uint8_t v)
    {
        for (int i = 0; i < n; ++i)
            if (t[i] == v)
                return;
        if (n < 16)
            t[n++] = v;
    }
};

bool tyres_types(uintptr_t U, TypeSet &out)
{
    uint64_t nt = *reinterpret_cast<const uint64_t *>(U + kUpNumTrees);
    if (nt == 0 || nt > 2)
        return false;
    for (uint64_t k = 0; k < nt; ++k) {
        uintptr_t T = U + kUpTrees + k * kUpTreeStride;
        uintptr_t b = *reinterpret_cast<const uint64_t *>(T + 0x10);
        uintptr_t e = *reinterpret_cast<const uint64_t *>(T + 0x18);
        if (!b || e < b || (e - b) % kInstSize || (e - b) / kInstSize > 0x180
            || !region_readable(reinterpret_cast<void *>(b), e - b))
            continue;
        for (uintptr_t u = b; u < e; u += kInstSize) {
            const uint8_t *p = reinterpret_cast<const uint8_t *>(u);
            if (p[kInstDept] != kDeptTyres || p[kInstRowFlag] != kTeamRow)
                continue;
            if (*reinterpret_cast<const uint32_t *>(u + kInstHash) == kHubHash)
                continue;
            out.add(p[kInstType]);
        }
    }
    return true;
}

void derive_tyres(swofty::Context &c)
{
    float F = c.xmm0.f32[0];
    auto *perf = reinterpret_cast<float *>(c.rsp + kFrmPerf);
    auto *pot = reinterpret_cast<float *>(c.rsp + kFrmPot);
    uintptr_t U = *reinterpret_cast<const uintptr_t *>(c.rsp + kFrmUp);
    bool first = g_perf_log[2].fetch_add(1) < 24;
    if (!std::isfinite(F) || F <= 0.0f || !U || !region_readable(reinterpret_cast<void *>(U), kUpSize)) {
        if (first)
            swofty::log("[rnd-tyres] pie5 perf: dept 4 values left as computed (F=%.4f Upgrades=0x%llx unusable)",
                        (double)F, (unsigned long long)U);
        return;
    }
    uint32_t team = *reinterpret_cast<const uint32_t *>(U + kUpTeam);
    uintptr_t sb = *reinterpret_cast<const uintptr_t *>(U + kUpStatsBeg);
    uintptr_t se = *reinterpret_cast<const uintptr_t *>(U + kUpStatsEnd);
    if (!sb || se < sb || (se - sb) % kStatSize || (se - sb) / kStatSize > 4096
        || !region_readable(reinterpret_cast<void *>(sb), se - sb)) {
        if (first)
            swofty::log("[rnd-tyres] pie5 perf: team 0x%x stat vector unusable; dept 4 values left as computed", team);
        return;
    }
    float native_max = 0.0f;
    for (uintptr_t s = sb; s < se; s += kStatSize)
        if (*reinterpret_cast<const uint8_t *>(s + kStatDept) == kDeptTyres)
            native_max += *reinterpret_cast<const float *>(s + kStatMax);
    if (native_max != 0.0f) {
        if (first)
            swofty::log("[rnd-tyres] pie5 perf: team 0x%x has native dept-4 stats (max %.4f); engine values kept "
                        "perf=%.4f potential=%.4f",
                        team, (double)native_max, (double)*perf, (double)*pot);
        return;
    }
    TypeSet T;
    tyres_types(U, T);
    float cur = 0.0f, max = 0.0f;
    int matched = 0;
    for (int i = 0; i < T.n; ++i) {
        for (uintptr_t s = sb; s < se; s += kStatSize) {
            if (*reinterpret_cast<const uint8_t *>(s + kStatType) != T.t[i])
                continue;
            cur += *reinterpret_cast<const float *>(s + kStatCur);
            max += *reinterpret_cast<const float *>(s + kStatMax);
            ++matched;
            break;
        }
    }
    float engine_perf = std::isfinite(*perf) ? *perf : 0.0f;
    float p = engine_perf + cur / F;
    float q = max > 0.0f ? max / F : p;
    if (!std::isfinite(p) || !std::isfinite(q)) {
        if (first)
            swofty::log("[rnd-tyres] pie5 perf: team 0x%x derived values not finite; left as computed", team);
        return;
    }
    if (p < 0.0f)
        p = 0.0f;
    if (p > 1.0f)
        p = 1.0f;
    if (q < p)
        q = p;
    if (q > 1.0f)
        q = 1.0f;
    *perf = p;
    *pot = q;
    if (first) {
        if (matched == 0)
            swofty::log("[rnd-tyres] pie5 perf: team 0x%x Tyres types=%d matched no stat entry; potential = perf = "
                        "%.4f",
                        team, T.n, (double)p);
        else
            swofty::log("[rnd-tyres] pie5 perf: team 0x%x dept 4 from %d Tyres type(s) [%02x %02x %02x] (%d stat "
                        "match): F=%.4f engine perf=%.4f cur=%.4f max=%.4f -> perf=%.4f potential=%.4f",
                        team, T.n, T.n > 0 ? T.t[0] : 0xff, T.n > 1 ? T.t[1] : 0xff, T.n > 2 ? T.t[2] : 0xff,
                        matched, (double)F, (double)engine_perf, (double)cur, (double)max, (double)p, (double)q);
    }
}

} // namespace

// ---------------------------------------------------------------------------
// Piece 1 - slice: Career_Populate_RnDPieSlices.
//
// r14d holds the department code for the whole loop body (written once at the
// loop head, callee-saved across every call).
//   site_Career_RnDPieSliceNameJoin      join after the name jump table; rdi = name.
//                                        The default case left rdi =
//                                        "error_department" -> "tyres". Everything
//                                        that follows (lng_rnd_dept_%s, the VM child
//                                        hash, department_name, the icon path) reads rdi.
//   site_Career_RnDPieSliceDescFormat    the description Ne_Format_String call of the
//                                        colour-switch DEFAULT case (redirected); its
//                                        name argument "error_department" -> "tyres"
//                                        (lng_rnd_dept_tyres_overview_description).
//   site_Career_RnDPieSliceDefaultColour right after the default `mov eax,0xffffffff`;
//                                        eax -> 0xff0078ff before the join stores it
//                                        as the slot's colour.
//   site_Career_RnDPieSliceIconArgs      the icon path Format("%s.%s.%s", pkg,
//                                        "rnd_wheel_icon", rdi): the 5th arg
//                                        [rsp+0x20] was just set from rdi, which is
//                                        the department name on the normal path and
//                                        "shutdown" / "regulation" on the two other
//                                        paths into the format. Only our own "tyres"
//                                        pointer (set at the name join) becomes
//                                        "maintenance"; every other value is left.

SWOFTY_SITE(site_Career_RnDPieSliceNameJoin, swofty::Context &c)
{
    if (!g_live.load() || !code4(c) || c.rdi != err_name())
        return;
    c.rdi = reinterpret_cast<uintptr_t>(kTyresName);
    if (g_slice_log[0].fetch_add(1) < 4)
        swofty::log("[rnd-tyres] pie5 slice: code 4 name error_department -> tyres (VM child tyres, "
                    "lng_rnd_dept_tyres)");
}

SWOFTY_REDIRECT(site_Career_RnDPieSliceDescFormat, int64_t, NeStr **out, const char *fmt, const char *name)
{
    if (g_live.load() && reinterpret_cast<uintptr_t>(name) == err_name()) {
        name = kTyresName;
        if (g_slice_log[1].fetch_add(1) < 4)
            swofty::log("[rnd-tyres] pie5 slice: code 4 description key -> lng_rnd_dept_tyres_overview_description");
    }
    return orig(out, fmt, name);
}

SWOFTY_SITE(site_Career_RnDPieSliceDefaultColour, swofty::Context &c)
{
    if (!g_live.load() || !code4(c) || static_cast<uint32_t>(c.rax) != kColWhite)
        return;
    c.rax = kColTyres;
    if (g_slice_log[2].fetch_add(1) < 4)
        swofty::log("[rnd-tyres] pie5 slice: code 4 colour white -> 0x%08x", kColTyres);
}

SWOFTY_SITE(site_Career_RnDPieSliceIconArgs, swofty::Context &c)
{
    if (!g_live.load() || !code4(c))
        return;
    auto *arg5 = reinterpret_cast<uintptr_t *>(c.rsp + 0x20);
    if (*arg5 != reinterpret_cast<uintptr_t>(kTyresName))
        return;
    const char *frame = rnd_tyres_icon::wheel(kIconBorrow);
    *arg5 = reinterpret_cast<uintptr_t>(frame);
    if (g_slice_log[3].fetch_add(1) < 4)
        swofty::log("[rnd-tyres] pie5 slice: code 4 wheel icon -> rnd_wheel_icon.%s%s", frame,
                    rnd_tyres_icon::live() ? "" : " (borrowed, no tyres frame)");
}

// ---------------------------------------------------------------------------
// Pieces 2 and 2b - focus and open.
//
// The class-B (R&D > Performance) department_focused handler compares the data
// chars (rbx) to aero/chassis/maintenance/engine, returns on no match, otherwise
// stores the index to g_Career_RnDOverviewSelectedDept and runs the common block
// site_Career_RnDPieFocusJoin with rsi = this, rdi = the name:
// Vm_GetOrCreate_ChildNode(djb2(name)) -> focused department -> raise
// highlight_pie_sector. The class-A (Team HQ research overview) focus path is a
// byte clone of it, so both are hooked on their first strcmp, where the code
// still differs (site_Career_RnDPieFocusNameCmp, site_Career_RnDResearchFocusNameCmp),
// and the handlers compare the data string themselves (the game's compare is a
// plain strcmp).
//   focus   data "tyres": rdi = "tyres", rip = site_Career_RnDPieFocusJoin,
//           skipping the four compares AND the index store. A written ctx.rip
//           resumes there with the original rsp; the block needs only rsi/rdi.
//   open    a "Tyres is the focused department" flag next to the index:
//             set      by focus (data "tyres")
//             cleared  when either handler matches a stock name, i.e. exactly when
//                      the stock index store runs
//             read     by rnd_departments.cpp at the box-grid first-tab join
//                      (tyres_focus_pending), which selects the Tyres tab instead
//             restore  site_Career_RnDPieFocusRestore, populate's
//                      `cmp r14b,al ; jne` (r14 = code, al = the index when < 4,
//                      else 0, so an out-of-range index would focus aero): with the
//                      flag set, al = 4 for code 4 and 0xff for codes 0..3, so only
//                      the Tyres slot becomes focused_department when the
//                      Performance page is rebuilt.

SWOFTY_SITE(site_Career_RnDPieFocusNameCmp, swofty::Context &c)
{
    if (!g_live.load())
        return;
    const char *s = reinterpret_cast<const char *>(c.rbx);
    if (!s || !region_readable(s, 1))
        return;
    if (cstr_is(s, kTyresName)) {
        c.rdi = reinterpret_cast<uintptr_t>(kTyresName);
        c.rip = site_Career_RnDPieFocusJoin.address();
        g_tyres_focus.store(true);
        if (g_focus_log.fetch_add(1) < 8)
            swofty::log("[rnd-tyres] pie5 focus: department_focused \"tyres\" -> focused_department = tyres, "
                        "highlight_pie_sector raised (selection index untouched)");
        return;
    }
    int index = stock_index(s);
    if (index >= 0)
        clear_tyres_focus(index, "R&D > Performance");
}

SWOFTY_SITE(site_Career_RnDResearchFocusNameCmp, swofty::Context &c)
{
    if (!g_live.load())
        return;
    const char *s = reinterpret_cast<const char *>(c.rbx);
    if (!s || !region_readable(s, 1))
        return;
    int index = stock_index(s);
    if (index >= 0)
        clear_tyres_focus(index, "Team HQ research overview");
}

SWOFTY_SITE(site_Career_RnDPieFocusRestore, swofty::Context &c)
{
    if (!g_live.load() || !g_tyres_focus.load())
        return;
    uint8_t want = code4(c) ? kDeptTyres : 0xff;
    c.rax = (c.rax & ~0xffull) | want;
    if (code4(c) && g_open_log[1].fetch_add(1) < 8)
        swofty::log("[rnd-tyres] pie5 open: populate restores focused_department = tyres (Tyres was the last focused "
                    "tile)");
}

// ---------------------------------------------------------------------------
// Piece 3 - performance: the per-team performance publisher.
//
// The department list (built once per team: codes whose summed stat max is non-zero)
// lives at [rsp+0xa0] (bytes, capacity 8) with its count at [rsp+0xa8]. It feeds
// several per-team loops (the performance chart's stacked bars and pips) AND the
// department_performance loop. Only the latter gets code 4: the stored count is
// never changed.
//   site_Career_RnDPerfDeptLoopEnd  `lea rbx,[rsp+rax] ; add rbx,0xa0` (loop end
//                                   pointer) with rax = count: when 4 is absent and
//                                   count < 8, store 4 at [rsp+0xa0+count] (unused
//                                   list storage) and pass count+1.
//   site_Career_RnDPerfDeptName     after perf ([rsp+0x158]) and potential
//                                   ([rsp+0x154]) are stored, xmm0 = F (the
//                                   field-wide best potential), r8 = name for
//                                   "lng_rnd_dept_%s": r8 -> "tyres"; derive the values.
//   site_Career_RnDPerfChildName    rax = name copied into the child-name buffer:
//                                   -> "tyres" (child department_performance.tyres).
//   site_Career_RnDPerfDeptColour   eax = k_Career_RnDCardColourTable[code] (bound
//                                   < 6), the dept_colour value: white -> 0xff0078ff.
//   site_Career_RnDPerfGotoName     r8 = name for "rnd_goto_%s_department": -> "tyres".
// rbp points at the current code byte inside [rsp+0xa0, rsp+0xa8) throughout the
// loop.
//
// The value. The engine computes, per department d:
//   perf      = (sum cur(stat entries of d) + pending(d)) / F
//   potential = sum max(stat entries of d) / F
// with stat entries = Upgrades +0x1a38..+0x1a40 (0x20 each: +0 type, +1 dept,
// +0xc cur, +0x10 max) and F = the field-wide best department potential. In the
// stock data no stat entry has dept 4, so potential(4) = 0 and perf(4) is only
// the pending gain of completed Tyres upgrades. When sum max(dept 4) == 0 the
// department is derived from the attribute types the Tyres upgrades develop:
//   T = { +0x70 type of this team's dept-4 instances with row flag 0xff,
//         excluding the hub (+0x58 == djb2("default_255")) }
//   perf      += sum over t in T of cur(first stat entry of type t) / F
//   potential  = sum over t in T of max(first stat entry of type t) / F
// (first entry per type, exactly like the engine's pending(d) lookup), then
// 0 <= perf <= potential <= 1. If T matches no stat entry, potential = perf.

SWOFTY_SITE(site_Career_RnDPerfDeptLoopEnd, swofty::Context &c)
{
    if (!g_live.load())
        return;
    uint64_t n = c.rax;
    if (n == 0 || n >= 8 || *reinterpret_cast<const uint64_t *>(c.rsp + kFrmCount) != n)
        return;
    auto *list = reinterpret_cast<uint8_t *>(c.rsp + kFrmList);
    for (uint64_t i = 0; i < n; ++i) {
        if (list[i] == kDeptTyres) {
            if (g_perf_log[0].fetch_add(1) < 2)
                swofty::log("[rnd-tyres] pie5 perf: the engine already lists dept 4 (native stat entries); not added");
            return;
        }
    }
    list[n] = kDeptTyres;
    c.rax = n + 1;
    if (g_perf_log[0].fetch_add(1) < 2)
        swofty::log("[rnd-tyres] pie5 perf: department_performance loop gets dept 4 (%llu -> %llu entries; the "
                    "stored list is unchanged)",
                    (unsigned long long)n, (unsigned long long)(n + 1));
}

SWOFTY_SITE(site_Career_RnDPerfDeptName, swofty::Context &c)
{
    if (!g_live.load() || !perf_code4(c))
        return;
    if (c.r8 == err_name())
        c.r8 = reinterpret_cast<uintptr_t>(kTyresName);
    derive_tyres(c);
}

SWOFTY_SITE(site_Career_RnDPerfChildName, swofty::Context &c)
{
    if (!g_live.load() || !perf_code4(c) || c.rax != err_name())
        return;
    c.rax = reinterpret_cast<uintptr_t>(kTyresName);
    if (g_perf_log[3].fetch_add(1) < 2)
        swofty::log("[rnd-tyres] pie5 perf: dept 4 published as department_performance.tyres");
}

SWOFTY_SITE(site_Career_RnDPerfDeptColour, swofty::Context &c)
{
    if (!g_live.load() || !perf_code4(c) || static_cast<uint32_t>(c.rax) != kColWhite)
        return;
    c.rax = kColTyres;
    if (g_perf_log[5].fetch_add(1) < 2)
        swofty::log("[rnd-tyres] pie5 perf: dept 4 dept_colour white -> 0x%08x", kColTyres);
}

SWOFTY_SITE(site_Career_RnDPerfGotoName, swofty::Context &c)
{
    if (!g_live.load() || !perf_code4(c) || c.r8 != err_name())
        return;
    c.r8 = reinterpret_cast<uintptr_t>(kTyresName);
    if (g_perf_log[4].fetch_add(1) < 2)
        swofty::log("[rnd-tyres] pie5 perf: dept 4 dept_colour_anim -> rnd_goto_tyres_department");
}

namespace rnd_fifth_pie {

void install(uintptr_t base)
{
    log_hooked(base, "perf", site_Career_RnDPerfDeptLoopEnd.address(), "department_performance loop bound (rax)");
    log_hooked(base, "perf", site_Career_RnDPerfDeptName.address(), "perf/potential + lng name (r8)");
    log_hooked(base, "perf", site_Career_RnDPerfChildName.address(), "child-name copy (rax)");
    log_hooked(base, "perf", site_Career_RnDPerfDeptColour.address(), "dept_colour (eax)");
    log_hooked(base, "perf", site_Career_RnDPerfGotoName.address(), "rnd_goto name (r8)");
    log_hooked(base, "slice", site_Career_RnDPieSliceNameJoin.address(), "populate name join (rdi)");
    log_hooked(base, "slice", site_Career_RnDPieSliceDescFormat.address(), "populate default description (r8)");
    log_hooked(base, "slice", site_Career_RnDPieSliceDefaultColour.address(), "populate default colour (eax)");
    log_hooked(base, "slice", site_Career_RnDPieSliceIconArgs.address(), "populate wheel-icon arg ([rsp+0x20])");
    log_hooked(base, "focus", site_Career_RnDPieFocusNameCmp.address(), "focus handler name compare");
    log_hooked(base, "open", site_Career_RnDPieFocusNameCmp.address(), "R&D > Performance stock focus");
    log_hooked(base, "open", site_Career_RnDResearchFocusNameCmp.address(), "Team HQ research overview stock focus");
    log_hooked(base, "open", site_Career_RnDPieFocusRestore.address(), "populate focus restore compare");
    g_live.store(true);
    swofty::log("[rnd-tyres] pie5 perf INSTALLED: department_performance.tyres published per team");
    swofty::log("[rnd-tyres] pie5 slice INSTALLED: code 4 populates as tyres / 0x%08x / icon %s", kColTyres,
                rnd_tyres_icon::wheel(kIconBorrow));
    swofty::log("[rnd-tyres] pie5 focus INSTALLED: data \"tyres\" resumes at 0x%llx",
                (unsigned long long)site_Career_RnDPieFocusJoin.address());
    swofty::log("[rnd-tyres] pie5 open INSTALLED: the Tyres focus is tracked for the departments screen's first tab "
                "and restored on the Performance page");
}

bool tyres_focus_pending()
{
    return g_live.load() && g_tyres_focus.load();
}

bool open_tracking_live()
{
    return g_live.load();
}

} // namespace rnd_fifth_pie
