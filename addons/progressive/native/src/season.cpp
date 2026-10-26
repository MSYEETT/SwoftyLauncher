// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career: the season apply (DESIGN.md D6). On the first idle Team HQ of a season the
// store has not applied yet, in a career filed under f1_career_progressive:
//   1. year = min(start + Championship.season_index, 2026) (season_logic.hpp);
//   2. the plan: the year's line-up fitted around the player, its moves, releases and numbers, each
//      slot's car and each seated driver's look (season_logic.hpp, data/years.json);
//   3. one batch through the Career Mode Editor's Driver Career sequence (seats.hpp): every move
//      announced (MessageDriverMoveConfirmed) and written with the seat's team and car keys, every
//      released driver's contract released, announced to no team and written teamless; then the
//      number-only and car/look rows; every row read back against the plan (written back on a
//      mismatch); the new seats' contracts signed; numbers for seated rows without one; one
//      finalise against the line-up from before the batch;
//   4. the team names of the year through swofty::lang (also on every career load).
// The cars and looks are written while the option season_visuals is on (the default; experiment E1
// showed a Team HQ rewrite of vehicle_ref and look in the next session).

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <map>
#include <string>
#include <vector>

#include <swofty/career.hpp>
#include <swofty/career_content.hpp>
#include <swofty/lang.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>
#include <swofty/storage.hpp>

#include "names.hpp"
#include "progressive.hpp"
#include "season_logic.hpp"
#include "seats.hpp"
#include "store.hpp"

SWOFTY_SETTING(std::string_view, start_season, "2026");
SWOFTY_SETTING(std::string_view, rnd_mode, "real_life");
SWOFTY_SETTING(std::string_view, team_moves, "follow_driver");
SWOFTY_SETTING(bool, season_visuals, true);
SWOFTY_SETTING(bool, test_next_season, false);

