// Host unit test for the core's career tracker (career_state.hpp): flow-state transitions, the Team
// HQ answer and the component poll go in, the career events come out of settle(). The clock is a
// plain millisecond value. The Driver Career sequence is the one a real run logged: components say
// driver_career before Team HQ opens, then StateTeamHQ -> StateTeamHQUpdate Updating, then the Team
// HQ answer "not My Team". run_events_test.sh builds and runs it.
#include "../src/core/career_state.hpp"

#include <cstdio>
#include <string>

using runtime::career::Components;
using runtime::career::djb2;
using runtime::career::Edges;
using runtime::career::Tracker;

static int fails;

static void expect(bool ok, const std::string &what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what.c_str());
    fails += !ok;
}

static void go(Tracker &t, const char *state, uint32_t from, uint32_t to, uint64_t now)
{
    t.flow(djb2(state), from, to, state, true, now);
}

static bool quiet(const Edges &e)
{
    return !e.loaded && !e.unloaded && e.hub < 0;
}

int main()
{
    {
        Tracker t;
        expect(!t.state().loaded && t.mode() == SWOFTY_CAREER_NONE && !t.last(), "fresh tracker: no career, no flow");
        expect(quiet(t.settle()), "fresh tracker publishes nothing");

        Components dc;
        dc.entity = dc.career = true;
        dc.mode = SWOFTY_CAREER_DRIVER;
        dc.humans = 1;
        dc.player = true;
        dc.driver = 0x00ab0000u;
        dc.team = 0x00070000u;
        t.components(dc, 1000);
        Edges e = t.settle();
        SwoftyCareerState s = t.state();
        expect(e.loaded && !e.unloaded && e.hub < 0, "driver career: career.loaded from the components alone");
        expect(s.mode == SWOFTY_CAREER_DRIVER && s.components == SWOFTY_CAREER_COMPONENTS_LIVE &&
                   s.team_hq_mode == SWOFTY_CAREER_NONE,
               "driver career: mode from components, Team HQ answer none");
        expect(std::string(s.key) == "dc-00ab0000", "driver career key dc-<driver>");

        go(t, "StateTeamHQ", 0, 1, 1100);
        go(t, "StateTeamHQ", 1, 2, 1150);
        go(t, "StateTeamHQUpdate", 0, 1, 1200);
        expect(quiet(t.settle()), "Team HQ entering: no hub yet");
        go(t, "StateTeamHQUpdate", 1, 2, 1300);
        e = t.settle();
        expect(e.hub == 1 && !e.loaded, "StateTeamHQUpdate Updating: career.hub entered");
        t.team_hq_answer(false);
        expect(quiet(t.settle()) && t.state().team_hq_mode == SWOFTY_CAREER_DRIVER &&
                   t.mode() == SWOFTY_CAREER_DRIVER,
               "Team HQ answers not My Team: still driver career, no churn");

        uint32_t st = 0;
        uint64_t age = 0;
        expect(t.status_of(djb2("StateTeamHQUpdate"), st, age, 2050) && st == SWOFTY_FLOW_UPDATING && age == 750,
               "flow_status: status and age");
        expect(!t.status_of(djb2("StateNeverSeen"), st, age, 2050) && st == SWOFTY_FLOW_NONE, "unseen state: none");
        expect(t.last() && t.last()->hash == djb2("StateTeamHQUpdate") && t.last()->to == SWOFTY_FLOW_UPDATING &&
                   t.last()->transitions == 4,
               "last flow transition and count");

        go(t, "StateTeamHQToSession", 0, 1, 3000);
        e = t.settle();
        expect(e.hub == 0 && !e.unloaded, "time to a session: career.hub left, career stays loaded");
        go(t, "StateTeamHQToSession", 1, 0, 3100);
        go(t, "StateTeamHQUpdate", 2, 0, 3100);
        go(t, "StateTeamHQ", 2, 0, 3200);
        go(t, "StateSession", 0, 2, 3300);
        expect(quiet(t.settle()) && t.loaded(), "in the session the career is still loaded");

        Components none;
        t.components(none, 9000);
        go(t, "StateSession", 2, 0, 9000);
        e = t.settle();
        expect(e.unloaded && e.old_mode == SWOFTY_CAREER_DRIVER && !e.loaded, "components gone: career.unloaded");
        expect(!t.state().loaded && !t.state().key[0], "unloaded: no key");
    }

    {
        Tracker t;
        Components mt;
        mt.entity = mt.career = true;
        mt.mode = SWOFTY_CAREER_MY_TEAM;
        mt.humans = 1;
        mt.player = true;
        mt.driver = 0x00110000u;
        mt.team = 0x00ff0000u;
        go(t, "StateTeamHQ", 0, 1, 10);
        go(t, "StateTeamHQUpdate", 0, 2, 20);
        t.components(mt, 30);
        Edges e = t.settle();
        expect(e.loaded && e.hub == 1, "my team: loaded then hub in one settle");
        expect(std::string(t.state().key) == "mt-00ff0000-00110000", "my team key mt-<team>-<driver>");
        go(t, "StateTeamHQTimeAdvance", 0, 1, 40);
        expect(t.settle().hub == 0, "time advance: hub left");
        go(t, "StateTeamHQTimeAdvance", 1, 0, 50);
        expect(t.settle().hub == 1, "time advance done: hub entered again");

        Components other = mt;
        other.team = 0x00010000u;
        t.components(other, 60);
        e = t.settle();
        expect(e.hub == 0 && e.unloaded && e.loaded, "another career key: hub left, unloaded, loaded");
        expect(t.settle().hub == 1, "and hub entered again for the new career");
    }

    {
        Tracker t;
        go(t, "StateTeamHQ", 0, 1, 0);
        expect(t.flow_career() && t.loaded() && t.mode() == SWOFTY_CAREER_NONE, "flow only: loaded, mode unknown");
        expect(quiet(t.settle()), "no career.loaded while the mode is unknown");
        t.team_hq_answer(true);
        Edges e = t.settle();
        expect(e.loaded && t.mode() == SWOFTY_CAREER_MY_TEAM && !t.state().key[0],
               "flow only: the Team HQ answer decides the mode, no key without components");
        go(t, "StateTeamHQ", 1, 0, 100);
        e = t.settle();
        expect(e.unloaded && !t.loaded() && t.state().team_hq_mode == SWOFTY_CAREER_NONE,
               "leaving Team HQ without a session clears the flow career");
    }

    {
        Tracker t;
        Components entity_only;
        entity_only.entity = true;
        go(t, "StateTeamHQ", 0, 1, 0);
        go(t, "StateTeamHQUpdate", 0, 2, 0);
        t.team_hq_answer(false);
        t.components(entity_only, 100);
        expect(!t.loaded(), "entity without a CareerComponent: not loaded while within the grace period");
        t.components(entity_only, 100 + runtime::career::COMPONENT_GRACE_MS + 1);
        expect(t.loaded() && t.state().components == SWOFTY_CAREER_COMPONENTS_FALLBACK &&
                   t.mode() == SWOFTY_CAREER_DRIVER,
               "components broken in Team HQ: fall back to the flow states and the Team HQ answer");
    }

    {
        Tracker t;
        for (uint32_t i = 0; i < runtime::career::MAX_STATES + 10; i++) {
            char n[32];
            snprintf(n, sizeof n, "StateX%u", i);
            t.flow(djb2(n), 0, 1, n, true, i);
        }
        uint32_t st;
        uint64_t age;
        expect(t.status_of(djb2("StateX0"), st, age, 0) && !t.status_of(djb2("StateX300"), st, age, 0),
               "state table is bounded");
        t.flow(djb2("StateTeamHQUpdate"), 0, 2, "StateTeamHQUpdate", true, 0);
        expect(t.status(runtime::career::TeamHqUpdate) == SWOFTY_FLOW_NONE,
               "a full table drops new states (documented limit)");
    }

    printf("%s: %d failure(s)\n", fails ? "FAIL" : "ok  ", fails);
    return fails ? 1 : 0;
}
