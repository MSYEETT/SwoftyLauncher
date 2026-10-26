// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career: the real mid-season line-up changes (data/years.json `changes`, tools/gen.py
// from the Jolpica race entries) placed on the career's calendar. Pure logic, built on the host by
// native/test.
//
//   career_round  the career race a real round's changes apply before: the career weekend on the
//                 round's track when the career calendar has it and it lies within a quarter of the
//                 season (at least 3 races) of the round's place in the real season; otherwise the
//                 same fraction of the season, real round k of N -> round(k * M / N) of M. Real round
//                 1 is always career round 1. Over the changes in real order the rounds never go back.
//   window        a change is due from its career round; a change with `until` ends at the career
//                 round of the real round the regular driver came back, or, when that is not later,
//                 after as many career races as it lasted in real life (a one-off stand-in: the next
//                 race brings the regular back).
//   keep          the player is never swapped out: a stand-in involving his driver (in or out) is
//                 skipped, and so is a change taking his driver out of a seat unless the same round
//                 moves his driver into another seat (follow mode moves the player with it; in
//                 choose mode the season fit remaps it, season_logic.hpp).
//   at_round      the year's line-up at the career race `round`: each seat's driver is the incoming
//                 driver of its latest due change in real order, else the season's driver; when two
//                 seats would hold one driver, the seat whose change is older gives way to its earlier
//                 holder.

#pragma once
#include <algorithm>
#include <cmath>
#include <functional>
#include <string>
#include <vector>

#include "season_logic.hpp"

