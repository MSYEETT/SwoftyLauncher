---
title: Seasons and GP/TT content
description: A whole GP/TT season in code - content packages and era boxes as typed registries, other addons building on a season in place or as a variant with its own box, and each box's view of the tracks, in standard library 2.0.
---

# Seasons and GP/TT content

A GP/TT season is a `content::Package` record (the entrant list, its teams, cars and drivers) and a `content::EraBox` that opens it, and every piece is a record another addon can build on.

## The header

<Example root="swofty-sdk" code-file="include/swofty/systems/content.hpp">
<template #code>

```cpp
#include <swofty/registry.hpp>

namespace swofty::content {

struct Seat {
    Ref<drivers::Identity> driver;                 // the driver
    int number = 0;                                // the car number this season
    Ref<drivers::Look> look;                       // the look this season (driver_customisation)
};

struct Team {
    std::string key;                               // the team's key in the package: "ferrari"
    Ref<teams::Identity> team;                     // the team record
    Ref<cars::Vehicle> vehicle;                    // its car
    Ref<sounds::EngineSound> engine_sound;         // the car's sound in this package; empty: the vehicle's
    Ref<liveries::Livery> livery;                  // the car's livery in this package; empty: the vehicle's
    std::vector<Seat> drivers;                     // its seats
};

struct Package {                                   // a GP/TT season build: an entrant list and its rows
    Text title;                                    // the season's name: a lang key ("lng_cs_f1_2017")
    std::string entrant_list;                      // "cs_f1_2017"
    int season = 0;                                // the year
    Keyed<Team, &Team::key> teams;                 // the grid, matched by key: p.teams.at("ferrari")
    sessions::Rules rules;                         // the box's session rules: refuelling, race fuel (see Sessions)
    std::string setup_mode = "swofty_f1";          // the setup mode its entrant list is added to
    StatId stat_id = StatId::automatic;            // the entrant list's id
};

struct EraBox {                                    // an entry in the GP/TT era picker
    Ref<Package> package;                          // the package it opens
    Text title;                                    // its title (default: the package's)
    std::string image;                             // "#overrides.home_screen.tiles_solo_6x6[cs_f1_2017]"
    int order = 0;                                 // its place in the Addons row
};

inline Registry<Package> packages{"content.package", { .applies = Applies::content }};   // its entrant list rows are added at boot
inline Registry<EraBox> era_boxes{"era.box", { .applies = Applies::read }};   // when the picker is built

}
```

</template>
</Example>

## Classic Seasons 2017, in code

<Split>

Today Classic Seasons 2017 ships its season as about 700 build operations: car packages, `file.put`s for `cs17_*.bdl`, 430 raw audio records and rows for the entrant list, teams and pit times. In 2.0 it registers the season as records, and the core applies them at boot.

- **The era box** is what the player picks, and it points at a package.
- **The package** is the season build: the entrant list, the teams, their cars and drivers.
- **The vehicle** is one car, with its sound and livery ([Cars](/std/systems/cars)).
- **The engine sound** is a bundle, plus where its mixing settings and tuning come from ([Sounds](/std/systems/sounds)).

