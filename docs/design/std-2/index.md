---
title: Standard library 2.0
description: The design for Swofty Launcher 2.0. The registry is the center of the standard library; providers fill it, layers override it, hooks and services connect addons, operations apply it to the game.
---

# Standard library 2.0

**Status:** proposal. **Ships as:** Swofty Launcher 2.0.0, `SWOFTY_API_VERSION` 9.

In 2.0 the standard library is built around one idea: **the registry**. A registry is shared data that any addon can provide, read and override. Everything else in 2.0 either fills a registry, changes it, reacts to it or applies it to the game.

- **Each addon registers its own keys.** A GP/TT season, a car, an engine sound, a career season: each one is a record under the id of the addon that brings it.
- **Records are bases others build on.** Another addon can extend a base in place (an engine sound for the 2017 Ferrari, everywhere) or make a variant of it with its own key (an "F1 2017 (V10)" GP/TT entry).
- **The standard library only ever grows.** 2.0 adds abilities and takes none away. Everything 1.x could do still works, and nothing is ever removed in a later version either (see [Versioning](./versioning)).

## The registry in one example

Progressive Career knows who drove for which team in each season. In 2.0 it puts that into the `career.season` registry instead of keeping it private:

```cpp
// progressive.dll: provide the base value of every season
#include <swofty/registry.hpp>
#include <swofty/career/season.hpp>

using swofty::career::Season;

static std::optional<swofty::json::Value> season(std::string_view id)
{
    // id is "<data group>/<year>", e.g. "f1_career_progressive/2017"
    std::optional<int> year = progressive::year_of(id);
    if (!year)
        return std::nullopt;
    Season s = progressive::history().season(*year); // built from data/years.json
    return s.to_json();
}

SWOFTY_ON_LOAD
{
    swofty::registry::provide(Season::registry, season);
}
```

A companion addon changes the 2017 grid without touching Progressive's files. This is a static layer, a JSON file named in its manifest:

```json
// tails.whatif-history/layers/2017.json, a layer of career.season "f1_career_progressive/2017"
{
  "teams": [
    { "slot": 1, "seats": ["fernando_alonso", "valtteri_bottas"] },
    { "slot": 4, "seats": ["lewis_hamilton", "stoffel_vandoorne"] }
  ]
}
```

Any code that needs the grid reads the **effective** value, which is Progressive's base with every layer applied in load order:

```cpp
// any DLL: read the season the player will race
std::optional<Season> s = swofty::registry::get<Season>("f1_career_progressive/2017");
if (s) {
    for (const auto &team : s->teams)
        swofty::log("slot %d: %s and %s", team.slot, team.seats[0].c_str(), team.seats[1].c_str());
}
// slot 1: fernando_alonso and valtteri_bottas
// slot 4: lewis_hamilton and stoffel_vandoorne
```

When a new career starts, the core freezes the effective seasons into that career, so later edits change new careers and not the running one. Then Progressive applies the season to the game through a core operation:

```cpp
swofty::career::apply_lineup(*s); // the core's verified seat code, not Progressive's own
```

## A base someone else builds on

Classic Seasons 2017 registers the F1 2017 GP/TT season under its own keys:

```json
// swofty.classic-seasons-2017/registry/content.json
{
  "content.package": {
    "swofty.classic-seasons-2017/f1_2017": {
      "entrant_list": "cs_f1_2017",
      "teams": [ { "key": "ferrari", "vehicle": "swofty.classic-seasons-2017/ferrari" } ]
    }
  },
  "vehicle": {
    "swofty.classic-seasons-2017/ferrari": { "label": "cs17_ferrari", "engine_sound": "swofty.classic-seasons-2017/ferrari" }
  },
  "era.box": {
    "swofty.classic-seasons-2017/f1_2017": { "package": "swofty.classic-seasons-2017/f1_2017", "order": 2017 }
  }
}
```

An engine sound pack adds its sound under its own key, and points the 2017 Ferrari at it. That's a layer on Classic Seasons' record. There's no DLL, and no copied files:

```json
// mod.esp/registry/content.json
{ "engine_sound": { "mod.esp/v10_screamer": { "bundle": "sounds/v10_screamer.bdl", "settings_from": "game/ferrari_2025" } } }
```

```json
// mod.esp/layers/ferrari_2017.json, a layer of vehicle "swofty.classic-seasons-2017/ferrari"
{ "engine_sound": "mod.esp/v10_screamer" }
```

When the profile builds, the launcher resolves the records and writes the bundle and its database rows into the game ([Build time and run time](./build)).

## The map

