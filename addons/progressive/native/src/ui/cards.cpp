// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career - Choose Driver and Team Select show the start season (progressive_cards.dll).
//
// Choose Driver (vt_Game_DriverCareerDriverSelectViewModel slot 13, 0x141a5adc0) takes only driver
// keys from the official lists F1CAREER_/F2CAREER_/ICONS_<suffix> (0x141a5f2d0 pushes every list's
// keys, with no check for a key it already has). Everything a card shows comes from the career's
// setup list, the game mode's +0xe18 (the registry's f1_career_progressive list): the team badge
// (0x1416a5440, overrides.teams.badges_broadcast.<team>), and the populate's copy of its rows (vm
// +0x1470) gives the focused driver's look, number and team (0x141a5d180, 0x1416a3630) and the pick
// (0x141a5df70 stores the copy's look and team). Team Select (0x141878ea0, 0x141877ec0) takes each
// team's car from its first seat in +0xe18 and the drivers it shows from the team's first two rows.
//
// So the populate of a Progressive wizard below 2026 gets a view of the setup list dressed as the
// start season (cards_logic.hpp build()). +0xe18 points at the view, a private copy of the list
// header whose row vector is ours. The view stays through Team Select: the line-up is never copied
// from it, because Game_CareerSetup_ResetEntrants and the line-up builder 0x140c2a870 both copy the
// registry's list by stat id and point +0xe18 back at it when the career is built. The view is
// also put back when the wizard is left (a career tile press or career.starting) and at Team HQ.
// During the populate the driver pool (ICONS) and the F2 list read empty, so the cards are the
// season list's drivers only.

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <cstdint>
#include <cstdio>
#include <cstring>
#include <iterator>
#include <string>
#include <string_view>

#include <swofty/events.hpp>
#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include "../cards_logic.hpp"

