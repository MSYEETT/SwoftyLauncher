---
title: What you can build
description: What you can make for Swofty Launcher - content, whole game systems, careers, in-game code, launcher pages and imported mods - built on the registry, and where each part of the docs starts.
---

# What you can build

You build **addons**: a folder with a `swofty.json` that players install on the **Addons** page. An addon can add content (a season, a car, a livery, a track, an engine sound), a whole system the game doesn't have (a tyre range, a points system, a career), code that runs in the game, or pages in the launcher. Players can also drop **mods** (`.serps` packages, game-file zips, track zips) on the importer, which turns them into addons.

Everything an addon adds goes into **the registry**: shared data every addon can provide, read and build on. Start there.

## One addon, start to finish

An engine sound pack that gives the 2017 Ferrari a V10. It's pure data: no code, no copied files.

```text
mod.esp/
  swofty.json
  registry.json
  registry/content.json
  layers/ferrari_2017.json
  sounds/v10_screamer.bdl
```

```json
// swofty.json
{
  "schema_version": 1,
  "id": "mod.esp",
  "name": "V10 Engine Sounds",
  "version": "1.0.0",
  "author": "you",
  "category": "cars",
  "compat": { "game": "f1_25", "launcher": ">=2.0.0" }
}
```

```json
// registry.json
{
  "provides":  [ { "registry": "engine_sound", "file": "registry/content.json" } ],
  "overrides": [ { "registry": "vehicle", "file": "layers/ferrari_2017.json", "scope": "global" } ],
  "requires":  [ { "record": "vehicle:swofty.classic-seasons-2017/ferrari" } ]
}
```

```json
// registry/content.json: your sound, under your own key
{ "engine_sound": { "mod.esp/v10_screamer": { "bundle": "sounds/v10_screamer.bdl", "settings_from": "game/ferrari_2025" } } }
```

```json
// layers/ferrari_2017.json: a layer on Classic Seasons' 2017 Ferrari
{ "swofty.classic-seasons-2017/ferrari": { "engine_sound": "mod.esp/v10_screamer" } }
```

```sh
swofty addons install ./mod.esp
swofty addons enable mod.esp
swofty build          # the launcher resolves the records and writes the bundle and its database rows
swofty run            # Time Trial, F1 2017, Ferrari
```

```text
[registry] vehicle swofty.classic-seasons-2017/ferrari: layer from mod.esp sets /engine_sound = mod.esp/v10_screamer
[build]    engine sound mod.esp/v10_screamer: bundle esp_v10_screamer, settings from ferrari_2025
```

## The parts of the docs

- **[The registry](/std/)**: what every addon builds on. Records under your own keys, providers, layers, bases and variants, scopes, build time and run time, versioning.
- **[Game systems](/std/systems/)**: one page per system the game has: cars, liveries, teams, drivers, helmets and suits, sounds, tracks, tyres, power units, sessions and weekends, weather, AI, career, UI and strings, graphics. Each says what you can provide and change, the hook points, and the limits.
  - **[Your own systems](/std/systems/custom)**: bring a system the game doesn't have, and let others extend it.
- **[Connecting addons](/std/events)**: events through the whole game, hooks on game functions, services between addons, the core's operations, and dependencies with guards that stop players cleanly when something's missing.
- **[In-game code](/reference/addons/native-plugins)**: engine plugins (Windows DLLs) with the mixin system and the in-game libraries: events, overlay, telemetry, database reads, storage, career, popups, the race HUD, strings.
- **[Launcher plugins](/reference/addons/launcher-api)**: pages in the launcher, importers, build steps and `swofty` commands, and how a page edits registry layers.
- **[Mods and importing](/std/importer)**: what the importer recognises in a mod, and how to make `.serps` packages and track zips.
- **[Packaging and publishing](/reference/addons/publishing)**: `swofty.json`, `registry.json`, and publishing to the Addon Browser.
- **[Reference](/reference/cli)**: the command line, the C ABI, compatibility with 1.x, build operations, the game database, ERP archives and the Engine API.

## Tools

```text
swofty              the command line: install and enable addons, import mods, build a profile, run the game,
                    follow its logs, restore the game folder                         /reference/cli
Swofty UI           browse, render and edit menu scenes (.idf), export checked scenes; releases ship it as
                    bin/swofty-ui                                                    /reference/ui/swofty-ui
tools/gamedb.py     dump the game database, get or set one value by path             /reference/gamedb/tools
tools/map_check.py  check a profile built with tracks against its game, using the build's plan.json
tools/idf.py        print a menu scene's elements and edit them from the command line /reference/ui/idf
```

The Python tools are in the repository's `tools/` folder. Releases don't ship them, except Swofty UI.

## Addons today

Everything addons do today keeps working: operations in `swofty.json`, engine plugins, launcher plugins and imported mods. See [Compatibility](/std/compatibility).
