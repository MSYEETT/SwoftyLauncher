---
title: Career
description: Career modes, contents, seasons, calendars, ratings, the driver market, contracts, R&D, facilities, economy, sponsors, characters and wizards in code, with the career events and operations.
---

# Career

The career system is Driver Career and My Team: the mode and its tile, the seasons and line-ups, the calendar, ratings, contracts, R&D, money and the people in it.

## The header

<Split>

`swofty/systems/career.hpp` has fourteen registries. The main ones:

- `career::modes`: a mode row, its save container and data group (`career.mode`);
- `career::contents`: a career tile and the data it clones (`career.content`);
- `career::seasons`: a season's line-up and mid-season changes (`career.season`);
- `career::calendars`: an event list (`career.calendar`);
- `career::ratings`: a driver's ratings at the start of a season (`career.driver_ratings`).

The rest: `markets`, `contract_rules`, `rnd_trees`, `rnd_departments`, `facilities`, `economy`, `sponsors`, `characters`, `wizards`.

<template #example>
<Example root="swofty-sdk" code-file="include/swofty/systems/career.hpp">
<template #code>

```cpp
#pragma once
#include <swofty/registry.hpp>
#include <swofty/events.hpp>
#include <swofty/systems/drivers.hpp>
#include <swofty/systems/teams.hpp>
#include <swofty/systems/tracks.hpp>

namespace swofty::career {

enum class Kind { driver_career, my_team };

struct Mode {
    std::string data_group;                    // partitions 8 career tables and the save slots: "f1_career_progressive"
    std::string save_container;                // save_data.name: the mode's own container in the profile save
    Text title;                                // the career tile's title
    Kind kind = Kind::driver_career;
    StatId stat_id = StatId::automatic;        // a game_modes.ModeChampionship row
};
SWOFTY_RECORD(Mode, data_group, save_container, title, kind, stat_id)
inline Registry<Mode> modes{"career.mode", { .applies = Applies::boot }};

struct Content {
    Ref<Mode> mode;
    Path tile;                                 // the tile image
    Ref<Content> model;                        // clone the 8 data-group tables, championship and components from it
    Ref<Calendar> calendar;
};
SWOFTY_RECORD(Content, mode, tile, model, calendar)
inline Registry<Content> contents{"career.content", { .applies = Applies::boot }};   // (tile, gate, slot predicate)

struct SeasonTeam {
    int slot = 0;                              // 1-11, the career team that carries this season's team
    Ref<teams::Identity> team;                 // empty: the slot is empty this season
    std::array<Ref<drivers::Identity>, 2> seats;
};
SWOFTY_RECORD(SeasonTeam, slot, team, seats)

enum class ChangeKind { swap, stand_in };

struct SeasonChange {
    int round = 0, slot = 0, seat = 0;         // from this round on, this seat
    Ref<drivers::Identity> out, in;
    ChangeKind kind = ChangeKind::swap;
    std::optional<int> until_round;            // stand_in only
};
SWOFTY_RECORD(SeasonChange, round, slot, seat, out, in, kind, until_round)

struct Season {
    static constexpr Version version{1, 0};
    std::string data_group;
    int year = 0;
    Keyed<SeasonTeam, &SeasonTeam::slot> teams;
    Keyed<SeasonChange, &SeasonChange::round, &SeasonChange::slot, &SeasonChange::seat> changes;
    json::Value extra;                         // fields from a newer minor, kept for the round trip
};
SWOFTY_RECORD(Season, data_group, year, teams, changes)
inline Registry<Season> seasons{"career.season", { .applies = Applies::read, .snapshot = true, .snapshot_budget = 65536 }};

struct Round {
    int round = 0;
    Text race;                                 // the race name: "lng_gp_usa"
    Ref<tracks::Venue> venue;                  // resolves to the first enabled track that provides it
    std::chrono::year_month_day date;          // the game dates weekends from the track data in the mode's year
};
SWOFTY_RECORD(Round, round, race, venue, date)

struct Calendar {
    Keyed<Round, &Round::round> rounds;        // schedule.event_lists rows
    int seasons = 10;                          // _season_instancecount_: the career length
    std::vector<int> transfer_windows;         // the round after which a window opens
};
inline Registry<Calendar> calendars{"career.calendar", { .applies = Applies::boot }};

struct DriverRatings {
    Ref<Season> season;
    Ref<drivers::Identity> driver;
    int experience, racecraft, awareness, pace;   // 0..100
};
SWOFTY_RECORD(DriverRatings, season, driver, experience, racecraft, awareness, pace)
inline Registry<DriverRatings> ratings{"career.driver_ratings", { .applies = Applies::read, .snapshot = true, .snapshot_budget = 65536 }};

struct RndDepartment {
    Text name;
    Colour colour;
    Path icon;
    float cost_scale = 1.0f;                   // UpgradeDepartmentEntry economy scale
};
inline Registry<RndDepartment> rnd_departments{"career.rnd.department", { .applies = Applies::content }};

struct Character {
    Ref<Character> replaces;                   // the stock character it stands in for, per data group
    Text first_name, last_name;
    Path portrait;                             // the core makes the message_avatars slice
    std::vector<std::string> mute;             // speech lines it mutes: "agent", "agent_cutscene"
};
inline Registry<Character> characters{"career.character", { .applies = Applies::content }};

inline Registry<Market> markets{"career.market", { .applies = Applies::read, .snapshot = true }};   // retirements, firings, availability
inline Registry<ContractRules> contract_rules{"career.contract_rules", { .applies = Applies::boot }};   // salary, rating ranges, secret meetings
inline Registry<RndTree> rnd_trees{"career.rnd.tree", { .applies = Applies::boot }};             // one team's tree: nodes, links, coordinates
inline Registry<Facility> facilities{"career.facility", { .applies = Applies::boot }};           // a My Team facility and its perk tree
inline Registry<Economy> economy{"career.economy", { .applies = Applies::boot }};                // resource point rates, budgets, initial cash
inline Registry<Sponsor> sponsors{"career.sponsor", { .applies = Applies::boot }};               // a sponsor and its loyalty track
inline Registry<Wizard> wizards{"career.wizard", { .applies = Applies::content }};               // new-career setup steps

namespace events {

struct SeasonStart {                           // a season begins: the first, then each after the break (notify)
    static constexpr const char *point = "career.season.start";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    int year;
    int season_index;
};

struct AddActivity {                           // Game_CareerActivities_AddActivity (cancel)
    static constexpr const char *point = "career.calendar.add_activity";
    static constexpr uint32_t version = 1;
    static constexpr uint8_t contract_event = 0x0f;
    uint32_t size, payload_version;
    void *calendar;                            // the driver calendar the activity goes on
    const void *activity;                      // the activity definition
    uint32_t driver;                           // the calendar's driver, stat id << 16
    uint32_t day;                              // before handlers may move it
    uint8_t type;                              // the activity type
    bool player;                               // the calendar is the human player's
};

struct StandingsRebuild {                      // Career_Rebuild_SeasonStandings (modify)
    static constexpr const char *point = "career.standings.rebuild";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    void *championship;                        // the standings the game just rebuilt
};

// also: Loaded (career.loaded), Unloaded (career.unloaded), ContractDecide (career.contract.decide), ContractSign (career.contract.sign),
// SeasonProcess (career.season.process), LineupChanges (career.lineup.changes),
// WeekendCreate (career.weekend.create), RndStart (career.rnd.start), and the lifecycle in Events

} // namespace events

struct ApplyReport {                           // what apply_lineup did, seat by seat
    size_t changed = 0, unchanged = 0, refused = 0;
    std::vector<std::string> refusals;         // one line per refused seat: which seat, and why
};

Result apply_lineup(const Season &season, ApplyReport *report = nullptr);
Result set_seat(int slot, int seat, Ref<drivers::Identity> driver);
Result set_driver_ratings(Ref<drivers::Identity> driver, const DriverRatings &ratings);
Result set_car_number(Ref<drivers::Identity> driver, int number);
Result swap_drivers(Ref<drivers::Identity> a, Ref<drivers::Identity> b);   // the game's full move sequence
Result apply_dropped_scores(const events::StandingsRebuild &s, int best_results);   // keeps gross totals for the UI

} // namespace swofty::career
```

