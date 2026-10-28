// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Host unit test for race_numbers.cpp: the checks drivers.number.set runs before it writes a number,
// and the numbers drivers.swap gives and checks after a swap.
#include <cstdio>
#include <string>

#include "race_numbers.hpp"

using namespace ce::numbers;
static int fails = 0;
#define CHECK(c) do { if (!(c)) { std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c); ++fails; } } while (0)

static uint32_t k(uint32_t id) { return id << 16; }
static std::string code(const Plan &p) { return p.error.substr(0, p.error.find(':')); }

int main() {
    // Verstappen #1, Norris #4, the player #2, an F2 free agent #7, a row without a number
    std::vector<Row> rows = {{k(9), 1}, {k(12), 4}, {k(0x3e) | 1, 2}, {k(174), 7}, {k(200), kNone}};

    Plan p = plan(rows, k(9), 33, false);
    CHECK(p.error.empty() && p.row == 0 && p.swap_with == -1 && !p.unchanged);
    // the player may change his own number
    p = plan(rows, k(0x3e) | 1, 44, false);
    CHECK(p.error.empty() && p.row == 2);
    // 1..99, whole numbers only
    for (double bad : {0.0, 100.0, -3.0, 4.5, 255.0})
        CHECK(code(plan(rows, k(9), bad, false)) == "bad_value");
    CHECK(plan(rows, k(9), 99, false).error.empty() && plan(rows, k(9), 1, false).unchanged);
    // a number another row holds, even a teamless one, is refused unless the op swaps
    CHECK(code(plan(rows, k(9), 4, false)) == "taken");
    CHECK(code(plan(rows, k(9), 7, false)) == "taken");
    p = plan(rows, k(9), 4, true);
    CHECK(p.error.empty() && p.row == 0 && p.swap_with == 1);
    // a swap needs a number to give back
    CHECK(code(plan(rows, k(200), 4, true)) == "bad_args");
    CHECK(plan(rows, k(200), 50, true).error.empty());
    // a driver without an entrant row has no number
    CHECK(code(plan(rows, k(300), 50, false)) == "no_entrant");

    // after a swap: a swapped driver whose row has a team and no number gets one from the game
    const uint32_t martins = k(146), hadjar = k(149), lawson = k(150), tsolov = k(174);
    // the in-game report: free agent Martins (no number) into Hadjar's (#6) VCARB seat; Hadjar is released
    // and keeps #6, as the game's replace keeps the released driver's number
    std::vector<Row> replaced = {{lawson, 30, true}, {martins, kNone, true}, {hadjar, 6, false}, {tsolov, 7, false}};
    CHECK(unnumbered(replaced, {martins, hadjar}) == std::vector<uint32_t>{martins});
    // a free agent who already has a number keeps it
    replaced[1].number = 41;
    CHECK(unnumbered(replaced, {martins, hadjar}).empty());
    // the released driver without a number stays without one: he has no team
    std::vector<Row> both = {{martins, kNone, true}, {hadjar, kNone, false}};
    CHECK(unnumbered(both, {martins, hadjar}) == std::vector<uint32_t>{martins});
    // two race drivers trading seats keep their numbers; one seated without a number (a 0.3.0 replace) gets one
    std::vector<Row> traded = {{lawson, 30, true}, {martins, kNone, true}, {k(9), 1, true}};
    CHECK(unnumbered(traded, {lawson, k(9)}).empty());
    CHECK(unnumbered(traded, {k(9), martins}) == std::vector<uint32_t>{martins});
    // only the swapped drivers, each once, in the swap's order
    CHECK(unnumbered(traded, {}).empty());
    CHECK(unnumbered({{martins, kNone, true}, {lawson, kNone, true}}, {lawson, martins}) ==
          (std::vector<uint32_t>{lawson, martins}));
    CHECK(unnumbered(traded, {k(300)}).empty());

    // the number the game gave: 1..99 and no other row of the championship holds it, teamless rows included
    std::vector<Row> given = {{lawson, 30, true}, {martins, 12, true}, {hadjar, 6, false}, {tsolov, 7, false}};
    CHECK(sole_holder(given, martins).empty());
    given[1].number = 7;
    CHECK(sole_holder(given, martins) == "number 7 is on another entrant row too");
    given[1].number = 6;
    CHECK(sole_holder(given, martins) == "number 6 is on another entrant row too");
    given[1].number = kNone;
    CHECK(sole_holder(given, martins) == "he has no number");
    given[1].number = 0;
    CHECK(sole_holder(given, martins) == "his number 0 is outside 1..99");
    given[1].number = 100;
    CHECK(sole_holder(given, martins) == "his number 100 is outside 1..99");
    CHECK(sole_holder(given, k(300)) == "he has no entrant row");

    std::printf("%s (%d failure(s))\n", fails ? "FAILED" : "ALL OK", fails);
    return fails ? 1 : 0;
}
