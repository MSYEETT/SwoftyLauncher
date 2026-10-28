---
title: Build a what-if history editor
description: A companion addon for Progressive Career with a launcher page where players change each season's starting grid and each driver's ratings, applied as a layer on Progressive's seasons without touching its files.
---

# Build a what-if history editor

You'll build `tails.what-if`, a companion addon for Progressive Career. It adds a page to the launcher with a season picker, the season's grid and a ratings table. A player puts Alonso in the 2017 Mercedes, raises his pace, and the next Progressive career races that history.

Progressive registers each season's grid and ratings as registry records. Your addon layers those records, so it never changes Progressive's files.

## The folder

```text
tails.what-if/
├── swofty.json
├── registry.json
├── launcher/                 the launcher page, in Rust
│   ├── Cargo.toml
│   └── src/main.rs
├── bin/
│   ├── whatif                built from launcher/ for Linux
│   └── whatif.exe            and for Windows
└── native/                   the in-game code
    ├── build.sh
    ├── whatif.dll
    └── src/
        ├── whatif.cpp
        └── whatif.hpp        reads the page's file
```

The addon has two halves. The **launcher page** saves the player's edits for the active profile. The **DLL** reads them when the game starts and layers Progressive's seasons.

## swofty.json

<Split>

`requires` makes Progressive a dependency, so it loads first and the launcher stops Play when it's missing. The `launcher` block adds the page and asks the launcher to run the page's build step before each Play.

`registry.json` names the two registries the addon layers, with the major version it was written against.

<template #example>
<Example root="tails.what-if" data-file="swofty.json" files="registry.json, launcher/Cargo.toml, launcher/src/main.rs, bin/whatif, bin/whatif.exe, native/build.sh, native/src/whatif.cpp, native/src/whatif.hpp, native/whatif.dll">
<template #data>

```json
{
  "schema_version": 1,
  "id": "tails.what-if",
  "name": "What If History",
  "description": "Your own starting grids and driver ratings for Progressive Career.",
  "version": "1.0.0",
  "author": "Tails",
  "category": "career",
  "compat": { "game": "f1_25", "launcher": ">=1.6.0" },
  "requires": [ { "id": "swofty.progressive" } ],
  "operations": { "native": [ { "dll": "native/whatif.dll" } ] },
  "launcher": {
    "program": { "windows": "bin/whatif.exe", "linux": "bin/whatif" },
    "sections": [ { "id": "history", "title": "What if history", "icon": "history" } ],
    "build": true
  }
}
```

```json
// registry.json
{
  "requires": [
    { "registry": "career.season", "version": "^1" },
    { "registry": "career.driver_ratings", "version": "^1" }
  ]
}
```

</template>
</Example>
</template>
</Split>

## What Progressive registers

Each Progressive season is a `career::Season` record, `swofty.progressive/2016` to `swofty.progressive/2026`. Its `teams` are keyed by slot, and each team has two seats. Each driver's ratings for a season are a `career::DriverRatings` record, keyed by year and driver:

```text
career.season           swofty.progressive/2017
  teams[slot 1]           team game/mercedes, seats [game/lewis_hamilton, game/valtteri_bottas]
  teams[slot 2]           team game/ferrari,  seats [game/sebastian_vettel, game/kimi_raikkonen]
career.driver_ratings   swofty.progressive/2017/game/lewis_hamilton
  experience 94, racecraft 93, awareness 90, pace 95
```

