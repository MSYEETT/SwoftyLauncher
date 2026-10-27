---
title: Game systems
description: Every game system in standard library 2.0 as a typed C++ header - its records, registries, events and operations, when each applies in the game, how ready the reverse engineering is, and the limits the core enforces for every registry.
---

# Game systems

Each game system is a C++ header under `swofty/systems/`, with its record structs, typed registry handles, events and operations, in its own namespace.

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/sounds.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto v10 = sounds::engine_sounds.add("v10_screamer", { .bundle = file("sounds/v10_screamer.bdl"),
                                                            .settings_from = sounds::engine_sounds["game/ferrari_2025"] });
    cars::vehicles.layer(cars::vehicles["swofty.classic-seasons-2017/ferrari"], [=](cars::Vehicle &v) { v.engine_sound = v10; });
}

SWOFTY_ADDON()
```

Every system page has the header, a complete addon modelled on a real one, smaller examples for the rest, and the readiness and limits. Each main example also shows the same addon as data files, for addons with no DLL.

## The map

- **[Seasons and GP/TT content](/std/content)** `swofty/systems/content.hpp`: `content::packages`, `era_boxes`.
- **[Cars](./cars)** `swofty/systems/cars.hpp`: `cars::vehicles`, `packages`, `handling`, `damage_models`, `parts`.
- **[Liveries](./liveries)** `swofty/systems/liveries.hpp`: `liveries::liveries`, `decal_sets`, `sponsor_packs`, `ai_assignments`.
- **[Teams](./teams)** `swofty/systems/teams.hpp`: `teams::identities`, `logos`, `pit_times`.
- **[Drivers](./drivers)** `swofty/systems/drivers.hpp`: `drivers::identities`, `heads`, `looks`, `stats`.
- **[Helmets, suits and gloves](./wearables)** `swofty/systems/wearables.hpp`: `wearables::helmets`, `suits`, `gloves`, `season_sets`.
- **[Sounds](./sounds)** `swofty/systems/sounds.hpp`: `sounds::engine_sounds`, `car_audio`.
- **[Tracks](./tracks)** `swofty/systems/tracks.hpp`: `tracks::tracks`, `venues`, `data`, `per_package`, `layers`, `slots`, `ai_lines`.
- **[Tyres](./tyres)** `swofty/systems/tyres.hpp`: `tyres::physical`, `visual`, `classes`, `per_event`, `allocations`, `wear_scales`, `rules`.
- **[Power units, ERS and DRS](./power-units)** `swofty/systems/power.hpp`: `power::units`, `suppliers`, `drs_rules`, `component_rules`.
- **[Sessions and weekends](./sessions)** `swofty/systems/sessions.hpp`: `sessions::modes`, `formats`, `lengths`, `points`, `rules_profiles`, `safety_car`, `grids`.
- **[Weather](./weather)** `swofty/systems/weather.hpp`: `weather::climates`, `forecasts`.
- **[AI](./ai)** `swofty/systems/ai.hpp`: `ai::vehicle_configs`, `package_maps`, `track_packs`, `general`, `difficulty`.
- **[Career](./career)** `swofty/systems/career.hpp`: `career::contents`, `modes`, `seasons`, `calendars`, `ratings`, `markets`, `contract_rules`, `rnd_trees`, `rnd_departments`, `facilities`, `economy`, `sponsors`, `characters`, `wizards`.
- **[UI and strings](./ui)** `swofty/systems/ui.hpp`: `ui::screens`, `tiles`, `texture_slices`, `routes`, `options`, `hud_widgets`, `strings`.
- **[Graphics, replays and telemetry](./graphics)** `swofty/systems/graphics.hpp`: `graphics::looks`, `passes`, `settings`.
- **[Your own systems](./custom)**: an addon's own registries, scope, projection, scoped hooks and service.

## When a registry applies

Addons run live in the game. `SWOFTY_REGISTER` only declares; the core applies each change at the moment the game reads that data. Each registry says when, with `RegistryOptions::applies` (`x-applies` in its schema):

```cpp
inline Registry<Vehicle> vehicles{"vehicle", { .applies = Applies::boot }};   // database rows, before the database loads
inline Registry<PackageTrack> per_package{"track.per_package", { .applies = Applies::content }};   // when a box, career or season is active; put back after
inline Registry<TrackData> data{"track.data", { .applies = Applies::session }};   // at session load
inline Registry<Logo> logos{"team.logo", { .applies = Applies::read }};   // on every read, through the registry
```

Files the game opens by path (archives, engine bundles, VTFs) are served in memory through the core's runtime asset route when the game opens them.

An addon can also register and layer while the game runs, from `SWOFTY_ON_LOAD` or an event. The change takes effect at the next moment the game reads that data, and the core says when:

```text
[registry] layer from tails.whatif-history on career.season f1_career_progressive/2017 registered at run time; applies from the next session load
[registry] layer from mod.esp on vehicle game/ferrari_2026 registered at run time; applies from the next game start (boot data)
```

## Limits every registry inherits

The game has hard limits. In 1.x every addon finds them alone, usually as a crash. In 2.0 the core owns them once, for every registry.

**Stat ids.** The pools are small and differ per table. A record leaves `stat_id` at `StatId::automatic`, and the core allocates it:

```cpp
drivers::identities.variant("alain_prost", drivers::identities["game/jarno_opmeer"], [](drivers::Identity &d) {
    d.number = 1;                                  // d.stat_id stays StatId::automatic
});
```

```text
[registry] stat ids: driver f1990/alain_prost -> 241 (pool 208-254, 57 free after this)
[registry] stat ids: setup mode f1990/f1_1990 -> 16 (pool 5,7,16-31)
```

```text
entrant lists  0-255          drivers  0-254 (58 free)       teams  to 514
vehicles       to 538         heads    265+                  setup modes and seasons  <= 31 (mask bits)
```

An id the core allocated stays the same at every later start, so saves keep pointing at the same rows. Track ids are pinned for the install (see [Dependencies and guards](/std/dependencies)).

**The record budget.** The game's value map is sized when the database loads and grows at 838,860 records; growing it after load leaves dangling pointers. The core counts the records addons register in `SWOFTY_REGISTER` and sizes the map before the game loads the database. Until that lands (an RE item), the room is about 95k records minus a 4,096 margin. A registration past the room is refused, naming the addon:

```text
[registry] room: 81,240 of 91,904 records used (swofty.classic-seasons-* 15,300, mod.hockenheim 1,712, ...)
[registry] register: mod.bigpack refused 14,980 records: past the database room (10,664 left); nothing from mod.bigpack applied
```

**The grid.** 32 vehicle slots, a 27-car clamp at session enter, an AI table of 30, and a per-vehicle map that rehashes at the 26th car. A grid the build can't hold is refused when the registry resolves, naming the limit:

```cpp
sessions::grids.layer(sessions::grids["game/f1_2026"], [](sessions::Grid &g) { g.cars = 34; });
```

```text
[registry] session.grid game/f1_2026: edit by someone.biggrid refused: 34 cars, the game holds 32 vehicle slots (27 without
           the grid patch); the record keeps 26
