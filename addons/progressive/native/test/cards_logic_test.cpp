// SPDX-License-Identifier: AGPL-3.0-or-later

#include "cards_logic.hpp"

#include <cstdio>
#include <cstdlib>
#include <vector>

using namespace progressive::cards;

namespace {

int failures = 0;

void expect(bool ok, const char *what, int line)
{
    if (!ok) {
        std::printf("FAIL line %d: %s\n", line, what);
        ++failures;
    }
}

#define EXPECT(x) expect((x), #x, __LINE__)

struct Rec {
    uint8_t id_lo = 0;
    uint16_t id_hi = 0;
    uint32_t driver_customisation = kNoKey;
    uint32_t team_stat_id = kNoKey;
    uint32_t vehicle_ref = kNoKey;
    uint32_t vehicle_customisation = kNoKey;
    uint32_t badge_stat_id = kNoKey;
    uint16_t team_position = 0x00ff;
    uint8_t car_number = kNoNumber;
};

uint32_t team(int t)
{
    return 0x00120000u | static_cast<uint32_t>(t);
}

uint32_t car(int year, int t)
{
    return 0x00130000u | static_cast<uint32_t>((year % 100) << 8 | t);
}

Rec driver(int id)
{
    Rec r;
    r.id_lo = static_cast<uint8_t>(id);
    r.id_hi = 0x0007;
    r.driver_customisation = 0x00140000u | static_cast<uint32_t>(id);
    return r;
}

Rec seat(int id, int t, int pos, int year)
{
    Rec r = driver(id);
    r.team_stat_id = team(t);
    r.team_position = static_cast<uint16_t>(0x100 | pos);
    r.vehicle_ref = car(year, t);
    r.car_number = static_cast<uint8_t>(id);
    return r;
}

Rec season_seat(int id, int t, int pos, int year)
{
    Rec r = seat(id, t, pos, year);
    r.driver_customisation = 0x00150000u | static_cast<uint32_t>(id);
    r.car_number = static_cast<uint8_t>(100 + id);
    r.team_position = static_cast<uint16_t>(pos);
    return r;
}

// 11 teams of 2026 drivers 1..22 (team t seats 2t+1, 2t+2), free pool drivers 30..59, then two F2
// teams 20 and 21 with drivers 70..73.
std::vector<Rec> career_list()
{
    std::vector<Rec> v;
    for (int t = 0; t < 11; ++t)
        for (int p = 0; p < 2; ++p)
            v.push_back(seat(1 + 2 * t + p, t, p, 2026));
    for (int id = 30; id < 60; ++id)
        v.push_back(driver(id));
    for (int t = 20; t < 22; ++t)
        for (int p = 0; p < 2; ++p) {
            Rec r = seat(70 + 2 * (t - 20) + p, t, p, 2025);
            r.vehicle_customisation = 0x00160000u | static_cast<uint32_t>(t);
            v.push_back(r);
        }
    return v;
}

void f2_untouched(const std::vector<Rec> &career, const std::vector<Rec> &out, size_t n)
{
    for (int id = 70; id < 74; ++id) {
        const Rec *r = nullptr;
        for (size_t i = 0; i < n; ++i)
            if (out[i].id_lo == id)
                r = &out[i];
        const Rec &c = career[career.size() - 4 + static_cast<size_t>(id - 70)];
        EXPECT(r && r->team_stat_id == c.team_stat_id && r->team_position == c.team_position &&
               r->vehicle_ref == c.vehicle_ref && r->vehicle_customisation == c.vehicle_customisation);
    }
}

template <class F> int count_if(const std::vector<Rec> &v, size_t n, F f)
{
    int c = 0;
    for (size_t i = 0; i < n; ++i)
        c += f(v[i]) ? 1 : 0;
    return c;
}

const Rec *find(const std::vector<Rec> &v, size_t n, int id)
{
    for (size_t i = 0; i < n; ++i)
        if (v[i].id_lo == id)
            return &v[i];
    return nullptr;
}

void every_team_two_seats(const std::vector<Rec> &out, size_t n)
{
    for (int t = 0; t < 11; ++t) {
        int p0 = count_if(out, n, [&](const Rec &r) { return seated(r) && same_team(r.team_stat_id, team(t)) && (r.team_position & 0xff) == 0; });
        int p1 = count_if(out, n, [&](const Rec &r) { return seated(r) && same_team(r.team_stat_id, team(t)) && (r.team_position & 0xff) == 1; });
        EXPECT(p0 == 1);
        EXPECT(p1 == 1);
    }
}

void no_driver_twice(const std::vector<Rec> &out, size_t n)
{
    for (size_t i = 0; i < n; ++i)
        for (size_t j = i + 1; j < n; ++j)
            EXPECT(!same_driver(out[i], out[j]));
}

void ten_team_season()
{
    std::vector<Rec> career = career_list();
    // 2017: teams 0..9. Team 0 keeps driver 1 and takes pool driver 30; team 1 is pool 31, 32; the
    // others keep their 2026 pair. Team 10 (Cadillac) is not in the season.
    std::vector<Rec> season, first;
    season.push_back(season_seat(1, 0, 0, 2017));
    season.push_back(season_seat(30, 0, 1, 2017));
    season.push_back(season_seat(31, 1, 0, 2017));
    season.push_back(season_seat(32, 1, 1, 2017));
    for (int t = 2; t < 10; ++t)
        for (int p = 0; p < 2; ++p)
            season.push_back(season_seat(1 + 2 * t + p, t, p, 2017));
    for (int t = 0; t < 11; ++t)
        for (int p = 0; p < 2; ++p)
            first.push_back(season_seat(40 + 2 * t + p, t, p, 2016));

    std::vector<Rec> out(kMaxRows);
    Stats st{};
    size_t n = build<Rec>({career.data(), career.size()}, {season.data(), season.size()}, {first.data(), first.size()},
                          out.data(), out.size(), st);
    EXPECT(n == career.size());
    EXPECT(st.season == 20);
    EXPECT(st.missing == 0);
    EXPECT(st.teams == 11);
    EXPECT(st.filled == 2);
    every_team_two_seats(out, n);
    no_driver_twice(out, n);
    f2_untouched(career, out, n);

    const Rec *r30 = find(out, n, 30);
    EXPECT(r30 && same_team(r30->team_stat_id, team(0)) && (r30->team_position & 0xff) == 1);
    EXPECT(r30 && r30->vehicle_ref == car(2017, 0) && r30->car_number == 130 && r30->driver_customisation == 0x0015001eu);
    const Rec *r5 = find(out, n, 5);
    EXPECT(r5 && r5->vehicle_ref == car(2017, 2) && r5->car_number == 105);

    // Team 10 keeps its 2026 drivers (21, 22), with the first season's car of that slot.
    const Rec *r21 = find(out, n, 21), *r22 = find(out, n, 22);
    EXPECT(r21 && seated(*r21) && same_team(r21->team_stat_id, team(10)) && r21->vehicle_ref == car(2016, 10));
    EXPECT(r22 && seated(*r22) && same_team(r22->team_stat_id, team(10)) && r22->vehicle_ref == car(2016, 10));

    // Drivers 2, 3, 4 lost their 2026 seats to the season and are off the grid.
    for (int id : {2, 3, 4}) {
        const Rec *r = find(out, n, id);
        EXPECT(r && !seated(*r) && none(r->team_stat_id) && none(r->vehicle_ref));
    }
    EXPECT(st.benched == 3);
    // A free driver the season does not use stays free.
    const Rec *r50 = find(out, n, 50);
    EXPECT(r50 && none(r50->team_stat_id));
}

void team_short_of_season_drivers()
{
    std::vector<Rec> career = career_list();
    // Team 3's season has one driver, pool 33; its seat 1 goes to a 2026 team 3 driver who is not
    // in the season (8; 7 is in the season at team 4).
    std::vector<Rec> season;
    season.push_back(season_seat(33, 3, 0, 2019));
    season.push_back(season_seat(7, 4, 0, 2019));
    season.push_back(season_seat(9, 4, 1, 2019));
    std::vector<Rec> out(kMaxRows);
    Stats st{};
    size_t n = build<Rec>({career.data(), career.size()}, {season.data(), season.size()}, {}, out.data(), out.size(), st);
    every_team_two_seats(out, n);
    no_driver_twice(out, n);
    f2_untouched(career, out, n);
    EXPECT(st.teams == 2);
    const Rec *r8 = find(out, n, 8);
    EXPECT(r8 && seated(*r8) && same_team(r8->team_stat_id, team(3)) && (r8->team_position & 0xff) == 1);
    EXPECT(r8 && r8->vehicle_ref == car(2019, 3));
    const Rec *r7 = find(out, n, 7);
    EXPECT(r7 && same_team(r7->team_stat_id, team(4)) && (r7->team_position & 0xff) == 0);
}

void season_driver_not_in_career()
{
    std::vector<Rec> career = career_list();
    std::vector<Rec> season;
    season.push_back(season_seat(90, 0, 0, 2018));
    std::vector<Rec> out(kMaxRows);
    Stats st{};
    size_t n = build<Rec>({career.data(), career.size()}, {season.data(), season.size()}, {}, out.data(), out.size(), st);
    EXPECT(st.missing == 1);
    EXPECT(st.season == 0);
    EXPECT(find(out, n, 90) == nullptr);
    every_team_two_seats(out, n);
    f2_untouched(career, out, n);
}

void too_small_output()
{
    std::vector<Rec> career = career_list();
    std::vector<Rec> out(4);
    Stats st{};
    EXPECT(build<Rec>({career.data(), career.size()}, {}, {}, out.data(), out.size(), st) == 0);
}

void take_back_policy()
{
    EXPECT(!takes_back("career.loaded", 0));
    EXPECT(!takes_back("career.loaded", 1));
    EXPECT(!takes_back("career.unloaded", 1));
    EXPECT(!takes_back("career.hub", 0));
    EXPECT(takes_back("career.hub", 1));
    EXPECT(takes_back("career.content.pressed", 0));
    EXPECT(takes_back("career.content.pressed", -1));
    EXPECT(takes_back("career.starting", 1));
    for (std::string_view e : kEvents)
        EXPECT(e != "career.loaded");
}

void hidden_lists()
{
    bool pool = false, f2 = false, career = false, season = false;
    for (unsigned s : kHiddenLists) {
        pool = pool || s == kPoolList;
        f2 = f2 || s == kF2List;
        career = career || s == kCareerList;
        season = season || (s >= kFirstSeasonList && s < kFirstSeasonList + (kLastListYear - kFirstYear + 1));
    }
    EXPECT(pool && f2 && !career && !season);
}

} // namespace

int main()
{
    ten_team_season();
    team_short_of_season_drivers();
    season_driver_not_in_career();
    too_small_output();
    take_back_policy();
    hidden_lists();
    if (failures) {
        std::printf("cards_logic_test: %d failure(s)\n", failures);
        return EXIT_FAILURE;
    }
    std::printf("cards_logic_test: ok\n");
    return EXIT_SUCCESS;
}
