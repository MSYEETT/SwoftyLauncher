// SPDX-License-Identifier: AGPL-3.0-or-later
//
// R&D: Tyres & Thermal Management - upgrade types minted natively.
//
// The stock game knows 19 upgrade types (0..18). An upgrade row's upgrade_type
// string is matched against the 19-entry name table k_Career_UpgradeTypeNameTable and
// anything unknown silently becomes 0 (Engine Power). This module mints ids 19+ for the
// Tyres department's own types and extends every place that enumerates or names the
// types, so a Completed upgrade of a new type reaches the car through its own
// <Upgrade name="radUpgrade..."> VTF block, scaled by its own progress:
//
//   resolvers   site_Career_UpgradeRowTypeJoin (Career_Load_UpgradeRows) and
//               site_Career_UpgradeMetaTypeJoin (upgrade_type_metadata loader): when the
//               stock 19-name loop finds nothing, our names give ids 19+.
//   loops       the per-vehicle fill, the department page category rows and the research
//               category rows iterate k_Career_UpgradeTypeOrder; the sites after their
//               order-table leas load an extended order table (0..18 then our ids) into
//               the loop registers instead. The stock table is never written.
//   apply       site_Career_ApplyUpgradeKeyFilter sends our keys down the 11..17 apply
//               call; site_Career_ApplyUpgradeBlockNameDefault returns our block name
//               where the game names only type < 0x12 (both in .data1 bodies).
//   UI labels   16 inlined type switches (cmp type,0x12 ; ja "INVALID UPGRADE") build
//               lng_rnd_<type> and <dept>_<type>; for our types the site loads our suffix
//               and jumps to the switch's join.
//   telemetry   three ElasticSearch reporters index the camel-case type table with no
//               bound (a read past it is a wild pointer); they get our camel-case name.
//   objectives  the weekend objectives keep a std::bitset<19> of Available types and
//               throw past 18; our types are left out of it.
//
// The metadata vector (U+0x1a38), the progress function and the type map are dynamic
// and keyed by type, so they need nothing. Every site is a KB binding: the mixin system
// installs all of them or none, and no id 19+ exists until install() (SWOFTY_ON_LOAD)
// has built the extended order table and turned the module live. Details:
// native/HOOKS.md v0.9.0.

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <atomic>
#include <cstdint>
#include <cstdio>
#include <cstring>

#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include "rnd_upgrade_types.hpp"

using namespace swofty::game;

