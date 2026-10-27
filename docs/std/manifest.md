---
title: Manifest
description: swofty.json names the addon; registry.json, next to it, only holds requires, recommends and data-file guards. Everything an addon registers is code, in standard library 2.0.
---

# Manifest

An addon names itself in `swofty.json`, the 1.x manifest, unchanged. Everything it registers is code. `registry.json`, an optional file next to it, only holds `requires`, `recommends` and data-file `guards`.

<Split>

Progressive Career 2.0 provides seasons and ratings, makes its career mode a variant of the stock one, replaces a hook, defines its own hook point and publishes a service. All of it is in the DLL.

Its manifests stay small:

- `swofty.json` is the 1.x manifest, apart from the version;
- `registry.json` only says what Progressive needs and recommends, so the launcher can check it before the game starts.

<template #example>
<Example root="swofty.progressive" code-file="native/src/progressive.cpp" files="swofty.json, registry.json, native/progressive.dll, data/years.json, api/progressive_api/hooks.hpp, api/progressive_api/history.hpp">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/hooks.hpp>
#include <swofty/services.hpp>
#include <swofty/systems/career.hpp>
#include <swofty/systems/tracks.hpp>

#include <progressive_api/hooks.hpp>     // shipped in api/ for other addons to include
#include <progressive_api/history.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    // its career mode: a variant of the stock 2026 Driver Career
    career::modes.variant("progressive", career::modes["game/f1_career_2026"], [](career::Mode &m) {
        m.data_group = "f1_career_progressive";
        m.save_container = "driver_career_progressive";
        m.title = "lng_progressive_title";
    });

    // every season from its own data/years.json: swofty.progressive/2016 ... /2025
    for (const career::Season &s : progressive::history().seasons())
        career::seasons.add(std::to_string(s.year), s);

    // ratings: computed when read, listed for the ids it knows
    career::ratings.provide(progressive::ratings, progressive::list_ratings);

    // the 2016 Russian Grand Prix needs Sochi; optional, so a missing track drops that round
    progressive::sochi = tracks::venues["venue/sochi"].optional();
}

SWOFTY_ON_LOAD
{
    hooks::replace<career::events::AddActivity>(progressive::keep_contract_off_calendar);
    hooks::define<progressive_api::SeasonApply>();
    services::publish(progressive::kHistory);
}

SWOFTY_ADDON()
```

</template>
</Example>
</template>
</Split>

```json
// swofty.json: the 1.x manifest, unchanged apart from the version
{
  "schema_version": 1,
  "id": "swofty.progressive",
  "name": "Progressive Career",
  "version": "2.0.0",
  "author": "Swofty",
  "kind": ["database", "native"],
  "category": "career",
  "compat": { "game": "f1_25", "launcher": ">=1.5.15" },
  "requires": [ { "id": "swofty.classic-seasons", "version": ">=0.6.0" } ],
  "operations": { "native": [ { "dll": "native/progressive.dll", "files": ["data/"] } ] }
}
```

```json
// registry.json: what it needs and recommends, checked before the game starts
{
  "requires": [
    { "registry": "career.season", "version": "^1" },
    { "registry": "career.driver_ratings", "version": "^1" }
  ],
  "recommends": [
    { "record": "track:venue/sochi", "for": "the 2016 Russian Grand Prix" }
  ]
}
```

```text
[registry] register: swofty.progressive (progressive.dll) 1 career.mode, 10 career.season, 1 provider on career.driver_ratings (early stage, 1.8 ms)
```

- **`swofty.json` gets no new fields.** It's still the 1.x manifest: id, name, version, compat, options, operations, launcher.
- **A 1.x launcher never opens `registry.json`,** and a 1.x core never calls `SWOFTY_REGISTER`. On launcher 1.5 this addon is Progressive Career 1.x: its database rows and DLLs, which keep their own `years.json` path. On 2.0 it adds the registry records the other addons build on.
- **The build turns on `"early": true`** for a DLL with a `SWOFTY_REGISTER`. You don't set it.
- **Headers others include** (`api/progressive_api/hooks.hpp`, `api/progressive_api/history.hpp`) ship in the addon's folder. Its docs page links them.

## What moved to code

Every field `registry.json` used to have is now a call. The manifest has no entry for anything an addon registers:

```cpp
// registries: an addon's own registry is a handle and a record struct
struct Preset { std::string name; int start_year = 2016; static constexpr Version version{1, 0}; };
SWOFTY_RECORD(Preset, name, start_year);
inline Registry<Preset> presets{"tails.whatif.presets"};

// provides: base records under your own keys, variants, and dynamic providers
presets.add("real_2016", { .name = "real_2016", .start_year = 2016 });
career::modes.variant("progressive", career::modes["game/f1_career_2026"], edit);
career::ratings.provide(compute, list);

// overrides: layers on anyone's records, at a scope
career::seasons.layer(career::seasons["swofty.progressive/2017"], edit);
career::seasons.layer(career::seasons["swofty.progressive/2017"], edit, Scope::profile);

// select: one layer for every match, now and later
cars::handling.layer(where<cars::Handling>([](const cars::Handling &h) { return h.cls == cars::Class::f1; }), edit);
ai::vehicle_configs.layer(under<ai::VehicleConfig>("swofty.classic-seasons-2017/"), edit);

// when: an edit that reads one of your options re-runs when the player changes it
cars::handling.layer(f1_cars, [](cars::Handling &h) {
    if (swofty::option<std::string>("damage") == "heavy")
        h.damage_scale = 1.6f;
});

// hooks and services: in SWOFTY_ON_LOAD
hooks::replace<career::events::AddActivity>(handler);
hooks::define<progressive_api::SeasonApply>();
services::publish(table);

