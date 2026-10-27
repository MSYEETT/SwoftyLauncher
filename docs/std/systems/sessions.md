---
title: Sessions and weekends
description: Setup modes, weekend formats, session lengths, points, rules profiles, safety car and grid size in code, with the entrant, grid, result and penalty events.
---

# Sessions and weekends

The session system is the setup mode a weekend runs under: its sessions, how the grid forms, the points, the rules and penalties, the safety car, and how many cars start.

## The header

<Split>

`swofty/systems/sessions.hpp` has seven registries:

- `sessions::modes`: a setup mode, the row everything else hangs off (`session.mode`);
- `sessions::formats`: a weekend structure: sessions, groups, grid rules (`session.weekend_format`);
- `sessions::lengths`: practice, qualifying and race length options (`session.length`);
- `sessions::points`: a standing preset (`session.points`);
- `sessions::rules_profiles`: a rule manager profile and its penalties (`session.rules_profile`);
- `sessions::safety_car`: safety car and red flag frequencies (`session.safety_car`);
- `sessions::grids`: an entrant setup mode: how many cars and teams (`session.grid`).

<template #example>
<Example root="swofty-sdk" code-file="include/swofty/systems/sessions.hpp">
<template #code>

```cpp
#pragma once
#include <swofty/registry.hpp>
#include <swofty/events.hpp>
#include <swofty/systems/drivers.hpp>
#include <swofty/systems/power.hpp>
#include <swofty/systems/tracks.hpp>

namespace swofty::sessions {

enum class Session : uint8_t {                 // the game's session id enum (0..18)
    practice_1 = 1, practice_2, practice_3,
    qualifying_1 = 5, qualifying_2, qualifying_3,
    qualifying_s = 9,                          // one-shot qualifying
    sprint_shootout_1 = 10, sprint_shootout_2, sprint_shootout_3,
    sprint_shootout_s,                         // one-shot sprint shootout
    race = 15, race_2, race_3,
    time_trial,
};

struct Points {
    std::vector<int> points;                   // per place, whole numbers; padded to the grid size
    std::optional<int> fastest_lap;            // fastest_lap_bonus; nullopt: none
    std::array<std::vector<int>, 3> reduced;   // reduced_points_data: races stopped at 75, 50 and 25 % distance
    StatId stat_id = StatId::automatic;        // a game_setup.session_standings row
};
inline Registry<Points> points{"session.points", { .applies = Applies::boot }};

struct GridRule;                               // how a race group's grid forms
GridRule grid_type(std::string_view game_type);                    // one of the game's grid types: "qualifying", ...
GridRule best_of(std::vector<Session> sessions);                   // each car's best time over several sessions

enum class GroupType { practice, qualifying, race };

struct Group {
    GroupType type;
    std::vector<Session> sessions;             // in order
    Text label;                                // the name the group shows; the core scopes it to this mode
    GridRule grid = grid_type("qualifying");   // race groups: how the grid forms
    Ref<Points> standings;                     // race groups: the points
};

struct Format {
    std::vector<Group> groups;                 // practice, qualifying and race groups of one weekend_structure
};
inline Registry<Format> formats{"session.weekend_format", { .applies = Applies::boot }};

struct Length {
    float laps_multiplier = 1.0f;              // race laps from the track's full distance
    int laps_override = 0;                     // fixed laps; 0: use the multiplier
    float tyre_wear_scale = 1.0f;
    int weather_slots = 0;                     // forecast slots across the session
    int flashback_limit = -1;                  // -1: unlimited
};
inline Registry<Length> lengths{"session.length", { .applies = Applies::boot }};

struct RulesProfile {
    bool blue_flags = true;
    int corner_cutting = 2;                    // 0 off .. 3 strict
    bool collisions = true;
    bool drive_through_replacement = false;    // a drive-through becomes a time penalty
    bool jump_starts = true;
    bool parc_ferme = true;
};
inline Registry<RulesProfile> rules_profiles{"session.rules_profile", { .applies = Applies::boot }};

struct SafetyCar {
    std::array<int, 8> max_deployments = { 0, 0, 3, 3, 2, 3, 5, 8 };   // per race length option
    std::array<int, 8> max_red_flags = { 0, 0, 1, 1, 1, 1, 2, 3 };
};
inline Registry<SafetyCar> safety_car{"session.safety_car", { .applies = Applies::boot }};

struct Grid {
    int cars = 22;                             // max_entrants; the core raises _entrant_instancecount_ and the track slots with it
    int teams = 11;                            // max_teams
    bool reuse_entrants = false;
};
inline Registry<Grid> grids{"session.grid", { .applies = Applies::boot }};

struct Mode {
    std::string branding;                      // a game_setup.branding row: logos, MFD, car setup
    Ref<Format> format;                        // weekend_options
    Ref<Length> race_length;
    Ref<RulesProfile> rules;
    Ref<SafetyCar> safety;
    Ref<Grid> grid;                            // entrant_setup_modes
    Ref<power::DrsRule> drs;                   // drs_allowed_lap_group
    bool pitstops = true;                      // enable_pitstops
    bool night_starts = false;                 // night_starts_enabled
    std::vector<Ref<tracks::Track>> restricted_tracks;
    StatId stat_id = StatId::automatic;        // a game_setup_modes row: 31 or lower (a u32 mask)
};
inline Registry<Mode> modes{"session.mode", { .applies = Applies::boot }};

struct Rules {                                 // a box's session rules: the value content::Package::rules holds
    Ref<Mode> mode;                            // the setup mode the box runs under; empty: the box's default
    Ref<Points> points;                        // empty: the mode's
    bool refuelling = false;
    std::optional<float> race_fuel_kg;         // the race fuel load; nullopt: the track's
};
SWOFTY_RECORD(Rules, mode, points, refuelling, race_fuel_kg)

enum class PenaltyKind { warning, time, drive_through, stop_go, grid, disqualification };

namespace events {

struct Load { /* session.load (modify): the session about to load; see Events */ };

struct EntrantsSelect {                        // Sess_Select_Entrants
    static constexpr const char *point = "session.entrants.select";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    std::span<const Ref<drivers::Identity>> candidates;   // the pool
    uint32_t limit;                            // max_entrants
    std::vector<Ref<drivers::Identity>> *chosen;          // before handlers may fill it
};

struct GridBuild {                             // Game_GridSetup_SetupStartingGrid
    static constexpr const char *point = "session.grid.build";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    const GridRule *rule;                      // the race group's grid rule
    std::array<uint8_t, 32> *vehicle_id;       // the starting order; before handlers may write it
    uint8_t count;
};

struct GroupResult {                           // Sess_Build_GroupResult
    static constexpr const char *point = "session.grid.group_result";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    uint8_t group;                             // the session group whose result is built
    void *result;                              // GroupResult rows: car, best time, position
};

struct PenaltyIssue {                          // Sess_Issue_TimePenalty, _GridPlacePenalty, Sess_Apply_DecidedPenalties
    static constexpr const char *point = "session.penalty.issue";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    uint32_t car;
    PenaltyKind kind;                          // drive_through, stop_go, time, grid, ...
    float amount;                              // seconds or places; before handlers may change it; replace = veto
};

// also: WeekendStart (session.weekend.start), ResultsFinalise (session.results.finalise),
// StateEnter (session.state.enter), SafetyCarDeploy (session.safety_car.deploy)

} // namespace events

Result set_grid(std::span<const uint8_t> order);                   // during session.grid.build
Result issue_penalty(uint32_t car, PenaltyKind kind, float amount);

} // namespace swofty::sessions
```

