// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career - Choose Driver lists the start year's drivers (progressive_suffix.dll).
//
// The career tile router (FeTile_Route_CareerItem 0x141725620, decomp lines 135-150) copies the
// tile item's suffix (item +0x188, an NeStr, text at +0x09) into the wizard state (item +0x150):
// a char buffer {+0x148 capacity, +0x150 chars} that grows when the text does not fit. The
// driver select formats F1CAREER_%s, F2CAREER_%s and ICONS_%s with the suffix. The tile registers
// PG26; after the router, and after every wizard step (the setup page sets the start season after
// the press), a wizard of f1_career_progressive whose suffix reads PG + two digits gets the start
// season's two digits written over them, in place: same length, inside the capacity, so nothing is
// reallocated. Lists F1CAREER_PG16 .. F1CAREER_PG26 come with the addon's data.

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <cstdlib>
#include <cstring>
#include <string>

#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

SWOFTY_SETTING(std::string_view, start_season, "2026");

namespace {

constexpr const char *kGroup = "f1_career_progressive";
constexpr uintptr_t kItemDataGroup = 0x180;  // career content item: data group NeStr
constexpr uintptr_t kItemWizard = 0x150;     // career content item [0x2a]: the wizard state
constexpr uintptr_t kWizardGroup = 0x128;    // wizard state: data group chars
constexpr uintptr_t kWizardSuffixCap = 0x148; // wizard state: suffix buffer capacity
constexpr uintptr_t kWizardSuffix = 0x150;   // wizard state: suffix buffer chars
constexpr uintptr_t kNeStrChars = 0x09;

int g_logged = 0;

bool writable(uintptr_t p, size_t n)
{
    MEMORY_BASIC_INFORMATION mbi{};
    if (VirtualQuery(reinterpret_cast<const void *>(p), &mbi, sizeof mbi) != sizeof mbi || mbi.State != MEM_COMMIT ||
        (mbi.Protect & PAGE_GUARD) || !(mbi.Protect & (PAGE_READWRITE | PAGE_EXECUTE_READWRITE)))
        return false;
    return p + n <= reinterpret_cast<uintptr_t>(mbi.BaseAddress) + mbi.RegionSize;
}

std::string text_at(uintptr_t p)
{
    std::string s;
    if (p)
        swofty::mem::c_string(p, s, 64);
    return s;
}

int start_year()
{
    std::string v(start_season.get());
    int y = std::atoi(v.c_str());
    return y < 2016 ? 2016 : y > 2026 ? 2026 : y;
}

void rewrite(uintptr_t wizard, const char *when)
{
    uintptr_t group = 0, chars = 0;
    uint64_t cap = 0;
    if (!wizard || !swofty::mem::read(wizard + kWizardGroup, group) || text_at(group) != kGroup ||
        !swofty::mem::read(wizard + kWizardSuffixCap, cap) || !swofty::mem::read(wizard + kWizardSuffix, chars))
        return;
    std::string now = text_at(chars);
    int year = start_year();
    char want[8];
    std::snprintf(want, sizeof want, "PG%02d", year % 100);
    bool shape = now.size() == 4 && now[0] == 'P' && now[1] == 'G' && now[2] >= '0' && now[2] <= '9' && now[3] >= '0' &&
                 now[3] <= '9';
    if (now == want)
        return;
    if (!shape || cap < 5 || !writable(chars + 2, 2)) {
        if (g_logged++ < 8)
            swofty::log("[progressive] suffix: %s: wizard suffix '%s' (capacity %llu) left alone; Choose Driver keeps it",
                        when, now.c_str(), static_cast<unsigned long long>(cap));
        return;
    }
    std::memcpy(reinterpret_cast<void *>(chars + 2), want + 2, 2);
    swofty::log("[progressive] suffix: %s: %s -> %s (start season %d): Choose Driver lists F1CAREER_%s", when, now.c_str(),
                want, year, want);
}

} // namespace

SWOFTY_WRAP(FeTile_Route_CareerItem, swofty::game::Action *, swofty::game::HomeItem *item, swofty::game::Action *out)
{
    swofty::game::Action *r = orig(item, out);
    auto at = reinterpret_cast<uintptr_t>(item);
    uintptr_t group = 0, wizard = 0;
    if (at && swofty::mem::read(at + kItemDataGroup, group) && group && text_at(group + kNeStrChars) == kGroup &&
        swofty::mem::read(at + kItemWizard, wizard))
        rewrite(wizard, "tile press");
    return r;
}

SWOFTY_WRAP(Game_CareerWizard_PushNextStep, void, void *wizard)
{
    orig(wizard);
    rewrite(reinterpret_cast<uintptr_t>(wizard), "wizard step");
}

SWOFTY_ON_LOAD
{
    swofty::log("[progressive] suffix: Choose Driver follows the start season (now %d)", start_year());
}

SWOFTY_ADDON()
