---
title: Standard library 2.0
description: The design for Swofty Launcher 2.0. The registry is the center of the standard library; addons register records and layers in code, hooks and services connect them, and the core applies everything live in the game.
---

# Standard library 2.0

**Status:** proposal. **Ships as:** Swofty Launcher 2.0.0, `SWOFTY_API_VERSION` 9.

In 2.0 the standard library is built around one idea: **the registry**. A registry is shared, typed data that any addon can provide, read and change, in code. Everything else in 2.0 either fills a registry, changes it, reacts to it or applies it to the game.

- **Each addon registers its own keys.** A GP/TT season, a car, an engine sound, a career season: each one is a record under the id of the addon that brings it, added with `add` in `SWOFTY_REGISTER`.
- **Records are bases others build on.** Another addon can extend a base in place with a `layer` (an engine sound for the 2017 Ferrari, everywhere) or make a `variant` of it under its own key (an "F1 2017 (V10)" GP/TT entry).
- **Everything is live in the game.** The core applies each change when the game reads that data. The manifest only names the addon and its dependencies.
- **The standard library only ever grows.** 2.0 adds abilities and takes none away. Everything 1.x could do still works, and nothing is ever removed in a later version either (see [Versioning](/std/versioning)).

## The registry in one example

<Split>

Progressive Career knows who drove for which team in each season. In 2.0 it adds that to the `career.season` registry instead of keeping it private: one record per year, under its own id.

<template #example>
<Example root="swofty.progressive" code-file="native/src/register.cpp" files="swofty.json, data/years.json, native/progressive.dll">
<template #code>

```cpp
#include <swofty/events.hpp>
#include <swofty/registry.hpp>
#include <swofty/systems/career.hpp>

#include "history.hpp"   // progressive::history(), built from data/years.json

using namespace swofty;
using namespace swofty::events;

SWOFTY_REGISTER
{
    for (const progressive::Year &y : progressive::history().years())
        career::seasons.add(std::to_string(y.year), y.to_season());   // "swofty.progressive/2017"
}

SWOFTY_ON_LOAD
{
    on<career::events::SeasonStart>().after([](const career::events::SeasonStart &e, const Context &ctx) {
        if (auto s = career::seasons.get(career::seasons["swofty.progressive/" + std::to_string(e.year)]))
            career::apply_lineup(*s);    // the core's verified seat code, not Progressive's own
    });
}

SWOFTY_ADDON()
```

</template>
</Example>
</template>
</Split>

<Split>

A companion addon changes the 2017 grid without touching Progressive's files. Its edit runs on Progressive's season, in load order.

<template #example>
<Example root="tails.whatif-history" code-file="native/src/whatif.cpp" data-file="registry/layers.json" files="swofty.json, native/whatif.dll">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/career.hpp>
#include <swofty/systems/drivers.hpp>

using namespace swofty;

static Ref<drivers::Identity> d(const char *key) { return drivers::identities[std::string("game/") + key]; }

