// SPDX-License-Identifier: AGPL-3.0-or-later
//
// The season plan over the committed data/years.json and a career shaped like a new Progressive
// career: the stock 2026 grid on the race seats, every other pool driver a free agent, two
// fictional reserves.

#include "season_logic.hpp"

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>

using namespace progressive;
using season::CareerRow;

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

season::Data g_data;

int stat(const std::string &label)
{
    const season::PoolDriver *p = g_data.driver(label);
    return p ? p->stat : -1;
}

season::Resolve resolver(bool cs_installed = true)
{
    season::Resolve r;
    r.driver_stat = stat;
    r.vehicle_stat = [cs_installed](const std::string &label) {
        if (label.rfind("cs", 0) == 0)
            return cs_installed ? 1000 + static_cast<int>(std::hash<std::string>{}(label) % 1000) : -1;
        for (auto &s : g_data.slots)
            if (s.vehicle_2026 == label)
                return s.vehicle_2026_stat;
        return -1;
    };
    r.look_known = [cs_installed](int s) { return cs_installed || s < 420 || s >= 700; };
    r.rating = [](uint32_t) { return 75.0; };
    return r;
}

std::vector<CareerRow> career(const std::string &player)
{
    std::vector<CareerRow> rows;
    const season::Year *y = g_data.year(2026);
    std::vector<int> seated;
    for (int i = 0; i < season::kSlots; ++i)
        for (auto &d : y->teams[i].drivers) {
            CareerRow r;
            r.driver = static_cast<uint32_t>(stat(d.driver));
            r.team = g_data.slots[i].team_stat;
            r.position = d.position;
            r.car = true;
            r.human = d.driver == player;
            r.number = static_cast<uint8_t>(d.number);
            rows.push_back(r);
            seated.push_back(stat(d.driver));
        }
    for (auto &p : g_data.pool)
        if (std::find(seated.begin(), seated.end(), p.stat) == seated.end()) {
            CareerRow r;
            r.driver = static_cast<uint32_t>(p.stat);
            r.number = p.number > 0 ? static_cast<uint8_t>(p.number) : season::kNoNumber;
            rows.push_back(r);
        }
    rows.push_back({900, g_data.slots[0].team_stat, 2, false, false, season::kNoNumber});
    rows.push_back({901, g_data.slots[1].team_stat, 2, false, false, season::kNoNumber});
    return rows;
}

uint32_t at(const season::Plan &p, int slot1, int seat)
{
    return p.target[slot1 - 1][seat];
}

bool seated_once(const season::Plan &p)
{
    std::vector<uint32_t> all;
    for (auto &s : p.target)
        for (uint32_t d : s) {
            if (d == lineup::kNone || std::find(all.begin(), all.end(), d) != all.end())
                return false;
            all.push_back(d);
        }
    return all.size() == 22;
}

void years()
{
    EXPECT(season::year_for(2016, 0) == 2016);
    EXPECT(season::year_for(2016, 4) == 2020);
    EXPECT(season::year_for(2016, 10) == 2026);
    EXPECT(season::year_for(2016, 15) == 2026);
    EXPECT(season::year_for(2026, 3) == 2026);
    EXPECT(season::year_for(2010, 0) == 2016);
    EXPECT(season::year_for(2030, 0) == 2026);
}

void parsed()
{
    EXPECT(g_data.years.size() == 11);
    EXPECT(g_data.slots[0].team == "mercedes_26");
    EXPECT(g_data.slots[10].team == "gm_cadillac_26");
    EXPECT(g_data.year(2016)->teams[0].drivers.size() == 2);
    EXPECT(g_data.year(2017)->teams[10].kind == "filler");
    EXPECT(g_data.year(2017)->teams[10].drivers.empty());
    EXPECT(stat("lewis_hamilton") == 7);
}

void stock_2026_unchanged_for_its_own_year()
{
    auto rows = career("lando_norris");
    season::Plan p = season::plan(g_data, 2026, rows, resolver());
    EXPECT(p.error.empty());
    EXPECT(p.moves.empty());
    EXPECT(p.releases.empty());
    EXPECT(seated_once(p));
    for (auto &v : p.vehicles)
        EXPECT(v.from_year == 2026);
}

void hamilton_at_ferrari_in_2016()
{
    auto rows = career("lewis_hamilton");
    season::Plan p = season::plan(g_data, 2016, rows, resolver());
    EXPECT(p.error.empty());
    EXPECT(seated_once(p));
    EXPECT(p.player == 7u);
    EXPECT(p.player_slot == 1);
    EXPECT(at(p, 2, 0) == static_cast<uint32_t>(stat("sebastian_vettel")) ||
           at(p, 2, 1) == static_cast<uint32_t>(stat("sebastian_vettel")));
    EXPECT(at(p, 2, 0) == 7u || at(p, 2, 1) == 7u);
    uint32_t kimi = static_cast<uint32_t>(stat("kimi_raikkonen"));
    EXPECT(p.fit.displaced == kimi);
    EXPECT(at(p, 1, 0) == kimi || at(p, 1, 1) == kimi);
    EXPECT(at(p, 1, 0) == static_cast<uint32_t>(stat("nico_rosberg")) ||
           at(p, 1, 1) == static_cast<uint32_t>(stat("nico_rosberg")));
    for (auto &m : p.moves)
        EXPECT(m.driver != 7u);
    bool norris_out = false;
    for (uint32_t d : p.releases)
        norris_out = norris_out || d == static_cast<uint32_t>(stat("lando_norris"));
    EXPECT(norris_out);
    uint32_t rosberg = static_cast<uint32_t>(stat("nico_rosberg"));
    bool rosberg_in = false;
    for (auto &m : p.moves)
        rosberg_in = rosberg_in || (m.driver == rosberg && m.from.slot < 0 && m.to.slot == 0);
    EXPECT(rosberg_in);
    bool hadjar_cleared = false;
    for (auto &n : p.numbers)
        hadjar_cleared = hadjar_cleared || (n.driver == static_cast<uint32_t>(stat("isack_hadjar")) && n.number == season::kNoNumber);
    EXPECT(hadjar_cleared);
    EXPECT(p.vehicles[0].label == "cs16_mercedes");
    EXPECT(p.vehicles[10].label == "cs16_manor");
    bool kimi_look = false;
    for (auto &l : p.looks)
        kimi_look = kimi_look || (l.driver == kimi && l.label.find("2016-cs_ferrari_kimi") == 0);
    EXPECT(kimi_look);
}

