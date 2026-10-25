// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Career Mode Editor: racing number rules (race_numbers.hpp).

#include "race_numbers.hpp"

#include <cmath>

namespace ce::numbers {

Plan plan(const std::vector<Row> &rows, uint32_t driver, double number, bool swap)
{
    Plan p;
    if (!(number >= 1 && number <= 99) || number != std::floor(number)) {
        p.error = "bad_value: number must be an integer 1..99";
        return p;
    }
    auto want = static_cast<uint8_t>(number);
    for (size_t i = 0; i < rows.size(); ++i)
        if (rows[i].driver == driver)
            p.row = static_cast<int>(i);
    if (p.row < 0) {
        p.error = "no_entrant: the driver has no entrant row in this championship (listed under otherDrivers), so he "
                  "has no racing number";
        return p;
    }
    if (rows[p.row].number == want) {
        p.unchanged = true;
        return p;
    }
    for (size_t i = 0; i < rows.size(); ++i) {
        if (static_cast<int>(i) == p.row || rows[i].number != want)
            continue;
        if (!swap) {
            p.error = "taken: number " + std::to_string(want) + " belongs to another driver in this championship; pick "
                      "a free number or swap the two drivers' numbers";
            return p;
        }
        if (rows[p.row].number == kNone) {
            p.error = "bad_args: the driver has no number to give in a swap";
            return p;
        }
        p.swap_with = static_cast<int>(i);
        return p;
    }
    return p;
}

std::vector<uint32_t> unnumbered(const std::vector<Row> &rows, const std::vector<uint32_t> &moved)
{
    std::vector<uint32_t> out;
    for (uint32_t d : moved)
        for (auto &r : rows)
            if (r.driver == d && r.team && r.number == kNone) {
                out.push_back(d);
                break;
            }
    return out;
}

std::string sole_holder(const std::vector<Row> &rows, uint32_t driver)
{
    const Row *own = nullptr;
    for (auto &r : rows)
        if (r.driver == driver)
            own = &r;
    if (!own)
        return "he has no entrant row";
    if (own->number == kNone)
        return "he has no number";
    if (own->number < 1 || own->number > 99)
        return "his number " + std::to_string(own->number) + " is outside 1..99";
    for (auto &r : rows)
        if (&r != own && r.number == own->number)
            return "number " + std::to_string(own->number) + " is on another entrant row too";
    return {};
}

} // namespace ce::numbers