</template>
</Example>
</template>
</Split>

## Progressive's career mode

<Split>

Progressive Career plays Driver Career from 2016 to 2026 with each year's real line-ups and ratings. Its mode is a variant of the 2026 Driver Career with its own data group and save container, so its careers never mix with stock ones, and changes other addons make to the stock mode flow into it.

It reads its own `data/years.json` and registers a season and a set of ratings per year. At each season start it applies the line-up.

<template #example>
<Example root="swofty.progressive" code-file="native/src/register.cpp" data-file="registry/career.json" files="swofty.json, native/CMakeLists.txt, native/src/years.hpp, native/src/years.cpp, data/years.json, art/tile.png">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/events.hpp>
#include <swofty/systems/career.hpp>
#include "years.hpp"                               // progressive::years(): data/years.json, its own format

using namespace swofty;

static const events::Selector kProgressive = events::content("swofty.progressive/f1_career_progressive");

SWOFTY_REGISTER
{
    auto mode = career::modes.variant("progressive", career::modes["game/f1_career_2026"], [](career::Mode &m) {
        m.data_group = "f1_career_progressive";
        m.save_container = "driver_career_progressive";
        m.title = "lng_progressive_title";
    });

    career::contents.add("f1_career_progressive", {
        .mode = mode,
        .tile = file("art/tile.png"),
        .model = career::contents["game/f1_career_2026"],
    });

    for (const progressive::Year &y : progressive::years()) {
        auto season = career::seasons.add(std::to_string(y.year), {      // "swofty.progressive/2017"
            .data_group = "f1_career_progressive",
            .year = y.year,
            .teams = y.teams(),                    // slot, team, two seats
            .changes = y.changes(),                // mid-season swaps and stand-ins by round
        });
        for (const progressive::Rating &r : y.ratings)
            career::ratings.add(std::format("{}/{}", y.year, r.driver), {
                .season = season,
                .driver = drivers::identities[r.driver],
                .experience = r.experience, .racecraft = r.racecraft, .awareness = r.awareness, .pace = r.pace,
            });
    }
}

