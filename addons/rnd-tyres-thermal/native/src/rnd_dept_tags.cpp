// SPDX-License-Identifier: AGPL-3.0-or-later
//
// R&D: Tyres & Thermal Management - department 4's icon and colour in the R&D lists.
//
// Every department tag falls back to the frame "error_department" when its lookup misses:
// the lists test the upgrade's department byte (+0x72, dept < 4 ? table[dept]), the upgrade
// card its part type (0-18 map to a stock department, the Tyres types 19-22 do not) and the
// perks title the hash of return_department_name (four stock names). Colours come from a
// `< 6` table on the department byte whose [4] is a white spare.
// The name is already right for Tyres (assets/text.json maps
// lng_rnd_dept_error_department to "Tyres"), but the icon frames do not exist and the
// colour is white, so the R&D > In Progress "Part Information" row showed an empty icon
// and a grey label.
//
// Icons. Each tag builds "<pkg>.<atlas>.<frame>" with Ne_Format_String, the frame as the
// 5th argument [rsp+0x20]. A site on the `lea r9,[atlas]` right before the call (after
// [rsp+0x20] is stored) swaps the missing Tyres frame for an existing one:
//   rnd_dept_icons_small.error_department          -> maintenance (the frame the Tyres
//                                                     tile's wheel icon uses)
//   rnd_category_icons.error_department_<type>     -> <stock department>_<type>, the
//                                                     category icon the stock game shows
//                                                     for that upgrade type
//                                                     (tyre_wear -> chassis_tyre_wear); the
//                                                     Tyres types -> tyres_<type> once the
//                                                     card art is installed
//   rnd_department_strips.rnd_error_department_strip -> rnd_maintenance_strip
// The handlers key on these frame names, which stock departments, types and names never
// produce, so they act on Tyres only.
//
// Colours. A site on the store after each `< 6` colour-table read turns department 4's
// white into the Tyres orange: the R&D list rows (colour and light twin), the Team HQ R&D
// popups and the In Progress list. The tables stay stock; the In Progress one is shared
// with the Driver Career rewards.
//
// Every binding is a site_Career_* site; addresses and evidence live in the KB.

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <atomic>
#include <cstddef>
#include <cstdint>

#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include "rnd_dept_tags.hpp"
#include "rnd_tyres_icon.hpp"

using namespace swofty::game;