```

**The rest:**

- **Row counts.** A table reads only the rows below its `_instancecount_`. The core bumps it; nobody writes it.
- **Missing keys.** A copied row that lacks a key a stock row has reads garbage, not null. The core copies every key of the donor or base row, the printf-built and stack-built ones included (`tools/track_keys.py`, `deps.rs`).
- **Side data.** Rows are found by label too: setup presets by package folder, track data by mode mask, AI data by `ai_package_folder/asset_filename`. The core writes the side data with the row.
- **Save heap.** Profile saves live in a 3 MB `SAVE_DATA` heap. Career snapshots are budgeted (see [Scopes and snapshots](/std/scopes)).
- **Read once.** Driver statistics (at boot), AI handling and setup presets (at session enter) are read once. Edits on them apply at those moments, or through an event.
- **Both builds.** Every event and operation resolves on the Steam and the EA build, or reports `unavailable`.

## The order

Systems move into the core in this order. It follows readiness, how many addons need each system, and today's conflicts:

1. **Career seasons, line-ups and contracts:** the most L2 code, and two addons already wrap the same functions.
2. **Content at boot:** vehicle, package, engine sound, livery, wearables, heads, team and driver identity, texture slices. The most addons, and most of today's file conflicts. The core owns stat-id allocation and the record budget from day one, because these registries fill the database.
3. **Tracks and track data.**
4. **Tyres:** the custom tyre systems.
5. **Session rules:** grid size, weekend formats, points.
6. **UI and strings.**
7. **Later:** AI, power unit and DRS at run time, weather, graphics, telemetry, replays. The reverse engineering they need is scheduled now: the AI difficulty application, the DRS decision, the engine component selector, the attribute map, a single team-logo resolver and the per-box track list builder.