</template>
</Example>
</template>
</Split>

## F1 1990's weekend

<Split>

A 1990 weekend has two practice sessions, Friday qualifying, Saturday qualifying and the race. The grid comes from each car's better time over the two qualifying sessions. Points go 9-6-4-3-2-1, with no point for the fastest lap, and only a driver's best 11 results count.

The mode is a variant of the 2026 mode, so it carries all of its 280 keys. The grid size follows the addon's `cars` option, 22 or 26.

<template #example>
<Example root="swofty.f1-1990" code-file="native/src/rules.cpp" data-file="registry/rules.json" files="swofty.json, native/CMakeLists.txt">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/events.hpp>
#include <swofty/systems/sessions.hpp>
#include <swofty/systems/career.hpp>

using namespace swofty;
using sessions::Session;
using sessions::GroupType;

SWOFTY_REGISTER
{
    auto points = sessions::points.add("f1_1990_race", {
        .points = { 9, 6, 4, 3, 2, 1 },
        .fastest_lap = std::nullopt,
        .reduced = { { { 4, 3, 2, 1, 1 } }, { { 4, 3, 2, 1, 1 } }, { { 4, 3, 2, 1, 1 } } },   // half points, rounded
    });

    auto weekend = sessions::formats.add("two_qualifying", { .groups = {
        { .type = GroupType::practice,   .sessions = { Session::practice_1, Session::practice_2 } },
        { .type = GroupType::qualifying, .sessions = { Session::sprint_shootout_s }, .label = "lng_f1990_qualifying_1" },
        { .type = GroupType::qualifying, .sessions = { Session::qualifying_s },      .label = "lng_f1990_qualifying_2" },
        { .type = GroupType::race,       .sessions = { Session::race },
          .grid = sessions::best_of({ Session::sprint_shootout_s, Session::qualifying_s }),
          .standings = points },
    } });

    auto grid = sessions::grids.variant("f1_1990", sessions::grids["game/f1_26"], [](sessions::Grid &g) {
        g.cars = option<int>("cars");                      // 22 or 26; applies from the next game start
        g.teams = g.cars / 2;
    });

    sessions::modes.variant("f1_1990", sessions::modes["game/f1_26"], [=](sessions::Mode &m) {
        m.format = weekend;
        m.grid = grid;
    });
}