namespace {

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
static_assert(sizeof *k_Career_UpgradeTypeOrder.get() == kStockTypes);

constexpr size_t kConsumerHooks = 22;
constexpr size_t kOrderSites = 4;

const char kUnknown[] = "unknown";

std::atomic<bool> g_live{false};
std::atomic<int> g_resolve_log{0};
std::atomic<int> g_ui_log{0};
std::atomic<int> g_tele_log{0};
std::atomic<int> g_name_log{0};
std::atomic<int> g_obj_log{0};

bool is_live()
{
    return g_live.load(std::memory_order_acquire);
}

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

void log_resolve(const char *where, const NewType *t)
{
    if (g_resolve_log.fetch_add(1) < 48)
        swofty::log("[rnd-tyres] upgrade types: %s \"%s\" -> type %u (%s)", where, t->name, (unsigned)t->id,
                    t->block);
}

// ---------------------------------------------------------------------------
// Extended order table: k_Career_UpgradeTypeOrder (0..18) then our ids.

uint8_t g_order[kStockTypes + kNumTypes];

uintptr_t order_begin()
{
    return reinterpret_cast<uintptr_t>(g_order);
}

uintptr_t order_end()
{
    return reinterpret_cast<uintptr_t>(g_order + sizeof g_order);
}

// ---------------------------------------------------------------------------
// UI type switches. Each is `cmp <type>,0x12 ; ja <default>` -> jump table, and the
// default is `lea r8|r9,["INVALID UPGRADE"] ; jmp <join>`; the join formats
// lng_rnd_%s (r8) or the <dept>_<type> category frame (r9). A site sits on the default
// lea or on the cmp head; a head redirect leaves the registers as the default path does
// (only the jump-table dispatch writes rax/rcx). Our type loads the suffix and jumps to
// the join.

enum class Reg { RAX, RCX, RSI, R9, R14, R8 };

uintptr_t &reg_ref(swofty::Context &c, Reg r)
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

struct UiSwitch {
    Reg type_reg;
    uint8_t width;
    Reg dst;
    const char *what;
};

void on_ui(swofty::Context &c, const UiSwitch &s, uintptr_t join)
{
    if (!is_live())
        return;
    uint64_t v = reg_ref(c, s.type_reg);
    if (s.width == 1)
        v &= 0xff;
    else if (s.width == 4)
        v &= 0xffffffffull;
    const NewType *t = find_id(v);
    if (!t)
        return;
    reg_ref(c, s.dst) = reinterpret_cast<uintptr_t>(t->suffix);
    c.rip = join;
    if (g_ui_log.fetch_add(1) < 24)
        swofty::log("[rnd-tyres] upgrade types: %s type %u -> \"%s\"", s.what, (unsigned)t->id, t->suffix);
}

// ---------------------------------------------------------------------------
// Telemetry: the camel-case type name table (19 entries, then string bytes).

const char *camel_of(uint64_t t)
{
    const NewType *n = find_id(t);
    return n ? n->camel : kUnknown;
}

bool tele_type(const swofty::Context &c, uint64_t &t)
{
    t = c.rax & 0xffffffffull;
    return is_live() && t >= kStockTypes;
}

void tele_log(const char *what, uint64_t t)
{
    if (g_tele_log.fetch_add(1) < 8)
        swofty::log("[rnd-tyres] upgrade types: telemetry %s type %u -> \"%s\"", what, (unsigned)t, camel_of(t));
}

} // namespace

// ---------------------------------------------------------------------------
// Resolvers. At each join of the 19-name compare loop the index register is 0x13 when
// nothing matched (id register = 0): rows r14 = index, r15 = id, r13 = string; metadata
// rsi = index, r14 = id, r12 = string.

SWOFTY_SITE(site_Career_UpgradeRowTypeJoin, swofty::Context &c)
{
    if (!is_live() || c.r14 != kStockTypes)
        return;
    const NewType *t = match_name(reinterpret_cast<const char *>(c.r13));
    if (!t)
        return;
    c.r15 = t->id;
    log_resolve("upgrade row", t);
}

SWOFTY_SITE(site_Career_UpgradeMetaTypeJoin, swofty::Context &c)
{
    if (!is_live() || c.rsi != kStockTypes)
        return;
    const NewType *t = match_name(reinterpret_cast<const char *>(c.r12));
    if (!t)
        return;
    c.r14 = t->id;
    log_resolve("upgrade_type_metadata row", t);
}

// ---------------------------------------------------------------------------
// Loops over k_Career_UpgradeTypeOrder. Vehicle fill: r15 = pointer (start site), rax =
// end reloaded every iteration (end site, only while r15 walks our table). Department
// page: r14/r15; research rows: r12/r13, both set at the loop entry jmp.

SWOFTY_SITE(site_Career_VehicleFillOrderStart, swofty::Context &c)
{
    if (is_live())
        c.r15 = order_begin();
}

SWOFTY_SITE(site_Career_VehicleFillOrderEnd, swofty::Context &c)
{
    if (c.r15 >= order_begin() && c.r15 <= order_end())
        c.rax = order_end();
}

SWOFTY_SITE(site_Career_RnDDeptPageTypeLoop, swofty::Context &c)
{
    if (!is_live())
        return;
    c.r14 = order_begin();
    c.r15 = order_end();
}

SWOFTY_SITE(site_Career_RnDResearchTypeLoop, swofty::Context &c)
{
    if (!is_live())
        return;
    c.r12 = order_begin();
    c.r13 = order_end();
}

