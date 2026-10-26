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
// start season: the season list pg_f1_<year>'s drivers on their season seats with the season's look,
// car, number and livery, each team left with two rows (a team the season lacks keeps its 2026 seats
// or other free drivers, with the 2016 car of that slot when there is one), everyone else off the
// grid. +0xe18 points at the view, a private copy of the list header whose row vector is ours. The
// line-up is never copied from it: Game_CareerSetup_ResetEntrants and the line-up builder 0x140c2a870
// both copy the registry's list by stat id and point +0xe18 back at it. The view is also taken back
// on a career tile press, career load and Team HQ. During the populate the ICONS list (the driver
// pool) reads empty, so a pool driver of the season is not listed twice.

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>

#include <swofty/events.hpp>
#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

namespace {

using swofty::game::EntrantRecord;

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
constexpr unsigned kCareerList = 120;
constexpr unsigned kPoolList = 121;
constexpr unsigned kFirstSeasonList = 122;
constexpr int kFirstYear = 2016;
constexpr int kLastListYear = 2025;
constexpr uint32_t kNoKey = 0x3fff0000u;
constexpr uint8_t kNoNumber = 0xff;
constexpr size_t kMaxRows = 512;
constexpr size_t kMaxTeams = 32;

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

bool none(uint32_t key)
{
    return (key & 0xffff0000u) == kNoKey;
}

bool same_driver(const EntrantRecord &a, const EntrantRecord &b)
{
    return a.id_lo == b.id_lo && a.id_hi == b.id_hi;
}

bool same_team(uint32_t a, uint32_t b)
{
    return ((a ^ b) & 0xffff00ffu) == 0;
}

bool seated(const EntrantRecord &r)
{
    return !none(r.team_stat_id) && (r.team_position >> 8) && (r.team_position & 0xff) < 2;
}

struct Rows {
    const EntrantRecord *begin = nullptr;
    size_t count = 0;
};

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
    if (rd<uintptr_t>(g_mode + kModeSetupList) == view()) {
        bool ok = put(g_mode + kModeSetupList, g_list);
        swofty::log("[progressive] cards: %s: the setup list is the career's own again%s", why, ok ? "" : " (write failed)");
    }
    g_mode = 0;
}

struct Stats {
    size_t season = 0, missing = 0, looks = 0, cars = 0, numbers = 0, filled = 0, benched = 0, teams = 0;
};

size_t build(const Rows &career, const Rows &season, const Rows &first_season, Stats &st)
{
    bool used[kMaxRows] = {};
    uint32_t teams[kMaxTeams];
    size_t team_count = 0, n = 0;
    uint16_t free_position = 0x00ff;
    for (size_t i = 0; i < career.count; ++i) {
        const EntrantRecord &c = career.begin[i];
        if (none(c.team_stat_id))
            free_position = c.team_position;
        if (!seated(c))
            continue;
        bool known = false;
        for (size_t t = 0; t < team_count; ++t)
            known = known || same_team(teams[t], c.team_stat_id);
        if (!known && team_count < kMaxTeams)
            teams[team_count++] = c.team_stat_id;
    }
    st.teams = team_count;

    auto find = [&](const EntrantRecord &y) -> size_t {
        for (size_t i = 0; i < career.count; ++i)
            if (!used[i] && same_driver(career.begin[i], y))
                return i;
        return career.count;
    };
    auto car_for = [&](uint32_t team, uint32_t fallback) -> uint32_t {
        for (const Rows *rows : {&season, &first_season})
            for (size_t i = 0; i < rows->count; ++i)
                if (same_team(rows->begin[i].team_stat_id, team) && !none(rows->begin[i].vehicle_ref))
                    return rows->begin[i].vehicle_ref;
        return fallback;
    };

    for (size_t t = 0; t < team_count && n < kMaxRows; ++t) {
        uint32_t team = teams[t];
        int placed = 0;
        for (int pos = 0; pos < 2; ++pos)
            for (size_t k = 0; k < season.count && n < kMaxRows; ++k) {
                const EntrantRecord &y = season.begin[k];
                if (!same_team(y.team_stat_id, team) || (y.team_position & 0xff) != pos)
                    continue;
                size_t i = find(y);
                if (i == career.count) {
                    ++st.missing;
                    continue;
                }
                EntrantRecord r = career.begin[i];
                if (!none(y.driver_customisation)) {
                    r.driver_customisation = y.driver_customisation;
                    ++st.looks;
                }
                if (!none(y.vehicle_ref)) {
                    r.vehicle_ref = y.vehicle_ref;
                    ++st.cars;
                }
                r.vehicle_customisation = y.vehicle_customisation;
                if (y.car_number != kNoNumber) {
                    r.car_number = y.car_number;
                    ++st.numbers;
                }
                r.team_stat_id = y.team_stat_id;
                r.team_position = static_cast<uint16_t>(0x100 | placed);
                g_rows[n++] = r;
                used[i] = true;
                ++placed;
                ++st.season;
            }
        for (int pass = 0; pass < 2 && placed < 2; ++pass)
            for (size_t i = 0; i < career.count && placed < 2 && n < kMaxRows; ++i) {
                const EntrantRecord &c = career.begin[i];
                bool fits = pass == 0 ? seated(c) && same_team(c.team_stat_id, team) : none(c.team_stat_id);
                if (used[i] || !fits)
                    continue;
                bool in_season = false;
                for (size_t k = 0; k < season.count; ++k)
                    in_season = in_season || same_driver(season.begin[k], c);
                if (in_season)
                    continue;
                EntrantRecord r = c;
                r.team_stat_id = team;
                r.team_position = static_cast<uint16_t>(0x100 | placed);
                r.vehicle_ref = car_for(team, seated(c) ? c.vehicle_ref : kNoKey);
                if (none(r.vehicle_ref))
                    for (size_t j = 0; j < career.count; ++j)
                        if (seated(career.begin[j]) && same_team(career.begin[j].team_stat_id, team))
                            r.vehicle_ref = career.begin[j].vehicle_ref;
                r.vehicle_customisation = kNoKey;
                g_rows[n++] = r;
                used[i] = true;
                ++placed;
                ++st.filled;
            }
    }
    for (size_t i = 0; i < career.count && n < kMaxRows; ++i) {
        if (used[i])
            continue;
        EntrantRecord r = career.begin[i];
        if (seated(r)) {
            r.team_stat_id = kNoKey;
            r.vehicle_ref = kNoKey;
            r.vehicle_customisation = kNoKey;
            r.badge_stat_id = kNoKey;
            r.team_position = free_position;
            ++st.benched;
        }
        g_rows[n++] = r;
    }
    return n;
}

