// SPDX-License-Identifier: AGPL-3.0-or-later
//
// R&D: Tyres & Thermal Management - department 4's icon and colour in the R&D lists
// (flag tyres_dept_tags, default on, install-time).
//
// Every per-upgrade department tag picks its name, icon frame and colour from the
// upgrade's department byte (+0x72) with the same shape: dept < 4 ? table[dept] :
// "error_department" for names, and a `< 6` colour table whose [4] is a white spare.
// The name is already right for Tyres (assets/text.json maps
// lng_rnd_dept_error_department to "Tyres"), but the icon frames do not exist and the
// colour is white, so the R&D > In Progress "Part Information" row showed an empty icon
// and a grey label.
//
// Icons. Each tag builds "<pkg>.<atlas>.<frame>" with Ne_Format_String, the frame as the
// 5th argument [rsp+0x20]. A MidHook on the `lea r9,[atlas]` right before the call
// (after [rsp+0x20] is stored) swaps department 4's missing frame for an existing one:
//   rnd_dept_icons_small.error_department          -> maintenance (the frame the Tyres
//                                                     tile's wheel icon uses)
//   rnd_category_icons.error_department_<type>     -> <stock department>_<type>, the
//                                                     category icon the stock game shows
//                                                     for that upgrade type
//                                                     (tyre_wear -> chassis_tyre_wear); the
//                                                     Tyres types -> tyres_<type> once the
//                                                     card art is installed
//   rnd_department_strips.rnd_error_department_strip -> rnd_maintenance_strip
// Stock departments never produce these frames, so the hooks act on department 4 only.
//
// Colours. Two single-reader tables get [4] = Tyres orange after a layout check; the
// in-progress list's table is shared with the Driver Career rewards, so that list gets a
// hook on its own colour store instead.
//
// One exactly-one `.xtls` scan gates the piece (the in-progress strip format, hit must be
// 0x141f3d6b0); every other site is byte-verified at its fixed RVA (no ASLR) and installs
// on its own, so a mismatch at one site only drops that site. Build d873a1c8.

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <array>
#include <atomic>
#include <cstdint>
#include <utility>

#include <safetyhook.hpp>
#include <swofty/mixin.hpp>

#include "rnd_dept_tags.hpp"
#include "rnd_tyres_icon.hpp"