// ---------------------------------------------------------------------------
// Apply. The key filter (edx = map key, `lea eax,[rdx-0xb] ; cmp al,6`) passes 11..17
// only; our keys jump to its call. The block-name default ("?" past type 0x11) returns
// our block name at the join (rdx = name).

SWOFTY_SITE(site_Career_ApplyUpgradeKeyFilter, swofty::Context &c)
{
    if (is_live() && find_id(c.rdx & 0xff) && (c.rdx >> 8) == 0)
        c.rip = site_Career_ApplyUpgradeKeyCall.address();
}

SWOFTY_SITE(site_Career_ApplyUpgradeBlockNameDefault, swofty::Context &c)
{
    if (!is_live())
        return;
    const NewType *t = find_id(c.rdx & 0xff);
    if (!t)
        return;
    c.rdx = reinterpret_cast<uintptr_t>(t->block);
    c.rip = site_Career_ApplyUpgradeBlockNameJoin.address();
    if (g_name_log.fetch_add(1) < 8)
        swofty::log("[rnd-tyres] upgrade types: apply 0x14fa63050 type %u names block %s", (unsigned)t->id,
                    t->block);
}

// ---------------------------------------------------------------------------
// UI type switches: site, type register and width, destination register, log name. The
// join binding is the site's name + Join.

