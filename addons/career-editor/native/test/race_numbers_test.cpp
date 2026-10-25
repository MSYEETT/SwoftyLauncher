// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Host unit test for race_numbers.cpp: the checks drivers.number.set runs before it writes a number.
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

    std::printf("%s (%d failure(s))\n", fails ? "FAILED" : "ALL OK", fails);
    return fails ? 1 : 0;
}
