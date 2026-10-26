// SPDX-License-Identifier: AGPL-3.0-or-later
//
// The contract scene a follow_driver career skips (scene_logic.hpp): arming, the steps that passed
// through and the outro's log text.

#include "scene_logic.hpp"

#include <cstdio>

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

    if (failures) {
        std::printf("scene_logic_test: %d failure(s)\n", failures);
        return 1;
    }
    std::printf("scene_logic_test: ok\n");
    return 0;
}