namespace {

using swofty::game::EntrantRecord;
namespace pc = progressive::cards;

static_assert(pc::kEventHub == SWOFTY_EVENT_CAREER_HUB);
static_assert(pc::kEventContentPressed == SWOFTY_EVENT_CAREER_CONTENT_PRESSED);
static_assert(pc::kEventStarting == SWOFTY_EVENT_CAREER_STARTING);

constexpr const char *kGroup = "f1_career_progressive";
constexpr uintptr_t kVmWizard = 0x438;
constexpr uintptr_t kVmEntrants = 0x440;
constexpr uintptr_t kVmFlow = 0x448;
constexpr uintptr_t kVmCardsBegin = 0x2c0;
constexpr uintptr_t kVmCardsEnd = 0x2c8;
constexpr size_t kCardSize = 0x170;
constexpr uintptr_t kFlowHolder = 0x88;
constexpr uintptr_t kHolderGameMode = 0x260;
constexpr uintptr_t kModeSetupList = 0xe18;
constexpr uintptr_t kWizardGroup = 0x128;
constexpr uintptr_t kWizardSuffix = 0x150;
constexpr uintptr_t kEntrantsLists = 0x560;
constexpr size_t kListSlot = 0x10;
constexpr uintptr_t kListBegin = 0x10;
constexpr uintptr_t kListEnd = 0x18;
constexpr uintptr_t kListCapacity = 0x20;
constexpr size_t kListHeader = 0x100;
constexpr size_t kMaxRows = pc::kMaxRows;

alignas(16) uint8_t g_header[kListHeader];
EntrantRecord g_rows[kMaxRows];
uintptr_t g_mode = 0;
uintptr_t g_list = 0;
int g_logged = 0;

uintptr_t view()
{
    return reinterpret_cast<uintptr_t>(g_header);
}

template <class T> T rd(uintptr_t p)
{
    T v{};
    swofty::mem::read(p, v);
    return v;
}

bool writable(uintptr_t p, size_t n)
{
    MEMORY_BASIC_INFORMATION mbi{};
    if (VirtualQuery(reinterpret_cast<const void *>(p), &mbi, sizeof mbi) != sizeof mbi || mbi.State != MEM_COMMIT ||
        (mbi.Protect & PAGE_GUARD) || !(mbi.Protect & (PAGE_READWRITE | PAGE_EXECUTE_READWRITE)))
        return false;
    return p + n <= reinterpret_cast<uintptr_t>(mbi.BaseAddress) + mbi.RegionSize;
}

bool put(uintptr_t p, uintptr_t v)
{
    if (!writable(p, sizeof v))
        return false;
    std::memcpy(reinterpret_cast<void *>(p), &v, sizeof v);
    return true;
}

std::string text_at(uintptr_t p)
{
    std::string s;
    if (p)
        swofty::mem::c_string(p, s, 64);
    return s;
}

using Rows = pc::Rows<EntrantRecord>;

bool rows_of(uintptr_t list, Rows &out)
{
    out = {};
    uintptr_t b = rd<uintptr_t>(list + kListBegin), e = rd<uintptr_t>(list + kListEnd);
    if (!list || !b || e < b || (e - b) % sizeof(EntrantRecord) || (e - b) / sizeof(EntrantRecord) > kMaxRows ||
        !swofty::mem::readable(b, e - b))
        return false;
    out.begin = reinterpret_cast<const EntrantRecord *>(b);
    out.count = (e - b) / sizeof(EntrantRecord);
    return true;
}

uintptr_t list_at(uintptr_t entrants, unsigned stat)
{
    uintptr_t b = rd<uintptr_t>(entrants + kEntrantsLists), e = rd<uintptr_t>(entrants + kEntrantsLists + 8);
    if (!b || e < b || stat >= (e - b) / kListSlot)
        return 0;
    uintptr_t list = rd<uintptr_t>(b + stat * kListSlot + 8);
    return swofty::mem::readable(list, kListHeader) ? list : 0;
}

void take_back(const char *why)
{
    if (!g_mode)
        return;
    uintptr_t setup = rd<uintptr_t>(g_mode + kModeSetupList);
    if (setup == view()) {
        bool ok = put(g_mode + kModeSetupList, g_list);
        swofty::log("[progressive] cards: %s: the setup list is the career's own again%s", why, ok ? "" : " (write failed)");
    } else if (g_logged++ < 16) {
        swofty::log("[progressive] cards: %s: the game already pointed the setup list back at %s 0x%llx", why,
                    setup == g_list ? "the career's own list" : "another list", static_cast<unsigned long long>(setup));
    }
    g_mode = 0;
}

bool dress(uintptr_t vm, uintptr_t mode, uintptr_t entrants, uintptr_t career_list, int year)
{
    unsigned stat = pc::kFirstSeasonList + static_cast<unsigned>(year - pc::kFirstYear);
    Rows career{}, season{}, first{};
    if (!rows_of(career_list, career) || !rows_of(list_at(entrants, stat), season) || !season.count) {
        if (g_logged++ < 8)
            swofty::log("[progressive] cards: %d: the career list (%u) or the season list (%u) has no rows; Choose Driver "
                        "keeps the 2026 looks",
                        year, pc::kCareerList, stat);
        return false;
    }
    rows_of(list_at(entrants, pc::kFirstSeasonList), first);
    pc::Stats st{};
    size_t n = pc::build(career, season, first, g_rows, kMaxRows, st);
    if (!n || st.season == 0)
        return false;
    std::memcpy(g_header, reinterpret_cast<const void *>(career_list), kListHeader);
    uintptr_t b = reinterpret_cast<uintptr_t>(&g_rows[0]);
    std::memcpy(g_header + kListBegin, &b, sizeof b);
    uintptr_t e = b + n * sizeof(EntrantRecord);
    std::memcpy(g_header + kListEnd, &e, sizeof e);
    uint64_t cap = n;
    std::memcpy(g_header + kListCapacity, &cap, sizeof cap);
    if (!put(mode + kModeSetupList, view()))
        return false;
    g_mode = mode;
    g_list = career_list;
    if (g_logged++ < 16)
        swofty::log("[progressive] cards: Choose Driver %d (vm 0x%llx): %zu season driver(s) on their %d seats (looks %zu, "
                    "cars %zu, numbers %zu), %zu not in the career list, %zu filler seat(s), %zu 2026 seat(s) left free, "
                    "%zu team(s), %zu row(s); kept through Team Select",
                    year, static_cast<unsigned long long>(vm), st.season, year, st.looks, st.cars, st.numbers, st.missing,
                    st.filled, st.benched, st.teams, n);
    return true;
}

int suffix_year(uintptr_t wizard)
{
    std::string s = text_at(rd<uintptr_t>(wizard + kWizardSuffix));
    if (s.size() != 4 || s[0] != 'P' || s[1] != 'G' || s[2] < '0' || s[2] > '9' || s[3] < '0' || s[3] > '9')
        return 0;
    return 2000 + (s[2] - '0') * 10 + (s[3] - '0');
}

void on_event(const SwoftyEvent *ev, void *)
{
    if (pc::takes_back(ev->name, ev->a))
        take_back(ev->name);
}

struct Hidden {
    uintptr_t list = 0, end = 0;
};

} // namespace

