---
title: Content registries
description: Registries for game content beyond careers - season packages, GP/TT boxes, cars, engine sounds, liveries, helmets, tracks and identities - in standard library 2.0.
---

# Content registries

The registry covers the game's content, not just careers. A season build for Grand Prix and Time Trial, a car, an engine sound, a livery, a helmet and a track are each a record that one addon provides under its own key, and that others layer or build variants of.

## A GP/TT season, start to finish

Classic Seasons 2017 today ships its season as build operations: car packages, `file.put` ops for `cs17_*.bdl`, and database rows for the entrant list, audio and teams. In 2.0 it describes the season as records, and the launcher turns them into those same operations at build (see [Build time and run time](/std/build)).

```json
// swofty.classic-seasons-2017/registry/content.json
{
  "engine_sound": {
    "swofty.classic-seasons-2017/ferrari": {
      "bundle": "sounds/cs17_ferrari.bdl",
      "settings_from": "game/ferrari_2025",
      "tuning_from": "game/ferrari_2025"
    },
    "swofty.classic-seasons-2017/mercedes": { "bundle": "sounds/cs17_mercedes.bdl", "settings_from": "game/mercedes_2025", "tuning_from": "game/mercedes_2025" }
  },

  "vehicle": {
    "swofty.classic-seasons-2017/ferrari": {
      "package": "cs_2017_ferrari",
      "label": "cs17_ferrari",
      "donor": "game/ferrari_2025",
      "engine_sound": "swofty.classic-seasons-2017/ferrari",
      "livery": "swofty.classic-seasons-2017/ferrari"
    }
  },

  "content.package": {
    "swofty.classic-seasons-2017/f1_2017": {
      "title": "lng_cs_era_2017",
      "entrant_list": "cs_f1_2017",
      "season": 2017,
      "teams": [
        {
          "key": "ferrari",
          "name": "lng_cs_team_17_ferrari",
          "vehicle": "swofty.classic-seasons-2017/ferrari",
          "drivers": [
            { "driver": "sebastian_vettel", "number": 5, "helmet": "swofty.classic-seasons-2017/vettel" },
            { "driver": "kimi_raikkonen",   "number": 7, "helmet": "swofty.classic-seasons-2017/raikkonen" }
          ]
        }
      ]
    }
  },

  "era.box": {
    "swofty.classic-seasons-2017/f1_2017": {
      "package": "swofty.classic-seasons-2017/f1_2017",
      "image": "#overrides.home_screen.tiles_solo_6x6[cs_f1_2017]",
      "order": 2017
    }
  }
}
```

Each piece is a record another addon can build on:

- the **era box** is what the player picks in GP/TT, and it points at a package;
- the **package** is the season build: the entrant list, the teams, their cars and drivers;
- the **vehicle** is one car, with its sound and livery;
- the **engine sound** is a bundle, plus where its mixing settings and tuning come from.

## Another addon adds an engine sound

An engine sound pack brings its own sound under its own key, then points the 2017 cars at it. It can do that in place, for every user of the 2017 base:

```json
// mod.esp/registry/content.json
{
  "engine_sound": {
    "mod.esp/v10_screamer": {
      "bundle": "sounds/v10_screamer.bdl",
      "settings_from": "game/ferrari_2025",
      "tuning_from": "game/ferrari_2025"
    }
  }
}
```

```json
// mod.esp/layers/f1_2017.json, a layer of vehicle "swofty.classic-seasons-2017/ferrari"
{ "engine_sound": "mod.esp/v10_screamer" }
```

Or it can add a separate "F1 2017 (V10)" entry and leave the real one alone, as a [variant](/std/bases#build-a-variant-with-extends) of the package with its own box. Either way, Classic Seasons doesn't know the pack exists, and the pack doesn't copy any of Classic Seasons' files.

## Another addon adds a livery to one car

```json
// a livery addon: its livery under its own key
{
  "livery": {
    "someone.liveries/ferrari_2017_monza": { "textures": "liveries/ferrari_2017_monza/", "base": "swofty.classic-seasons-2017/ferrari" }
  }
}
```

```json
// a layer of vehicle "swofty.classic-seasons-2017/ferrari"
{ "livery": "someone.liveries/ferrari_2017_monza" }
```

## The content registries

The planned registries, by what they describe. Each one's records are provided under the provider's own keys, and `game/` ids describe the stock game:

```text
era.box            a GP/TT entry: the package it opens, its title, image and order
content.package    a season build: entrant list, season, teams with their vehicle and drivers
vehicle            a car: package, label, donor car, engine sound, livery, handling, AI data
engine_sound       an engine bundle, its mixing settings and its tuning donor
livery             a livery's textures, for one vehicle
helmet             a driver helmet's textures
track              a track: its venue, layout files, and fuel, tyre and AI data per season
team.identity      a team's names, logo, colours and car number rules
driver.identity    a driver's names, nationality, look and number
career.content     a career tile in Driver Career or My Team, and its data group
career.season      a career season's line-up and changes (see Registries)
career.driver_ratings  a driver's ratings at the start of a season
```

The 1.x libraries become some of these: era boxes become `era.box`, and career contents become `career.content` (see [Compatibility](/std/compatibility)).

## What the player sees

The launcher shows each GP/TT entry with the addons that built it:

```text
F1 2017                          Classic Seasons 2017
  Ferrari  engine: V10 Screamer  (mod.esp)
           livery: Monza 2017    (someone.liveries)
F1 2017 (V10)                    mod.esp, on Classic Seasons 2017
```

That comes straight from the effective records and their [origins](/std/reading#origins).