namespace pg {

namespace {

using namespace progressive;
using names::Kind;
using namespace swofty::game;

constexpr const char *kGroup = "f1_career_progressive";
constexpr const char *kYearsPath = "data/years.json";

season::Data g_data;
bool g_data_ok = false;
int g_failures = 0;
std::string g_names_for;
int g_names_year = 0;

bool data()
{
    if (g_data_ok)
        return true;
    std::string text, err;
    if (swofty::storage::read(kYearsPath, text) != swofty::storage::Result::ok) {
        swofty::log("[progressive] season: %s is missing from swofty/addons/swofty.progressive/", kYearsPath);
        return false;
    }
    g_data_ok = season::parse(text, g_data, err);
    swofty::log("[progressive] season: %s %s (%zu years, %zu pool drivers)%s%s", kYearsPath,
                g_data_ok ? "loaded" : "REFUSED", g_data.years.size(), g_data.pool.size(), err.empty() ? "" : ": ",
                err.c_str());
    return g_data_ok;
}

uint32_t career_seed()
{
    auto *w = static_cast<const WeatherAndTimeOfDayComponent *>(
        swofty::career::component_by_desc(k_Career_WeatherAndTimeOfDayComponentDesc.get()));
    WeatherAndTimeOfDayComponent c{};
    return w && swofty::mem::read(w, c) ? c.random_seed : 0;
}

int setting_year()
{
    std::string v(start_season.get());
    int y = std::atoi(v.c_str());
    return season::clamp_year(y ? y : 2026);
}

std::string label_or(Kind k, uint32_t stat)
{
    std::string l = names::label(k, stat);
    return l.empty() ? std::to_string(stat) : l + "(" + std::to_string(stat) + ")";
}

store::Career *career_entry(const swofty::career::State &state, uint32_t season_index, uint32_t driver)
{
    if (!store::load())
        return nullptr;
    uint32_t seed = career_seed();
    char key[64];
    std::snprintf(key, sizeof key, "pg-%08x-%u", seed, driver);
    if (store::Career *c = store::find(key))
        return c;
    if (season_index > 0) {
        auto same = store::by_driver(driver);
        if (same.size() == 1) {
            swofty::log("[progressive] store: no entry %s at season %u; taking the only entry of this driver (%s), "
                        "its seed changed from %08x",
                        key, season_index, same[0]->key.c_str(), same[0]->seed);
            same[0]->key = key;
            same[0]->seed = seed;
            store::save();
            return same[0];
        }
    }
    store::Career c;
    c.key = key;
    c.driver = driver;
    c.seed = seed;
    c.start = setting_year();
    c.rnd = std::string(rnd_mode.get());
    c.moves = std::string(team_moves.get());
    c.created_season = static_cast<int>(season_index);
    store::Career &added = store::add(c);
    store::save();
    swofty::log("[progressive] store: %s career %s (career key %s): start %d, R&D %s, team moves %s%s",
                season_index ? "ADOPTED a running" : "new", key, state.key[0] ? state.key : "-", c.start, c.rnd.c_str(),
                c.moves.c_str(), season_index ? " (no entry for it; the options as they are now)" : "");
    if (!seed)
        swofty::log("[progressive] store: WARNING the career's rnd_seed reads 0, so two careers of this driver share "
                    "an entry");
    return &added;
}

season::Resolve resolver()
{
    season::Resolve r;
    r.driver_stat = [](const std::string &label) {
        uint32_t s = 0;
        if (names::stat(Kind::driver, label, s))
            return static_cast<int>(s);
        const season::PoolDriver *p = g_data.driver(label);
        return p ? p->stat : -1;
    };
    r.vehicle_stat = [](const std::string &label) {
        uint32_t s = 0;
        return names::stat(Kind::vehicle, label, s) ? static_cast<int>(s) : -1;
    };
    r.look_known = [](int s) { return s >= 0 && names::known(Kind::look, static_cast<uint32_t>(s)); };
    r.rating = [](uint32_t stat) -> double {
        void *c = swofty::career::component_by_desc(k_Career_DriverCareerPlayerDriverStatisticsComponentDesc.get());
        if (!c)
            c = swofty::career::component_by_desc(k_Career_DriverStatisticsComponentDesc.get());
        if (!c || !Career_Get_DriverOverallRating.address())
            return -1;
        uint32_t key = stat << 16;
        float v = Career_Get_DriverOverallRating(c, static_cast<uint8_t *>(c) + offsetof(DriverStatisticsComponent, stats_data),
                                                 &key, true);
        return std::isfinite(v) && v > 0 ? v : -1;
    };
    return r;
}

std::vector<season::CareerRow> view(const std::vector<Row> &rows)
{
    std::vector<season::CareerRow> out;
    for (auto &r : rows) {
        season::CareerRow c;
        c.driver = stat_of(driver_of(r.rec));
        c.team = teamless(r.rec) ? -1 : static_cast<int>(stat_of(r.rec.team_stat_id));
        c.position = position_of(r.rec);
        c.car = has_car(r.rec);
        c.human = human(r.rec);
        c.number = r.rec.car_number;
        out.push_back(c);
    }
    return out;
}

int row_index(const std::vector<Row> &rows, uint32_t stat)
{
    int hit = -1;
    for (size_t i = 0; i < rows.size(); ++i)
        if (stat_of(driver_of(rows[i].rec)) == stat && (driver_of(rows[i].rec) & 0xff) == 0) {
            if (hit >= 0)
                return -2;
            hit = static_cast<int>(i);
        }
    return hit;
}

void set_names(int year, const char *why)
{
    if (!swofty::lang::available() || !data())
        return;
    const season::Year *y = g_data.year(year);
    size_t set = 0;
    for (int i = 0; i < season::kSlots; ++i)
        for (auto &key : g_data.slots[i].name_keys) {
            if (key.empty())
                continue;
            if (year >= season::kLastYear || !y || y->teams[i].name.empty())
                swofty::lang::clear(key);
            else if (swofty::lang::set(key, y->teams[i].name) == swofty::lang::Result::ok)
                ++set;
        }
    swofty::lang::refresh();
    g_names_year = year;
    swofty::log("[progressive] team names %d (%s): %zu override(s)%s", year, why, set,
                year >= season::kLastYear ? ", the 2026 names" : "");
}

void clear_names()
{
    if (!g_names_year || !swofty::lang::available())
        return;
    for (auto &s : g_data.slots)
        for (auto &key : s.name_keys)
            if (!key.empty())
                swofty::lang::clear(key);
    swofty::lang::refresh();
    g_names_year = 0;
}

struct Write {
    int row = -1;
    EntrantWriteData data{};
    uint16_t position = 0;
    uint8_t number = kNoNumber;
    bool move = false, release = false;
    uint32_t old_team = kNoKey, new_team = kNoKey;
    std::string why;
};

EntrantRecord expected(const Row &r, const Write &w)
{
    EntrantRecord x = r.rec;
    x.driver_customisation = w.data.driver_customisation;
    x.team_stat_id = w.data.team_stat_id;
    x.vehicle_ref = w.data.vehicle_ref;
    x.driver_reg_value = w.data.driver_reg_value;
    x.vehicle_customisation = w.data.vehicle_customisation;
    x.badge_stat_id = w.data.badge_stat_id;
    x.const_ffff0012 = w.data.seat_value;
    x.team_position = w.position;
    x.car_number = w.number;
    return x;
}

std::string seat_name(const season::Seat &s)
{
    if (s.slot < 0)
        return "free agent";
    return g_data.slots[s.slot].team + " seat " + std::to_string(s.index + 1);
}

bool apply(uintptr_t e, const season::Plan &p, const std::vector<Row> &rows, bool visuals)
{
    uintptr_t manager = contracts_manager();
    if (!manager) {
        swofty::log("[progressive] season %d REFUSED: no DriverCareerContractsManagerComponent", p.year);
        return false;
    }
    std::map<int, Write> writes;
    auto write_for = [&](int idx) -> Write & {
        auto it = writes.find(idx);
        if (it != writes.end())
            return it->second;
        Write w;
        w.row = idx;
        w.data = own_data(rows[idx]);
        w.position = rows[idx].rec.team_position;
        w.number = rows[idx].rec.car_number;
        return writes.emplace(idx, w).first->second;
    };
    std::vector<uintptr_t> objects;
    if (!contract_objects(manager, objects)) {
        swofty::log("[progressive] season %d REFUSED: the contract list is unreadable", p.year);
        return false;
    }
    uint32_t human_key = p.player == lineup::kNone ? kNoKey : p.player << 16;
    std::vector<Signing> signings;
    for (auto &m : p.moves) {
        int idx = row_index(rows, m.driver);
        if (idx < 0) {
            swofty::log("[progressive] season %d REFUSED: driver %s has no single row", p.year,
                        label_or(Kind::driver, m.driver).c_str());
            return false;
        }
        const Row &seat = rows[p.holder[m.to.slot][m.to.index]];
        Write &w = write_for(idx);
        w.data = seat_data(rows[idx], seat);
        w.position = seat.rec.team_position;
        w.move = true;
        w.old_team = rows[idx].rec.team_stat_id;
        w.new_team = seat.rec.team_stat_id;
        w.why = seat_name(m.from) + " -> " + seat_name(m.to);
        if (!contract_fits(contract_of(objects, driver_of(rows[idx].rec)), seat.rec.team_stat_id)) {
            Signing s;
            if (const char *why = plan_signing(objects, human_key, driver_of(rows[idx].rec), driver_of(seat.rec),
                                               seat.rec.team_stat_id, s))
                swofty::log("[progressive] season %d WARNING %s can't sign for %s: %s", p.year,
                            label_or(Kind::driver, m.driver).c_str(), g_data.slots[m.to.slot].team.c_str(), why);
            else
                signings.push_back(s);
        }
    }
    for (uint32_t d : p.releases) {
        int idx = row_index(rows, d);
        if (idx < 0)
            continue;
        Write &w = write_for(idx);
        w.data = released_data(rows[idx]);
        w.position = 0;
        w.release = true;
        w.old_team = rows[idx].rec.team_stat_id;
        w.why = "released";
    }
    for (auto &n : p.numbers) {
        int idx = row_index(rows, n.driver);
        if (idx >= 0 && !human(rows[idx].rec))
            write_for(idx).number = n.number;
    }
    if (visuals) {
        for (int i = 0; i < season::kSlots; ++i)
            for (int k = 0; k < lineup::kSeats; ++k) {
                int idx = row_index(rows, p.target[i][k]);
                const season::VehicleSet &v = p.vehicles[i];
                if (idx < 0 || v.stat < 0)
                    continue;
                Write &w = write_for(idx);
                w.data.vehicle_ref = (static_cast<uint32_t>(v.stat) << 16) | (w.data.vehicle_ref & 0xffffu);
            }
        for (auto &l : p.looks) {
            int idx = row_index(rows, l.driver);
            if (idx >= 0 && l.stat > 0)
                write_for(idx).data.driver_customisation = static_cast<uint32_t>(l.stat) << 16;
        }
    }
    std::vector<int> order;
    for (auto &[idx, w] : writes) {
        EntrantRecord x = expected(rows[idx], w);
        if (std::memcmp(&rows[idx].rec, &x, sizeof x) != 0)
            order.push_back(idx);
    }
    std::stable_sort(order.begin(), order.end(), [&](int a, int b) {
        auto rank = [&](int i) { return writes[i].move ? 0 : writes[i].release ? 1 : 2; };
        return rank(a) < rank(b);
    });
    swofty::log("[progressive] season %d plan: %zu move(s), %zu release(s), %zu number change(s), cars and looks %s; "
                "%zu row(s) to write, %zu contract(s) to sign",
                p.year, p.moves.size(), p.releases.size(), p.numbers.size(), visuals ? "on" : "off (season_visuals)",
                order.size(), signings.size());
    for (int idx : order) {
        const Write &w = writes[idx];
        swofty::log("[progressive]   %s %s: %s", w.move ? "move" : w.release ? "release" : "update",
                    label_or(Kind::driver, stat_of(driver_of(rows[idx].rec))).c_str(),
                    w.why.empty() ? "number, car or look" : w.why.c_str());
    }
    if (order.empty())
        return true;

    Baseline baseline(rows);
    std::vector<int> written;
    auto back = [&]() {
        size_t ok = 0;
        for (auto it = written.rbegin(); it != written.rend(); ++it) {
            std::vector<Row> now;
            const Row *r = entrant_rows(e, now) && now.size() > static_cast<size_t>(*it) ? &now[*it] : nullptr;
            if (r && write_row(e, *r, own_data(rows[*it]), rows[*it].rec.team_position, rows[*it].rec.car_number))
                ++ok;
        }
        swofty::log("[progressive] season %d: wrote back %zu/%zu row(s)%s", p.year, ok, written.size(),
                    ok == written.size() ? "" : "; RELOAD THE SAVE without saving");
    };
    for (int idx : order) {
        const Write &w = writes[idx];
        std::vector<Row> now;
        if (!entrant_rows(e, now) || now.size() != rows.size() || now[idx].at != rows[idx].at) {
            swofty::log("[progressive] season %d FAILED: the line-up changed during the batch", p.year);
            back();
            return false;
        }
        bool found = false;
        if (w.release && !release_contract(e, manager, driver_of(rows[idx].rec), found))
            swofty::log("[progressive] season %d WARNING %s: contract not released", p.year,
                        label_or(Kind::driver, stat_of(driver_of(rows[idx].rec))).c_str());
        if ((w.move || w.release) &&
            !announce(manager, driver_of(rows[idx].rec), w.old_team, w.release ? kNoKey : w.new_team,
                      w.release ? kReasonReleased : kReasonSigned))
            swofty::log("[progressive] season %d WARNING %s: MessageDriverMoveConfirmed not posted", p.year,
                        label_or(Kind::driver, stat_of(driver_of(rows[idx].rec))).c_str());
        if (!write_row(e, now[idx], w.data, w.position, w.number)) {
            swofty::log("[progressive] season %d FAILED writing %s", p.year,
                        label_or(Kind::driver, stat_of(driver_of(rows[idx].rec))).c_str());
            back();
            return false;
        }
        written.push_back(idx);
    }
    std::vector<Row> after;
    bool ok = entrant_rows(e, after) && after.size() == rows.size();
    for (size_t i = 0; ok && i < rows.size(); ++i) {
        auto it = writes.find(static_cast<int>(i));
        bool mine = it != writes.end() && std::find(written.begin(), written.end(), static_cast<int>(i)) != written.end();
        const EntrantRecord &got = after[i].rec;
        if (!mine) {
            if (std::memcmp(&got, &rows[i].rec, sizeof got) != 0) {
                swofty::log("[progressive] season %d: %s changed though it was not written", p.year,
                            label_or(Kind::driver, stat_of(driver_of(rows[i].rec))).c_str());
                ok = false;
            }
            continue;
        }
        EntrantRecord want = expected(rows[i], it->second);
        if (got.team_stat_id != want.team_stat_id || got.vehicle_ref != want.vehicle_ref ||
            got.driver_customisation != want.driver_customisation || got.car_number != want.car_number ||
            driver_of(got) != driver_of(want)) {
            swofty::log("[progressive] season %d: %s did not read back as planned (%s)", p.year,
                        label_or(Kind::driver, stat_of(driver_of(rows[i].rec))).c_str(), row_text(got).c_str());
            ok = false;
        }
    }
    if (!ok) {
        back();
        return false;
    }
    size_t signed_now = 0;
    for (auto &s : signings)
        if (sign_contract(e, manager, s))
            ++signed_now;
        else
            swofty::log("[progressive] season %d WARNING %s's contract for his new seat was not signed", p.year,
                        label_or(Kind::driver, stat_of(s.driver)).c_str());
    size_t numbered = 0;
    if (uintptr_t cn = car_numbers(e, manager)) {
        std::vector<Row> now;
        if (entrant_rows(e, now))
            for (auto &r : now)
                if (has_car(r.rec) && r.rec.car_number == kNoNumber && !entity_busy(e) && unique_identity(e, r)) {
                    Career_DriverNumbers_CheckExisting(reinterpret_cast<void *>(cn), driver_of(r.rec));
                    ++numbered;
                }
    }
    uint64_t changes = 0;
    bool fin = true;
    if (!p.moves.empty() || !p.releases.empty())
        fin = finalise(e, baseline, changes);
    swofty::log("[progressive] season %d applied: %zu row(s) written, %zu/%zu contract(s) signed, %zu racing number(s) "
                "drawn, finalise %s (%llu line-up change(s))",
                p.year, written.size(), signed_now, signings.size(), numbered,
                p.moves.empty() && p.releases.empty() ? "not needed" : fin ? "posted" : "FAILED",
                static_cast<unsigned long long>(changes));
    return true;
}

std::atomic<int> g_rnd{0};
std::atomic<bool> g_test_armed{false};
std::string g_current;

int rnd_code(const std::string &m)
{
    return m == "real_life" ? 1 : m == "base" ? 2 : m == "hybrid" ? 3 : 0;
}

} // namespace

bool progressive_career()
{
    return swofty::career::is(kGroup);
}

void season_loaded(const swofty::career::State &)
{
    if (!progressive_career()) {
        clear_names();
        return;
    }
    names::load();
}

void season_unloaded()
{
    g_rnd = 0;
    g_current.clear();
    clear_names();
    g_names_for.clear();
    g_failures = 0;
}

void season_hub(const swofty::career::State &state)
{
    if (!progressive_career())
        return;
    uintptr_t e = entity();
    SeasonInfo s;
    std::vector<Row> rows;
    if (!e || !season(e, s) || !entrant_rows(e, rows) || !data() || !names::load())
        return;
    uint32_t player = 0;
    for (auto &r : rows)
        if (human(r.rec))
            player = stat_of(driver_of(r.rec));
    store::Career *c = career_entry(state, s.index, player);
    if (!c)
        return;
    g_current = c->key;
    if (g_rnd.exchange(rnd_code(c->rnd)) != rnd_code(c->rnd))
        swofty::log("[progressive] R&D mode of this career: %s", c->rnd.c_str());
    if (g_test_armed.exchange(false)) {
        ++c->test_seasons;
        store::save();
        swofty::log("[progressive] TEST test_next_season: career %s moves on one season without racing it: game season "
                    "index %u + %d test season(s) = %u (turn the option off and on again for another)",
                    c->key.c_str(), s.index, c->test_seasons, s.index + c->test_seasons);
    }
    uint32_t index = s.index + static_cast<uint32_t>(std::max(0, c->test_seasons));
    int year = season::year_for(c->start, index);
    int shown = c->applied_season >= 0 && c->applied_year ? c->applied_year : season::kLastYear;
    if (g_names_for != c->key || g_names_year != shown) {
        g_names_for = c->key;
        set_names(shown, "career");
    }
    if (c->applied_season >= static_cast<int>(index))
        return;
    if (entity_busy(e) || g_failures >= 3)
        return;
    swofty::log("[progressive] season %u of career %s: year %d (start %d%s), applying", index, c->key.c_str(), year,
                c->start, c->test_seasons ? ", includes test seasons" : "");
    season::Plan p = season::plan(g_data, year, view(rows), resolver());
    for (auto &n : p.notes)
        swofty::log("[progressive]   note: %s", n.c_str());
    if (!p.error.empty()) {
        swofty::log("[progressive] season %d REFUSED: %s", year, p.error.c_str());
        ++g_failures;
        return;
    }
    for (int i = 0; i < season::kSlots; ++i)
        swofty::log("[progressive]   %s: %s + %s, car %s", g_data.slots[i].team.c_str(),
                    label_or(Kind::driver, p.target[i][0]).c_str(), label_or(Kind::driver, p.target[i][1]).c_str(),
                    p.vehicles[i].stat < 0 ? "unchanged" : p.vehicles[i].label.c_str());
    if (!apply(e, p, rows, season_visuals)) {
        ++g_failures;
        return;
    }
    c->applied_season = static_cast<int>(index);
    c->applied_year = year;
    store::save();
    set_names(year, "season applied");
    report("after season apply", state);
}

} // namespace pg

