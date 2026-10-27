---
title: Game systems
description: Every game system in standard library 2.0 - its registries, hook points and operations, how ready the reverse engineering is, and the limits the core enforces for every registry.
---

# Game systems

Every system an addon can touch has a domain here. Each domain lists:

- **registries:** what addons provide, layer and extend, at build time or run time;
- **hook points:** the game functions the core owns, one ordered chain each;
- **operations:** the core's verbs;
- **readiness:** how much of it the reverse engineering has verified;
- **limits:** the game's own limits that the core enforces for it.

The [first-party addons](/design/std-2/coverage) and [your own systems](./custom) are built from these.

## The systems

- **[Cars](./cars)**: car packages, handling, damage and parts as registries; the hook points and operations for cars at run time.
- **[Liveries](./liveries)**: liveries, decal sets and sponsor packs as registries, and the hook points for special liveries.
- **[Teams](./teams)**: team identity, logos per context and pit times as registries, with the logo and name hook points.
- **[Drivers](./drivers)**: driver identity, heads, looks and season ratings as registries, with the ratings, number and head hook points.
- **[Helmets, suits and gloves](./wearables)**: helmets, suits, gloves and team kits per season as registries.
- **[Sounds](./sounds)**: engine sounds and a car's audio data as registries, and the bundle hook point.
- **[Tracks](./tracks)**: tracks, per-season track data, world layers and slots as registries, with the track data and fuel hook points.
- **[Tyres](./tyres)**: tyre compounds, classes, allocations, wear and rules as registries, with the class, allocation and set-choice hook points.
- **[Power units, ERS and DRS](./power-units)**: power units, engine suppliers, DRS and component rules as registries, with the engine wear and penalty hook points.
- **[Sessions and weekends](./sessions)**: setup modes, weekend formats, points, rules profiles, safety car and grid as registries, with the session hook points.
- **[Weather](./weather)**: track climate and scripted forecasts as registries, and the weather setup hook point.
- **[AI](./ai)**: aI vehicle configs, difficulty maps and baked track packs as registries, with the AI config hook points.
- **[Career](./career)**: career modes, seasons, calendars, contracts, R&D, economy and characters as registries, with the career hook points.
- **[UI and strings](./ui)**: era boxes, screens, tiles, texture slices, options, HUD widgets and strings as registries, with the UI hook points.
- **[Graphics, replays and telemetry](./graphics)**: looks, post-process passes, graphics settings, replay cameras and telemetry channels.
- **[Seasons and GP/TT content](/std/content)**: a whole GP/TT season as records, and other addons building on it.
- **[Your own systems](./custom)**: an addon's own registries, scope, projection, scoped hooks and service.

## Limits every registry inherits

The game has hard limits. In 1.x every addon finds them alone, usually as a crash. In 2.0 the core owns them, once, for every registry:

```text
row counts     A table only reads rows below its _instancecount_. Appliers bump it; nobody writes it by hand.
missing keys   A copied row that lacks a key a stock row has reads garbage, not null. Appliers copy every key
               of the donor row, including the printf-built and stack-built ones (tools/track_keys.py, deps.rs).
side data      Rows are found by label too: setup presets by package folder, track data by mode mask, AI data
               by ai_package_folder/asset_filename. Appliers write the side data with the row.
record budget  The game's value map is sized at load and grows at 838,860 records; runtime edits get about 95k.
               The launcher counts every applier's records and bakes the database when it has to.
stat ids       Small, per-table pools: entrant lists 0-255, drivers 0-254 (58 free), teams to 514, vehicles to
               538, setup modes and seasons <= 31. The core allocates them; a record says "stat_id": "auto".
grid           32 vehicle slots, a 27-car clamp at session enter, an AI table of 30, a wheel map that rehashes
               at the 26th car. session.grid refuses a size the build can't hold, naming the limit.
save heap      Profile saves live in a 3 MB SAVE_DATA heap. Snapshots are budgeted (see Scopes and snapshots).
boot-once      Driver statistics, AI handling and setup presets are read once (at boot or session enter).
               Runtime layers on them fire at those moments, through hook points.
EA build       Every hook point and operation resolves on both the Steam and the EA build, or reports unavailable.
```

```json
// a record asking the core for ids instead of picking them
{ "driver.identity": { "f1990/alain_prost": { "stat_id": "auto", "name": "lng_f1990_prost", "number": 1 } } }
```

```text
[registry] stat ids: driver f1990/alain_prost -> 241 (pool 208-254, 57 free after this)
[registry] stat ids: setup mode f1990/f1_1990 -> 16 (pool 5,7,16-31)
```

An id the core allocated stays the same in every later build of the profile, so saves keep pointing at the same rows.

## The order

Domains move into the core in this order. It follows readiness, how many addons need each domain, and today's conflicts:

1. **Career seasons, line-ups and contracts:** the most L2 code, and two addons already wrap the same functions.
2. **Content at build time:** vehicle, package, engine sound, livery, wearables, heads, team and driver identity, texture slices. The most addons, and most of today's file conflicts.
3. **Tracks and track data.**
4. **Tyres:** the custom tyre systems.
5. **Session rules:** grid size, weekend formats, points.
6. **UI and strings.**
7. **Later:** AI, power unit and DRS at run time, weather, graphics, telemetry, replays. The reverse engineering they need is scheduled now: the AI difficulty application, the DRS decision, the engine component selector, and the attribute map.