SWOFTY_ON_LOAD
{
    // best 11 results: the core rebuilds the standings, then drops the rest
    events::on<career::events::StandingsRebuild>().in(events::content("swofty.f1-1990/f1_career_1990"))
        .after([](const career::events::StandingsRebuild &s, const events::Context &) {
            career::apply_dropped_scores(s, 11);           // keeps the gross totals for the UI
        });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
{
  "session.points": {
    "f1_1990_race": { "points": [9, 6, 4, 3, 2, 1], "fastest_lap": null,
                      "reduced": [[4, 3, 2, 1, 1], [4, 3, 2, 1, 1], [4, 3, 2, 1, 1]] }
  },
  "session.weekend_format": {
    "two_qualifying": { "groups": [
      { "type": "practice",   "sessions": ["practice_1", "practice_2"] },
      { "type": "qualifying", "sessions": ["sprint_shootout_s"], "label": "lng_f1990_qualifying_1" },
      { "type": "qualifying", "sessions": ["qualifying_s"],      "label": "lng_f1990_qualifying_2" },
      { "type": "race", "sessions": ["race"], "grid": { "best_of": ["sprint_shootout_s", "qualifying_s"] },
        "standings": "swofty.f1-1990/f1_1990_race" }
    ] }
  },
  "session.grid": {
    "f1_1990": { "extends": "game/f1_26", "cars": 26, "teams": 13 }
  },
  "session.mode": {
    "f1_1990": { "extends": "game/f1_26", "format": "swofty.f1-1990/two_qualifying", "grid": "swofty.f1-1990/f1_1990" }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: swofty.f1-1990 (f1_1990_rules.dll) 1 session.points, 1 session.weekend_format, 1 session.grid,
           1 session.mode (early stage, 0.5 ms)
[registry] stat ids: session.mode swofty.f1-1990/f1_1990 -> 16 (pool 5, 7, 16-31)
[registry] applied at boot: session.mode 1 (280 keys from game/f1_26), session.weekend_format 1, session.points 1 (padded to 26),
           session.grid 1 (26 entrants, option cars) (412 database records)
[events] session.grid.build in setup mode swofty.f1-1990/f1_1990: grid rule best_of(sprint_shootout_s, qualifying_s) by the core
[events] career.standings.rebuild in swofty.f1-1990/f1_career_1990: 1 after handler (swofty.f1-1990)
```

The data version has no dropped scores: those need the handler. A data file can't follow an option either: it states one grid size, or ships one file per value with a guard.

## Grid rules

`best_of` is a grid rule the core adds to `session.grid.build`, so no addon reimplements it. For a rule the core doesn't have, replace the group result yourself:

```cpp
events::on<sessions::events::GroupResult>().in(events::setup_mode("swofty.f1-1990/f1_1990"))
    .replace([](sessions::events::GroupResult &g, const events::Context &) {
        return f1990::rank_reverse_top(g, 8);              // true: handled; the game's result isn't built
    });
```

A group result is built only when the group's last session id is 7, 8, 9, 12, 13 or 14 (mask 0x7380 in 0x140af83a0). A Q1/Q2 knockout can't be a format: it needs a core hook there.

## Lengths, rules and the safety car

```cpp
sessions::lengths.layer(sessions::lengths["game/race_50"], [](sessions::Length &l) { l.flashback_limit = 3; });

auto strict = sessions::rules_profiles.variant("strict", sessions::rules_profiles["game/f1_26"], [](sessions::RulesProfile &r) {
    r.corner_cutting = 3;
    r.drive_through_replacement = false;
});

sessions::safety_car.layer(sessions::safety_car["game/default"], [](sessions::SafetyCar &s) {
    s.max_deployments[7] = 4;                              // the longest race length: 4 safety cars, not 8
});
```

## Events and operations

```cpp
// a 20-second time penalty instead of every drive-through
events::on<sessions::events::PenaltyIssue>().when(events::session_type({ SessionType::race }))
    .before([](sessions::events::PenaltyIssue &p, const events::Context &) {
        if (p.kind == sessions::PenaltyKind::drive_through) {
            p.kind = sessions::PenaltyKind::time;
            p.amount = 20.0f;
        }
    });

// a reverse grid for a sprint, from inside the grid build
events::on<sessions::events::GridBuild>().in(events::content("mod.reverse/sprint")).before(
    [](sessions::events::GridBuild &g, const events::Context &) {
        std::reverse(g.vehicle_id->begin(), g.vehicle_id->begin() + std::min<int>(g.count, 10));
    });

sessions::issue_penalty(car, sessions::PenaltyKind::grid, 3.0f);
```

## Readiness and limits

- **Ready:** the entrant, grid, result and safety car handlers are L2, used by Custom Grid Size and F1 1990.
- **Partial:** points and penalties: the data is ready, the runtime code is L1.
- **Unknown:** flags and red flags, the knockout cut writer, red-flag restarts, penalty decision thresholds at run time, the difficulty preset selector.
- **Grid:** 32 vehicle slots, a 27-car clamp at session enter, an AI table of 30, a wheel map that rehashes at the 26th car, replay holders sized for 22 to 24 cars. `sessions::grids` refuses a size the game can't hold, naming the limit.
- **Points** are whole numbers in arrays of 23, padded to the grid size. 1990's half points are rounded.
- **Setup modes** take stat ids of 31 or lower (free: 5, 7, 16 to 31). A mode needs the content set and an available mode row.
- **Option restrictions** are tags shared across modes (`gamesetup_f1`).