namespace progressive::midseason {

using season::Change;
using season::Data;
using season::Year;

constexpr int kOpen = 1 << 30;

inline int base_track(int t)
{
    switch (t) {
    case 34:
        return 0;
    case 21:
    case 35:
        return 3;
    case 24:
    case 36:
        return 13;
    case 37:
        return 27;
    case 38:
        return 30;
    case 22:
    case 39:
        return 7;
    case 25:
    case 40:
        return 17;
    case 41:
        return 26;
    case 23:
        return 15;
    default:
        return t;
    }
}

inline int tolerance(int m)
{
    return std::max(3, (m + 3) / 4);
}

inline int career_round(const std::vector<int> &real_tracks, int k, const std::vector<int> &career)
{
    int n = static_cast<int>(real_tracks.size());
    int m = static_cast<int>(career.size());
    if (m <= 0 || k <= 1)
        return 1;
    if (n <= 0)
        return 1;
    if (k > n)
        return m + 1;
    double frac = static_cast<double>(k) * m / n;
    int f = std::clamp(static_cast<int>(std::floor(frac + 0.5)), 1, m);
    int t = real_tracks[k - 1];
    if (t >= 0)
        for (int i = 0; i < m; ++i)
            if (base_track(career[i]) == t) {
                if (std::abs((i + 1) - frac) <= tolerance(m))
                    return i + 1;
                break;
            }
    return f;
}

struct Window {
    int from = 1;
    int to = kOpen;
};

struct Skip {
    int change = -1;
    std::string why;
};

inline std::vector<bool> keep(const Year &y, const std::function<bool(const std::string &)> &is_player,
                              std::vector<Skip> *skipped = nullptr)
{
    std::vector<bool> kept(y.changes.size(), true);
    auto mine = [&](const std::string &label) { return is_player && is_player(label); };
    for (size_t i = 0; i < y.changes.size(); ++i) {
        const Change &c = y.changes[i];
        if (c.kind == "stand_in" && (mine(c.in) || mine(c.out))) {
            kept[i] = false;
            if (skipped)
                skipped->push_back({static_cast<int>(i), mine(c.in) ? "a stand-in drive of the player's driver; the "
                                                                      "player keeps his seat"
                                                                    : "a stand-in for the player's driver; the player "
                                                                      "is never swapped out"});
        }
    }
    for (size_t i = 0; i < y.changes.size(); ++i) {
        const Change &c = y.changes[i];
        if (!kept[i] || !mine(c.out))
            continue;
        bool moves = false;
        for (size_t j = 0; j < y.changes.size(); ++j)
            moves = moves || (kept[j] && j != i && y.changes[j].round == c.round && mine(y.changes[j].in));
        if (!moves) {
            kept[i] = false;
            if (skipped)
                skipped->push_back({static_cast<int>(i), "it takes the player's driver out of his seat; the player is "
                                                         "never swapped out"});
        }
    }
    return kept;
}

inline std::vector<int> real_order(const Year &y)
{
    std::vector<int> order(y.changes.size());
    for (size_t i = 0; i < order.size(); ++i)
        order[i] = static_cast<int>(i);
    std::stable_sort(order.begin(), order.end(), [&](int a, int b) { return y.changes[a].round < y.changes[b].round; });
    return order;
}

inline std::vector<Window> windows(const Year &y, const std::vector<int> &career, const std::vector<bool> &kept)
{
    std::vector<Window> w(y.changes.size());
    int floor_round = 1;
    for (int i : real_order(y)) {
        const Change &c = y.changes[i];
        int from = std::max(career_round(y.real_tracks, c.round, career), floor_round);
        if (kept.empty() || kept[i])
            floor_round = from;
        w[i].from = from;
        if (c.until > c.round) {
            int back = career_round(y.real_tracks, c.until, career);
            w[i].to = back > from ? back : from + (c.until - c.round);
        }
    }
    return w;
}

struct At {
    int year = 0;
    int round = 1;
    int real_round = 0;
    std::vector<int> career;
    std::vector<bool> kept;
    std::vector<Window> windows;
    std::vector<int> active;
    std::vector<Skip> skipped;
    std::vector<std::string> notes;
    Year line_up;
};

inline int real_round_at(const Year &y, const std::vector<int> &career, int round)
{
    int k = 0;
    for (int i = 1; i <= static_cast<int>(y.real_tracks.size()); ++i)
        if (career_round(y.real_tracks, i, career) <= round)
            k = i;
    return k;
}

inline const season::SeatDriver *base_driver(const Year &y, const std::string &label)
{
    for (auto &t : y.teams)
        for (auto &d : t.drivers)
            if (d.driver == label)
                return &d;
    return nullptr;
}

inline std::vector<int> seat_order(const season::Team &t)
{
    std::vector<int> idx(t.drivers.size());
    for (size_t i = 0; i < idx.size(); ++i)
        idx[i] = static_cast<int>(i);
    std::stable_sort(idx.begin(), idx.end(),
                     [&](int a, int b) { return t.drivers[a].position < t.drivers[b].position; });
    return idx;
}

// The season's line-up with the given changes in force; `due` lists change indices, each seat
// taking the latest in real order of those on it, a seat that would repeat a driver falling back.
inline Year line_up_with(const Year &y, const std::vector<int> &due, std::vector<int> &active,
                         std::vector<std::string> &notes)
{
    Year out = y;
    using Cands = std::vector<int>;
    std::vector<std::vector<Cands>> cands(season::kSlots, std::vector<Cands>(lineup::kSeats));
    std::vector<int> order = due;
    std::stable_sort(order.begin(), order.end(), [&](int a, int b) { return y.changes[a].round > y.changes[b].round; });
    for (int i : order) {
        const Change &c = y.changes[i];
        cands[c.slot][c.seat].push_back(i);
    }
    std::vector<std::vector<size_t>> pick(season::kSlots, std::vector<size_t>(lineup::kSeats, 0));
    auto seat_driver = [&](int s, int k) -> std::string {
        if (pick[s][k] < cands[s][k].size())
            return y.changes[cands[s][k][pick[s][k]]].in;
        std::vector<int> idx = seat_order(y.teams[s]);
        return k < static_cast<int>(idx.size()) ? y.teams[s].drivers[idx[k]].driver : std::string();
    };
    auto seat_round = [&](int s, int k) {
        return pick[s][k] < cands[s][k].size() ? y.changes[cands[s][k][pick[s][k]]].round : 0;
    };
    for (int guard = 0; guard < 64; ++guard) {
        bool clash = false;
        for (int s = 0; s < season::kSlots && !clash; ++s)
            for (int k = 0; k < lineup::kSeats && !clash; ++k) {
                std::string a = seat_driver(s, k);
                if (a.empty())
                    continue;
                for (int s2 = 0; s2 < season::kSlots && !clash; ++s2)
                    for (int k2 = 0; k2 < lineup::kSeats && !clash; ++k2) {
                        if ((s2 == s && k2 == k) || seat_driver(s2, k2) != a)
                            continue;
                        bool first_older = seat_round(s, k) < seat_round(s2, k2);
                        int gs = first_older ? s : s2, gk = first_older ? k : k2;
                        if (pick[gs][gk] >= cands[gs][gk].size()) {
                            gs = first_older ? s2 : s;
                            gk = first_older ? k2 : k;
                        }
                        if (pick[gs][gk] >= cands[gs][gk].size())
                            continue;
                        notes.push_back(a + " would sit twice; slot " + std::to_string(gs + 1) + " seat " +
                                        std::to_string(gk + 1) + " keeps its earlier driver");
                        ++pick[gs][gk];
                        clash = true;
                    }
            }
        if (!clash)
            break;
    }
    active.clear();
    for (int s = 0; s < season::kSlots; ++s) {
        season::Team &t = out.teams[s];
        std::vector<int> idx = seat_order(y.teams[s]);
        for (int k = 0; k < lineup::kSeats && k < static_cast<int>(idx.size()); ++k) {
            if (pick[s][k] >= cands[s][k].size())
                continue;
            int ci = cands[s][k][pick[s][k]];
            const Change &c = y.changes[ci];
            active.push_back(ci);
            season::SeatDriver sd;
            sd.driver = c.in;
            sd.number = c.number;
            sd.position = y.teams[s].drivers[idx[k]].position;
            sd.look = c.look;
            const season::SeatDriver *own = base_driver(y, c.in);
            sd.rating = own ? own->rating : -1;
            t.drivers[idx[k]] = sd;
        }
    }
    std::sort(active.begin(), active.end());
    return out;
}

inline At at_round(const Data &d, int year, const std::vector<int> &career, int round,
                   const std::function<bool(const std::string &)> &is_player)
{
    At a;
    a.year = year;
    a.round = round;
    a.career = career;
    const Year *y = d.year(year);
    if (!y)
        return a;
    a.line_up = *y;
    a.kept = keep(*y, is_player, &a.skipped);
    a.windows = windows(*y, career, a.kept);
    a.real_round = real_round_at(*y, career, round);
    std::vector<int> due;
    for (size_t i = 0; i < y->changes.size(); ++i)
        if (a.kept[i] && a.windows[i].from <= round && round < a.windows[i].to)
            due.push_back(static_cast<int>(i));
    a.line_up = line_up_with(*y, due, a.active, a.notes);
    return a;
}

// The line-up with exactly the changes a career recorded (when its calendar cannot be read).
inline At at_ids(const Data &d, int year, int round, const std::vector<std::string> &ids)
{
    At a;
    a.year = year;
    a.round = round;
    const Year *y = d.year(year);
    if (!y)
        return a;
    std::vector<int> due;
    for (size_t i = 0; i < y->changes.size(); ++i)
        if (std::find(ids.begin(), ids.end(), y->changes[i].id) != ids.end())
            due.push_back(static_cast<int>(i));
    a.kept.assign(y->changes.size(), true);
    a.line_up = line_up_with(*y, due, a.active, a.notes);
    return a;
}

inline std::string ids_text(const Year &y, const std::vector<int> &active)
{
    std::string out;
    for (int i : active)
        out += (out.empty() ? "" : ",") + y.changes[i].id;
    return out;
}

inline std::vector<std::string> split_ids(const std::string &text)
{
    std::vector<std::string> out;
    size_t at = 0;
    while (at <= text.size()) {
        size_t comma = text.find(',', at);
        std::string id = text.substr(at, comma == std::string::npos ? std::string::npos : comma - at);
        if (!id.empty())
            out.push_back(id);
        if (comma == std::string::npos)
            break;
        at = comma + 1;
    }
    return out;
}

} // namespace progressive::midseason
