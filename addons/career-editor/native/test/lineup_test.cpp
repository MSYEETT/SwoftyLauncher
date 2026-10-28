// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Host unit test for lineup.cpp: the seat-count check driver swaps run before they commit, and the
// contract a driver signs when he takes a seat.
#include <cstdio>
#include <string>

#include "lineup.hpp"

using namespace ce::lineup;
static int fails = 0;
#define CHECK(c) do { if (!(c)) { std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c); ++fails; } } while (0)

static uint32_t k(uint32_t id) { return id << 16; }

int main() {
    const uint32_t vcarb = k(482), redbull = k(477), none = kNone;
    const uint32_t car1 = k(0xda), car2 = k(0xdb), car3 = k(0xd2);
    const uint32_t tsolov = k(174), hadjar = k(149), lawson = k(150), other = k(194);
    std::vector<Seat> before = {{lawson, vcarb, car1}, {hadjar, vcarb, car2}, {tsolov, none, none}, {other, redbull, car3}};

    // the dar1us replace done right: the free agent takes the released driver's car
    std::vector<Seat> after = {{lawson, vcarb, car1}, {tsolov, vcarb, car2}, {hadjar, none, none}, {other, redbull, car3}};
    CHECK(seats_kept(before, after).empty());

    // a replace that loses the second VCARB car is refused
    std::vector<Seat> lost = {{lawson, vcarb, car1}, {tsolov, none, none}, {hadjar, none, none}, {other, redbull, car3}};
    CHECK(seats_kept(before, lost) == "team 482 would race 1 car(s) instead of 2");

    // the incoming row keeping no car but the team (a reserve) is the same loss
    std::vector<Seat> reserve = {{lawson, vcarb, car1}, {tsolov, vcarb, none}, {hadjar, none, none}, {other, redbull, car3}};
    CHECK(seats_kept(before, reserve) == "team 482 would race 1 car(s) instead of 2");

    // two race drivers trading seats keep every team's cars
    std::vector<Seat> swapped = {{lawson, vcarb, car1}, {other, vcarb, car2}, {tsolov, none, none}, {hadjar, redbull, car3}};
    CHECK(seats_kept(before, swapped).empty());

    // one car on two rows, or a car moving to another team, is caught even when the counts match
    std::vector<Seat> doubled = {{lawson, vcarb, car1}, {tsolov, vcarb, car1}, {hadjar, none, none}, {other, redbull, car3}};
    CHECK(seats_kept(before, doubled) == "car 0xda0000 of team 482 would have two drivers");
    std::vector<Seat> moved = {{lawson, vcarb, car1}, {tsolov, vcarb, car3}, {hadjar, none, none}, {other, redbull, car2}};
    CHECK(!seats_kept(before, moved).empty());

    // a team that gains a car
    std::vector<Seat> gained = {{lawson, vcarb, car1}, {hadjar, vcarb, car2}, {tsolov, redbull, k(0xd3)}, {other, redbull, car3}};
    CHECK(seats_kept(before, gained) == "team 477 would race 2 car(s) instead of 1");

    // the key's padding byte does not count
    std::vector<Seat> padded = before;
    padded[0].team |= 0x100;
    CHECK(seats_kept(before, padded).empty());

    // terms: the outgoing driver's active contract for the seat's team comes first
    std::vector<Contract> cs = {{lawson, vcarb, true}, {hadjar, vcarb, true}, {tsolov, none, false}};
    CHECK(terms_for(cs, hadjar, vcarb) == 1);
    // released or for another team: a teammate's active contract for that team
    cs[1].active = false;
    CHECK(terms_for(cs, hadjar, vcarb) == 0);
    cs[1] = {hadjar, redbull, true};
    CHECK(terms_for(cs, hadjar, vcarb) == 0);
    // the human player's contract is never copied to an AI driver
    std::vector<Contract> with_player = {{k(9), vcarb, true, true}, {hadjar, vcarb, false}, {lawson, vcarb, true}};
    CHECK(terms_for(with_player, hadjar, vcarb) == 2);
    with_player.pop_back();
    CHECK(terms_for(with_player, hadjar, vcarb) == -1);
    // nobody holds an active contract for the team, or the seat has no team
    CHECK(terms_for(cs, hadjar, k(480)) == -1);
    CHECK(terms_for(cs, hadjar, none) == -1);

    std::printf("%s (%d failure(s))\n", fails ? "FAILED" : "ALL OK", fails);
    return fails ? 1 : 0;
}