#define RND_UI_SWITCH(site, type_reg, width, dst, what)                                                              \
    SWOFTY_SITE(site, swofty::Context &c)                                                                           \
    {                                                                                                               \
        static constexpr UiSwitch s{Reg::type_reg, width, Reg::dst, what};                                         \
        on_ui(c, s, site##Join.address());                                                                          \
    }

RND_UI_SWITCH(site_Career_UpgradeCardTypeLabel, R14, 1, R8, "upgrade card 0x1417f8090")
RND_UI_SWITCH(site_Career_Unk141814320TypeLabel, RAX, 8, R8, "0x141814320")
RND_UI_SWITCH(site_Career_Unk1418169f0TypeLabel, RAX, 8, R8, "0x1418169f0")
RND_UI_SWITCH(site_Career_RnDResultTypeLabel, RAX, 8, R8, "0x1418b02b0 label")
RND_UI_SWITCH(site_Career_RnDResultTypeIcon, RCX, 8, R9, "0x1418b02b0 icon")
RND_UI_SWITCH(site_Career_TeamHqRnDPopupTypeIcon, RCX, 8, R9, "Team HQ R&D popup 0x1419be1e0 icon")
RND_UI_SWITCH(site_Career_RnDPurchaseScreenTypeLabel, RAX, 8, R8, "0x141c4fde0")
RND_UI_SWITCH(site_Career_RnDUpgradeInfoTypeLabel, RAX, 8, R8, "0x141c6e1b0")
RND_UI_SWITCH(site_Career_RnDResearchRowTypeLabel, R9, 4, R8, "research category row 0x141c76b60 label")
RND_UI_SWITCH(site_Career_RnDResearchRowTypeIcon, RSI, 1, R9, "research category row 0x141c76b60 icon")
RND_UI_SWITCH(site_Career_RnDTodoTypeLabel, RAX, 8, R8, "0x141c80600")
RND_UI_SWITCH(site_Career_RnDDeptRowTypeLabel, RSI, 4, R8, "department category row 0x141c82470 label")
RND_UI_SWITCH(site_Career_RnDDeptRowTypeIcon, RSI, 1, R9, "department category row 0x141c82470 icon")
RND_UI_SWITCH(site_Career_RnDPurchasePanelTypeLabel, RAX, 8, R8, "develop-upgrade panel 0x141c8d540")
RND_UI_SWITCH(site_Career_RnDInProgressTypeLabel, RAX, 8, R8, "in-progress list 0x141f3bec0 label")
RND_UI_SWITCH(site_Career_RnDInProgressTypeIcon, RCX, 8, R9, "in-progress list 0x141f3bec0 icon")

// ---------------------------------------------------------------------------
// Telemetry (eax = type): upgrade fitted (UpgradeFittingChangedMessage; rcx = writer from
// rbx), upgrade failed, upgrade record. Our types get our camel-case name, others past 18 get "unknown".

SWOFTY_SITE(site_Career_TeleResearchTypeName, swofty::Context &c)
{
    uint64_t t;
    if (!tele_type(c, t))
        return;
    c.rdx = reinterpret_cast<uintptr_t>(camel_of(t));
    c.rcx = c.rbx;
    c.rip = site_Career_TeleResearchTypeNameCall.address();
    tele_log("upgrade fitted 0x14071e470", t);
}

SWOFTY_SITE(site_Career_TeleFailedTypeName, swofty::Context &c)
{
    uint64_t t;
    if (!tele_type(c, t))
        return;
    c.rdx = reinterpret_cast<uintptr_t>(camel_of(t));
    c.rip = site_Career_TeleFailedTypeNameJoin.address();
    tele_log("upgrade failed 0x14071e810", t);
}

SWOFTY_SITE(site_Career_TeleRecordTypeName, swofty::Context &c)
{
    uint64_t t;
    if (!tele_type(c, t))
        return;
    c.rdx = reinterpret_cast<uintptr_t>(camel_of(t));
    c.rip = site_Career_TeleRecordTypeNameCall.address();
    tele_log("upgrade record 0x140777d50", t);
}

// ---------------------------------------------------------------------------
// Weekend objectives (Team HQ -> session) put the types of the team's Available upgrades
// into a std::bitset<19> that filters the upgrade-discount rewards; a type past 18 reaches
// the bitset's throw ("invalid bitset position"). rdx = upgrade instance, type at +0x70.
// The rewards name stock types only, so our types skip to the next instance, as if those
// upgrades were not Available.

SWOFTY_SITE(site_Career_WeekendObjectiveType, swofty::Context &c)
{
    if (!is_live())
        return;
    const NewType *t = find_id(rd<uint8_t>(c.rdx + 0x70));
    if (!t)
        return;
    c.rip = site_Career_WeekendObjectiveNext.address();
    if (g_obj_log.fetch_add(1) < 8)
        swofty::log("[rnd-tyres] upgrade types: weekend objectives 0x14295c0a0 leaves Available type %u (%s) out "
                    "of the discount type bitset",
                    (unsigned)t->id, t->camel);
}

namespace rnd_upgrade_types {

bool live()
{
    return ::is_live();
}

int effect_dept(uint8_t type)
{
    const NewType *t = find_id(type);
    return (t && ::is_live()) ? t->effect_dept : -1;
}

void install(uintptr_t)
{
    swofty::log("[rnd-tyres] upgrade types: %u new types (Power Unit Cooling %s)", (unsigned)kNumTypes,
                kShipPuCooling ? "shipped" : "not shipped");
    std::memcpy(g_order, *k_Career_UpgradeTypeOrder, kStockTypes);
    for (size_t i = 0; i < kNumTypes; ++i)
        g_order[kStockTypes + i] = kAllTypes[i].id;
    g_live.store(true);
    char ids[160];
    size_t o = 0;
    for (size_t i = 0; i < kNumTypes && o + 48 < sizeof ids; ++i)
        o += static_cast<size_t>(std::snprintf(ids + o, sizeof ids - o, "%s%u %s", i ? ", " : "",
                                               (unsigned)kAllTypes[i].id, kAllTypes[i].block));
    swofty::log("[rnd-tyres] upgrade types INSTALLED: %s; %zu consumer hooks, 6 order-table leas -> extended order "
                "table 0x%llx (%zu types, %zu sites), 2 resolver hooks",
                ids, kConsumerHooks, (unsigned long long)order_begin(), sizeof g_order, kOrderSites);
}

} // namespace rnd_upgrade_types
