// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Career Mode Editor: the rules drivers.number.set checks before it writes a racing number, and the
// numbers drivers.swap gives and checks after a swap (protocol/drivers.md). No game access, so
// native/test builds it on the host.

#pragma once
#include <cstdint>
#include <string>
#include <vector>

namespace ce::numbers {

constexpr uint8_t kNone = 0xff; // EntrantRecord::car_number of a row without a number

// One entrant row of the championship: its driver key, racing number and whether it has a team.
struct Row {
    uint32_t driver = 0;
    uint8_t number = kNone;
    bool team = false;
};

struct Plan {
    std::string error;  // "<code>: <detail>", empty when the change can be written
    int row = -1;       // the driver's row
    int swap_with = -1; // the row that gets the driver's old number, or -1
    bool unchanged = false;
};

// A number is 1..99 and free when no other row of the championship holds it; with `swap` the row
// holding it takes the driver's old number instead, which needs the driver to have one.
Plan plan(const std::vector<Row> &rows, uint32_t driver, double number, bool swap);

// After a swap: the drivers of `moved` whose rows have a team and no number. The game's signings give
// such a row a free number (Career_DriverNumbers_CheckExisting) after its entrant write.
std::vector<uint32_t> unnumbered(const std::vector<Row> &rows, const std::vector<uint32_t> &moved);

// Empty when the driver's row holds a number 1..99 that no other row of the championship holds;
// otherwise what is wrong.
std::string sole_holder(const std::vector<Row> &rows, uint32_t driver);

} // namespace ce::numbers
