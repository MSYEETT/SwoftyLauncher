// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Career Mode Editor: line-up and contract rules the driver ops check before they commit
// (protocol/drivers.md). No game access, so native/test builds it on the host.

#pragma once
#include <cstdint>
#include <string>
#include <vector>

namespace ce::lineup {

constexpr uint32_t kNone = 0x3fff0000u; // stat id 0x3fff in the high word: no team, no car

// One entrant row: its driver, team and car keys as the row stores them.
struct Seat {
    uint32_t driver = 0;
    uint32_t team = kNone;
    uint32_t vehicle = kNone;
};

// Empty when every team keeps exactly the cars it had (same team and car keys, each car on one row);
// otherwise what would break.
std::string seats_kept(const std::vector<Seat> &before, const std::vector<Seat> &after);

// One driver's contract object: the team key of its record, whether it is active, and whether it is
// the human player's.
struct Contract {
    uint32_t driver = 0;
    uint32_t team = kNone;
    bool active = false;
    bool human = false;
};

// The contract whose terms a driver signs when he takes a seat on `team`: the outgoing driver's while
// it is active for that team, otherwise another AI driver's active contract for that team. -1 when
// there is none.
int terms_for(const std::vector<Contract> &contracts, uint32_t outgoing, uint32_t team);

} // namespace ce::lineup