// guards: in SWOFTY_ON_LOAD
guards::on(guards::Point::career_content_open, check);
```

- **Registries:** [The registry](/std/registry). **Provides:** [Keys, bases and variants](/std/bases), [Providers](/std/providers). **Overrides:** [Layers](/std/layers), [Scopes](/std/scopes).
- **Selectors:** [Selectors](/std/layers#selectors). **Options:** [Option-bound layers](/std/layers#option-bound-layers).
- **Hooks and services:** [Hooks](/std/hooks), [Events](/std/events), [Services](/std/services). **Guards:** [Guards](/std/dependencies#guards).
- **No `editable` flag.** A launcher page writes its addon's own per-profile state, and the addon's code applies it (see [The launcher side](/std/launcher#editing-a-layer-from-a-launcher-page)).

## Options stay in swofty.json

An option stays in `swofty.json`, where ADDON SETTINGS and the launcher already find it (see [options](/reference/addons/manifest#options)). An edit reads it with `swofty::option<T>()`:

<Split>

Heavier Damage Realism has one damage level option instead of 16 pre-built copies of every car. Its edit reads the option, so the core re-runs it when the player changes the level.

<template #example>
<Example root="william09leo.heavier-damage-realism" code-file="native/src/damage.cpp" files="swofty.json, native/damage.dll">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto f1 = where<cars::Handling>([](const cars::Handling &h) { return h.cls == cars::Class::f1; });

    cars::handling.layer(f1, [](cars::Handling &h) {
        std::string level = swofty::option<std::string>("damage");   // declared in swofty.json
        if (level == "heavy")
            h.damage_scale = 1.6f;
        else if (level == "brutal")
            h.damage_scale = 2.4f;
    });
}

SWOFTY_ADDON()
```

</template>
</Example>
</template>
</Split>

```json
// swofty.json
{ "options": [ { "id": "damage", "type": "choice", "values": ["realistic", "heavy", "brutal"], "default": "realistic" } ] }
```

## An addon only useful with 2.0

Tails' "what if" addon only layers registries, so it's useless without 2.0 and says so with `"compat": { "launcher": ">=2.0.0" }` in `swofty.json`. A 1.x launcher then says it needs a newer launcher instead of installing something that does nothing.

```json
// swofty.json
{
  "schema_version": 1,
  "id": "tails.whatif-history",
  "name": "What If History",
  "version": "1.0.0",
  "author": "Tails",
  "kind": ["native"],
  "category": "career",
  "compat": { "game": "f1_25", "launcher": ">=2.0.0" },
  "operations": { "native": [ { "dll": "native/whatif.dll" } ] },
  "launcher": {
    "program": { "windows": "bin/whatif.exe", "linux": "bin/whatif" },
    "sections": [ { "id": "history", "title": "What if history" } ],
    "build": true
  }
}
```

```json
// registry.json
{
  "requires": [
    { "registry": "career.season", "version": "^1" },
    { "registry": "career.driver_ratings", "version": "^1" },
    { "service": "progressive.history", "version": ">=1" }
  ]
}
```

Its layers are in its DLL (see [Scopes](/std/scopes)), and its page is in [The launcher side](/std/launcher#editing-a-layer-from-a-launcher-page).

## Requirements in code and in the manifest

<Split>

In code, a requirement is stated where it's used:

- a hard `Ref` is **required**: the core checks it in game, and a guard stops the player with a popup;
- `.optional()` is **recommended**: a missing record drops the element that holds it.

The `registry.json` copy lets the launcher check before the game starts, and before the addon has ever run. It's optional: without it, the in-game guards still stop the player cleanly.

An addon without code states the same in `registry.json` only.

<template #example>
<Example root="mod.esp" code-file="native/src/esp.cpp" data-file="registry.json" files="swofty.json, sounds/v10_screamer.bdl, native/esp.dll">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/tracks.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    // a hard reference: required
    auto ferrari = cars::vehicles["swofty.classic-seasons-2017/ferrari"];

    // optional: recommended
    auto sochi = tracks::venues["venue/sochi"].optional();

    // ...
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
{
  "requires":   [ { "record": "vehicle:swofty.classic-seasons-2017/ferrari" } ],
  "recommends": [ { "record": "track:venue/sochi", "for": "the 2016 Russian Grand Prix" } ]
}
```

</template>
</Example>
</template>
</Split>

## Data-file guards

An addon with code adds guards in code. An addon without code declares them in `registry.json`:

```json
{
  "guards": [
    { "on": "career.start", "mode": "my_team",
      "block": { "title": "My Team not available",
                 "body": "This addon only works in Driver Career. To play My Team, turn the addon off in the launcher, then relaunch." } }
  ]
}
```

See [Guards](/std/dependencies#guards) for the points, `block`, `require` and the code form.

## The fields of registry.json

- **`requires`:** an array of what this addon needs, each one of:
  - `{ "registry": "career.season", "version": "^1" }`
  - `{ "service": "progressive.history", "version": ">=1" }`
  - `{ "hook": "career.calendar.add_activity", "version": ">=1" }`
  - `{ "record": "vehicle:swofty.classic-seasons-2017/ferrari", "version": ">=1.0.0" }` (`version` optional)

  Play stops while one is missing. See [Dependencies and guards](/std/dependencies).
- **`recommends`:** the same entries, optional, each with a `for` shown to players. They never stop Play.
- **`guards`:** declarative guards, for addons without code.

## Addon requirements stay in swofty.json

`swofty.json`'s 1.x `requires` (`[{id, version}]`, see [requires](/reference/addons/manifest#requires-conflicts-load-after)) keeps meaning "this addon needs that addon". `registry.json`'s `requires` adds finer dependencies: a single track, a registry version, a service, a hook point. The launcher checks both, in the same place, with the same messages.
