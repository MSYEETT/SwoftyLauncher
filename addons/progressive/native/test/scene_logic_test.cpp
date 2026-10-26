// SPDX-License-Identifier: AGPL-3.0-or-later
//
// The contract scene a follow_driver career skips (scene_logic.hpp): arming, the steps that passed
// through and the outro's log text.

#include "scene_logic.hpp"

#include <cstdio>
#include <cstring>
#include <map>

using namespace progressive::scene;

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

struct Heap {
    std::map<uintptr_t, uintptr_t> words;

    template <class T> bool read(uintptr_t a, T &out) const
    {
        auto it = words.find(a);
        if (it == words.end())
            return false;
        out = static_cast<T>(it->second);
        return true;
    }
};

constexpr CalendarLayout kCal{0x410, 0x418, 0x80, 0x40, 0x48, 0xc8};

} // namespace

int main()
{
    Skip s;
    EXPECT(!s.on && !s.update_skipped && s.steps == 0);
    EXPECT(s.passed() == "no step reached");

    s.arm(true, 0x01dc0000, 2017, 92);
    EXPECT(s.on && s.contract_end && s.team == 0x01dc0000 && s.year == 2017 && s.target == 92);
    s.mark(kTeamSelect);
    s.mark(kIntro);
    s.mark(kTerms);
    s.mark(kOutro);
    EXPECT(s.passed() == "team select, agent intro, contract terms, outro");
    EXPECT(s.summary() ==
           "contract scene skipped (end of his contract: team select, agent intro, contract terms, outro), signed team 476 "
           "for 2017 (one season, target 92)");

    s.update_skipped = true;
    s.arm(false, 0x01df0000, 2016, 87);
    EXPECT(s.on && !s.update_skipped && s.steps == 0 && !s.contract_end);
    s.mark(kIntro);
    s.mark(kOutro);
    s.mark(kIntro);
    EXPECT(s.summary() == "contract scene skipped (career start, no contract yet: agent intro, outro), signed team 479 "
                          "for 2016 (one season, target 87)");

    s.arm(false, 0x01df0000, 0, 80);
    EXPECT(s.summary() == "contract scene skipped (career start, no contract yet: no step reached), signed team 479 "
                          "(one season, target 80)");

    s.clear();
    EXPECT(!s.on && !s.tried && s.steps == 0 && s.team == 0);

    s.arm(true, 0x01dc0000, 2017, 92);
    EXPECT(s.on && s.tried);
    s.clear();
    s.tried = true;
    EXPECT(!s.on && s.tried);

    EXPECT(route(0x5e3c3c2d) == Route::no_contract);
    EXPECT(route(0xd2985448) == Route::contract_ends);
    EXPECT(route(0x7c9b1ec4) == Route::none);
    EXPECT(route(0) == Route::none);
    EXPECT(std::string(route_name(Route::contract_ends)) == "his contract ends: the scene starts at the team select");
    EXPECT(std::string(route_name(Route::no_contract)) == "no contract yet: the scene starts at the agent intro");
    EXPECT(std::string(route_name(Route::none)) == "no contract scene follows");

    {
        Heap h;
        constexpr uintptr_t sched = 0x1000, seasons = 0x2000, drivers0 = 0x10000, drivers1 = 0x20000;
        h.words[sched + 0x410] = seasons;
        h.words[sched + 0x418] = seasons + 2 * 0x80;
        h.words[seasons + 0x40] = drivers0;
        h.words[seasons + 0x48] = drivers0 + 22 * 0xc8;
        h.words[seasons + 0x80 + 0x40] = drivers1;
        h.words[seasons + 0x80 + 0x48] = drivers1 + 22 * 0xc8;
        EXPECT(driver_calendar(h, sched, drivers0, kCal));
        EXPECT(driver_calendar(h, sched, drivers0 + 21 * 0xc8, kCal));
        EXPECT(!driver_calendar(h, sched, drivers0 + 22 * 0xc8, kCal));
        EXPECT(!driver_calendar(h, sched, drivers0 + 0x10, kCal));
        EXPECT(driver_calendar(h, sched, drivers1 + 5 * 0xc8, kCal));
        EXPECT(!driver_calendar(h, sched, 0x30000, kCal));
        EXPECT(!driver_calendar(h, 0, drivers0, kCal));
        EXPECT(!driver_calendar(h, sched, 0, kCal));
        h.words[sched + 0x418] = seasons + 0x81;
        EXPECT(!driver_calendar(h, sched, drivers0, kCal));
        h.words[sched + 0x418] = seasons + (kMaxSeasons + 1) * 0x80;
        EXPECT(!driver_calendar(h, sched, drivers0, kCal));
        h.words.erase(sched + 0x418);
        EXPECT(!driver_calendar(h, sched, drivers0, kCal));
    }
    EXPECT(kContractEvent == 0x0f);

    if (failures) {
        std::printf("scene_logic_test: %d failure(s)\n", failures);
        return 1;
    }
    std::printf("scene_logic_test: ok\n");
    return 0;
}