See [Career](/std/systems/career#progressive-s-career-mode) for the full records.

## The page

<Split>

The page lists Progressive's seasons from its `registry.lock.json`, which ships inside the addon. It shows each season's current grid and ratings from the last session's `swofty/registry/effective.json`, which the core writes each time a session loads. Before the game has run once, the page shows the seasons and the player's own edits only.

A save writes the edit into the addon's state for the active profile with `set_state`. The build step copies that state into the addon's folder in the game, where the DLL reads it. Each profile has its own edits, so a "what if" profile and a real-history profile sit side by side.

The `Host` type and the message loop come from the [launcher plugin template](/reference/addons/launcher-api).

<template #example>
<Example root="tails.what-if" code-file="launcher/src/main.rs" files="swofty.json, registry.json, launcher/Cargo.toml, bin/whatif, bin/whatif.exe, native/build.sh, native/src/whatif.cpp, native/src/whatif.hpp, native/whatif.dll">
<template #code>

```rust
use serde_json::{Value, json};

const PROGRESSIVE: &str = "swofty.progressive/";
const RATINGS: [&str; 4] = ["experience", "racecraft", "awareness", "pace"];

// The player's edits: { "seats": { season: { slot: [a, b] } }, "ratings": { record: { field: n } } }
fn edits(host: &mut Host) -> Value {
    match host.call("get_state", Value::Null) {
        Ok(v) if v.is_object() => v,
        _ => json!({ "seats": {}, "ratings": {} }),
    }
}

// The last session's values, or null before the game has run once
fn last_session(host: &mut Host) -> Value {
    host.call("game_dir", Value::Null).ok()
        .and_then(|d| d.as_str().map(|d| format!("{d}/swofty/registry/effective.json")))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null)
}

fn view_season(host: &mut Host, season: &str) -> Value {
    let mine = edits(host);
    let last = last_session(host);

    let grid: Vec<Value> = last["career.season"][season]["value"]["teams"]
        .as_array().into_iter().flatten()
        .map(|team| {
            let slot = team["slot"].to_string();
            let seats = mine["seats"][season].get(&slot).unwrap_or(&team["seats"]);
            json!({ "id": slot, "cells": [team["team"], seats[0], seats[1]],
                    "edit": [false, "driver", "driver"] })
        })
        .collect();

    let ratings: Vec<Value> = last["career.driver_ratings"].as_object().into_iter().flatten()
        .filter(|(_, r)| r["value"]["season"] == json!(season))
        .map(|(id, r)| {
            let mut cells = vec![r["value"]["driver"].clone()];
            for field in RATINGS {
                let value = mine["ratings"][id].get(field).unwrap_or(&r["value"][field]);
                cells.push(json!({ "text": value.to_string(),
                                   "edit": { "number": { "value": value, "min": 0, "max": 100 } } }));
            }
            json!({ "id": id, "cells": cells })
        })
        .collect();

    json!({ "subtitle": "What if history", "body": { "blocks": [
        { "table": { "id": format!("grid:{season}"), "columns": ["Team", "Seat 1", "Seat 2"], "rows": grid } },
        { "table": { "id": format!("ratings:{season}"),
                     "columns": ["Driver", "Experience", "Racecraft", "Awareness", "Pace"], "rows": ratings } },
    ] } })
}

// A cell edit: keep it in this addon's state for the active profile
fn save(host: &mut Host, table: &str, row: &str, values: &Value) -> Result<(), String> {
    let mut mine = edits(host);
    if let Some(season) = table.strip_prefix("grid:") {
        mine["seats"][season][row] = json!([values["Seat 1"], values["Seat 2"]]);
    } else if table.starts_with("ratings:") {
        for field in RATINGS {
            if let Some(n) = values.get(field).and_then(Value::as_i64).filter(|n| (0..=100).contains(n)) {
                mine["ratings"][row][field] = json!(n);
            }
        }
    }
    host.call("set_state", mine)?;          // the vanilla profile refuses this with an error
    host.invalidate("history");
    Ok(())
}

// The build step: put this profile's edits where the DLL reads them
fn build(&self, host: &mut Host) -> Result<Value, String> {
    let file = self.cache_dir.join("whatif.json");
    std::fs::write(&file, edits(host).to_string()).map_err(|e| e.to_string())?;
    Ok(json!({ "ops": [{
        "op": "file.put",
        "source": { "root": "cache", "path": "whatif.json" },
        "target": "swofty/addons/tails.what-if/profile/whatif.json"
    }] }))
}
```

</template>
</Example>
</template>
</Split>

`launcher/Cargo.toml` needs one dependency, `serde_json = "1"`. See [Host calls](/reference/addons/launcher-api/host-calls) and [The build reply](/reference/addons/launcher-api/build-reply) for `get_state`, `set_state`, `game_dir` and the build step.

## Apply the edits in the game

<Split>

`SWOFTY_REGISTER` runs at every game start, before the game loads its database. It reads the file the build step wrote and adds one layer per edited season and one per edited driver.

Each layer has `Scope::profile`: it applies while this profile plays. Progressive's seasons and ratings are snapshot registries, so a career copies each season's grid and ratings when that season starts. A new career gets the edits from its first season. A running career gets them for every season it hasn't reached yet, and keeps what it already started.

A layer only changes the fields it sets. Progressive's mid-season swaps, team names and everything else in the season still come from Progressive.

`whatif.hpp` is a small reader for the page's file. Any JSON library works.

<template #example>
<Example root="tails.what-if" code-file="native/src/whatif.cpp" data-file="registry/layers.json" files="swofty.json, registry.json, launcher/src/main.rs, native/build.sh, native/src/whatif.hpp, native/whatif.dll">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/storage.hpp>
#include <swofty/systems/career.hpp>
#include <swofty/systems/drivers.hpp>

#include "whatif.hpp"   // whatif::load: seats per season and slot, ratings per record and field

using namespace swofty;

SWOFTY_REGISTER
{
    std::string text;
    if (storage::read("profile/whatif.json", text) != storage::Result::ok)
        return;                                     // nothing edited in this profile
    const whatif::Edits edits = whatif::load(text);

    for (const auto &[season, grid] : edits.seats)
        career::seasons.layer(career::seasons[season], [grid](career::Season &s) {
            for (const auto &[slot, seats] : grid)
                if (auto *team = s.teams.find(slot))    // a slot Progressive removed is skipped
                    team->seats = { drivers::identities[seats[0]], drivers::identities[seats[1]] };
        }, Scope::profile);

    for (const auto &[record, fields] : edits.ratings)
        career::ratings.layer(career::ratings[record], [fields](career::DriverRatings &r) {
            if (fields.experience) r.experience = *fields.experience;
            if (fields.racecraft)  r.racecraft  = *fields.racecraft;
            if (fields.awareness)  r.awareness  = *fields.awareness;
            if (fields.pace)       r.pace       = *fields.pace;
        }, Scope::profile);
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
{
  "career.season": {
    "swofty.progressive/2017": {
      "teams": [ { "slot": 1, "seats": ["game/fernando_alonso", "game/valtteri_bottas"] } ]
    }
  },
  "career.driver_ratings": {
    "swofty.progressive/2017/game/fernando_alonso": { "pace": 96 }
  }
}
```

</template>
</Example>
</template>
</Split>

The **Without code** tab is a fixed preset: the same layers as a data file, with no launcher page. It applies in every profile where the addon is on. Ship one or the other, not both.

## Try it

Build the page for each system you list in `launcher.program`, and the DLL with the template's `native/build.sh`:

```sh
(cd launcher && cargo build --release && cp target/release/whatif ../bin/)
native/build.sh
swofty addons install ./tails.what-if
swofty addons enable swofty.progressive
swofty addons enable tails.what-if
```

1. Open **What if history** in the launcher and pick 2017.
2. Put `game/fernando_alonso` in the Mercedes' first seat, and set his pace to 96.
3. Press Play, or run `swofty run --apply`, and start a Progressive career in 2017.

`swofty/loader.log`:

```text
[registry] register: tails.what-if (whatif.dll) 2 layers (early stage, 0.3 ms)
[registry] career.season swofty.progressive/2017: edit by tails.what-if (profile) changed /teams/slot=1/seats/0
[registry] career.driver_ratings swofty.progressive/2017/game/fernando_alonso: edit by tails.what-if (profile) changed /pace
```

`swofty/registry/effective.json` shows each changed field with `tails.what-if` as its origin. If another addon changes the same seat, `swofty/registry/conflicts.json` lists both, and the later one in load order wins.

## When Progressive changes or is missing

The page keeps the player's edits by record id, and the DLL checks each one against what Progressive registers:

- **A season Progressive no longer has:** the layer targets a record that doesn't exist, so the core skips it and logs it.
- **A slot Progressive removed:** `teams.find` returns nothing, and that seat edit is skipped.
- **A driver that no longer exists:** the seat's reference can't resolve, the core rejects that edit and logs it, and the seat keeps Progressive's driver.
- **A rating record that's gone:** skipped, like a missing season.
- **Progressive turned off:** `requires` stops Play, and the launcher says Progressive is needed. Careers made with the edits also record Progressive as a dependency, and a [guard](/std/dependencies#guards) stops them loading without it.

## Publish

Package the addon with its DLL and both page binaries, and publish it: see [Packaging and publishing](/reference/addons/publishing). Leave the player's state out: it lives in their profile.