The registry and its subfeatures come first. Then the parts that connect addons to each other and to the game.

- **[The registry](./registry)**: named, shared collections of records.
  - **[Keys, bases and variants](./bases)**: every addon registers its own keys; bases others extend in place or build variants of.
  - **Records**: an id and a JSON value.
  - **Schemas**: the shape a registry's records must have (`x-key` arrays, snapshot settings).
  - **Typed records**: C++ structs for the core's registries, and `swofty::json` for everything else.
- **[Providers](./providers)**: who fills a registry's base.
  - **Static providers**: JSON files in the addon. The launcher reads them too.
  - **Dynamic providers**: a callback the core calls on a read, cached until `invalidate`.
- **[Reading](./reading)**: getting data out.
  - **Effective values**: base plus every layer.
  - **Origins**: which addon set a field.
  - **Listing**: every record id, or the ids under a prefix.
  - **Watching**: a callback when a record's effective value changes.
- **[Layers](./layers)**: how addons override each other.
  - **Patches**: JSON Merge Patch, with keyed arrays.
  - **Precedence**: load order, the same rule as files and mixins.
  - **Validation**: a patched record that breaks its schema drops that layer.
  - **Conflicts**: clashes are logged and shown in the launcher, never silent.
  - **Selectors**: one layer for every matching record, including ones added later.
  - **Option-bound layers**: a layer that applies while one of the addon's options has a value.
- **[Scopes and snapshots](./scopes)**: how long a change lasts.
  - **Global, profile and career scopes.**
  - **Career snapshots**: the effective values frozen into each career.
- **[Content registries](./content)**: GP/TT season packages, era boxes, cars, engine sounds, liveries, helmets, tracks and identities.
- **[Systems catalogue](./systems)**: every game system's registries, hook points, operations and readiness, and the limits the core enforces for all of them.
- **[Your own systems](./custom-systems)**: an addon's own registries, scope, projection, scoped hooks and service; a six-compound tyre system others extend.
- **[Dependencies and guards](./dependencies)**: needing addons, registries and single records like tracks; what happens when they're missing; guards; careers that remember what they were made with.
- **[Events](./events)**: the lifecycle from boot to the chequered flag, with the context of everything, scoped and overridable by content owners.
- **[Build time and run time](./build)**: which registries the launcher builds into the game, which the core reads in game, and the appliers between records and game files.
- **[Versioning](./versioning)**: how registries, hook payloads and tables change over time, only ever growing.
  - **Schema versions**: minor adds; a major adds a new version next to the old one.
  - **The tolerant reader rule**: ignore what you don't know.
  - **Every major forever**: adapters between versions.
  - **Status**: experimental, stable, superseded.
  - **ABI and table versions**, and **how a change is made**.
- **[Hooks](./hooks)**: one ordered chain per game function.
  - **Game hook points**, owned by the core.
  - **Phases**: `before`, `replace`, `after`.
  - **Addon hook points**: an addon's own extension points.
  - **Living with mixins**: raw mixins chain around hook points.
- **[Services](./services)**: one addon calling another, versioned, without `GetProcAddress`.
- **[Operations](./operations)**: the core's verbs that apply data to the game.
- **[The launcher side](./launcher)**: registries in the build, launcher pages that edit layers, the conflicts view and Send Logs.
- **[Manifest](./manifest)**: how an addon declares all of the above.
- **[The Simplified importer](./importer)**: imported mods recognised as records others stack on; everything else kept exactly as it builds today.
- **[Coverage](./coverage)**: every Addon Browser addon and the common imported mods, today and in 2.0.
- **[Compatibility](./compatibility)**: why every 1.x addon keeps working, and how the release proves it.
- **[The C ABI](./abi)**: the tables under the C++ wrappers.
- **[Rollout and docs](./rollout)**: the releases, and how the public docs are written.
- **[Implementation plan](./implementation)**: how parallel agents build 2.0.0 in rounds.

## Where 1.x is today

1.x has no shared data:

- **Private files:** an addon's storage is readable only by that addon, so nothing can read Progressive's `data/years.json`.
- **Private exports:** Progressive's DLLs talk to each other through `GetProcAddress` exports nobody else knows about.
- **Duplicate hooks:** Progressive Career and Career Editor each reverse-engineer and wrap the same contract, seat, rating and logo code.
- **Separate rules per library:** career contents, era boxes, Team HQ tabs, the career wizard, characters and strings each register, scope and resolve conflicts in their own way.

In 2.0 those libraries become registries and hook points underneath, while their 1.x calls keep working (see [Compatibility](./compatibility)).