SWOFTY_ON_LOAD
{
    events::on<career::events::SeasonStart>().in(kProgressive)
        .after([](const career::events::SeasonStart &e, const events::Context &ctx) {
            auto season = ctx.record<career::Season>();    // the career's season, from its snapshot
            career::ApplyReport report;
            career::apply_lineup(*season, &report);
            swofty::log("[progressive] %d: %zu seat(s) set, %zu unchanged, %zu refused", e.year,
                        report.changed, report.unchanged, report.refused);
        });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
{
  "career.mode": {
    "progressive": { "extends": "game/f1_career_2026", "data_group": "f1_career_progressive",
                     "save_container": "driver_career_progressive", "title": "lng_progressive_title" }
  },
  "career.content": {
    "f1_career_progressive": { "mode": "swofty.progressive/progressive", "tile": "art/tile.png",
                               "model": "game/f1_career_2026" }
  },
  "career.season": {
    "2017": { "data_group": "f1_career_progressive", "year": 2017,
              "teams": [ { "slot": 1, "team": "game/mercedes", "seats": ["game/lewis_hamilton", "game/valtteri_bottas"] } ],
              "changes": [ { "round": 15, "slot": 7, "seat": 1, "out": "game/daniil_kvyat", "in": "game/pierre_gasly" } ] }
  },
  "career.driver_ratings": {
    "2017/game/lewis_hamilton": { "season": "swofty.progressive/2017", "driver": "game/lewis_hamilton",
                                  "experience": 94, "racecraft": 93, "awareness": 90, "pace": 95 }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: swofty.progressive (progressive.dll) 1 career.mode, 1 career.content, 11 career.season,
           242 career.driver_ratings (early stage, 3.8 ms)
[registry] stat ids: career.mode swofty.progressive/progressive -> 82
[registry] applied at boot: career.mode 1, career.content 1 (8 data-group tables cloned from game/f1_career_2026) (1,204 database records)
[registry] content swofty.progressive/f1_career_progressive active: career snapshot of career.season 11, career.driver_ratings 242 (61 KB)
[progressive] 2016: 22 seat(s) set, 0 unchanged, 0 refused
```

The data version has no season-start handler: it lists the seasons, and applying a line-up needs code.

## Calendars

```cpp
auto calendar = career::calendars.add("f1_1990", {
    .rounds = {
        { .round = 1, .race = "lng_gp_usa", .venue = tracks::venues["venue/phoenix"].or_else(tracks::venues["venue/las_vegas"]),
          .date = 1990y / 3 / 11 },
        { .round = 4, .race = "lng_gp_monaco", .venue = tracks::venues["venue/monaco"], .date = 1990y / 5 / 27 },
    },
    .seasons = 1,
});
```

The game dates every weekend from the track data in the mode's year. Saves keep their season list, so a longer calendar reaches new careers only.

## Contracts and the market

```cpp
// no retirements in Progressive careers: the real ones come from the seasons
career::markets.layer(career::markets["game/f1_career_2026"], [](career::Market &m) { m.retirements.clear(); });
```

## R&D

```cpp
// the Tyres department, coloured for 1990
career::rnd_departments.layer(career::rnd_departments["swofty.rnd-tyres-thermal/tyres"], [](career::RndDepartment &d) {
    d.colour = Colour("#22A45D");
    d.cost_scale = 0.8f;
});
```

The game draws four departments natively. A fifth needs the native layer R&D Tyres built, which moves into the core as the `career.rnd.department` applier. Team trees are found through 12 compiled-in tokens; a 13th tree needs native code.

## Characters and wizards

```cpp
career::characters.add("flavio", {
    .replaces = career::characters["game/elena"],
    .first_name = "lng_f1990_agent_first",
    .last_name = "lng_f1990_agent_last",
    .portrait = file("art/flavio.png"),
    .mute = { "agent", "agent_cutscene" },
});
```

## Events

```cpp
// Progressive: a Follow-your-driver player signs in the first Team HQ, so the contract event never goes on his calendar
events::on<career::events::AddActivity>().in(kProgressive)
    .replace([](career::events::AddActivity &a, const events::Context &) {
        if (a.type != career::events::AddActivity::contract_event || !a.player || progressive::follow_mode() != 1)
            return false;
        return progressive::sign_instead(a);
    });
```

## Operations

```cpp
career::set_seat(7, 1, drivers::identities["game/pierre_gasly"]);
career::set_driver_ratings(drivers::identities["game/lando_norris"], { .experience = 70, .racecraft = 88, .awareness = 85, .pace = 90 });
career::set_car_number(drivers::identities["game/lando_norris"], 1);
career::swap_drivers(drivers::identities["game/alex_albon"], drivers::identities["game/logan_sargeant"]);
```

A swap follows the game's own sequence (move confirmed and the entrant write, contract state 3, the line-up changes, three moves-finalised posts), or the career crashes. Every operation runs on the flow thread.

## Readiness and limits

- **The most ready system:** 59 career and 46 game functions at L2, saves at L3. Ready: contracts, line-ups, standings, resource points, upgrade start, cancel and cost, the calendar, activities, the wizard step, save containers.
- **Partial:** facilities, sponsors, the market, Team HQ tabs (L1).
- **Unknown:** the resource point spend, regulation changes, a 13th team tree.
- **The save heap** is 3 MB. Career snapshots have a budget per registry.
- **R&D:** four departments drawn natively; upgrade types are a table of 19 and a `std::bitset<19>` (ids of 19 and up throw without native code).
- **The 10-season career limit** is calendar data (`_season_instancecount_`), not code. Career days are 16-bit (about 200 seasons).
- **Saves keep the season list,** and the upgrade loader drops instances it doesn't know.
- **Hashed per-team career tables** can't gain rows. **Eight tables** are partitioned by data group.
- **My Team** freezes when the upgrade availability is refreshed over and over. **Follow-career scene paths** crashed new Progressive careers (#411).
