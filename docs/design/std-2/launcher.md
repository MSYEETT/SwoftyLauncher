---
title: The launcher side
description: Registries in the build, launcher pages that read and edit registry layers, the conflicts view and Send Logs, in standard library 2.0.
---

# The launcher side

The launcher sees the same registries as the game. A launcher page reads effective values and writes its addon's profile layer, and the next Play builds exactly that into the game.

## Registries in the build

When a profile builds, the launcher copies every enabled addon's static providers and layers, plus the profile's layers, into the game folder with an index in load order:

```text
<game>/swofty/registry/
  index.json                                   load order, schema files, which file is which
  schemas/career.season/1.0.json               every declared schema version
  provide/swofty.progressive/career.season.json
  provide/swofty.progressive/career.driver_ratings.json
  global/tails.whatif-history/career.season.json         static layers from the manifest
  profile/tails.whatif-history/career.season.json        this profile's layer, from $DATA/registry/<profile>/
```

```json
// index.json
{
  "version": 1,
  "order": ["swofty.core", "swofty.classic-seasons", "swofty.progressive", "tails.whatif-history"],
  "registries": {
    "career.season": {
      "owner": "swofty.core",
      "versions": ["1.0"],
      "provide": ["provide/swofty.progressive/career.season.json"],
      "layers": [
        { "addon": "tails.whatif-history", "scope": "global",  "file": "global/tails.whatif-history/career.season.json" },
        { "addon": "tails.whatif-history", "scope": "profile", "file": "profile/tails.whatif-history/career.season.json" }
      ]
    }
  }
}
```

The core reads the index at start. The launcher validates every layer against its schema at build, so a broken layer is reported in the launcher before the game starts. The core validates again, because dynamic providers only exist in game.

## Editing a layer from a launcher page

A launcher plugin uses five host calls. This is the "what if" page from Tails' addon, written against the launcher plugin template:

```rust
use serde_json::{Value, json};

const SEASONS: &str = "career.season";
const GROUP: &str = "f1_career_progressive";

// the page: one row per season, one cell per slot
fn view_history(host: &mut Host) -> Value {
    let ids = host.call("registry_list", json!({ "registry": SEASONS, "prefix": format!("{GROUP}/") }))
        .unwrap_or(json!([]));
    let mut rows = Vec::new();
    for id in ids.as_array().into_iter().flatten().filter_map(Value::as_str) {
        // effective value as the game will see it: base + static layers + this profile's layers
        let season = host.call("registry_get", json!({ "registry": SEASONS, "id": id })).unwrap_or(Value::Null);
        let year = season["year"].as_i64().unwrap_or(0);
        for team in season["teams"].as_array().into_iter().flatten() {
            rows.push(json!({
                "id": format!("{id}#{}", team["slot"]),
                "cells": [year, team["team"], team["seats"][0], team["seats"][1]],
                "edit": [false, false, "driver", "driver"],
            }));
        }
    }
    json!({ "subtitle": "What if history", "body": { "table": {
        "columns": ["Season", "Team", "Seat 1", "Seat 2"], "rows": rows } } })
}

// a cell edit: write this addon's profile layer for that season
fn edit_seat(host: &mut Host, id: &str, slot: i64, seats: [&str; 2]) -> Result<(), String> {
    let mut layer = host.call("registry_layer", json!({ "registry": SEASONS, "id": id }))?;  // null when none yet
    if layer.is_null() {
        layer = json!({ "teams": [] });
    }
    let teams = layer["teams"].as_array_mut().ok_or("bad layer")?;
    teams.retain(|t| t["slot"] != json!(slot));
    teams.push(json!({ "slot": slot, "seats": seats }));
    // validated by the launcher: a driver in two seats comes back as an error naming the path
    host.call("set_registry_layer", json!({ "registry": SEASONS, "id": id, "patch": layer }))?;
    host.invalidate("history");
    Ok(())
}
```

The host calls:

```text
registry_schemas                          {registry: {owner, versions, status}} for the profile
registry_list   {registry, prefix?}       the record ids
registry_get    {registry, id}            the effective value (static providers + layers; dynamic providers aren't running)
registry_layer  {registry, id}            this addon's profile layer for the record, or null
set_registry_layer {registry, id, patch}  validate and save this addon's profile layer; patch null clears it
```

`set_registry_layer` needs `"editable": true` on that registry in the addon's `overrides`. Without it, the call fails with `not editable`. An addon can only write its own layer.

## The conflicts view

The profile's conflicts list (`conflicts.json` today) gains the registry conflicts:

```json
{
  "registry": [
    {
      "kind": "field",
      "registry": "career.season",
      "id": "f1_career_progressive/2017",
      "path": "/teams/slot=1/seats",
      "addons": ["tails.whatif-history", "someone.realism"],
      "wins": "someone.realism"
    },
    {
      "kind": "invalid_layer",
      "registry": "career.season",
      "id": "f1_career_progressive/2018",
      "addon": "tails.whatif-history",
      "error": "/teams/slot=4/seats/1 \"kimi_raikkonen\" already holds a seat"
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

The launcher shows them on the profile, next to file conflicts. Reordering addons resolves field conflicts the same way it resolves file conflicts.

## Send Logs

The bundle adds `registry/effective.json`, written by the core at career load and on a crash. It holds each registry's effective records, with the origin of every field that a layer changed:

```json
{
  "career.season": {
    "f1_career_progressive/2017": {
      "value": { "year": 2017, "teams": [ { "slot": 1, "team": "mercedes", "seats": ["fernando_alonso", "valtteri_bottas"] } ] },
      "origins": { "/teams/slot=1/seats": "tails.whatif-history" },
      "snapshot": "1.0"
    }
  }
}
```

The triage bot can then say "the grid is from tails.whatif-history's layer" instead of guessing.
