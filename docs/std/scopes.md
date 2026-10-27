---
title: Scopes and snapshots
description: Global, profile and career scopes, and the career snapshot that freezes a career's data, in standard library 2.0.
---

# Scopes and snapshots

Every [layer](/std/layers) has a **scope** that says how long it lasts and what it applies to.

```cpp
enum class Scope {
    global,   // every profile the addon is on in; the addon's own files and code
    profile,  // one launcher profile; written by the addon's launcher pages
    career,   // one career; written in game while that career is loaded
};
```

## Global

A global layer applies wherever the addon is on. Static layers in the manifest are global unless they say otherwise. A layer written from code with `Scope::global` lasts until the addon clears it or the game closes.

```json
{ "overrides": [ { "registry": "career.season", "file": "layers/seasons.json", "scope": "global" } ] }
```

## Profile

A profile layer belongs to one launcher profile. Launcher pages write it, and the build copies it into the game. Players can keep a "what if" profile and a real-history profile side by side, and switch between them in the launcher.

```text
$DATA/registry/<profile>/<addon id>/<registry>.json     written by the launcher
<game>/swofty/registry/profile/<addon id>/<registry>.json   copied in by the build
```

In game, profile layers are read-only. An addon that wants an in-game change uses `global` (this session) or `career` (this career).

## Career

A career layer belongs to the career that's loaded, and is saved with it. Use it for changes made inside a career, for example an in-game editor that moves a driver mid-season:

```cpp
// in game, during a career: from round 8, Ocon drives for Mercedes in this career only
auto patch = swofty::json::parse(R"({
    "changes": [ { "round": 8, "slot": 1, "seat": 1, "out": "valtteri_bottas", "in": "esteban_ocon", "kind": "swap" } ]
})");
swofty::registry::layer("career.season", "f1_career_progressive/2017", patch, swofty::registry::Scope::career);
```

Writing a career layer with no career loaded returns `late`.

## Career snapshots

A registry whose schema has `"x-snapshot": "career"` is frozen into every new career.

```text
new career, data group f1_career_progressive
  career.starting
    core: snapshot career.season         34 records    41 KB -> 9 KB compressed
    core: snapshot career.driver_ratings 412 records   88 KB -> 17 KB compressed
    core: saved in the career's swofty_registry container
  ...
  player edits the what-if page in the launcher (profile layer changes)
  -> this career still reads the snapshot; careers started after the edit get the new grid

load the career again later
  career.loaded
    core: career.season and career.driver_ratings read from the snapshot (schema 1.0, current 1.1: read as is)
```

While a career is loaded:

- `get` returns **snapshot ⊕ career layers** for snapshot registries. Global and profile layers written after the career started don't reach it.
- Registries without `x-snapshot` keep reading live: base plus every layer.
- [Watchers](/std/reading#watching) fire once for every snapshot record when the career loads, and again when it unloads.

The snapshot lives in a core-owned save container, `swofty_registry`, inside the profile save, next to the containers career contents keep. An addon that isn't installed when the save is written doesn't lose its records: the save splice keeps containers it doesn't recognise.

### Budget

The game's save heap is small: a full `SAVE_DATA` heap crashed a 1.5 test build. So each snapshot registry has a budget:

```json
{ "x-snapshot": "career", "x-snapshot-budget": 65536 }
```

The budget is in bytes after compression, per career. When a registry's snapshot is over its budget, the core doesn't save it, logs the sizes, and that career reads the registry live. A whole career's `swofty_registry` container is capped at 256 KB.

```text
[registry] career.driver_ratings: snapshot 71 KB compressed, budget 64 KB; not saved, this career reads it live
```

### Migration

A snapshot keeps the schema version it was written with. On load:

- **Same major, older or newer minor:** read as is. Fields the reader doesn't know are kept (see [the tolerant reader rule](/std/versioning#the-tolerant-reader-rule)).
- **Older major:** the registry owner's migrate function converts it. Without one, the career keeps reading the old values through the version adapter and the core logs that it did.

```cpp
// the core's migration for career.season 1.x -> 2.0 (an example: 2.0 splits seats into objects)
swofty::registry::migrate("career.season", 1, 2, [](swofty::json::Value v1) {
    for (auto &team : v1["teams"].items()) {
        swofty::json::Value seats = swofty::json::array();
        for (auto &driver : team["seats"].items())
            seats.push(swofty::json::object({ { "driver", driver }, { "number", nullptr } }));
        team["seats"] = seats;
    }
    return v1;
});
```
