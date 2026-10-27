---
title: Rollout and docs
description: The releases that ship standard library 2.0, what each must prove, and how its public docs are written.
---

# Rollout and docs

## The releases

Each release goes through the usual flow: a prep commit on main, the main CI, then the tag. Each one needs swofty's go-ahead in the chat where it's prepared.

### 2.0.0: the registry

**Ships:**

- the core's registry: records, schemas and `x-` keywords, owned keys, static and dynamic providers, layers, bases and variants (`extends`), scopes, career snapshots, adapters, origins, watching;
- services;
- the hooks table, with addon-defined points only (no game points yet);
- `swofty::json` and the typed structs for `career.season` and `career.driver_ratings`;
- the launcher side: registry files in the build, build-time resolution with the core's appliers for `era.box`, `content.package`, `vehicle` and `engine_sound`, the five host calls, registry conflicts, `registry/effective.json` in Send Logs, `requires` checks before Play;
- **Classic Seasons**: every season provides its `content.package`, `era.box`, `vehicle` and `engine_sound` records under its own keys, and its build comes out the same as 0.6.x;
- **Progressive Career 2.0**: provider of both registries, reading the effective values in all its DLLs, the `progressive.history` service and the `progressive.season.apply` point;
- `templates/std/whatif`: a small addon with a launcher page that edits `career.season`;
- `templates/std/engine-sound`: a data-only addon that adds an engine sound under its own key, layers it onto the 2017 Ferrari, and adds an "F1 2017 (V10)" variant with its own GP/TT box;
- the compatibility gate (`tools/compat/`), `tools/std/schemacheck.py`, and the snapshot fixtures.

**Done when:**

- the gate passes on every Addon Browser addon;
- Progressive 2.0 matches 1.2 in game: the same seats, swaps, ratings, logos, fuel and follow contracts in a 2016 and a 2017 career;
- the example addon's 2017 change shows on Choose Driver, on the grid, in follow mode and in the season's ratings, and a career started before the change keeps the old grid;
- Classic Seasons on records builds the same game files as 0.6.x, and the engine sound example plays its bundle on the 2017 Ferrari in Time Trial and shows the V10 variant as its own box;
- the public docs below are written.

### 2.1.0: the 1.x libraries move in, the first game hook points

**Ships:**

- career contents, Team HQ tabs, the wizard, characters, strings and options stored in registries underneath their 1.x calls (era boxes already are, from 2.0.0);
- the remaining content registries: `livery`, `helmet`, `track`, with their appliers;
- the game hook points `career.calendar.add_activity`, `career.contract.decide`, `career.contract.sign` and `session.fuel.load`.

**Done when:**

- the shim tests pass;
- Progressive and Career Editor use the hook points instead of their own wraps of those functions;
- the gate shows no 1.x addon's chain changed.

### 2.2.0: identities and operations

**Ships:** `team.identity` (names, logos, numbers), `driver.identity`, and the operations `apply_lineup`, `set_seat`, `set_driver_ratings`, `set_car_number`, `team::set_logo` and `session::set_track_fuel`.

**Done when:** Progressive and Career Editor call the operations instead of their own copies of that code.

### After 2.2

A new registry, hook point or operation ships as experimental first. It becomes stable once two addons use it (see [Status](/std/versioning#status)).

## The docs

The public docs for 2.0 are written by hand, the same way these design pages are laid out. Nothing in them is generated.

### The order

The docs start with the registry, then go through it one subfeature at a time, and only then reach the parts that connect addons:

```text
Standard library
  Start here                 the registry in one example (provider, layer, reader), then the map
  The registry               records, schemas, typed records, swofty::json
    Keys, bases, variants    your own keys; extending a base in place; variants with extends
    Providers                static, dynamic, both
    Reading                  effective values, origins, listing, watching
    Layers                   patches, precedence, validation, conflicts
    Scopes and snapshots     global, profile, career; snapshots, budget, migration
    Versioning               minor and major, the tolerant reader, adapters, status
  Hooks                      phases, game points, your own points, living with mixins
  Services
  Operations
  The launcher side          registry files, launcher pages, conflicts, Send Logs
  Manifest
  Content registries         GP/TT packages, era boxes, cars, engine sounds, liveries, helmets, tracks
  Build time and run time    appliers, x-resolve, the order of the build
  Registries                 one page per core registry: what it holds, a full example record,
                             a full layer, who provides it, its version history
  Hook points                one page per point: its payload, its thread, a full handler
  Moving from 1.x            nothing breaks; how to adopt the registry at your own pace
  The C ABI
```

### How each page is written

- **One sentence, then code.** Each page opens with a sentence saying what the subfeature is, then a complete example: a whole provider, a whole layer file, a whole handler, with the includes and `SWOFTY_ON_LOAD`. Then the details, each with a smaller example.
- **Code over prose.** If a sentence can be a code comment next to the line it describes, it goes there.
- **Few tables.** Tables only for small fixed lookups such as result codes. Everything else is code, a short list, or an annotated log.
- **Real logs.** Where the core logs something (a conflict, a dropped layer, a snapshot), the page shows the actual log line, so readers recognise it in their own `loader.log`.
- **Examples that compile.** Every C++, Rust and JSON example longer than a few lines lives in a real addon under `templates/std/`, and the page includes it with VitePress's `<<<` file include. CI builds those addons and validates their JSON against the schemas, so an example can't drift from the API.
- **Status at the top.** A page about an experimental or superseded feature starts with a VitePress `<Badge>` and names the release it arrived in.
- **The same voice as the current docs:** short, direct sentences that state facts.

### What changes in the current docs

- The 1.x library pages (`/reference/std/*`) stay. Each gets a short note at the top linking to its 2.0 registry page, and a line saying the 1.x calls keep working.
- `/reference/addons/manifest` gains the new fields, each linking to its page.
- `/reference/addons/launcher-api` gains the five registry host calls, with the Rust example from [The launcher side](/std/launcher).
- Every release note gets a "Standard library" section, quoted from each registry's `CHANGES.md`.