SWOFTY_VTABLE(vt_Game_DriverCareerDriverSelectViewModel, 13, void *, void *vm)
{
    auto at = reinterpret_cast<uintptr_t>(vm);
    uintptr_t wizard = at ? rd<uintptr_t>(at + kVmWizard) : 0;
    if (!wizard || text_at(rd<uintptr_t>(wizard + kWizardGroup)) != kGroup) {
        take_back("another wizard's Choose Driver");
        return orig(vm);
    }
    uintptr_t entrants = rd<uintptr_t>(at + kVmEntrants);
    uintptr_t holder = rd<uintptr_t>(rd<uintptr_t>(at + kVmFlow) + kFlowHolder);
    uintptr_t mode = holder ? rd<uintptr_t>(holder + kHolderGameMode) : 0;
    uintptr_t career_list = entrants ? list_at(entrants, pc::kCareerList) : 0;
    uintptr_t setup = mode ? rd<uintptr_t>(mode + kModeSetupList) : 0;
    int year = suffix_year(wizard);
    if (g_mode && g_mode != mode)
        take_back("Choose Driver of another game mode");
    bool ours = career_list && (setup == career_list || setup == view());
    if (setup == view() && career_list)
        put(mode + kModeSetupList, career_list);
    g_mode = 0;
    if (!ours && g_logged++ < 8)
        swofty::log("[progressive] cards: Choose Driver: the game mode's setup list 0x%llx is not f1_career_progressive "
                    "0x%llx; cards left as they are",
                    static_cast<unsigned long long>(setup), static_cast<unsigned long long>(career_list));
    if (ours && year >= pc::kFirstYear && year <= pc::kLastListYear)
        dress(at, mode, entrants, career_list, year);

    Hidden hidden[std::size(pc::kHiddenLists)];
    size_t hid = 0;
    for (size_t i = 0; ours && i < std::size(pc::kHiddenLists); ++i) {
        uintptr_t list = list_at(entrants, pc::kHiddenLists[i]);
        uintptr_t end = list ? rd<uintptr_t>(list + kListEnd) : 0;
        if (list && end && list != career_list && put(list + kListEnd, rd<uintptr_t>(list + kListBegin)))
            hidden[hid++] = {list, end};
    }
    void *r = orig(vm);
    for (size_t i = 0; i < hid; ++i)
        put(hidden[i].list + kListEnd, hidden[i].end);
    if (ours && g_logged++ < 16) {
        uintptr_t cb = rd<uintptr_t>(at + kVmCardsBegin), ce = rd<uintptr_t>(at + kVmCardsEnd);
        swofty::log("[progressive] cards: Choose Driver %d populated %ld card(s), %zu of %zu list(s) left out (the driver "
                    "pool and F2)",
                    year, cb && ce >= cb ? static_cast<long>((ce - cb) / kCardSize) : -1L, hid,
                    std::size(pc::kHiddenLists));
    }
    return r;
}

SWOFTY_ON_LOAD
{
    uint32_t token = 0;
    for (std::string_view ev : pc::kEvents)
        swofty::events::subscribe(ev.data(), on_event, nullptr, &token);
    swofty::log("[progressive] cards: Choose Driver and Team Select show the start season's drivers, looks and cars");
}

SWOFTY_ADDON()