extern "C" __declspec(dllexport) int ProgressiveRndMode()
{
    return pg::g_rnd.load();
}

SWOFTY_ON_CHANGE(test_next_season, bool on)
{
    if (on) {
        pg::g_test_armed = true;
        swofty::log("[progressive] TEST test_next_season on: the next Team HQ of a Progressive career applies the "
                    "following season");
    }
}

SWOFTY_ON_LOAD
{
    if (test_next_season) {
        pg::g_test_armed = true;
        swofty::log("[progressive] TEST test_next_season is on at load: the next Team HQ of a Progressive career "
                    "applies the following season");
    }
}

extern "C" __declspec(dllexport) int ProgressiveFollowMode()
{
    pg::store::Career *c = pg::g_current.empty() ? nullptr : pg::store::find(pg::g_current);
    return !c ? -1 : c->moves == "follow_driver" ? 1 : 0;
}

extern "C" __declspec(dllexport) int ProgressiveFollowTeam(uint32_t driver_stat, uint32_t *team_stat, int *year)
{
    using namespace progressive;
    pg::store::Career *c = pg::g_current.empty() ? nullptr : pg::store::find(pg::g_current);
    if (!c || c->moves != "follow_driver" || !pg::data())
        return 0;
    int now = c->applied_season >= 0 && c->applied_year ? c->applied_year : season::clamp_year(c->start);
    int next = now + 1;
    if (year)
        *year = next;
    if (next > season::kLastYear)
        return 0;
    for (auto &p : pg::g_data.pool) {
        uint32_t s = 0;
        int stat = pg::names::stat(pg::names::Kind::driver, p.driver, s) ? static_cast<int>(s) : p.stat;
        if (stat < 0 || static_cast<uint32_t>(stat) != driver_stat)
            continue;
        auto it = p.slot_by_year.find(next);
        if (it == p.slot_by_year.end() || it->second < 1 || it->second > season::kSlots)
            return 0;
        int team = pg::g_data.slots[it->second - 1].team_stat;
        if (team < 0)
            return 0;
        if (team_stat)
            *team_stat = static_cast<uint32_t>(team);
        swofty::log("[progressive] follow: driver %u races for %s in %d", driver_stat,
                    pg::g_data.slots[it->second - 1].team.c_str(), next);
        return 1;
    }
    return 0;
}
