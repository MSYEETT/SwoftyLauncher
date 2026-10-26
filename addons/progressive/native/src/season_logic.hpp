// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career: the season apply plan (DESIGN.md D3, D6, D7, D10). Pure logic over
// data/years.json and an abstract view of the career's entrant rows, so native/test builds it on
// the host.
//
//   year_for   season i of a career that starts in `start` is min(start + i, 2026).
//   parse      data/years.json (tools/gen.py): per year and career slot 1-11 the real line-up,
//              vehicle row, team name and looks; the career slots (team rows); the driver pool.
//   plan       the target seat of every race seat for year Y: the real line-up fitted around the
//              player (lineup_logic.hpp), restricted to drivers the career can move (a single
//              entrant row that is a race seat or a free agent; never a reserve, never the player),
//              the player kept on his own row and seat; then the moves, releases, racing numbers,
//              and per slot the year's vehicle and per seated driver the year's look (each falling
//              back to the nearest year that resolves, and never to no look).
//   Mid-season (midseason_logic.hpp): plan takes the year's line-up at the career's round in place
//   of the season's line-up (`at`), with the player's seat in his slot pinned to the one he took at
//   the season's start.

#pragma once
#include <algorithm>
#include <array>
#include <cstdint>
#include <cstdlib>
#include <functional>
#include <map>
#include <string>
#include <vector>

#include "../../../../native/src/core/json.hpp"
#include "lineup_logic.hpp"

namespace progressive::season {

constexpr int kFirstYear = 2016;
constexpr int kLastYear = 2026;
constexpr int kSlots = lineup::kSlots;
constexpr uint8_t kNoNumber = 0xff;

inline int clamp_year(int y)
{
    return y < kFirstYear ? kFirstYear : y > kLastYear ? kLastYear : y;
}

inline int year_for(int start, uint32_t season_index)
{
    long long y = static_cast<long long>(clamp_year(start)) + season_index;
    return y > kLastYear ? kLastYear : static_cast<int>(y);
}

struct Look {
    std::string label;
    int stat = -1;
};

struct SeatDriver {
    std::string driver;
    int number = -1;
    int position = 0;
    Look look;
    double rating = -1;
};

struct Team {
    int slot = 0;
    std::string kind, name, lang, vehicle;
    int vehicle_stat = -1;
    double gap_pct = -1;
    std::vector<SeatDriver> drivers;
};

// A real line-up change of the year (tools/gen.py from the Jolpica race entries): from real round
// `round` driver `in` races seat `seat` (team position) of career slot `slot` (0-based) in place
// of `out`; until real round `until` (0 = for the rest of the season). kind: stand_in (`out` did
// not race meanwhile), move (`out` raced for another team), replace (`out` left the grid).
struct Change {
    std::string id, out, in, kind, circuit;
    int round = 0, until = 0, slot = -1, seat = 0, number = -1, track = -1, until_track = -1;
    Look look;
};

struct Year {
    int year = 0;
    std::array<Team, kSlots> teams{};
    std::vector<int> real_tracks;
    std::vector<std::string> circuits;
    std::vector<Change> changes;
};

struct Slot {
    int slot = 0;
    std::string team, vehicle_2026;
    int team_stat = -1, vehicle_2026_stat = -1;
    std::array<std::string, 3> name_keys{};
};

struct PoolDriver {
    std::string driver;
    int stat = -1;
    std::map<int, int> slot_by_year;
    std::map<int, Look> looks;
    std::map<int, int> numbers;
    Look default_look;
    int number = -1;
};

struct Data {
    std::map<int, Year> years;
    std::array<Slot, kSlots> slots{};
    std::vector<PoolDriver> pool;