namespace {

constexpr uintptr_t kDeptTyres = 4;
constexpr uint32_t kColTyres = 0xff0078ff;
constexpr uint32_t kColTyresLight = 0xff99c9ff;
constexpr uint32_t kColWhite = 0xffffffff;

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

// Icon sites: the `lea r9,[atlas]` of each Ne_Format_String("%s.%s.%s", pkg, atlas, frame),
// with the frame already stored in [rsp+0x20].
enum class TagKind { DeptIcon, Category, Strip };

enum Tag : size_t {
    kUpgradeList,
    kListRow,
    kUpgradeCard,
    kPerksTitle,
    kPopup,
    kResearchRow,
    kDeptRow,
    kInProgressPart,
    kInProgressStrip,
    kNumTags
};

struct TagSite {
    TagKind kind;
    const char *what;
    uintptr_t (*at)();
};

const TagSite kTags[kNumTags] = {
    {TagKind::DeptIcon, "R&D upgrade list department icon", site_Career_RnDUpgradeListDeptIcon.address},
    {TagKind::DeptIcon, "R&D list row department icon", site_Career_RnDListRowDeptIcon.address},
    {TagKind::DeptIcon, "upgrade card department icon", site_Career_UpgradeCardDeptIcon.address},
    {TagKind::DeptIcon, "department perks title icon", site_Career_RnDPerksTitleDeptIcon.address},
    {TagKind::Category, "Team HQ R&D popup category icon", site_Career_RnDPopupCategoryIcon.address},
    {TagKind::Category, "research category row icon", site_Career_RnDResearchRowCategoryIcon.address},
    {TagKind::Category, "department category row icon", site_Career_RnDDeptRowCategoryIcon.address},
    {TagKind::Category, "in-progress list part icon", site_Career_RnDInProgressPartIcon.address},
    {TagKind::Strip, "in-progress list colour strip", site_Career_RnDInProgressStrip.address},
};

std::atomic<int> g_tag_log[kNumTags];

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

void retag(swofty::Context &c, Tag i)
{
    const char **arg = reinterpret_cast<const char **>(c.rsp + 0x20);
    const char *f = *arg;
    if (!f || !region_readable(f, 1))
        return;
    const char *to = nullptr;
    switch (kTags[i].kind) {
    case TagKind::DeptIcon:
        if (cstr_is(f, kErrDept))
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
    if (g_tag_log[i].fetch_add(1) < 4) {
        char was[48];
        size_t k = 0;
        for (; k + 1 < sizeof was && f[k]; ++k)
            was[k] = f[k];
        was[k] = '\0';
        swofty::log("[rnd-tyres] dept tag: %s \"%s\" -> \"%s\"", kTags[i].what, was, to);
    }
    *arg = to;
}

// Colour sites: the store of `dept <= 5 ? table[dept] : -1`, with the movsx'd department
// still in its index register:
//   RnDListRowColourStore       mov [rsp+0x3c],r8d   colour r8d, department rdx
//   RnDListRowLightColourStore  mov [rsp+0x44],ecx   colour ecx, department rdx
//   RnDPopupColourStore         mov [rsp+0x8c],ecx   colour ecx, department rax
//   RnDInProgressColourStore    mov [rsp+0x6c],eax   colour eax, department rcx
enum Colour : size_t { kListRowColour, kListRowLight, kPopupColour, kInProgressColour, kNumColours };

struct ColourSite {
    const char *what;
    uint32_t tyres;
    uintptr_t (*at)();
};

const ColourSite kColours[kNumColours] = {
    {"R&D list row colour", kColTyres, site_Career_RnDListRowColourStore.address},
    {"R&D list row light colour", kColTyresLight, site_Career_RnDListRowLightColourStore.address},
    {"Team HQ R&D popup colour", kColTyres, site_Career_RnDPopupColourStore.address},
    {"in-progress list colour", kColTyres, site_Career_RnDInProgressColourStore.address},
};

std::atomic<int> g_colour_log[kNumColours];

void recolour(uintptr_t &reg, uintptr_t dept, Colour i)
{
    if (dept != kDeptTyres || static_cast<uint32_t>(reg) != kColWhite)
        return;
    reg = (reg & ~0xffffffffull) | kColours[i].tyres;
    if (g_colour_log[i].fetch_add(1) < 4)
        swofty::log("[rnd-tyres] dept tag: %s for department 4 white -> 0x%08x", kColours[i].what, kColours[i].tyres);
}

} // namespace

SWOFTY_SITE(site_Career_RnDUpgradeListDeptIcon, swofty::Context &c)
{
    retag(c, kUpgradeList);
}

SWOFTY_SITE(site_Career_RnDListRowDeptIcon, swofty::Context &c)
{
    retag(c, kListRow);
}

SWOFTY_SITE(site_Career_UpgradeCardDeptIcon, swofty::Context &c)
{
    retag(c, kUpgradeCard);
}

SWOFTY_SITE(site_Career_RnDPerksTitleDeptIcon, swofty::Context &c)
{
    retag(c, kPerksTitle);
}

SWOFTY_SITE(site_Career_RnDPopupCategoryIcon, swofty::Context &c)
{
    retag(c, kPopup);
}

SWOFTY_SITE(site_Career_RnDResearchRowCategoryIcon, swofty::Context &c)
{
    retag(c, kResearchRow);
}

SWOFTY_SITE(site_Career_RnDDeptRowCategoryIcon, swofty::Context &c)
{
    retag(c, kDeptRow);
}

SWOFTY_SITE(site_Career_RnDInProgressPartIcon, swofty::Context &c)
{
    retag(c, kInProgressPart);
}

SWOFTY_SITE(site_Career_RnDInProgressStrip, swofty::Context &c)
{
    retag(c, kInProgressStrip);
}

SWOFTY_SITE(site_Career_RnDListRowColourStore, swofty::Context &c)
{
    recolour(c.r8, c.rdx, kListRowColour);
}

SWOFTY_SITE(site_Career_RnDListRowLightColourStore, swofty::Context &c)
{
    recolour(c.rcx, c.rdx, kListRowLight);
}

SWOFTY_SITE(site_Career_RnDPopupColourStore, swofty::Context &c)
{
    recolour(c.rcx, c.rax, kPopupColour);
}

SWOFTY_SITE(site_Career_RnDInProgressColourStore, swofty::Context &c)
{
    recolour(c.rax, c.rcx, kInProgressColour);
}

namespace rnd_dept_tags {

void install(uintptr_t base)
{
    for (const TagSite &s : kTags) {
        uintptr_t at = s.at();
        swofty::log("[rnd-tyres] dept tag: hooked %s @0x%llx (base+0x%llx)", s.what, (unsigned long long)at,
                    (unsigned long long)(at - base));
    }
    for (const ColourSite &s : kColours) {
        uintptr_t at = s.at();
        swofty::log("[rnd-tyres] dept tag: hooked %s store @0x%llx (base+0x%llx)", s.what, (unsigned long long)at,
                    (unsigned long long)(at - base));
    }
    swofty::log("[rnd-tyres] dept tags INSTALLED: %u/%u icon sites; department 4 shows the %s department icon, the %s "
                "strip, the stock category icon of its upgrade type and the Tyres orange",
                static_cast<unsigned>(kNumTags), static_cast<unsigned>(kNumTags), rnd_tyres_icon::dept(kDeptIcon),
                rnd_tyres_icon::strip(kStripFrame));
}

} // namespace rnd_dept_tags
