// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career: each team's R&D model performance from the year's real pace (pure logic, host
// tested).
//
// A team's R&D model (Upgrades, one per team in UpgradesComponent) carries per upgrade type a 0x20
// byte metadata entry (vector at model +0x1a38..+0x1a40, built from the team's upgrades.2026 tree
// rows): +0 type, +1 department, +4 mass and +8 bhp adjustments (what the vehicle fill 0x141238cb0
// reads), +0xc performance_rating_base, +0x10 performance_rating_fully_upgraded, +0x1c scale. Team
// HQ's VEHICLE PERFORMANCE COMPARISON (0x141890e60) and every team ranking by car performance
// (driver market, objectives, rivalries: callers of 0x143518350) read a team's performance as
//   sum of rating_base + sum over completed upgrades of (rating_full - rating_base) x their weight
// share of the type (0x143518350). The physics never reads the ratings: the vehicle fill takes only
// the upgrade progress, the mass and the bhp. So the stock 2026 trees decide the chart's order (base
// sums 403-585 across the 46 trees) whatever car a season brings.
//
// The plan sets each model's rating base sum to the year's target, target = kTop - kPerPct x (gap %
// + delta %), gap the team's real median qualifying gap to the fastest team (years.json pace.gap_pct;
// a slot without one counts as the year's slowest) and delta the in-season development at the
// current round (Real life only, development_logic.hpp). Every entry's base moves by the same share
// of the difference. Real life also sets rating_full = rating_base, so completed upgrades (the AI's
// R&D still runs under the freeze) add nothing and the chart shows exactly the target; Hybrid moves
// rating_full with the base, so R&D gains keep their size on top of the season's baseline. Targets
// depend only on the year, the round and the data, so every apply and reload gives the same values.

#pragma once
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <vector>

#include "season_logic.hpp"

namespace progressive::rnd_model {

constexpr double kTop = 560.0;
constexpr double kPerPct = 40.0;
constexpr double kMin = 250.0;
constexpr double kMax = 700.0;
constexpr double kEps = 0.01;

struct Meta {
    uint8_t type = 0, department = 0;
    float base = 0, full = 0;
};

inline double year_gap(const season::Data &d, int year, int slot)
{
    const season::Year *y = d.year(year);
    if (!y || slot < 0 || slot >= season::kSlots)
        return 0.0;
    double gap = y->teams[slot].gap_pct;
    if (gap < 0)
        for (auto &t : y->teams)
            gap = std::max(gap, t.gap_pct);
    return gap < 0 ? 0.0 : gap;
}

inline double target(double gap_pct, double delta_pct)
{
    return std::clamp(kTop - kPerPct * (gap_pct + delta_pct), kMin, kMax);
}

inline double base_sum(const std::vector<Meta> &m)
{
    double s = 0;
    for (auto &e : m)
        s += e.base;
    return s;
}

inline double gain_room(const std::vector<Meta> &m)
{
    double s = 0;
    for (auto &e : m)
        s += e.full - e.base;
    return s;
}

// Rewrites m so its base sum is `goal`. flatten: rating_full = rating_base (Real life). Otherwise
// rating_full keeps its distance to the base, or takes `room[i]` when given (the distance first seen
// this session, to undo a Real life flatten). Returns whether anything changed.
inline bool apply(std::vector<Meta> &m, double goal, bool flatten, const std::vector<float> *room = nullptr)
{
    if (m.empty())
        return false;
    double shift = (goal - base_sum(m)) / static_cast<double>(m.size());
    bool changed = false;
    for (size_t i = 0; i < m.size(); ++i) {
        Meta &e = m[i];
        float base = static_cast<float>(e.base + shift);
        float gap = room && i < room->size() ? (*room)[i] : e.full - e.base;
        float full = flatten ? base : base + gap;
        if (std::fabs(base - e.base) > kEps || std::fabs(full - e.full) > kEps)
            changed = true;
        e.base = base;
        e.full = full;
    }
    return changed;
}

} // namespace progressive::rnd_model