namespace {

constexpr uintptr_t kImageBase = 0x140000000ull;

constexpr uintptr_t rva(uintptr_t va)
{
    return va - kImageBase;
}

constexpr uint8_t  kDeptTyres = 4;
constexpr uint32_t kColTyres = 0xff0078ff;
constexpr uint32_t kColTyresLight = 0xff99c9ff;
constexpr uint32_t kColWhite = 0xffffffff;

SWOFTY_SETTING(bool, tyres_dept_tags, true);

SWOFTY_ON_CHANGE(tyres_dept_tags, bool on)
{
    swofty::log("[rnd-tyres] tyres_dept_tags toggled -> %s; install-time, relaunch to apply", on ? "on" : "off");
}

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

bool starts_with(const char *s, const char *lit, size_t &len)
{
    for (len = 0; lit[len]; ++len)
        if (s[len] != lit[len])
            return false;
    return true;
}

bool cstr_is(const char *s, const char *lit)
{
    size_t n = 0;
    return starts_with(s, lit, n) && s[n] == '\0';
}

const char kErrDept[] = "error_department";
const char kErrDeptPrefix[] = "error_department_";
const char kErrStrip[] = "rnd_error_department_strip";
const char kDeptIcon[] = "maintenance";
const char kStripFrame[] = "rnd_maintenance_strip";
const char kCategoryFallback[] = "chassis_tyre_wear";

struct CategoryFrame {
    const char *type;
    const char *frame;
};

// overrides.uidb career.rnd_category_icons (and its rnd_om_xm copy): the frame of every
// upgrade type under its stock department.
const CategoryFrame kCategoryFrames[] = {
    {"tyre_wear", "chassis_tyre_wear"},
    {"brakes", "chassis_brakes"},
    {"chassis_weight_redist", "chassis_chassis_weight_redist"},
    {"chassis_weight_reduction", "chassis_chassis_weight_reduction"},
    {"drag", "aero_drag"},
    {"drs", "aero_drs"},
    {"front_downforce", "aero_front_downforce"},
    {"rear_downforce", "aero_rear_downforce"},
    {"engine_power", "engine_engine_power"},
    {"ers", "engine_ers"},
    {"fuel_consumption", "engine_fuel_consumption"},
    {"control_electronics", "maintenance_control_electronics"},
    {"energy_store", "maintenance_energy_store"},
    {"gearbox", "maintenance_gearbox"},
    {"general_maintenance", "maintenance_general_maintenance"},
    {"internal_combustion", "maintenance_internal_combustion"},
    {"mgu_heat", "maintenance_mgu_heat"},
    {"mgu_kinetic", "maintenance_mgu_kinetic"},
    {"turbo_charger", "maintenance_turbo_charger"},
};

struct OwnCategory {
    const char *type;
    const char *own;
    const char *borrowed;
};

// The Tyres types minted by rnd_upgrade_types.cpp. Stock category frames are <department>_<type>
// (chassis_brakes); tools/build_card_art.py appends ours as tyres_<type>, and they are used once
// rnd_category_icons grew (rnd_tyres_icon::category_live), else a borrowed stock frame.
const OwnCategory kOwnCategories[] = {
    {"tyre_thermal", "tyres_tyre_thermal", "chassis_tyre_wear"},
    {"brake_cooling", "tyres_brake_cooling", "chassis_brakes"},
    {"tyre_life", "tyres_tyre_life", "chassis_tyre_wear"},
    {"pu_cooling", "tyres_pu_cooling", "maintenance_internal_combustion"},
};

enum class TagKind { DeptIcon, Category, Strip };

struct TagSite {
    uintptr_t win_va;
    uintptr_t hook_va;
    TagKind kind;
    const char *what;
    uint8_t bytes[27];
};

// Window = `mov [rsp+0x20],<frame>` through the Ne_Format_String call; the hook sits on
// the `lea r9,[atlas]` inside it.
const TagSite kSites[] = {
    {0x14174dbf5ull, 0x14174dc04ull, TagKind::DeptIcon, "R&D upgrade list 0x14174cb20 department icon",
     {0x48, 0x89, 0x7c, 0x24, 0x20, 0x48, 0x89, 0xe9, 0x48, 0x8d, 0x15, 0xb5, 0x85, 0xee,
      0x03, 0x4c, 0x8d, 0x0d, 0xcf, 0xe0, 0xfb, 0x03, 0xe8, 0xe0, 0x54, 0xa1, 0x02}},
    {0x14174ee96ull, 0x14174eea5ull, TagKind::DeptIcon, "R&D upgrade list 0x14174e170 department icon",
     {0x48, 0x89, 0x7c, 0x24, 0x20, 0x48, 0x89, 0xd9, 0x48, 0x8d, 0x15, 0x14, 0x73, 0xee,
      0x03, 0x4c, 0x8d, 0x0d, 0x2e, 0xce, 0xfb, 0x03, 0xe8, 0x3f, 0x42, 0xa1, 0x02}},
    {0x1417f87a3ull, 0x1417f87afull, TagKind::DeptIcon, "upgrade card 0x1417f8090 department icon",
     {0x48, 0x89, 0x5c, 0x24, 0x20, 0x48, 0x8d, 0x15, 0x0a, 0xda, 0xe3, 0x03, 0x4c, 0x8d,
      0x0d, 0x24, 0x35, 0xf1, 0x03, 0x4c, 0x89, 0xf9, 0xe8, 0x32, 0xa9, 0x96, 0x02}},
    {0x141c88763ull, 0x141c8876full, TagKind::DeptIcon, "department perks title icon 0x141c87af0",
     {0x4c, 0x89, 0x64, 0x24, 0x20, 0x48, 0x8d, 0x15, 0x4a, 0xda, 0x9a, 0x03, 0x4c, 0x8d,
      0x0d, 0x64, 0x35, 0xa8, 0x03, 0x4c, 0x89, 0xf9, 0xe8, 0x72, 0xa9, 0x4d, 0x02}},
    {0x1419bf5f4ull, 0x1419bf600ull, TagKind::Category, "Team HQ R&D popup 0x1419be1e0 category icon",
     {0x4c, 0x89, 0x74, 0x24, 0x20, 0x48, 0x8d, 0x15, 0xb9, 0x6b, 0xc7, 0x03, 0x4c, 0x8d,
      0x0d, 0x18, 0xa7, 0xd5, 0x03, 0x48, 0x89, 0xd9, 0xe8, 0xe1, 0x3a, 0x7a, 0x02}},
    {0x141c7800cull, 0x141c78018ull, TagKind::Category, "research category row 0x141c76b60 icon",
     {0x4c, 0x89, 0x7c, 0x24, 0x20, 0x48, 0x8d, 0x15, 0xa1, 0xe1, 0x9b, 0x03, 0x4c, 0x8d,
      0x0d, 0x00, 0x1d, 0xaa, 0x03, 0x4c, 0x89, 0xf1, 0xe8, 0xc9, 0xb0, 0x4e, 0x02}},
    {0x141c83914ull, 0x141c83920ull, TagKind::Category, "department category row 0x141c82470 icon",
     {0x4c, 0x89, 0x7c, 0x24, 0x20, 0x48, 0x8d, 0x15, 0x99, 0x28, 0x9b, 0x03, 0x4c, 0x8d,
      0x0d, 0xf8, 0x63, 0xa9, 0x03, 0x4c, 0x89, 0xf1, 0xe8, 0xc1, 0xf7, 0x4d, 0x02}},
    {0x141f3da1aull, 0x141f3da29ull, TagKind::Category, "in-progress list 0x141f3bec0 part icon",
     {0x4c, 0x89, 0x74, 0x24, 0x20, 0x48, 0x89, 0xd9, 0x48, 0x8d, 0x15, 0x90, 0x87, 0x6f,
      0x03, 0x4c, 0x8d, 0x0d, 0xef, 0xc2, 0x7d, 0x03, 0xe8, 0xbb, 0x56, 0x22, 0x02}},
    {0x141f3d70eull, 0x141f3d71dull, TagKind::Strip, "in-progress list 0x141f3bec0 colour strip",
     {0x48, 0x89, 0x5c, 0x24, 0x20, 0x4c, 0x89, 0xf9, 0x48, 0x8d, 0x15, 0x9c, 0x8a, 0x6f,
      0x03, 0x4c, 0x8d, 0x0d, 0xa5, 0x06, 0x82, 0x03, 0xe8, 0xc7, 0x59, 0x22, 0x02}},
};
constexpr size_t kNumSites = sizeof(kSites) / sizeof(kSites[0]);

safetyhook::MidHook g_site_hooks[kNumSites];
std::atomic<int> g_site_log[kNumSites];
uintptr_t g_err = 0;

const char *category_frame(const char *type)
{
    for (const auto &m : kOwnCategories)
        if (cstr_is(type, m.type))
            return rnd_tyres_icon::category_live() ? m.own : m.borrowed;
    for (const auto &m : kCategoryFrames)
        if (cstr_is(type, m.type))
            return m.frame;
    return kCategoryFallback;
}

template <size_t I> void on_tag(safetyhook::Context &c)
{
    const char **arg = reinterpret_cast<const char **>(c.rsp + 0x20);
    const char *f = *arg;
    if (!f || !region_readable(f, 1))
        return;
    const char *to = nullptr;
    switch (kSites[I].kind) {
    case TagKind::DeptIcon:
        if (reinterpret_cast<uintptr_t>(f) == g_err || cstr_is(f, kErrDept))
            to = rnd_tyres_icon::dept(kDeptIcon);
        break;
    case TagKind::Category: {
        size_t n = 0;
        if (starts_with(f, kErrDeptPrefix, n))
            to = category_frame(f + n);
        break;
    }
    case TagKind::Strip:
        if (cstr_is(f, kErrStrip))
            to = rnd_tyres_icon::strip(kStripFrame);
        break;
    }
    if (!to)
        return;
    if (g_site_log[I].fetch_add(1) < 4) {
        char was[48];
        size_t k = 0;
        for (; k + 1 < sizeof was && f[k]; ++k)
            was[k] = f[k];
        was[k] = '\0';
        swofty::log("[rnd-tyres] dept tag: %s \"%s\" -> \"%s\"", kSites[I].what, was, to);
    }
    *arg = to;
}

using TagFn = void (*)(safetyhook::Context &);
template <size_t... I> constexpr auto make_tag_fns(std::index_sequence<I...>)
{
    return std::array<TagFn, sizeof...(I)>{&on_tag<I>...};
}
constexpr auto kTagFns = make_tag_fns(std::make_index_sequence<kNumSites>{});

// In-progress list colour: 0x141f3dab5 movsx rcx,[r13+0x72] ; eax = rcx <= 5 ?
// 0x14576f5d8[rcx] : -1 ; 0x141f3dacf mov [rsp+0x6c],eax. The table is also read by the
// Driver Career rewards (0x1420bcfb0, 0x1420c1bd0), so the store is hooked, not the table.
constexpr uintptr_t kVaListColWin = 0x141f3dab5ull;
constexpr uintptr_t kVaListColour = 0x141f3dacfull;
const uint8_t kListColBytes[] = {0x49, 0x0f, 0xbe, 0x4d, 0x72, 0xb8, 0xff, 0xff, 0xff, 0xff, 0x48, 0x83, 0xf9,
                                 0x05, 0x77, 0x0a, 0x48, 0x8d, 0x05, 0x0c, 0x1b, 0x83, 0x03, 0x8b, 0x04, 0x88,
                                 0x89, 0x44, 0x24, 0x6c, 0x4c, 0x8b, 0xbc, 0x24, 0xb8, 0x01, 0x00, 0x00};

safetyhook::MidHook g_list_colour_hook;
std::atomic<int> g_list_colour_log{0};

void on_list_colour(safetyhook::Context &c)
{
    uintptr_t u = c.r13;
    if (!u || !region_readable(reinterpret_cast<void *>(u + 0x72), 1))
        return;
    if (*reinterpret_cast<const uint8_t *>(u + 0x72) != kDeptTyres || static_cast<uint32_t>(c.rax) != kColWhite)
        return;
    c.rax = (c.rax & ~0xffffffffull) | kColTyres;
    if (g_list_colour_log.fetch_add(1) < 4)
        swofty::log("[rnd-tyres] dept tag: in-progress list colour for department 4 white -> 0x%08x", kColTyres);
}

struct ColourTable {
    uintptr_t va;
    uint32_t stock[6];
    uint32_t tyres;
    const char *what;
};

const ColourTable kTables[] = {
    {0x14570ddd8ull, {0xffeead00, 0xff3ec58d, 0xff16deff, 0xff7a2aed, kColWhite, kColWhite}, kColTyres,
     "R&D upgrade list 0x14174e170 colour"},
    {0x14570ddf0ull, {0xfff9df9c, 0xff7dc48d, 0xff69dfef, 0xffb589f9, kColWhite, kColWhite}, kColTyresLight,
     "R&D upgrade list 0x14174e170 light colour"},
    {0x145736548ull, {0xffeead00, 0xff3ec58d, 0xff16deff, 0xff7a2aed, kColWhite, kColWhite}, kColTyres,
     "Team HQ R&D popup 0x1419be1e0 colour"},
};

void write_table(uintptr_t base, const ColourTable &t)
{
    auto *tbl = reinterpret_cast<uint32_t *>(base + rva(t.va));
    if (!region_readable(tbl, 6 * sizeof(uint32_t))) {
        swofty::log("[rnd-tyres] dept tag: %s table @0x%llx not readable; left stock (SAFE)", t.what,
                    (unsigned long long)(uintptr_t)tbl);
        return;
    }
    if (tbl[4] == t.tyres) {
        swofty::log("[rnd-tyres] dept tag: %s [4] already 0x%08x", t.what, t.tyres);
        return;
    }
    for (int i = 0; i < 6; ++i) {
        if (tbl[i] != t.stock[i]) {
            swofty::log("[rnd-tyres] dept tag: %s table [%d]=%08x differs from stock; left stock (SAFE)", t.what, i,
                        tbl[i]);
            return;
        }
    }
    DWORD old = 0;
    if (!VirtualProtect(&tbl[4], sizeof(uint32_t), PAGE_READWRITE, &old)) {
        swofty::log("[rnd-tyres] dept tag: VirtualProtect failed for %s; left stock (SAFE)", t.what);
        return;
    }
    *reinterpret_cast<volatile uint32_t *>(&tbl[4]) = t.tyres;
    DWORD tmp = 0;
    VirtualProtect(&tbl[4], sizeof(uint32_t), old, &tmp);
    swofty::log("[rnd-tyres] dept tag: %s [4] white -> 0x%08x (@0x%llx)", t.what, t.tyres,
                (unsigned long long)(uintptr_t)&tbl[4]);
}

bool hook_site(safetyhook::MidHook &slot, uintptr_t addr, void (*fn)(safetyhook::Context &), const char *what)
{
    auto r = safetyhook::MidHook::create(reinterpret_cast<void *>(addr), fn);
    if (!r) {
        swofty::log("[rnd-tyres] dept tag: MidHook::create failed for %s @0x%llx (err %d); site NOT hooked (SAFE)",
                    what, (unsigned long long)addr, (int)r.error().type);
        return false;
    }
    slot = std::move(*r);
    return true;
}

} // namespace

