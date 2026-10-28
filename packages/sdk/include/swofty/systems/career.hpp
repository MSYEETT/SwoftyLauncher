// SPDX-License-Identifier: MIT
#pragma once
#include <array>
#include <atomic>
#include <chrono>
#include <map>
#include <optional>
#include <swofty/career.hpp>
#include <swofty/events.hpp>
#include <swofty/registry.hpp>
#include <swofty/systems/content.hpp>
#include <swofty/systems/forward.hpp>

// Career: modes, contents, seasons and their boxes, calendars, ratings, the market, contracts, R&D,
// facilities, economy, sponsors, characters and wizards. The 1.x career library (swofty/career.hpp) keeps
// its names in this namespace: its Result is the operations' result here, and its Mode enum stays the 1.x
// mode, so the career.mode record struct is ModeRecord. apply_lineup, set_driver_ratings and swap_drivers call
// the core's career operations (SwoftyCoreCareerOps) and return unavailable on a core without them; set_seat,
// set_car_number and apply_dropped_scores return unavailable.

namespace swofty::career {

enum class Kind { driver_career, my_team };
SWOFTY_ENUM(Kind, driver_career, my_team)
enum class Freeze { per_season, at_creation };
SWOFTY_ENUM(Freeze, per_season, at_creation)
enum class AfterLast { market, hold, end };
SWOFTY_ENUM(AfterLast, market, hold, end)
enum class ChangeKind { swap, stand_in, move, replace };
SWOFTY_ENUM(ChangeKind, swap, stand_in, move, replace)

struct ModeRecord {
    static constexpr Version version{1, 0};
    std::string data_group;
    std::string save_container;
    Text title;
    Kind kind = Kind::driver_career;
    Freeze freeze = Freeze::per_season;
    StatId stat_id = StatId::automatic;
    json::Value extra;
};
SWOFTY_RECORD(ModeRecord, data_group, save_container, title, kind, freeze, stat_id)
inline Registry<ModeRecord> modes{"career.mode", {.applies = Applies::boot}};

struct Content {
    static constexpr Version version{1, 0};
    Ref<ModeRecord> mode;
    Path tile;
    Ref<Content> model;
    Ref<Calendar> calendar;
    AfterLast after_last = AfterLast::market;
    json::Value extra;
};
SWOFTY_RECORD(Content, mode, tile, model, calendar, after_last)
inline Registry<Content> contents{"career.content", {.applies = Applies::boot}};

struct SeasonTeam {
    int slot = 0;
    Ref<teams::Identity> team;
    std::array<Ref<drivers::Identity>, 2> seats;
};
SWOFTY_RECORD(SeasonTeam, slot, team, seats)

struct SeasonChange {
    int round = 0, slot = 0, seat = 0;
    Ref<drivers::Identity> out, in;
    ChangeKind kind = ChangeKind::swap;
    std::optional<int> until_round;
};
SWOFTY_RECORD(SeasonChange, round, slot, seat, out, in, kind, until_round)

struct Season {
    static constexpr Version version{1, 0};
    std::string data_group;
    int year = 0;
    Ref<::swofty::content::Package> box;
    Ref<Calendar> calendar;
    Keyed<SeasonTeam, &SeasonTeam::slot> teams;
    Keyed<SeasonChange, &SeasonChange::round, &SeasonChange::slot, &SeasonChange::seat> changes;
    json::Value extra;
};
SWOFTY_RECORD(Season, data_group, year, box, calendar, teams, changes)
inline Registry<Season> seasons{"career.season",
                                {.applies = Applies::read, .snapshot = true, .snapshot_budget = 65536}};

struct Round {
    int round = 0;
    Text race;
    Ref<tracks::Venue> venue;
    std::chrono::year_month_day date;
};
SWOFTY_RECORD(Round, round, race, venue, date)

struct Calendar {
    static constexpr Version version{1, 0};
    Keyed<Round, &Round::round> rounds;
    int seasons = 10;
    std::vector<int> transfer_windows;
    json::Value extra;
};
SWOFTY_RECORD(Calendar, rounds, seasons, transfer_windows)
inline Registry<Calendar> calendars{"career.calendar", {.applies = Applies::boot}};

struct DriverRatings {
    static constexpr Version version{1, 0};
    Ref<Season> season;
    Ref<drivers::Identity> driver;
    float experience = 0, racecraft = 0, awareness = 0, pace = 0;
    json::Value extra;
};
SWOFTY_RECORD(DriverRatings, season, driver, experience, racecraft, awareness, pace)
inline Registry<DriverRatings> ratings{"career.driver_ratings",
                                       {.applies = Applies::read, .snapshot = true, .snapshot_budget = 65536}};

struct RndDepartment {
    static constexpr Version version{1, 0};
    Text name;
    Colour colour;
    Path icon;
    float cost_scale = 1.0f;
    json::Value extra;
};
SWOFTY_RECORD(RndDepartment, name, colour, icon, cost_scale)
SWOFTY_RECORD_REGISTRY(RndDepartment, "career.rnd.department")
inline Registry<RndDepartment> rnd_departments{"career.rnd.department", {.applies = Applies::content}};

struct Character {
    static constexpr Version version{1, 0};
    Ref<Character> replaces;
    Text first_name, last_name;
    Path portrait;
    std::vector<std::string> mute;
    json::Value extra;
};
SWOFTY_RECORD(Character, replaces, first_name, last_name, portrait, mute)
SWOFTY_RECORD_REGISTRY(Character, "career.character")
inline Registry<Character> characters{"career.character", {.applies = Applies::content}};

struct Retirement {
    std::chrono::year_month_day date;
    std::vector<Ref<drivers::Identity>> drivers;
};
SWOFTY_RECORD(Retirement, date, drivers)

struct Market {
    static constexpr Version version{1, 0};
    std::string data_group;
    std::vector<Retirement> retirements;
    bool firings = true;
    std::vector<Ref<drivers::Identity>> available;
    json::Value extra;
};
SWOFTY_RECORD(Market, data_group, retirements, firings, available)
SWOFTY_RECORD_REGISTRY(Market, "career.market")
inline Registry<Market> markets{"career.market", {.applies = Applies::read, .snapshot = true}};

struct ContractRules {
    static constexpr Version version{1, 0};
    std::string data_group;
    std::map<std::string, std::array<int, 2>> rating_ranges;
    std::array<int64_t, 2> salary{};
    int secret_meetings = 9;
    bool counteroffers = true;
    json::Value extra;
};
SWOFTY_RECORD(ContractRules, data_group, rating_ranges, salary, secret_meetings, counteroffers)
inline Registry<ContractRules> contract_rules{"career.contract_rules", {.applies = Applies::boot}};

struct RndUpgrade {
    int cost = 0, weighting = 0, size = 0;
    int type = 0;
    Ref<RndDepartment> department;
    std::vector<int> links;
    std::array<int, 2> grid_coordinates{};
    int unlock_at_facility_level = 0;
};
SWOFTY_RECORD(RndUpgrade, cost, weighting, size, type, department, links, grid_coordinates, unlock_at_facility_level)

struct RndTree {
    static constexpr Version version{1, 0};
    std::string data_group;
    std::string token;
    std::vector<Ref<RndDepartment>> departments;
    std::vector<RndUpgrade> nodes;
    json::Value extra;
};
SWOFTY_RECORD(RndTree, data_group, token, departments, nodes)
inline Registry<RndTree> rnd_trees{"career.rnd.tree", {.applies = Applies::boot}};

struct Facility {
    static constexpr Version version{1, 0};
    Text name;
    std::string group;
    std::vector<std::string> perk_tree;
    StatId stat_id = StatId::automatic;
    json::Value extra;
};
SWOFTY_RECORD(Facility, name, group, perk_tree, stat_id)
inline Registry<Facility> facilities{"career.facility", {.applies = Applies::boot}};

struct Economy {
    static constexpr Version version{1, 0};
    std::string data_group;
    std::map<std::string, float> resource_point_rates;
    std::map<std::string, int64_t> budgets;
    int64_t initial_cash = 0;
    json::Value extra;
};
SWOFTY_RECORD(Economy, data_group, resource_point_rates, budgets, initial_cash)
inline Registry<Economy> economy{"career.economy", {.applies = Applies::boot}};

struct Sponsor {
    static constexpr Version version{1, 0};
    Text name;
    Path logo;
    bool title = false;
    std::vector<int> loyalty;
    StatId stat_id = StatId::automatic;
    json::Value extra;
};
SWOFTY_RECORD(Sponsor, name, logo, title, loyalty, stat_id)
inline Registry<Sponsor> sponsors{"career.sponsor", {.applies = Applies::boot}};

struct WizardStep {
    std::string route;
    Text title;
    std::vector<std::string> options;
};
SWOFTY_RECORD(WizardStep, route, title, options)

struct Wizard {
    static constexpr Version version{1, 0};
    Ref<Content> content;
    std::vector<WizardStep> steps;
    json::Value extra;
};
SWOFTY_RECORD(Wizard, content, steps)
inline Registry<Wizard> wizards{"career.wizard", {.applies = Applies::content}};

namespace events {

struct SeasonStart {
    static constexpr const char *point = "career.season.start";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    int32_t year;
    int32_t season_index;
};

struct AddActivity {
    static constexpr const char *point = "career.calendar.add_activity";
    static constexpr uint32_t version = 1;
    static constexpr uint8_t contract_event = 0x0f;
    uint32_t size, payload_version;
    void *calendar;
    const void *activity;
    uint32_t driver;
    uint32_t day;
    uint8_t type;
    bool player;
};

struct StandingsRebuild {
    static constexpr const char *point = "career.standings.rebuild";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    void *championship;
};

#define SWOFTY_CAREER_EVENT_(T, name)                                                                                  \
    struct T {                                                                                                         \
        static constexpr const char *point = name;                                                                     \
        static constexpr uint32_t version = 1;                                                                         \
        uint32_t size, payload_version;                                                                                \
    };
SWOFTY_CAREER_EVENT_(Created, "career.created")
SWOFTY_CAREER_EVENT_(Loaded, "career.loaded")
SWOFTY_CAREER_EVENT_(Unloaded, "career.unloaded")
SWOFTY_CAREER_EVENT_(SeasonEnd, "career.season.end")
SWOFTY_CAREER_EVENT_(SeasonBreak, "career.season.break")
SWOFTY_CAREER_EVENT_(Saving, "career.saving")
#undef SWOFTY_CAREER_EVENT_

}

struct ApplyReport {
    size_t changed = 0, unchanged = 0, refused = 0;
    std::vector<std::string> refusals;
};

namespace detail {

inline std::atomic<const SwoftyCareerOps *> &ops_override()
{
    static std::atomic<const SwoftyCareerOps *> t{nullptr};
    return t;
}

inline void set_ops_table(const SwoftyCareerOps *t) { ops_override() = t; }

inline const SwoftyCareerOps *ops_table()
{
    if (auto t = ops_override().load())
        return t->version >= 1 ? t : nullptr;
#if defined(_WIN32)
    static std::atomic<const SwoftyCareerOps *> found{nullptr};
    if (auto t = found.load())
        return t;
    HMODULE core = GetModuleHandleW(L"swofty_core.dll");
    auto get = core ? reinterpret_cast<SwoftyCoreCareerOpsFn>(GetProcAddress(core, "SwoftyCoreCareerOps")) : nullptr;
    const SwoftyCareerOps *t = get ? get() : nullptr;
    if (t && t->version < 1)
        t = nullptr;
    found = t;
    return t;
#else
    return nullptr;
#endif
}

inline const SwoftyCareerOps *live_ops()
{
    auto t = ops_table();
    return t && t->available && t->available() ? t : nullptr;
}

inline Result ops_result(int r)
{
    return r == SWOFTY_ASSET_OK ? Result::ok
           : r == SWOFTY_ASSET_LATE ? Result::late
           : r == SWOFTY_ASSET_UNAVAILABLE ? Result::unavailable
                                           : Result::invalid;
}

inline bool stat_of(const SwoftyCareerOps *t, const Ref<drivers::Identity> &d, uint32_t &out)
{
    std::string id(d.id());
    return !id.empty() && t->driver_stat(id.c_str(), &out) == SWOFTY_ASSET_OK;
}

}

// The core's career operations (swofty_api.h SwoftyCareerOps, the core export SwoftyCoreCareerOps). On a core
// without them every call returns unavailable.
inline const SwoftyCareerOps *ops() { return detail::live_ops(); }

// A data group's career team slots: slot N (1-based) races as team stat team_stats[N-1]. apply_lineup reads
// the table of the season's data group.
inline Result set_slots(std::string_view data_group, const std::vector<uint32_t> &team_stats)
{
    auto t = detail::live_ops();
    if (!t)
        return Result::unavailable;
    return detail::ops_result(t->set_slots(std::string(data_group).c_str(), team_stats.data(), team_stats.size()));
}

inline Result apply_lineup(const Season &season, ApplyReport *report = nullptr)
{
    if (report)
        *report = {};
    auto t = detail::live_ops();
    if (!t)
        return Result::unavailable;
    std::string request = "{\"season\":" + to_json(season) + "}";
    std::string out(1024, '\0');
    size_t len = 0;
    int r = t->apply_lineup({request.data(), request.size()}, out.data(), out.size(), &len);
    if (r == SWOFTY_TOO_SMALL) {
        out.assign(len + 1, '\0');
        r = t->apply_lineup({request.data(), request.size()}, out.data(), out.size(), &len);
    }
    if (report && len < out.size()) {
        json::Value v = json::parse(std::string_view(out.data(), len));
        report->changed = static_cast<size_t>(v["changed"].as_int());
        report->unchanged = static_cast<size_t>(v["unchanged"].as_int());
        report->refused = static_cast<size_t>(v["refused"].as_int());
        if (const json::Array *list = v["refusals"].if_array())
            for (const json::Value &why : *list)
                report->refusals.emplace_back(why.as_string());
    }
    return detail::ops_result(r);
}

inline Result set_seat(int, int, Ref<drivers::Identity>) { return Result::unavailable; }

inline Result set_driver_ratings(Ref<drivers::Identity> driver, const DriverRatings &ratings)
{
    auto t = detail::live_ops();
    if (!t)
        return Result::unavailable;
    uint32_t stat = 0;
    if (!detail::stat_of(t, driver, stat))
        return Result::invalid;
    const uint16_t ids[] = {0, 1, 3, 4};
    const float values[] = {ratings.experience, ratings.racecraft, ratings.awareness, ratings.pace};
    return detail::ops_result(t->set_driver_ratings(stat, ids, values, 4));
}

inline Result set_car_number(Ref<drivers::Identity>, int) { return Result::unavailable; }

inline Result swap_drivers(Ref<drivers::Identity> a, Ref<drivers::Identity> b)
{
    auto t = detail::live_ops();
    if (!t)
        return Result::unavailable;
    uint32_t sa = 0, sb = 0;
    if (!detail::stat_of(t, a, sa) || !detail::stat_of(t, b, sb))
        return Result::invalid;
    return detail::ops_result(t->swap_drivers(sa, sb));
}

inline Result apply_dropped_scores(const events::StandingsRebuild &, int) { return Result::unavailable; }

}