bool dress(uintptr_t vm, uintptr_t mode, uintptr_t entrants, uintptr_t career_list, int year)
{
    uintptr_t season_list = list_at(entrants, kFirstSeasonList + static_cast<unsigned>(year - kFirstYear));
    uintptr_t first_list = list_at(entrants, kFirstSeasonList);
    Rows career{}, season{}, first{};
    if (!rows_of(career_list, career) || !rows_of(season_list, season) || !season.count) {
        if (g_logged++ < 8)
            swofty::log("[progressive] cards: %d: the career list (%u) or the season list (%u) has no rows; Choose Driver "
                        "keeps the 2026 looks",
                        year, kCareerList, kFirstSeasonList + static_cast<unsigned>(year - kFirstYear));
        return false;
    }
    rows_of(first_list, first);
    Stats st{};
    size_t n = build(career, season, first, st);
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
                    "%zu team(s), %zu row(s)",
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
    if (!std::strcmp(ev->name, SWOFTY_EVENT_CAREER_HUB) && ev->a != 1)
        return;
    take_back(ev->name);
}

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
    uintptr_t career_list = entrants ? list_at(entrants, kCareerList) : 0;
    uintptr_t pool = entrants ? list_at(entrants, kPoolList) : 0;
    uintptr_t setup = mode ? rd<uintptr_t>(mode + kModeSetupList) : 0;
    int year = suffix_year(wizard);
    take_back("Choose Driver");
    bool ours = career_list && (setup == career_list || setup == view());
    if (setup == view() && career_list)
        put(mode + kModeSetupList, career_list);
    if (!ours && g_logged++ < 8)
        swofty::log("[progressive] cards: Choose Driver: the game mode's setup list 0x%llx is not f1_career_progressive "
                    "0x%llx; cards left as they are",
                    static_cast<unsigned long long>(setup), static_cast<unsigned long long>(career_list));
    if (ours && year >= kFirstYear && year <= kLastListYear)
        dress(at, mode, entrants, career_list, year);

    uintptr_t pool_end = pool ? rd<uintptr_t>(pool + kListEnd) : 0;
    bool hid = ours && pool && pool_end && put(pool + kListEnd, rd<uintptr_t>(pool + kListBegin));
    void *r = orig(vm);
    if (hid)
        put(pool + kListEnd, pool_end);
    if (ours && g_logged++ < 16) {
        uintptr_t cb = rd<uintptr_t>(at + kVmCardsBegin), ce = rd<uintptr_t>(at + kVmCardsEnd);
        swofty::log("[progressive] cards: Choose Driver %d populated %ld card(s)%s", year,
                    cb && ce >= cb ? static_cast<long>((ce - cb) / kCardSize) : -1L,
                    hid ? ", the driver pool (ICONS) left out" : "");
    }
    return r;
}

SWOFTY_ON_LOAD
{
    uint32_t token = 0;
    for (const char *ev : {SWOFTY_EVENT_CAREER_LOADED, SWOFTY_EVENT_CAREER_HUB, SWOFTY_EVENT_CAREER_CONTENT_PRESSED})
        swofty::events::subscribe(ev, on_event, nullptr, &token);
    swofty::log("[progressive] cards: Choose Driver and Team Select show the start season's drivers, looks and cars");
}

SWOFTY_ADDON()