namespace rnd_dept_tags {

void install(uintptr_t base)
{
    if (!swofty::api || !base) {
        swofty::log("[rnd-tyres] dept tag: no API/module base; NOT installed (SAFE)");
        return;
    }
    if (!tyres_dept_tags.get()) {
        swofty::log("[rnd-tyres] tyres_dept_tags off; Tyres upgrades keep the empty icon and white tag in R&D lists");
        return;
    }
    const uint8_t kErrBytes[] = {'e', 'r', 'r', 'o', 'r', '_', 'd', 'e', 'p', 'a', 'r', 't', 'm', 'e', 'n', 't', 0};
    if (!bytes_are(base + rva(0x14587e10dull), kErrBytes, sizeof kErrBytes)) {
        swofty::log("[rnd-tyres] dept tag: \"error_department\" not at 0x14587e10d; build differs, NOT installed "
                    "(SAFE)");
        return;
    }
    g_err = base + rva(0x14587e10dull);
    uintptr_t hit = 0;
    int matches = swofty::api->scan(
        ".xtls", "4C 8D 0D 56 0A 94 03 40 80 FF 03 77 0B 48 8D 05 AC 10 83 03 4C 8B 0C F8 BA 80 00 00 00", &hit);
    if (matches != 1 || hit != base + rva(0x141f3d6b0ull)) {
        swofty::log("[rnd-tyres] dept tag: signature matched %d time(s) (hit 0x%llx); NOT installed (SAFE)", matches,
                    (unsigned long long)hit);
        return;
    }
    int hooked = 0;
    for (size_t i = 0; i < kNumSites; ++i) {
        const TagSite &s = kSites[i];
        uintptr_t win = base + rva(s.win_va);
        uintptr_t at = base + rva(s.hook_va);
        const uint8_t *lea = s.bytes + (s.hook_va - s.win_va);
        bool lea_ok = s.hook_va > s.win_va && s.hook_va - s.win_va + 7 <= sizeof s.bytes && lea[0] == 0x4c
                      && lea[1] == 0x8d && lea[2] == 0x0d;
        if (!lea_ok || !bytes_are(win, s.bytes, sizeof s.bytes)) {
            swofty::log("[rnd-tyres] dept tag: %s bytes differ from the disassembly; site NOT hooked (SAFE)", s.what);
            continue;
        }
        if (hook_site(g_site_hooks[i], at, kTagFns[i], s.what)) {
            ++hooked;
            swofty::log("[rnd-tyres] dept tag: hooked %s @0x%llx (base+0x%llx)", s.what, (unsigned long long)at,
                        (unsigned long long)rva(s.hook_va));
        }
    }
    if (bytes_are(base + rva(kVaListColWin), kListColBytes, sizeof kListColBytes)) {
        if (hook_site(g_list_colour_hook, base + rva(kVaListColour), &on_list_colour, "in-progress list colour"))
            swofty::log("[rnd-tyres] dept tag: hooked in-progress list colour store @0x%llx (base+0x%llx)",
                        (unsigned long long)(base + rva(kVaListColour)), (unsigned long long)rva(kVaListColour));
    } else {
        swofty::log("[rnd-tyres] dept tag: in-progress list colour bytes differ; colour left stock (SAFE)");
    }
    for (const auto &t : kTables)
        write_table(base, t);
    swofty::log("[rnd-tyres] dept tags INSTALLED: %d/%u icon sites; department 4 shows the %s department icon, the %s "
                "strip, the stock category icon of its upgrade type and the Tyres orange",
                hooked, static_cast<unsigned>(kNumSites), rnd_tyres_icon::dept(kDeptIcon), rnd_tyres_icon::strip(kStripFrame));
}

} // namespace rnd_dept_tags