void follow_mode_moves_the_player_in_the_first_season()
{
    auto rows = career("lewis_hamilton");
    season::Plan p = season::plan(g_data, 2016, rows, resolver(), true);
    EXPECT(p.error.empty());
    EXPECT(p.player_moves);
    EXPECT(seated_once(p));
    EXPECT(at(p, 1, 0) == 7u || at(p, 1, 1) == 7u);
    uint32_t vettel = static_cast<uint32_t>(stat("sebastian_vettel")), kimi = static_cast<uint32_t>(stat("kimi_raikkonen"));
    EXPECT((at(p, 2, 0) == vettel && at(p, 2, 1) == kimi) || (at(p, 2, 0) == kimi && at(p, 2, 1) == vettel));
    EXPECT(p.fit.displaced == lineup::kNone);
    bool moved = false;
    for (auto &m : p.moves)
        moved = moved || (m.driver == 7u && m.from.slot == 1 && m.to.slot == 0);
    EXPECT(moved);
    season::Plan same = season::plan(g_data, 2016, career("max_verstappen"), resolver(), true);
    EXPECT(!same.player_moves);
    for (auto &m : same.moves)
        EXPECT(m.driver != static_cast<uint32_t>(stat("max_verstappen")));
}

void ten_team_year_fills_manor()
{
    auto rows = career("lando_norris");
    season::Plan p = season::plan(g_data, 2017, rows, resolver());
    EXPECT(p.error.empty());
    EXPECT(seated_once(p));
    EXPECT(at(p, 11, 0) != lineup::kNone && at(p, 11, 1) != lineup::kNone);
    EXPECT(p.vehicles[10].label == "cs16_manor");
}

void missing_year_addon_falls_back()
{
    auto rows = career("lando_norris");
    season::Plan p = season::plan(g_data, 2018, rows, resolver(false));
    EXPECT(p.error.empty());
    EXPECT(p.vehicles[0].label == "mercedes_2026");
    for (auto &l : p.looks)
        EXPECT(l.stat >= 0 && (l.stat < 420 || l.stat >= 700));
}

void hybrid_budgets_follow_pace()
{
    EXPECT(season::hybrid_budget(g_data, 2016, 0, 1000) == 1000);
    int ferrari = season::hybrid_budget(g_data, 2016, 1, 1000);
    EXPECT(ferrari == 1466);
    int manor16 = season::hybrid_budget(g_data, 2016, 10, 1000);
    int manor17 = season::hybrid_budget(g_data, 2017, 10, 1000);
    EXPECT(manor16 > ferrari);
    int slowest17 = 1000;
    for (int i = 0; i < 10; ++i)
        slowest17 = std::max(slowest17, season::hybrid_budget(g_data, 2017, i, 1000));
    EXPECT(manor17 == slowest17);
}

void numbers_never_collide()
{
    auto rows = career("lando_norris");
    season::Plan p = season::plan(g_data, 2019, rows, resolver());
    std::map<uint32_t, uint8_t> num;
    for (auto &r : rows)
        num[r.driver] = r.number;
    for (auto &n : p.numbers)
        num[n.driver] = n.number;
    std::map<uint8_t, int> seen;
    for (auto &[d, n] : num)
        if (n != season::kNoNumber)
            ++seen[n];
    for (auto &s : p.target)
        for (uint32_t d : s)
            if (num[d] != season::kNoNumber)
                EXPECT(seen[num[d]] == 1);
}

} // namespace

int main(int argc, char **argv)
{
    const char *path = argc > 1 ? argv[1] : "../../data/years.json";
    std::ifstream f(path);
    std::stringstream ss;
    ss << f.rdbuf();
    std::string err;
    if (!season::parse(ss.str(), g_data, err)) {
        std::printf("season_logic_test: cannot parse %s: %s\n", path, err.c_str());
        return EXIT_FAILURE;
    }
    years();
    parsed();
    stock_2026_unchanged_for_its_own_year();
    hamilton_at_ferrari_in_2016();
    follow_mode_moves_the_player_in_the_first_season();
    ten_team_year_fills_manor();
    missing_year_addon_falls_back();
    numbers_never_collide();
    hybrid_budgets_follow_pace();
    if (failures) {
        std::printf("season_logic_test: %d failure(s)\n", failures);
        return EXIT_FAILURE;
    }
    std::printf("season_logic_test: ok\n");
    return EXIT_SUCCESS;
}