    const PoolDriver *driver(const std::string &label) const
    {
        for (auto &p : pool)
            if (p.driver == label)
                return &p;
        return nullptr;
    }
    const Year *year(int y) const
    {
        auto it = years.find(y);
        return it == years.end() ? nullptr : &it->second;
    }
};

inline int num(const json::Value &v, int def = -1)
{
    return v.type == json::Value::Num ? static_cast<int>(v.n) : def;
}

inline Look look_of(const json::Value &v)
{
    Look l;
    if (v.type == json::Value::Obj) {
        l.label = v["label"].str();
        l.stat = num(v["stat_id"]);
    }
    return l;
}

inline bool parse(const std::string &text, Data &out, std::string &err)
{
    out = Data{};
    json::Value j;
    if (!json::parse(text, j) || j.type != json::Value::Obj) {
        err = "years.json is not a JSON object";
        return false;
    }
    const json::Value &slots = j["slots"];
    if (slots.type != json::Value::Arr || slots.a.size() != kSlots) {
        err = "years.json slots[] must list 11 career slots";
        return false;
    }
    for (auto &s : slots.a) {
        int i = num(s["slot"]) - 1;
        if (i < 0 || i >= kSlots) {
            err = "years.json slot out of 1..11";
            return false;
        }
        Slot &o = out.slots[i];
        o.slot = i + 1;
        o.team = s["team"].str();
        o.team_stat = num(s["team_stat_id"]);
        o.vehicle_2026 = s["vehicle_2026"].str();
        o.vehicle_2026_stat = num(s["vehicle_2026_stat_id"]);
        const json::Value &n = s["names_2026"];
        o.name_keys = {n["display_name"].str(), n["display_name_short"].str(), n["display_name_super_short"].str()};
    }
    for (auto &[key, y] : j["seasons"].o) {
        int year = std::atoi(key.c_str());
        if (year < kFirstYear || year > kLastYear)
            continue;
        Year &yr = out.years[year];
        yr.year = year;
        for (auto &t : y["teams"].a) {
            int i = num(t["slot"]) - 1;
            if (i < 0 || i >= kSlots)
                continue;
            Team &o = yr.teams[i];
            o.slot = i + 1;
            o.kind = t["kind"].str();
            o.name = t["name"].str();
            o.lang = t["lang"].str();
            o.vehicle = t["vehicle"]["label"].str();
            o.vehicle_stat = num(t["vehicle"]["stat_id"]);
            const json::Value &pace = t["pace"];
            o.gap_pct = pace.type == json::Value::Obj && pace["gap_pct"].type == json::Value::Num ? pace["gap_pct"].n : -1;
            for (auto &d : t["drivers"].a) {
                SeatDriver sd;
                sd.driver = d["driver"].str();
                sd.number = num(d["number"]);
                sd.position = num(d["position"], 0);
                sd.look = look_of(d["look"]);
                sd.rating = d["rating"].type == json::Value::Num ? d["rating"].n : -1;
                if (!sd.driver.empty())
                    o.drivers.push_back(sd);
            }
        }
        for (auto &c : y["calendar"].a) {
            yr.real_tracks.push_back(num(c["track"]));
            yr.circuits.push_back(c["circuit"].str());
        }
        for (auto &c : y["changes"].a) {
            Change ch;
            ch.id = c["id"].str();
            ch.out = c["out"].str();
            ch.in = c["in"].str();
            ch.kind = c["kind"].str();
            ch.circuit = c["circuit"].str();
            ch.round = num(c["round"], 0);
            ch.until = num(c["until_round"], 0);
            ch.slot = num(c["slot"]) - 1;
            ch.seat = num(c["seat"], 0);
            ch.number = num(c["number"]);
            ch.track = num(c["track"]);
            ch.until_track = num(c["until_track"]);
            ch.look = look_of(c["look"]);
            if (ch.round >= 1 && ch.slot >= 0 && ch.slot < kSlots && ch.seat >= 0 && ch.seat < lineup::kSeats &&
                !ch.in.empty() && !ch.out.empty())
                yr.changes.push_back(ch);
        }
    }
    for (auto &p : j["pool"].a) {
        PoolDriver d;
        d.driver = p["driver"].str();
        d.stat = num(p["stat_id"]);
        for (auto &[k, v] : p["years"].o)
            d.slot_by_year[std::atoi(k.c_str())] = num(v);
        for (auto &[k, v] : p["looks"].o)
            d.looks[std::atoi(k.c_str())] = look_of(v);
        for (auto &[k, v] : p["numbers"].o)
            d.numbers[std::atoi(k.c_str())] = num(v);
        d.default_look = look_of(p["default_look"]);
        d.number = num(p["number"]);
        if (!d.driver.empty())
            out.pool.push_back(d);
    }
    if (out.years.empty() || out.pool.empty()) {
        err = "years.json has no seasons or no pool";
        return false;
    }
    return true;
}

// Hybrid R&D (DESIGN.md D9): a team's resource points for the season, base x (1 + gap / 2) with gap
// the team's median qualifying gap to the fastest team that year in percent, so the real
// season's slower teams get more to develop with. A slot without pace data (the 10-team years'
// filler) counts as the year's slowest team.
inline int hybrid_budget(const Data &d, int year, int slot, int base)
{
    const Year *y = d.year(year);
    if (!y || slot < 0 || slot >= kSlots)
        return base;
    double gap = y->teams[slot].gap_pct;
    if (gap < 0)
        for (auto &t : y->teams)
            gap = std::max(gap, t.gap_pct);
    if (gap < 0)
        gap = 0;
    double v = base * (1.0 + gap / 2.0);
    return v > 999999 ? 999999 : static_cast<int>(v + 0.5);
}

// One entrant row of the career, as the planner needs it.
struct CareerRow {
    uint32_t driver = 0;
    int team = -1;
    int position = -1;
    bool car = false;
    bool human = false;
    uint8_t number = kNoNumber;
};

struct Resolve {
    std::function<int(const std::string &)> driver_stat;
    std::function<int(const std::string &)> vehicle_stat;
    std::function<bool(int)> look_known;
    std::function<double(uint32_t)> rating;
};

struct Seat {
    int slot = -1;
    int index = -1;
    bool operator==(const Seat &o) const { return slot == o.slot && index == o.index; }
};

struct Move {
    uint32_t driver = 0;
    Seat from, to;
};

struct NumberSet {
    uint32_t driver = 0;
    uint8_t number = kNoNumber;
};

struct VehicleSet {
    std::string label;
    int stat = -1;
    int from_year = 0;
};

struct LookSet {
    uint32_t driver = 0;
    std::string label;
    int stat = -1;
    std::string from;
};

struct Plan {
    int year = 0;
    std::string error;
    lineup::Output fit;
    std::array<std::array<uint32_t, lineup::kSeats>, kSlots> target{};
    std::array<std::array<int, lineup::kSeats>, kSlots> holder{};
    std::vector<Move> moves;
    std::vector<uint32_t> releases;
    std::vector<NumberSet> numbers;
    std::array<VehicleSet, kSlots> vehicles{};
    std::vector<LookSet> looks;
    std::vector<std::string> notes;
    uint32_t player = lineup::kNone;
    int player_slot = -1;
    bool player_moves = false;
};

inline int slot_of_team(const Data &d, int team_stat)
{
    for (int i = 0; i < kSlots; ++i)
        if (d.slots[i].team_stat == team_stat && team_stat >= 0)
            return i;
    return -1;
}

inline const PoolDriver *pool_by_stat(const Data &d, const Resolve &r, uint32_t stat)
{
    for (auto &p : d.pool) {
        int s = r.driver_stat ? r.driver_stat(p.driver) : p.stat;
        if (s >= 0 && static_cast<uint32_t>(s) == stat)
            return &p;
    }
    return nullptr;
}

inline lineup::Grid grid_of(const Year &y, const Resolve &r, const std::function<bool(uint32_t)> &usable,
                            std::vector<std::string> *notes)
{
    lineup::Grid g = lineup::empty_grid();
    for (int i = 0; i < kSlots; ++i) {
        std::vector<SeatDriver> ds = y.teams[i].drivers;
        std::stable_sort(ds.begin(), ds.end(), [](const SeatDriver &a, const SeatDriver &b) { return a.position < b.position; });
        int k = 0;
        for (auto &sd : ds) {
            if (k >= lineup::kSeats)
                break;
            int s = r.driver_stat(sd.driver);
            if (s < 0) {
                if (notes)
                    notes->push_back(std::to_string(y.year) + " " + sd.driver + ": not in the gamedb; his seat goes to a filler");
                continue;
            }
            if (usable && !usable(static_cast<uint32_t>(s))) {
                if (notes)
                    notes->push_back(std::to_string(y.year) + " " + sd.driver +
                                     ": no free-agent or race-seat row in this career; his seat goes to a filler");
                continue;
            }
            g[i][k++] = static_cast<uint32_t>(s);
        }
    }
    return g;
}

inline int year_number(const Data &d, const Resolve &r, int year, uint32_t driver, const Year *at = nullptr)
{
    if (const Year *y = at ? at : d.year(year))
        for (auto &t : y->teams)
            for (auto &sd : t.drivers)
                if (sd.number >= 1 && sd.number <= 99 && r.driver_stat(sd.driver) == static_cast<int>(driver))
                    return sd.number;
    if (const PoolDriver *p = pool_by_stat(d, r, driver)) {
        auto it = p->numbers.find(year);
        if (it != p->numbers.end() && it->second >= 1 && it->second <= 99)
            return it->second;
        if (p->number >= 1 && p->number <= 99)
            return p->number;
    }
    return -1;
}

inline LookSet look_for(const Data &d, const Resolve &r, int year, uint32_t driver, const Year *at = nullptr)
{
    LookSet out;
    out.driver = driver;
    auto take = [&](const Look &l, const std::string &from) {
        if (out.stat >= 0 || l.stat < 0 || (r.look_known && !r.look_known(l.stat)))
            return;
        out.label = l.label;
        out.stat = l.stat;
        out.from = from;
    };
    if (at)
        for (auto &t : at->teams)
            for (auto &sd : t.drivers)
                if (r.driver_stat(sd.driver) == static_cast<int>(driver))
                    take(sd.look, std::to_string(year) + " mid-season");
    if (const Year *y = d.year(year))
        for (auto &t : y->teams)
            for (auto &sd : t.drivers)
                if (r.driver_stat(sd.driver) == static_cast<int>(driver))
                    take(sd.look, std::to_string(year));
    const PoolDriver *p = pool_by_stat(d, r, driver);
    if (p) {
        auto it = p->looks.find(year);
        if (it != p->looks.end())
            take(it->second, std::to_string(year));
        for (int dist = 1; out.stat < 0 && dist <= kLastYear - kFirstYear; ++dist)
            for (int y : {year - dist, year + dist}) {
                auto at = p->looks.find(y);
                if (at != p->looks.end())
                    take(at->second, "nearest year " + std::to_string(y));
            }
        take(p->default_look, "default look");
    }
    return out;
}

inline VehicleSet vehicle_for(const Data &d, const Resolve &r, int year, int slot)
{
    VehicleSet out;
    auto take = [&](const std::string &label, int y) {
        if (out.stat >= 0 || label.empty())
            return;
        int s = r.vehicle_stat(label);
        if (s < 0)
            return;
        out.label = label;
        out.stat = s;
        out.from_year = y;
    };
    if (const Year *y = d.year(year))
        take(y->teams[slot].vehicle, year);
    for (int dist = 1; out.stat < 0 && dist <= kLastYear - kFirstYear; ++dist)
        for (int y : {year - dist, year + dist})
            if (const Year *yr = d.year(y))
                take(yr->teams[slot].vehicle, y);
    take(d.slots[slot].vehicle_2026, kLastYear);
    return out;
}

// move_player (follow mode): when the player raced in the year (at the career's round, with `at`)
// for another slot, he moves to his real seat and the real line-ups stand around him.
// at: the year's line-up at the career's round (midseason_logic.hpp); the season's own otherwise.
// opening: the line-up at the season's first race. The player's seat in his slot is fixed for the
// year: his own seat when the season's line-up has him there, else the one the opening's fit takes.
// A later change of that seat's real driver sends the newcomer to the player's real seat (the fit's
// D7 rule); when the player has no real seat it is skipped, along with the change that filled the
// newcomer's old seat with the driver the player displaced.
inline Plan plan(const Data &d, int year, const std::vector<CareerRow> &rows, const Resolve &r, bool move_player = false,
                 const Year *at = nullptr, const Year *opening = nullptr)
{
    Plan p;
    p.year = clamp_year(year);
    for (auto &s : p.target)
        s.fill(lineup::kNone);
    for (auto &s : p.holder)
        s.fill(-1);
    const Year *base = d.year(p.year);
    const Year *y = at && base ? at : base;
    if (!y) {
        p.error = "years.json has no " + std::to_string(p.year);
        return p;
    }

    std::map<uint32_t, int> count;
    for (auto &row : rows)
        ++count[row.driver];
    std::map<uint32_t, Seat> seat_of;
    for (int i = 0; i < kSlots; ++i) {
        std::vector<int> race;
        for (size_t k = 0; k < rows.size(); ++k)
            if (rows[k].car && slot_of_team(d, rows[k].team) == i)
                race.push_back(static_cast<int>(k));
        std::stable_sort(race.begin(), race.end(), [&](int a, int b) { return rows[a].position < rows[b].position; });
        if (race.size() != lineup::kSeats) {
            p.error = "career slot " + std::to_string(i + 1) + " (" + d.slots[i].team + ") has " +
                      std::to_string(race.size()) + " race seats, not 2";
            return p;
        }
        for (int k = 0; k < lineup::kSeats; ++k) {
            p.holder[i][k] = race[k];
            seat_of[rows[race[k]].driver] = {i, k};
        }
    }
    int human = -1;
    for (size_t k = 0; k < rows.size(); ++k)
        if (rows[k].human)
            human = human < 0 ? static_cast<int>(k) : -2;
    if (human == -2) {
        p.error = "more than one human row";
        return p;
    }
    Seat player_seat;
    if (human >= 0) {
        p.player = rows[human].driver;
        auto it = seat_of.find(p.player);
        if (it != seat_of.end())
            player_seat = it->second;
        p.player_slot = player_seat.slot;
    }

    auto usable = [&](uint32_t s) {
        if (count[s] != 1)
            return false;
        if (s == p.player)
            return true;
        for (auto &row : rows)
            if (row.driver == s)
                return !row.human && (row.car || row.team < 0);
        return false;
    };

    lineup::Input in;
    in.real = grid_of(*y, r, usable, &p.notes);
    lineup::Grid prev, next;
    if (const Year *py = d.year(p.year - 1)) {
        prev = grid_of(*py, r, nullptr, nullptr);
        in.prev = &prev;
    }
    if (const Year *ny = d.year(p.year + 1)) {
        next = grid_of(*ny, r, nullptr, nullptr);
        in.next = &next;
    }
    in.player = p.player;
    in.player_slot = p.player_slot;
    int real_slot = -1;
    if (move_player && p.player_slot >= 0 && lineup::in_grid(in.real, p.player, &real_slot) && real_slot != p.player_slot) {
        in.player_slot = real_slot;
        p.player_moves = true;
        p.notes.push_back("follow mode: the player moves to his real " + std::to_string(p.year) + " team (" +
                          d.slots[real_slot].team + ")");
    }
    for (auto &pd : d.pool) {
        int s = r.driver_stat(pd.driver);
        if (s >= 0 && usable(static_cast<uint32_t>(s)) && static_cast<uint32_t>(s) != p.player)
            in.pool.push_back(static_cast<uint32_t>(s));
    }
    std::vector<uint32_t> rated(in.pool);
    for (const Year *yy : {y, base})
        for (auto &t : yy->teams)
            for (auto &sd : t.drivers) {
                int s = r.driver_stat(sd.driver);
                if (s >= 0)
                    rated.push_back(static_cast<uint32_t>(s));
            }
    for (uint32_t s : rated) {
        double v = -1;
        for (const Year *yy : {base, y})
            for (auto &t : yy->teams)
                for (auto &sd : t.drivers)
                    if (sd.rating >= 0 && r.driver_stat(sd.driver) == static_cast<int>(s))
                        v = sd.rating;
        if (v < 0 && r.rating)
            v = r.rating(s);
        in.ratings.push_back({s, static_cast<float>(v < 0 ? 70.0 : v)});
    }
    if (y != base && !p.player_moves && p.player_slot >= 0) {
        lineup::Grid main = grid_of(*base, r, usable, nullptr);
        lineup::Grid first = grid_of(opening ? *opening : *base, r, usable, nullptr);
        int own = -1;
        for (int k = 0; k < lineup::kSeats; ++k)
            if (main[p.player_slot][k] == p.player)
                own = k;
        in.take = own >= 0 ? own : lineup::take_seat(first, p.player, p.player_slot, in.ratings);
        lineup::Driver d0 = in.take >= 0 ? first[p.player_slot][in.take] : lineup::kNone;
        lineup::Driver o = in.take >= 0 ? in.real[p.player_slot][in.take] : lineup::kNone;
        int s2 = -1, k2 = -1;
        if (d0 != lineup::kNone && o != d0 && !lineup::in_grid(in.real, p.player)) {
            if (lineup::in_grid(in.real, d0, &s2, &k2))
                in.real[s2][k2] = o;
            in.real[p.player_slot][in.take] = d0;
            p.notes.push_back("mid-season: driver " + std::to_string(o) + " would come into the player's seat, and the "
                              "player's driver has no real seat to send him to; that change is skipped");
        }
    }
    p.fit = lineup::fit(in);
    if (std::string bad = lineup::check(p.fit); !bad.empty()) {
        p.error = "line-up fit: " + bad;
        return p;
    }
    for (auto &n : p.fit.notes)
        p.notes.push_back(n);

    for (int i = 0; i < kSlots; ++i)
        for (int k = 0; k < lineup::kSeats; ++k)
            p.target[i][k] = p.fit.at(i, k);
    if (player_seat.slot >= 0 && !p.player_moves) {
        auto &t = p.target[player_seat.slot];
        if (t[player_seat.index] != p.player)
            std::swap(t[0], t[1]);
    }
    std::vector<uint32_t> seated;
    for (int i = 0; i < kSlots; ++i)
        for (int k = 0; k < lineup::kSeats; ++k) {
            uint32_t want = p.target[i][k];
            if (want == lineup::kNone) {
                uint32_t keep = rows[p.holder[i][k]].driver;
                bool used = false;
                for (auto &s : p.target)
                    for (uint32_t x : s)
                        used = used || x == keep;
                if (used) {
                    p.error = "slot " + std::to_string(i + 1) + " seat " + std::to_string(k + 1) +
                              " has no driver left to race it";
                    return p;
                }
                p.target[i][k] = keep;
                p.notes.push_back("slot " + std::to_string(i + 1) + " seat " + std::to_string(k + 1) +
                                  " keeps its driver: the pool has nobody left");
            }
            seated.push_back(p.target[i][k]);
        }

    for (int i = 0; i < kSlots; ++i)
        for (int k = 0; k < lineup::kSeats; ++k) {
            uint32_t dr = p.target[i][k];
            if (dr == p.player && !p.player_moves)
                continue;
            auto it = seat_of.find(dr);
            Seat from = it == seat_of.end() ? Seat{} : it->second;
            if (from == Seat{i, k})
                continue;
            p.moves.push_back({dr, from, {i, k}});
        }
    for (auto &[dr, s] : seat_of)
        if (dr != p.player && std::find(seated.begin(), seated.end(), dr) == seated.end())
            p.releases.push_back(dr);

    std::map<uint8_t, std::vector<uint32_t>> holders;
    std::map<uint32_t, uint8_t> number_of;
    for (auto &row : rows) {
        number_of[row.driver] = row.number;
        if (row.number != kNoNumber)
            holders[row.number].push_back(row.driver);
    }
    auto is_seated = [&](uint32_t dr) { return std::find(seated.begin(), seated.end(), dr) != seated.end(); };
    auto drop = [&](uint32_t dr) {
        auto &v = holders[number_of[dr]];
        v.erase(std::remove(v.begin(), v.end(), dr), v.end());
        number_of[dr] = kNoNumber;
    };
    std::vector<std::pair<uint32_t, int>> wants;
    for (uint32_t dr : seated)
        if (dr != p.player)
            if (int w = year_number(d, r, p.year, dr, y); w > 0)
                wants.push_back({dr, w});
    for (int pass = 0; pass < 3 && !wants.empty(); ++pass) {
        std::vector<std::pair<uint32_t, int>> later;
        for (auto [dr, w] : wants) {
            auto n = static_cast<uint8_t>(w);
            bool blocked = false;
            for (uint32_t other : holders[n])
                blocked = blocked || (other != dr && (other == p.player || is_seated(other)));
            if (blocked) {
                later.push_back({dr, w});
                continue;
            }
            for (uint32_t other : std::vector<uint32_t>(holders[n]))
                if (other != dr) {
                    drop(other);
                    p.numbers.push_back({other, kNoNumber});
                }
            if (number_of[dr] == n)
                continue;
            if (number_of[dr] != kNoNumber)
                drop(dr);
            holders[n].push_back(dr);
            number_of[dr] = n;
            p.numbers.push_back({dr, n});
        }
        if (later.size() == wants.size())
            break;
        wants = later;
    }
    for (auto [dr, w] : wants)
        p.notes.push_back("driver " + std::to_string(dr) + " keeps his number: #" + std::to_string(w) +
                          " belongs to another driver on the grid");

    for (int i = 0; i < kSlots; ++i) {
        p.vehicles[i] = vehicle_for(d, r, p.year, i);
        if (p.vehicles[i].stat < 0)
            p.notes.push_back("slot " + std::to_string(i + 1) + ": no vehicle row resolves; its cars stay");
        else if (p.vehicles[i].from_year != p.year)
            p.notes.push_back("slot " + std::to_string(i + 1) + ": " + std::to_string(p.year) + " car missing, using " +
                              p.vehicles[i].label + " from " + std::to_string(p.vehicles[i].from_year));
    }
    for (uint32_t dr : seated) {
        LookSet l = look_for(d, r, p.year, dr, y != base ? y : nullptr);
        if (l.stat < 0)
            p.notes.push_back("driver " + std::to_string(dr) + ": no look resolves; his look stays");
        else
            p.looks.push_back(l);
    }
    return p;
}

} // namespace progressive::season