<template #example>
<Example root="swofty.classic-seasons-2017" code-file="native/src/content.cpp" data-file="registry/content.json" files="swofty.json, native/src/cars.cpp, sounds/cs17_ferrari.bdl, sounds/cs17_mercedes.bdl, sounds/cs17_honda.bdl, sounds/cs17_renault.bdl, lang/season.json">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/content.hpp>
#include <swofty/systems/drivers.hpp>
#include <swofty/systems/liveries.hpp>
#include <swofty/systems/sounds.hpp>
#include <swofty/systems/teams.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto ferrari_sound = sounds::engine_sounds.add("ferrari", {
        .bundle = file("sounds/cs17_ferrari.bdl"),
        .settings_from = sounds::engine_sounds["game/ferrari_2025"],
        .tuning_from = sounds::engine_sounds["game/ferrari_2025"],
    });

    auto ferrari = cars::vehicles.variant("ferrari", cars::vehicles["game/ferrari_2025"], [=](cars::Vehicle &v) {
        v.label = "cs17_ferrari";
        v.engine_sound = ferrari_sound;
        v.livery = liveries::liveries["swofty.classic-seasons-2017/ferrari"];
    });

    auto season = content::packages.add("f1_2017", {
        .title = "lng_cs_f1_2017",
        .entrant_list = "cs_f1_2017",
        .season = 2017,
        .teams = {
            { .key = "ferrari", .team = teams::identities["swofty.classic-seasons-2017/ferrari"], .vehicle = ferrari,
              .drivers = { { .driver = drivers::identities["game/sebastian_vettel"], .number = 5 },
                           { .driver = drivers::identities["game/kimi_raikkonen"],   .number = 7 } } },
            // mercedes, redbull, force_india, williams, mclaren, toro_rosso, haas, lotus, sauber
        },
    });

    content::era_boxes.add("f1_2017", {
        .package = season,
        .image = "#overrides.home_screen.tiles_solo_6x6[cs_f1_2017]",
        .order = 2017,
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/content.json
{
  "engine_sound": {
    "ferrari": { "bundle": "sounds/cs17_ferrari.bdl", "settings_from": "game/ferrari_2025", "tuning_from": "game/ferrari_2025" }
  },
  "vehicle": {
    "ferrari": { "extends": "game/ferrari_2025", "label": "cs17_ferrari",
                 "engine_sound": "swofty.classic-seasons-2017/ferrari", "livery": "swofty.classic-seasons-2017/ferrari" }
  },
  "content.package": {
    "f1_2017": {
      "title": "lng_cs_f1_2017", "entrant_list": "cs_f1_2017", "season": 2017,
      "teams": [
        { "key": "ferrari", "team": "swofty.classic-seasons-2017/ferrari", "vehicle": "swofty.classic-seasons-2017/ferrari",
          "drivers": [ { "driver": "game/sebastian_vettel", "number": 5 }, { "driver": "game/kimi_raikkonen", "number": 7 } ] }
      ]
    }
  },
  "era.box": {
    "f1_2017": { "package": "swofty.classic-seasons-2017/f1_2017",
                 "image": "#overrides.home_screen.tiles_solo_6x6[cs_f1_2017]", "order": 2017 }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: swofty.classic-seasons-2017 (classic_seasons_2017.dll) 4 engine_sound, 10 vehicle, 10 vehicle.package,
           10 team.identity, 1 content.package, 1 era.box (early stage, 1.8 ms)
[registry] applied at boot: engine_sound 4 (4 bundles served in memory), vehicle 10, vehicle.package 10, team.identity 10,
           content.package 1 (entrant list cs_f1_2017, 20 entrants; 1,912 database records)
[era]      era.box swofty.classic-seasons-2017/f1_2017: Addons row, order 2017
```

## The V10 pack builds on it, in place

<Split>

An engine sound pack brings its own sound under its own key, then gives it to the 2017 teams with a layer on the 2017 package. In place, every user of the 2017 package hears it: the 2017 GP/TT box, Progressive Career's 2017 season, anything else that reads it.

Classic Seasons doesn't know the pack exists, and the pack copies none of Classic Seasons' files.

<template #example>
<Example root="mod.esp" code-file="native/src/esp.cpp" data-file="registry/layers.json" files="swofty.json, sounds/v10_screamer.bdl, registry/sounds.json">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/sounds.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto v10 = sounds::engine_sounds.add("v10_screamer", {
        .bundle = file("sounds/v10_screamer.bdl"),
        .settings_from = sounds::engine_sounds["game/ferrari_2025"],
        .tuning_from = sounds::engine_sounds["game/ferrari_2025"],
    });

    content::packages.layer(content::packages["swofty.classic-seasons-2017/f1_2017"], [=](content::Package &p) {
        for (const char *team : { "mercedes", "ferrari", "redbull" })
            p.teams.at(team).engine_sound = v10;
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/layers.json (the engine sound itself is in registry/sounds.json)
{
  "content.package": {
    "swofty.classic-seasons-2017/f1_2017": {
      "teams": [
        { "key": "mercedes", "engine_sound": "mod.esp/v10_screamer" },
        { "key": "ferrari",  "engine_sound": "mod.esp/v10_screamer" },
        { "key": "redbull",  "engine_sound": "mod.esp/v10_screamer" }
      ]
    }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: mod.esp (esp.dll) 1 engine_sound, 1 layer on content.package (early stage, 0.4 ms)
[registry] content.package swofty.classic-seasons-2017/f1_2017: edit by mod.esp changed /teams/key=ferrari/engine_sound,
           /teams/key=mercedes/engine_sound, /teams/key=redbull/engine_sound -> mod.esp/v10_screamer
[registry] applied at boot: engine_sound 1 (1 bundle served in memory)
[registry] content swofty.classic-seasons-2017/f1_2017 active: 3 scoped changes applied (/teams engine_sound)
```

## Or as a variant, with its own box

<Split>

The pack can instead add a separate "F1 2017 (V10)" entry and leave the real one alone. The variant package starts from the 2017 package and gives three teams the V10. Its era box sits next to the original.

Every team, seat and field the variant doesn't change comes from the base, and later layers on the base flow in. The variant inherits the base package's [track view](#a-track-is-in-every-year).

<template #example>
<Example root="mod.esp" code-file="native/src/esp_variant.cpp" data-file="registry/content.json" files="swofty.json, sounds/v10_screamer.bdl, registry/sounds.json, lang/esp.json">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/content.hpp>
#include <swofty/systems/sounds.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto v10 = sounds::engine_sounds.add("v10_screamer", { .bundle = file("sounds/v10_screamer.bdl"),
                                                            .settings_from = sounds::engine_sounds["game/ferrari_2025"] });

    auto base = content::packages["swofty.classic-seasons-2017/f1_2017"];
    auto season = content::packages.variant("f1_2017_v10", base, [=](content::Package &p) {
        p.title = "lng_esp_era_2017_v10";
        p.entrant_list = "esp_f1_2017_v10";
        for (const char *team : { "mercedes", "ferrari", "redbull" })
            p.teams.at(team).engine_sound = v10;
    });

    content::era_boxes.add("f1_2017_v10", { .package = season, .order = 2017 });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/content.json
{
  "content.package": {
    "f1_2017_v10": {
      "extends": "swofty.classic-seasons-2017/f1_2017", "title": "lng_esp_era_2017_v10", "entrant_list": "esp_f1_2017_v10",
      "teams": [
        { "key": "mercedes", "engine_sound": "mod.esp/v10_screamer" },
        { "key": "ferrari",  "engine_sound": "mod.esp/v10_screamer" },
        { "key": "redbull",  "engine_sound": "mod.esp/v10_screamer" }
      ]
    }
  },
  "era.box": { "f1_2017_v10": { "package": "mod.esp/f1_2017_v10", "order": 2017 } }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] content.package mod.esp/f1_2017_v10: extends swofty.classic-seasons-2017/f1_2017; changed /title, /entrant_list,
           /teams/key=mercedes/engine_sound, /teams/key=ferrari/engine_sound, /teams/key=redbull/engine_sound
[era]      era.box mod.esp/f1_2017_v10: Addons row, order 2017 (after swofty.classic-seasons-2017/f1_2017)
```

[Events](/std/events) scoped to the 2017 package also run in "F1 2017 (V10)", because it's a variant of it.

## A track is in every year

<Split>

A track addon goes into the base that every box inherits, so Hockenheim appears in the 2017 box too, with its base data. Each box has its own view of the tracks in `tracks::per_package`, keyed `<package>/<track>`: which tracks it offers, in what order, and per-track data for that box only. An empty field comes from the base track data.

When the box is picked, the core applies its view to the track data the game reads, and puts it back when the box is left. See [Tracks](/std/systems/tracks).

<template #example>
<Example root="swofty.classic-seasons-2017" code-file="native/src/tracks.cpp" data-file="registry/tracks.json" files="swofty.json, native/src/content.cpp">
<template #code>

```cpp
SWOFTY_REGISTER
{
    auto box = content::packages["swofty.classic-seasons-2017/f1_2017"];

    tracks::per_package.add("f1_2017/monaco", { .package = box, .order = 6, .laps = 78 });
    tracks::per_package.add("f1_2017/sochi",  { .package = box, .order = 4, .laps = 52 });
    // tracks it doesn't list (Hockenheim from mod.hockenheim) are offered after these, with base data
}
```

</template>
<template #data>

```json
{
  "track.per_package": {
    "f1_2017/monaco": { "package": "swofty.classic-seasons-2017/f1_2017", "order": 6, "laps": 78 },
    "f1_2017/sochi":  { "package": "swofty.classic-seasons-2017/f1_2017", "order": 4, "laps": 52 }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] content swofty.classic-seasons-2017/f1_2017 active: 2 scoped changes applied (track view: 22 tracks, 2 ordered;
           mod.hockenheim/hockenheim with base data)
[registry] content mod.esp/f1_2017_v10 active: track view inherited from swofty.classic-seasons-2017/f1_2017
[registry] content swofty.classic-seasons-2017/f1_2017 ended: 2 scoped changes put back
```

## The content registries

- `era.box`, `content.package`: this page.
- `vehicle`, `vehicle.package`, `vehicle.handling`: [Cars](/std/systems/cars).
- `engine_sound`, `audio.car`: [Sounds](/std/systems/sounds).
- `livery`: [Liveries](/std/systems/liveries).
- `helmet`, `suit`, `gloves`, `wearable.season_set`: [Helmets, suits and gloves](/std/systems/wearables).
- `track`, `track.data`, `track.per_package`: [Tracks](/std/systems/tracks).
- `team.identity`: [Teams](/std/systems/teams). `driver.identity`: [Drivers](/std/systems/drivers).
- `career.content`, `career.season`, `career.driver_ratings`: [Career](/std/systems/career).

The 1.x libraries become some of these: era boxes (`swofty::era::add_box`) become `era.box`, and career contents become `career.content` (see [Compatibility](/std/compatibility)).

## What the player sees

The launcher shows each GP/TT entry with the addons that built it, from the last session's `registry/effective.json` and its [origins](/std/reading):

```text
F1 2017                          Classic Seasons 2017
  Ferrari  engine: V10 Screamer  (mod.esp)
           livery: Monza 2017    (someone.liveries)
F1 2017 (V10)                    mod.esp, on Classic Seasons 2017
```

## Readiness and limits

- **Packages and boxes: ready.** The core's era picker (`era_picker.cpp`, up to 64 boxes) and the entrant pipeline are L2; Classic Seasons ships nine seasons on them.
- **The entrant list must be in `swofty_f1`,** the host setup mode the core narrows to the pressed season.
- **Shared lists compose.** Every season appends its entrant list to `content_sets[2025|2026].available_entrant_lists_0` and `entrant_setup_modes[swofty_f1]`; the core merges the appends.
- **Scale:** a season year is about 700 records' worth of operations today (99 add-row, 170 set, 430 put-record for 2017).
