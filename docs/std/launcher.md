---
title: The launcher side
description: What the launcher does in standard library 2.0 (install, build, check dependencies, start the game), launcher pages that edit an addon's own per-profile data, the conflicts view and Send Logs.
---

# The launcher side

The launcher installs addons, builds their 1.x operations, checks the dependencies their manifests declare and starts the game. It never needs to know what addons register: registrations are code, and they're live in the game.

<Split>

Tails' "what if" addon edits Progressive Career's grids from a launcher page. The page never touches a registry:

1. the page saves the player's seats in the addon's own **per-profile state**;
2. the addon's **build step** puts that state in the addon's folder in the game;
3. the addon's **DLL** reads the file and applies it as a `Scope::profile` [layer](/std/scopes#profile).

The card is step 3. Steps 1 and 2 are the Rust page [below](#editing-a-layer-from-a-launcher-page).

<template #example>
<Example root="tails.whatif-history" code-file="native/src/whatif.cpp" files="swofty.json, registry.json, launcher/src/main.rs, bin/whatif.exe, native/whatif.dll">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/storage.hpp>
#include <swofty/systems/career.hpp>
#include <swofty/systems/drivers.hpp>

#include "whatif.hpp"   // whatif::parse: the page's file, as one Seat per edited seat

using namespace swofty;

SWOFTY_REGISTER
{
    // written by the build step from the active profile's state
    std::string text;
    if (storage::read("profile/whatif.json", text) != storage::Result::ok)
        return;                                         // nothing edited in this profile

    for (const whatif::Seat &seat : whatif::parse(text))   // { season, slot, seat, driver }
        career::seasons.layer(career::seasons[seat.season], [seat](career::Season &s) {
            s.teams.at(seat.slot).seats[seat.seat] = drivers::identities[seat.driver];
        }, Scope::profile);
}

SWOFTY_ADDON()
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: tails.whatif-history (whatif.dll) 2 layers on career.season (early stage, 0.2 ms)
[registry] career.season swofty.progressive/2017: edit by tails.whatif-history (profile) changed /teams/slot=1/seats/0
[registry] career.season swofty.progressive/2017: edit by tails.whatif-history (profile) changed /teams/slot=1/seats/1
```

## What the launcher does

- **Installs** addons, with their data files (`registry/*.json`) in their folders.
- **Builds** each addon's 1.x operations and its launcher plugin's build reply, in load order, as in 1.x.
- **Checks dependencies** before Play: `swofty.json`'s 1.x `requires` and `registry.json`'s `requires` (see [Manifest](/std/manifest) and [Dependencies and guards](/std/dependencies)).
- **Starts the game.** The core applies every registration, data file and layer in game, at the moment the game reads that data.

A profile of 1.x addons builds exactly as in 1.x (see [Compatibility](/std/compatibility)).

### The registry folder in the game

The build writes one index, so the core finds data-file addons (they have no DLL in `settings.json`'s `plugins`). The core writes the diagnostics:

```text
<game>/swofty/
  registry/
    index.json                          load order and each enabled addon's data files, written by the build
    effective.json                      written by the core at each session load and on a crash
    conflicts.json                      written by the core at each session load
  addons/
    mod.esp/registry/sounds.json        a data file, installed with its addon
    mod.esp/registry/layers.json
    tails.whatif-history/profile/whatif.json   that addon's per-profile data, put there by its build step
```

```json
// index.json
{
  "version": 1,
  "order": ["swofty.core", "swofty.classic-seasons-2017", "swofty.progressive", "mod.esp", "tails.whatif-history"],
  "data": {
    "mod.esp": ["addons/mod.esp/registry/sounds.json", "addons/mod.esp/registry/layers.json"]
  }
}
```

The core validates every data file and every edit when it applies them. A broken one is dropped, logged with the addon's name, and listed in `conflicts.json`.

## Editing a layer from a launcher page

A launcher page edits the addon's own data and nothing else, with the 1.x host calls: `get_state` and `set_state` for the per-profile state, the `build` reply to put it in the game, and `game_dir` to show the last session's grid. This is Tails' page, written against the launcher plugin template:

```rust
// launcher/src/main.rs
use serde_json::{Value, json};

// the page: one row per season and slot, from the last session's effective.json, with this profile's edits on top
fn view_history(host: &mut Host) -> Value {
    let edits = host.call("get_state", Value::Null).unwrap_or(Value::Null);   // null: nothing edited in this profile
    let last = host.call("game_dir", Value::Null).ok()
        .and_then(|d| d.as_str().map(|d| format!("{d}/swofty/registry/effective.json")))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .unwrap_or(Value::Null);                                              // null until the game has run once
    let mut rows = Vec::new();
    for (id, season) in last["career.season"].as_object().into_iter().flatten() {
        if !id.starts_with("swofty.progressive/") {
            continue;
        }
        let year = season["value"]["year"].as_i64().unwrap_or(0);
        for team in season["value"]["teams"].as_array().into_iter().flatten() {
            let slot = &team["slot"];
            let seats = edits[id].as_array().into_iter().flatten()
                .find(|t| &t["slot"] == slot).map(|t| t["seats"].clone())
                .unwrap_or_else(|| team["seats"].clone());
            rows.push(json!({
                "id": format!("{id}#{slot}"),
                "cells": [year, team["team"], seats[0], seats[1]],
                "edit": [false, false, "driver", "driver"],
            }));
        }
    }
    json!({ "subtitle": "What if history", "body": { "table": {
        "columns": ["Season", "Team", "Seat 1", "Seat 2"], "rows": rows } } })
}

// a cell edit: save the seats in this addon's state for the active profile
fn edit_seat(host: &mut Host, id: &str, slot: i64, seats: [&str; 2]) -> Result<(), String> {
    let mut edits = host.call("get_state", Value::Null)?;
    if edits.is_null() {
        edits = json!({});
    }
    let teams = edits[id].as_array().cloned().unwrap_or_default();
    let mut teams: Vec<Value> = teams.into_iter().filter(|t| t["slot"] != json!(slot)).collect();
    teams.push(json!({ "slot": slot, "seats": seats }));
    edits[id] = Value::Array(teams);
    host.call("set_state", edits)?;                    // an error on the vanilla profile
    host.invalidate("history");
    Ok(())
}

// the build step ("build": true in the launcher block): put this profile's edits where the DLL reads them
fn build(&self, host: &mut Host) -> Result<Value, String> {
    let edits = host.call("get_state", Value::Null)?;
    let file = self.cache_dir.join("whatif.json");
    std::fs::write(&file, edits.to_string()).map_err(|e| e.to_string())?;
    Ok(json!({ "ops": [{
        "op": "file.put",
        "source": { "root": "cache", "path": "whatif.json" },
        "target": "swofty/addons/tails.whatif-history/profile/whatif.json"
    }] }))
}
```

```text
launcher  {"id":7,"method":"build","params":{"options":{},"features":{"stable":["file"],"experimental":[]}}}
plugin    {"id":0,"method":"get_state","params":null}
launcher  {"id":0,"result":{"swofty.progressive/2017":[{"slot":1,"seats":["game/fernando_alonso","game/valtteri_bottas"]}]}}
plugin    {"id":7,"result":{"ops":[{"op":"file.put","source":{"root":"cache","path":"whatif.json"},"target":"swofty/addons/tails.whatif-history/profile/whatif.json"}]}}
```

- **An addon edits only its own data.** There's no registry host call and no declaration: the page writes its addon's state, and the addon's code turns it into its own layer.
- **Per profile:** `get_state` and `set_state` read and write the addon's entry in the active profile. Duplicating or renaming a profile copies it, so a "what if" profile and a real-history profile sit side by side.
- **The grid the page shows** comes from the last `effective.json` the core wrote. Before the game has run once, there's none, and the page shows only its own edits. Nothing depends on it.
- **The edit reaches the game on the next Play:** the build writes the file, and the DLL applies it at the next game start.

See [Host calls](/reference/addons/launcher-api/host-calls) and [The build reply](/reference/addons/launcher-api/build-reply) for the 1.x calls.

## The conflicts view

The core writes `registry/conflicts.json` at each session load. The launcher shows the last one on the profile, next to the file conflicts of the profile's last build (`conflicts.json`):

```json
{
  "registry": [
    {
      "kind": "field",
      "registry": "career.season",
      "id": "swofty.progressive/2017",
      "path": "/teams/slot=1/seats",
      "addons": ["tails.whatif-history", "someone.realism"],
      "wins": "someone.realism"
    },
    {
      "kind": "invalid_layer",
      "registry": "career.season",
      "id": "swofty.progressive/2018",
      "addon": "tails.whatif-history",
      "error": "/teams/slot=4/seats/1 \"game/kimi_raikkonen\" already holds a seat"
    },
    {
      "kind": "hook_unavailable",
      "point": "career.calendar.add_activity",
      "holder": "old_addon",
      "subscribers": ["swofty.progressive"]
    }
  ]
}
```

Reordering addons resolves field conflicts the same way it resolves file conflicts. The new order applies at the next game start.

## Send Logs

The bundle adds the core's `registry/effective.json` and `registry/conflicts.json`, written at each session load and on a crash. `effective.json` holds each registry's effective records, with the origin of every field that a layer changed:

```json
{
  "career.season": {
    "swofty.progressive/2017": {
      "value": { "year": 2017, "teams": [ { "slot": 1, "team": "game/mercedes", "seats": ["game/fernando_alonso", "game/valtteri_bottas"] } ] },
      "origins": { "/teams/slot=1/seats": "tails.whatif-history" },
      "snapshot": "1.0"
    }
  }
}
```

The triage bot can then say "the grid is from tails.whatif-history's layer" instead of guessing.