SWOFTY_REGISTER
{
    career::seasons.layer(career::seasons["swofty.progressive/2017"], [](career::Season &s) {
        s.teams.at(1).seats = { d("fernando_alonso"), d("valtteri_bottas") };
        s.teams.at(4).seats = { d("lewis_hamilton"), d("stoffel_vandoorne") };
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
{
  "career.season": {
    "swofty.progressive/2017": {
      "teams": [
        { "slot": 1, "seats": ["game/fernando_alonso", "game/valtteri_bottas"] },
        { "slot": 4, "seats": ["game/lewis_hamilton", "game/stoffel_vandoorne"] }
      ]
    }
  }
}
```

</template>
</Example>
</template>
</Split>

Any code that needs the grid reads the **effective** value: Progressive's base with every layer applied in load order.

```cpp
// any DLL: read the season the player will race
if (auto s = career::seasons.get(career::seasons["swofty.progressive/2017"]))
    for (const career::SeasonTeam &t : s->teams)
        swofty::log("slot %d: %s and %s", t.slot, std::string(t.seats[0].id()).c_str(), std::string(t.seats[1].id()).c_str());
```

```text
slot 1: game/fernando_alonso and game/valtteri_bottas
slot 4: game/lewis_hamilton and game/stoffel_vandoorne
```

When a new career starts, the core freezes the effective seasons into that career, so later changes reach new careers and not the running one (see [Career snapshots](/std/scopes#career-snapshots)).

## A base someone else builds on

Classic Seasons 2017 adds the F1 2017 GP/TT season, its cars and its era box under its own keys. An engine sound pack adds its sound under its own key and points the 2017 Ferrari at it. That's a layer on Classic Seasons' record: no copied files, and with data files, no DLL.

```cpp
// mod.esp
SWOFTY_REGISTER
{
    auto v10 = sounds::engine_sounds.add("v10_screamer", { .bundle = file("sounds/v10_screamer.bdl"),
                                                           .settings_from = sounds::engine_sounds["game/ferrari_2025"] });
    cars::vehicles.layer(cars::vehicles["swofty.classic-seasons-2017/ferrari"], [=](cars::Vehicle &v) { v.engine_sound = v10; });
}
```

At the next game start, the core puts the record into the game database before it loads and serves the bundle in memory ([When changes apply](/std/build)).

## The map

The registry and its subfeatures come first. Then the parts that connect addons to each other and to the game.

- **[The registry](/std/registry)**: named, typed collections of records.
  - **Handles**: `Registry<T>` in a header per system, `add`, `variant`, `provide`, `layer`, `get`.
  - **Records are structs**: `SWOFTY_RECORD`, `Ref<T>` between records, `Keyed` lists, `extra` for the round trip.
  - **Your own registry**: a struct, an `inline Registry<T>` and a `check`, in a header you ship.
  - **[Keys, bases and variants](/std/bases)**: every addon registers its own keys; bases others extend in place or build variants of; copies become variants.
- **[Providers](/std/providers)**: who fills a registry's base.
  - **`add`**: records you know up front, in `SWOFTY_REGISTER`.
  - **Dynamic providers**: a callback the core calls on a read, cached until `invalidate`.
- **[Reading](/std/reading)**: getting data out.
  - **Effective values**: base plus every layer, with `status` when there's nothing.
  - **Origins**: which addon set a field.
  - **Listing**: every record id, or the ids under a prefix.
  - **Watching**: a callback when a record's effective value changes.
- **[Layers](/std/layers)**: how addons change each other's records.
  - **Edits**: a function on the effective value; the core records what it changed.
  - **Precedence**: load order, the same rule as files and mixins.
  - **Validation**: an edit whose result breaks the record drops that layer.
  - **Conflicts**: clashes are logged and reported, never silent.
  - **Selectors**: `where` and `under`, one layer for every matching record, including ones added later.
  - **Option-bound layers**: an edit that reads `swofty::option<T>()` re-runs when the option changes.
- **[Scopes and snapshots](/std/scopes)**: how long a change lasts.
  - **Global, profile and career scopes.**
  - **Career snapshots**: the effective values frozen into each career.
- **[Content registries](/std/content)**: GP/TT season packages, era boxes, cars, engine sounds, liveries, helmets, tracks and identities.
- **[Systems catalogue](/std/systems/)**: every game system's registries, hook points, operations and readiness, and the limits the core enforces for all of them.
- **[Your own systems](/std/systems/custom)**: an addon's own registries, scope, projection, scoped hooks and service; a six-compound tyre system others extend.
- **[Dependencies and guards](/std/dependencies)**: needing addons, registries and single records like tracks; what happens when they're missing; guards; careers that remember what they were made with.
- **[Events](/std/events)**: the lifecycle from boot to the chequered flag, with the context of everything, scoped and overridable by content owners.
- **[When changes apply](/std/build)**: at boot, when content becomes active, at a session load or on every read; files served in memory; the core's appliers; registering while the game runs.
- **[Versioning](/std/versioning)**: how registries, hook payloads and tables change over time, only ever growing.
  - **Schema versions**: a minor adds; a major adds a new version next to the old one.
  - **The tolerant reader rule**: ignore what you don't know.
  - **Every major forever**: adapters between versions.
  - **Status**: experimental, stable, superseded.
  - **ABI and table versions**, and **how a change is made**.
- **[Hooks](/std/hooks)**: one ordered chain per game function.
  - **Game hook points**, owned by the core.
  - **Phases**: `before`, `replace`, `after`.
  - **Addon hook points**: an addon's own extension points.
  - **Living with mixins**: raw mixins chain around hook points.
- **[Services](/std/services)**: one addon calling another, versioned, without `GetProcAddress`.
- **[Operations](/std/operations)**: the core's verbs that apply data to the game.
- **[The launcher side](/std/launcher)**: what the launcher does and doesn't know, editing pages that write an addon's storage, the conflicts view and Send Logs.
- **[Manifest](/std/manifest)**: `registry.json` holds only dependencies; everything else is code.
- **[The Simplified importer](/std/importer)**: imported mods recognised as records others stack on; everything else kept exactly as it builds today.
- **[Coverage](/design/std-2/coverage)**: every Addon Browser addon and the common imported mods, today and in 2.0.
- **[Compatibility](/std/compatibility)**: why every 1.x addon keeps working, and how the release proves it.
- **[The C ABI](/std/abi)**: the registration export and the tables under the C++ API.
- **[Rollout and docs](/design/std-2/rollout)**: the releases, and how the public docs are written.
- **[Implementation plan](/design/std-2/implementation)**: how parallel agents build 2.0.0 in rounds.

## Where 1.x is today

1.x has no shared data:

- **Private files:** an addon's storage is readable only by that addon, so nothing can read Progressive's `data/years.json`.
- **Private exports:** Progressive's DLLs talk to each other through `GetProcAddress` exports nobody else knows about.
- **Duplicate hooks:** Progressive Career and Career Editor each reverse-engineer and wrap the same contract, seat, rating and logo code.
- **Separate rules per library:** career contents, era boxes, Team HQ tabs, the career wizard, characters and strings each register, scope and resolve conflicts in their own way.

In 2.0 those libraries become registries and hook points underneath, while their 1.x calls keep working (see [Compatibility](/std/compatibility)).
