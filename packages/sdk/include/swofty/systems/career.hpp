// SPDX-License-Identifier: MIT
#pragma once
#include <array>
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
// mode, so the career.mode record struct is ModeRecord. The operations ship in a later core and return
// unavailable until then.

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

inline Result apply_lineup(const Season &, ApplyReport *report = nullptr)
{
    if (report)
        *report = {};
    return Result::unavailable;
}
inline Result set_seat(int, int, Ref<drivers::Identity>) { return Result::unavailable; }
inline Result set_driver_ratings(Ref<drivers::Identity>, const DriverRatings &) { return Result::unavailable; }
inline Result set_car_number(Ref<drivers::Identity>, int) { return Result::unavailable; }
inline Result swap_drivers(Ref<drivers::Identity>, Ref<drivers::Identity>) { return Result::unavailable; }
inline Result apply_dropped_scores(const events::StandingsRebuild &, int) { return Result::unavailable; }

}
