---
title: Manifest
description: registry.json, next to swofty.json, declares an addon's registries, providers, layers, hooks, services and requirements in standard library 2.0.
---

# Manifest

An addon declares what it does with the standard library in **`registry.json`**, a file next to `swofty.json`. `swofty.json` gets no new fields.

```text
my-addon/
  swofty.json      the 1.x manifest, unchanged: id, name, version, compat, options, operations, launcher...
  registry.json    standard library 2.0: registries, provides, overrides, hooks, services, requires
```

- **A 1.x launcher never opens `registry.json`.** The addon still installs and runs there, with its 1.x parts.
- **A 2.0 launcher reads both.** The addon gains its registry records, layers, hooks and services.
- **An addon only useful with 2.0** (one that only layers registries, like a "what if" history editor) sets `"compat": { "launcher": ">=2.0.0" }` in `swofty.json`. That way a 1.x launcher says it needs a newer launcher instead of installing something that does nothing.

## Progressive Career 2.0

It provides two core registries, subscribes to hook points, defines its own point and publishes a service:

```json
// swofty.json: unchanged from 1.x apart from the version
{
  "schema_version": 1,
  "id": "swofty.progressive",
  "name": "Progressive Career",
  "version": "2.0.0",
  "author": "Swofty",
  "kind": ["database", "native"],
  "category": "career",
  "compat": { "game": "f1_25", "launcher": ">=1.5.15" },
  "requires": [ { "id": "swofty.classic-seasons", "version": ">=0.6.0" } ]
}
```

```json
// registry.json
{
  "provides": [
    { "registry": "career.season",         "file": "registry/seasons.json" },
    { "registry": "career.driver_ratings", "file": "registry/ratings.json", "dynamic": true },
    { "registry": "career.mode",           "file": "registry/mode.json" }
  ],
  "hooks": [
    { "point": "career.calendar.add_activity", "phase": "replace" },
    { "point": "career.contract.decide",       "phase": "replace" },
    { "define": "progressive.season.apply", "version": 1, "header": "api/progressive_api/hooks.hpp" }
  ],
  "services": [
    { "publish": "progressive.history", "version": 2, "header": "api/progressive_api/history.hpp" }
  ],
  "requires": [
    { "registry": "career.season", "version": "^1" },
    { "registry": "career.driver_ratings", "version": "^1" }
  ],
  "recommends": [
    { "record": "track:venue/sochi", "for": "the 2016 Russian Grand Prix" }
  ]
}
```

On launcher 1.5 this addon is Progressive Career 1.x: its database rows and DLLs, which keep their own `years.json` path. On 2.0 it adds the registry records the other addons build on.

## Tails' "what if" addon

It only overrides, so it's useless without 2.0 and says so:

```json
// swofty.json
{
  "schema_version": 1,
  "id": "tails.whatif-history",
  "name": "What If History",
  "version": "1.0.0",
  "author": "Tails",
  "kind": ["native"],
  "category": "career",
  "compat": { "game": "f1_25", "launcher": ">=2.0.0" },
  "launcher": {
    "program": { "windows": "bin/whatif.exe", "linux": "bin/whatif" },
    "sections": [ { "id": "history", "title": "What if history" } ]
  }
}
```

```json
// registry.json
{
  "overrides": [
    { "registry": "career.season",         "scope": "profile", "editable": true },
    { "registry": "career.driver_ratings", "scope": "profile", "editable": true },
    { "registry": "career.season",         "scope": "global",  "file": "layers/examples.json" }
  ],
  "requires": [
    { "registry": "career.season", "version": "^1" },
    { "registry": "career.driver_ratings", "version": "^1" },
    { "service": "progressive.history", "version": ">=1" }
  ]
}
```

## An addon that owns a registry

```json
// registry.json
{
  "registries": [
    { "name": "tails.whatif.presets", "schema": "schemas/tails.whatif.presets.1.0.json" }
  ],
  "provides": [
    { "registry": "tails.whatif.presets", "file": "registry/presets.json" }
  ]
}
```

## Option-bound and selector layers

An option stays in `swofty.json`, where ADDON SETTINGS and the launcher already find it. A layer in `registry.json` refers to it by id:

```json
// swofty.json
{ "options": [ { "id": "damage", "type": "choice", "values": ["realistic", "heavy", "brutal"], "default": "realistic" } ] }
```

```json
// registry.json
{
  "overrides": [
    { "registry": "vehicle.handling", "select": { "where": { "class": "f1" } }, "file": "layers/damage_heavy.json",
      "when": { "option": "damage", "is": "heavy" } }
  ]
}
```

## The fields of registry.json

- **`registries`:** registries this addon owns, `{name, schema}`. The name starts with the addon's id. Each later version adds its schema file; the newest is current. See [The registry](./registry).
- **`provides`:** base records, `{registry, file}` for a static provider, plus `"dynamic": true` when code provides too. See [Providers](./providers).
- **`overrides`:** layers, `{registry, scope, file?, editable?, select?, when?}`.
  - `file` is a static layer, and `editable` lets the addon's launcher pages write a profile layer.
  - `select` makes it a [selector](./layers#selectors) layer, and `when` binds it to an [option](./layers#option-bound-layers).
- **`hooks`:** hook points this addon subscribes to (`{point, phase}`), and points it defines (`{define, version, header}`). The launcher checks that every point exists. See [Hooks](./hooks) and [Events](./events).
- **`services`:** services this addon publishes, `{publish, version, header}`. See [Services](./services).
- **`requires`:** an array of what this addon needs: `{registry, version}`, `{service, version}`, `{hook, version}` or `{record, version?}`. Play stops while one is missing. See [Dependencies and guards](./dependencies).
- **`recommends`:** the same entries, optional, each with a `for` shown to players. They never stop Play.
- **`guards`:** declarative guards (see [Guards](./dependencies#guards)).

`header` paths point at C++ headers the addon ships for others to include. Its docs page links them.

## Addon requirements stay in swofty.json

`swofty.json`'s 1.x `requires` (`[{id, version}]`) keeps meaning "this addon needs that addon". `registry.json`'s `requires` adds finer dependencies: a single track, a registry version, a service. The launcher checks both, in the same place, with the same messages.
