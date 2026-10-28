// career::apply_teams' planner (native/src/core/career_ops/teams_plan.hpp) on the host.

#include <cstdio>
#include <string>
#include <vector>

#include "../../src/core/career_ops/teams_plan.hpp"

using namespace career_ops::teams;

static int failures = 0;
#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d: %s\n", __FILE__, __LINE__, #c);                                                    \
            ++failures;                                                                                                \
        }                                                                                                              \
    } while (0)

static SeasonIn season(int year, int teams, int closed = 0, const char *prefix = "t")
{
    SeasonIn s;
    s.year = year;
    for (int i = 1; i <= teams; ++i) {
        if (i == closed)
            continue;
        SlotIn x;
        x.slot = i;
        x.team = prefix + std::to_string(i);
        x.seats = {"a" + std::to_string(i), "b" + std::to_string(i)};
        s.slots.push_back(x);
    }
    return s;
}

static bool mentions(const std::vector<std::string> &v, const char *text)
{
    for (auto &s : v)
        if (s.find(text) != std::string::npos)
            return true;
    return false;
}

int main()
{
    {
        Table t = build_table({season(2012, 12), season(2013, 12, 12)}, false);
        CHECK(t.ok());
        CHECK(t.count() == 12);
        CHECK(t.rows[0] == "t1" && t.rows[11] == "t12");
        Plan p = plan_season(t, season(2012, 12));
        CHECK(p.ok() && p.count(Kind::open) == 12 && p.count(Kind::closed) == 0);
        Plan q = plan_season(t, season(2013, 12, 12));
        CHECK(q.ok() && q.count(Kind::open) == 11 && q.count(Kind::closed) == 1);
        CHECK(closed_slots(q) == std::vector<int>{12});
        CHECK(q.actions[11].row == "t12");
    }
    {
        Table t = build_table({season(2013, 12, 12), season(2012, 12)}, false);
        CHECK(t.ok() && t.rows[11] == "t12" && t.first_year[11] == 2012);
        CHECK(t.first_year[0] == 2012);
    }
    {
        Table t = build_table({season(2012, 12), season(2013, 12)}, true);
        CHECK(!t.ok() && mentions(t.refusals, "My Team"));
        Table u = build_table({season(2012, 11), season(2013, 11)}, true);
        CHECK(u.ok() && u.count() == 11);
    }
    {
        Table t = build_table({season(2012, 13)}, false);
        CHECK(!t.ok() && mentions(t.refusals, "at most 12"));
    }
    {
        Table t = build_table({season(2012, 11, 0)}, false);
        SeasonIn s = season(2013, 11, 0, "x");
        Plan p = plan_season(t, s);
        CHECK(p.ok() && p.count(Kind::dressed) == 11 && p.count(Kind::open) == 0);
        CHECK(p.actions[0].row == "t1" && p.actions[0].team == "x1");
    }
    {
        Table t = build_table({season(2012, 10), season(2013, 11)}, false);
        CHECK(t.ok() && t.count() == 11 && t.first_year[10] == 2013);
        Plan p = plan_season(t, season(2012, 10));
        CHECK(p.ok() && p.count(Kind::closed) == 1 && p.actions[10].row == "t11");
    }
    {
        Table t = build_table({season(2012, 11)}, false);
        Plan p = plan_season(t, season(2013, 12));
        CHECK(!p.ok() && mentions(p.refusals, "slot 12"));
    }
    {
        SeasonIn s = season(2012, 4);
        s.slots[3].team = "t1";
        Table t = build_table({s}, false);
        CHECK(!t.ok() && mentions(t.refusals, "already on another slot"));
    }
    {
        SeasonIn s = season(2012, 3);
        s.slots.push_back(s.slots[0]);
        Table t = build_table({s}, false);
        CHECK(!t.ok() && mentions(t.refusals, "listed twice"));
    }
    {
        SeasonIn s = season(2012, 3);
        s.slots[1].team.clear();
        Table t = build_table({s}, false);
        CHECK(!t.ok() && mentions(t.refusals, "has no team to race for"));
    }
    {
        SeasonIn s = season(2012, 4);
        s.slots.erase(s.slots.begin() + 1);
        Table t = build_table({s}, false);
        CHECK(!t.ok() && mentions(t.refusals, "slot 2: no season fills it"));
    }
    {
        SeasonIn s = season(2012, 2);
        s.slots[1].slot = 0;
        Table t = build_table({s}, false);
        CHECK(!t.ok() && mentions(t.refusals, "start at 1"));
    }
    {
        SeasonIn a = season(2012, 2), b = season(2013, 2);
        b.slots[1].team = "t1";
        Table t = build_table({a, b}, false);
        CHECK(!t.ok() && mentions(t.refusals, "already on another slot"));
        SeasonIn c = season(2012, 2);
        c.slots[1].team = "t1";
        c.slots[0].team = "t1";
        Table u = build_table({c}, false);
        CHECK(!u.ok());
    }
    {
        Table t = build_table({season(2012, 3)}, false);
        Plan p = plan_season(t, season(2013, 3, 2));
        std::vector<uint32_t> teams = {100, 101, 102};
        std::vector<SeatRow> rows = {{1, 100, 0, true, false}, {2, 101, 0, true, false}, {3, 101, 1, true, false},
                                     {4, 102, 0, true, false}, {5, 101, -1, false, false}};
        Closing c = close_slots(p, teams, rows);
        CHECK(c.refusals.empty() && c.releases == (std::vector<uint32_t>{2, 3}));
        rows[1].human = true;
        Closing d = close_slots(p, teams, rows);
        CHECK(d.releases == (std::vector<uint32_t>{3}) && mentions(d.refusals, "player's seat"));
    }
    {
        Table t = build_table({season(2012, 12)}, false);
        Plan p = plan_season(t, season(2012, 12));
        CHECK(summary(p) == "12 slot(s), 12 open, 0 closed, 0 dressed");
        Plan bad = plan_season(Table{}, season(2012, 1));
        CHECK(!bad.ok());
    }
    {
        const char *stock[] = {"mercedes_26", "ferrari_26", "redbull_26", "mclaren_26", "forceindia_26", "renault_26",
                               "williams_26", "tororosso_26", "sauber_26", "haas_26", "gm_cadillac_26"};
        auto year = [&](int y, const char *slot11) {
            SeasonIn x;
            x.year = y;
            for (int i = 0; i < 11; ++i) {
                SlotIn t;
                t.slot = i + 1;
                t.team = i == 10 ? slot11 : stock[i];
                x.slots.push_back(t);
            }
            return x;
        };
        SeasonIn request = year(2016, "ig.teampack/ig_racing");
        Table t = build_table({year(2016, "ig.teampack/ig_racing"), year(2017, "gm_cadillac_26")}, false);
        CHECK(plan_season(t, request).count(Kind::open) == 11);
        std::vector<std::string> physical(stock, stock + 11);
        anchor_rows(t, physical);
        Plan p = plan_season(t, request);
        CHECK(t.ok() && p.ok() && p.count(Kind::dressed) == 1 && p.count(Kind::open) == 10);
        CHECK(p.actions[10].kind == Kind::dressed && p.actions[10].row == "gm_cadillac_26" && p.actions[10].team == "ig.teampack/ig_racing");
        CHECK(plan_season(t, year(2017, "game/gm_cadillac_26")).count(Kind::open) == 11);
        CHECK(identity_id("gm_cadillac_26") == "game/gm_cadillac_26" && identity_id("ig.teampack/ig_racing") == "ig.teampack/ig_racing");
    }
    if (failures)
        return 1;
    std::printf("teams_plan_test: slot table (12 slots, first filling season, My Team and 13 refused, shared rows), open, dressed and closed slots, closing releases and the player's seat: ok\n");
    return 0;
}
