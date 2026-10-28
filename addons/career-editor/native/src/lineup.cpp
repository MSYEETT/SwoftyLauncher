// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Career Mode Editor: line-up and contract rules (lineup.hpp).

#include "lineup.hpp"

#include <algorithm>
#include <cstdio>
#include <map>
#include <utility>

namespace ce::lineup {

namespace {

constexpr uint32_t kKeyMask = 0xffff00ffu; // kind byte and stat id of a key; the byte between is padding

uint32_t key(uint32_t k)
{
    return k & kKeyMask;
}

bool has_car(const Seat &s)
{
    return (s.vehicle & 0xffff0000u) != kNone;
}

std::string hex(uint32_t v)
{
    char b[16];
    std::snprintf(b, sizeof b, "0x%x", v);
    return b;
}

} // namespace

std::string seats_kept(const std::vector<Seat> &before, const std::vector<Seat> &after)
{
    std::map<uint32_t, int> was, now;
    std::map<std::pair<uint32_t, uint32_t>, int> cars;
    for (auto &s : before)
        if (has_car(s)) {
            ++was[key(s.team) >> 16];
            ++cars[{key(s.team), s.vehicle}];
        }
    for (auto &s : after)
        if (has_car(s)) {
            ++now[key(s.team) >> 16];
            --cars[{key(s.team), s.vehicle}];
        }
    for (auto &[team, n] : was)
        if (now[team] != n)
            return "team " + std::to_string(team) + " would race " + std::to_string(now[team]) + " car(s) instead of " +
                   std::to_string(n);
    for (auto &[team, n] : now)
        if (!was.count(team))
            return "team " + std::to_string(team) + " would gain " + std::to_string(n) + " car(s)";
    for (auto &[car, n] : cars)
        if (n != 0)
            return "car " + hex(car.second) + " of team " + std::to_string(car.first >> 16) +
                   (n > 0 ? " would have no driver" : " would have two drivers");
    return {};
}

int terms_for(const std::vector<Contract> &contracts, uint32_t outgoing, uint32_t team)
{
    if ((team & 0xffff0000u) == kNone)
        return -1;
    int other = -1;
    for (size_t i = 0; i < contracts.size(); ++i) {
        const Contract &c = contracts[i];
        if (!c.active || key(c.team) != key(team))
            continue;
        if (key(c.driver) == key(outgoing))
            return static_cast<int>(i);
        if (other < 0 && !c.human)
            other = static_cast<int>(i);
    }
    return other;
}

} // namespace ce::lineup
