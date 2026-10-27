---
title: The registry
description: Registries, records, typed handles and record structs in standard library 2.0. You register records in code with SWOFTY_REGISTER.
---

# The registry

A **registry** is a named, typed collection of **records** that every addon in the profile shares. You add records, and change other addons' records, in code through the registry's handle.

<Split>

An engine sound pack adds its sound under its own key and points the 2017 Ferrari at it. `add` provides a record. `layer` changes someone else's.

Each registry is an `inline Registry<T>` in a system header. `T` is the record struct, so the compiler checks every field, and records point at each other with `Ref<T>`.

The **Without code** tab is the same addon with no DLL: the same records as data files in `registry/`.

<template #example>
<Example root="mod.esp" code-file="native/src/esp.cpp" data-file="registry/sounds.json" files="swofty.json, sounds/v10_screamer.bdl, native/esp.dll, registry/layers.json">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/sounds.hpp>
#include <swofty/systems/cars.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    // provide under your own key: the id is "mod.esp/v10_screamer"
    auto v10 = sounds::engine_sounds.add("v10_screamer", {
        .bundle = file("sounds/v10_screamer.bdl"),
        .settings_from = sounds::engine_sounds["game/ferrari_2025"],
    });

    // change another addon's record: an edit, run on its effective value in load order
    cars::vehicles.layer(cars::vehicles["swofty.classic-seasons-2017/ferrari"], [=](cars::Vehicle &v) {
        v.engine_sound = v10;
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/sounds.json: your records, under your own keys
{ "engine_sound": { "v10_screamer": { "bundle": "sounds/v10_screamer.bdl", "settings_from": "game/ferrari_2025" } } }
```

```json
// registry/layers.json: changes to other addons' records, by full id
{ "vehicle": { "swofty.classic-seasons-2017/ferrari": { "engine_sound": "mod.esp/v10_screamer" } } }
```

</template>
</Example>
</template>
</Split>

`swofty/loader.log`:

```text
[registry] register: mod.esp (esp.dll) 1 engine_sound, 1 layer on vehicle (early stage, 0.4 ms)
[registry] vehicle swofty.classic-seasons-2017/ferrari: edit by mod.esp changed /engine_sound -> mod.esp/v10_screamer
[registry] applied at boot: engine_sound 1 (1 bundle served in memory), vehicle 1 (3 database records)
```

## The handle

```cpp
namespace swofty {

template <class T> using Edit = std::function<void(T &)>;

template <class T> class Registry {
public:
    Registry(std::string_view name, RegistryOptions opts = {});
    std::string_view name() const;                                   // "vehicle", for logs, origins and data files

    // provide (your own keys)
    Ref<T> add(std::string_view key, T value);                       // base record "<your id>/<key>"
    Ref<T> variant(std::string_view key, Ref<T> base, Edit<T> edit); // extends base; base's layers flow in
    void provide(std::function<std::optional<T>(std::string_view id)> fn,
                 std::function<void(std::function<void(std::string_view)>)> list = {});   // dynamic
    void invalidate(Ref<T> id = {});
    void check(std::function<void(const T &, Errors &)> fn);         // the owner's own rules

    // change (anyone's records)
    void layer(Ref<T> target, Edit<T> edit, Scope scope = Scope::global);
    void layer(Select<T> which, Edit<T> edit, Scope scope = Scope::global);   // every match, now and later
    void clear_layer(Ref<T> target, Scope scope = Scope::global);    // drop this addon's layer

    // read
    Ref<T> operator[](std::string_view id) const;                   // reference by id (checked at resolve)
    std::optional<T> get(Ref<T> id) const;                           // the effective value
    Status status(Ref<T> id) const;                                  // why get returned nothing
    std::vector<Ref<T>> all(std::string_view prefix = {}) const;
    std::string origin(Ref<T> id, std::string_view field) const;     // which addon set a field ("engine_sound")
    Token watch(std::function<void(Ref<T>)> fn);
};

}
```

- `add`, `variant`: [Keys, bases and variants](/std/bases).
- `provide`, `invalidate`: [Providers](/std/providers).
- `get`, `status`, `all`, `origin`, `watch`: [Reading](/std/reading).
- `layer`, `clear_layer`, `check`, `where`, `under`, `option`: [Layers](/std/layers).
- `Scope`: [Scopes and snapshots](/std/scopes).
- `RegistryOptions`: when the game reads the data, in [When changes apply](/std/build).

`SWOFTY_REGISTER` runs once per game start, at the early stage, addon by addon in load order. You can also call the handle from `SWOFTY_ON_LOAD` and later, while the game runs: the change takes effect the next time the game reads that data.

## The core's registries

The core owns the registries that describe the game. Their handles live in one header per system under `swofty/systems/`, next to the system's events (`swofty::<system>::events`), hook payloads and operations (`swofty::cars::set_attribute(...)`):

```text
swofty/registry.hpp            Registry<T>, Ref<T>, Edit<T>, Select<T>, Scope, SWOFTY_REGISTER, swofty::option<T>()
swofty/systems/cars.hpp        swofty::cars      vehicles, packages, handling, damage_models, parts
swofty/systems/liveries.hpp    swofty::liveries  liveries, decal_sets, sponsor_packs, ai_assignments
swofty/systems/teams.hpp       swofty::teams     identities, logos, pit_times
swofty/systems/drivers.hpp     swofty::drivers   identities, heads, looks, stats
swofty/systems/wearables.hpp   swofty::wearables helmets, suits, gloves, season_sets
swofty/systems/sounds.hpp      swofty::sounds    engine_sounds, car_audio
swofty/systems/tracks.hpp      swofty::tracks    tracks, venues, data, layers, slots, ai_lines
swofty/systems/tyres.hpp       swofty::tyres     physical, visual, classes, per_event, allocations, wear_scales, rules
swofty/systems/power.hpp       swofty::power     units, suppliers, drs_rules, component_rules
swofty/systems/sessions.hpp    swofty::sessions  modes, formats, lengths, points, rules_profiles, safety_car, grids
swofty/systems/weather.hpp     swofty::weather   climates, forecasts
swofty/systems/ai.hpp          swofty::ai        vehicle_configs, package_maps, track_packs, general, difficulty
swofty/systems/career.hpp      swofty::career    contents, modes, seasons, calendars, ratings, markets, contract_rules,
                                                 rnd_trees, rnd_departments, facilities, economy, sponsors, characters, wizards
swofty/systems/content.hpp     swofty::content   packages (GP/TT season builds), era_boxes
swofty/systems/ui.hpp          swofty::ui        screens, tiles, texture_slices, routes, options, hud_widgets, strings
swofty/systems/graphics.hpp    swofty::graphics  looks, passes, settings
```

The 2.0 headers sit under `swofty/systems/`, so they never clash with 1.x headers like `swofty/career.hpp`. A handle's `name()` is the registry name the logs, origins and data files use: `cars::vehicles` is `vehicle`, `career::seasons` is `career.season`. Every system has a page in the [Systems catalogue](/std/systems/).

## Records are structs

A record is a plain struct. `SWOFTY_RECORD` lists its fields, which is its shape in data files, in Send Logs and in origin paths.

```cpp
// swofty/systems/career.hpp (abridged)
namespace swofty::career {

struct SeasonTeam {
    int slot = 0;                                   // 1-11, the career team that carries this season's team
    Ref<teams::Identity> team;                      // empty: the slot is empty this season
    std::array<Ref<drivers::Identity>, 2> seats;    // seat 0, seat 1
};
SWOFTY_RECORD(SeasonTeam, slot, team, seats)

enum class ChangeKind { swap, stand_in };

struct SeasonChange {
    int round = 0, slot = 0, seat = 0;
    Ref<drivers::Identity> out, in;
    ChangeKind kind = ChangeKind::swap;
    std::optional<int> until_round;                 // stand_in only
};
SWOFTY_RECORD(SeasonChange, round, slot, seat, out, in, kind, until_round)

struct Season {
    static constexpr Version version{1, 0};         // the schema this header was built against
    std::string data_group;
    int year = 0;
    Keyed<SeasonTeam, &SeasonTeam::slot> teams;     // matched by slot, not by position
    Keyed<SeasonChange, &SeasonChange::round, &SeasonChange::slot, &SeasonChange::seat> changes;
    json::Value extra;                              // fields from a newer minor, kept for the round trip
};
SWOFTY_RECORD(Season, data_group, year, teams, changes)

inline Registry<Season> seasons{"career.season", { .applies = Applies::read, .snapshot = true, .snapshot_budget = 65536 }};

}
```

- **`Ref<T>`** points at a record of another registry by id. Validation checks that it exists. `.optional()` and `.or_else(other)` say what happens when it doesn't (see [Dependencies and guards](/std/dependencies)).
- **`Keyed<T, &T::field...>`** is a list whose elements are matched by key. Edits, origins and conflicts name an element by its key (`teams/slot=1`), and a data file's layer changes one element without restating the list. A key can be several fields, as in `changes`. `at(key)` returns the element and appends it when it's missing, `find(key)` returns a pointer or null, `erase(key)` removes it.
- **`extra`** holds fields this header doesn't know. Code built against 1.0 reads a 1.1 record and writes it back without losing anything (see [the tolerant reader rule](/std/versioning#the-tolerant-reader-rule)).
- **`stat_id`:** the core allocates stat ids. A record's `stat_id` field is `StatId::automatic` unless you set it.
- **`RegistryOptions`** set when the game reads the registry's data (`applies`, see [When changes apply](/std/build)) and whether it's frozen into each career (`snapshot`, see [Career snapshots](/std/scopes#career-snapshots)).

## Your own registry

An addon declares a registry for its own data the same way the core does: a struct and an `inline Registry<T>`. Put them in a header you ship, so other addons can include it and add, layer and read.

<Split>

Tails' "what if" addon owns a registry of career setup presets. Its name starts with the addon's id.

The header is the registry's public shape. Other addons include it to add presets of their own or layer Tails' presets.

<template #example>
<Example root="tails.whatif-history" code-file="api/whatif/presets.hpp" files="swofty.json, native/src/whatif.cpp, native/whatif.dll">
<template #code>

```cpp
#pragma once
#include <swofty/registry.hpp>

namespace whatif {

enum class Rnd { real_life, hybrid };
enum class TeamMoves { follow_driver, choose_team };

struct Preset {
    static constexpr swofty::Version version{1, 0};
    std::string name;                                 // [a-z0-9_]+
    int start_year = 2016;                            // 2016-2026
    Rnd rnd = Rnd::real_life;
    TeamMoves team_moves = TeamMoves::follow_driver;
};
SWOFTY_RECORD(Preset, name, start_year, rnd, team_moves)

inline swofty::Registry<Preset> presets{"tails.whatif.presets"};

}
```

</template>
</Example>
</template>
</Split>

`check` adds the registry's own rules on top of the struct: ranges, patterns, anything the types can't say. It runs on the result of every edit:

```cpp
// native/src/whatif.cpp
SWOFTY_REGISTER
{
    whatif::presets.check([](const whatif::Preset &p, swofty::Errors &err) {
        if (p.start_year < 2016 || p.start_year > 2026)
            err.add("start_year", "must be 2016-2026");
        if (!swofty::is_key(p.name))
            err.add("name", "lowercase letters, digits and _ only");
    });
    whatif::presets.add("real_2016", { .name = "real_2016", .start_year = 2016 });
}
```

An addon that doesn't have your header can still read your registry, untyped:

```cpp
swofty::Registry<swofty::json::Value> presets{"tails.whatif.presets"};
if (auto p = presets.get(presets["tails.whatif-history/real_2016"])) {
    int year = (*p)["start_year"].as_int();                 // 2016
    std::string moves = (*p)["team_moves"].as_string();     // "follow_driver"
}
```

## Ids

```text
career.season                                  <- registry
  swofty.progressive/2016                      <- record id: <addon id>/<key>
  swofty.progressive/2017                      { data_group: "f1_career_progressive", year: 2017, teams: [...] }
career.driver_ratings
  swofty.progressive/2017/max_verstappen       { overall: 88, experience: 74, ... }
engine_sound
  game/ferrari_2025                            <- the core's record of the stock game
```

- **`add("key")` makes `<your addon id>/<key>`.** No addon can provide a record under another addon's id.
- **The core's records** describe the stock game and use `game/`: `game/ferrari_2025`, `game/f1_2026`.
- **Use `/` to build keys out of parts.** A prefix then lists a group: `swofty.progressive/` lists every Progressive season (see [Listing](/std/reading#listing)).

## Rules

- **Registry names** are dotted and lowercase. The core uses `career.`, `team.`, `driver.`, `session.`, `lang.`, `era.`, `hub.` and `addon.`. An addon's own registry starts with its id.
- **Values** are at most 1 MB as JSON.
- **A record has one base** from one [provider](/std/providers), or it's a [variant](/std/bases#build-a-variant-with-extends) of another record. Every change on top of it is a [layer](/std/layers).
- **Every write is validated** (the struct, every `Ref`, the owner's `check`) before it becomes visible.

## Without code

An addon with no DLL ships records as data files in `registry/*.json`. The core finds them by folder name; there's no manifest entry. A file holds records by registry name, then key:

```json
// registry/sounds.json
{ "engine_sound": { "v10_screamer": { "bundle": "sounds/v10_screamer.bdl", "settings_from": "game/ferrari_2025" } } }
```

Keys are your own: `v10_screamer` becomes `<your id>/v10_screamer`. A `Ref` is written as the record's full id. The SDK ships a JSON Schema per core registry and version for data files (`packages/sdk/std/registries/<name>/<major>.<minor>.json`), generated from the headers. Its keywords map to the code:

```text
x-registry                  Registry<T>::name()
x-version                   T::version
x-applies                   RegistryOptions::applies          boot | content | session | read
x-snapshot, -budget         RegistryOptions::snapshot, snapshot_budget
x-key                       Keyed<T, &T::field...>
x-ref                       Ref<T>
type, enum, required        the struct's field types
minimum, maximum, pattern   the owner's check
```

A data-only addon can't declare a registry of its own. It can add records to any registry and layer any record.
