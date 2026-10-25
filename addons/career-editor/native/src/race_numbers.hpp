// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Career Mode Editor: the rules drivers.number.set checks before it writes a racing number
// (protocol/drivers.md). No game access, so native/test builds it on the host.

#pragma once
#include <cstdint>
#include <string>
#include <vector>

namespace ce::numbers {

constexpr uint8_t kNone = 0xff; // EntrantRecord::car_number of a row without a number

// One entrant row of the championship: its driver key and racing number.
struct Row {
    uint32_t driver = 0;
    uint8_t number = kNone;
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

} // namespace ce::numbers
