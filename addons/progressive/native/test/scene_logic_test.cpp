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
    EXPECT(!s.on && s.steps == 0 && s.team == 0);

    if (failures) {
        std::printf("scene_logic_test: %d failure(s)\n", failures);
        return 1;
    }
    std::printf("scene_logic_test: ok\n");
    return 0;
}
