---
title: Manifest
description: The swofty.json fields an addon uses to declare registries, providers, layers, hooks, services and requirements in standard library 2.0.
---

# Manifest

An addon declares what it does with the standard library in `swofty.json`. All the new fields are optional. An addon that uses any of them sets `compat.launcher` to `>=2.0.0`, because a 1.x launcher rejects fields it doesn't know.

## Every field in one manifest

Progressive Career 2.0, which owns nothing, provides two core registries, defines a hook point and publishes a service:

```json
{
  "schema_version": 1,
  "id": "swofty.progressive",
  "name": "Progressive Career",
  "version": "2.0.0",
  "author": "Swofty",
  "kind": ["database", "native"],
  "category": "career",
  "compat": { "game": "f1_25", "launcher": ">=2.0.0" },

  "provides": [
    { "registry": "career.season",         "file": "registry/seasons.json" },
    { "registry": "career.driver_ratings", "file": "registry/ratings.json", "dynamic": true }
  ],

  "hooks": [
    { "point": "career.calendar.add_activity", "phase": "replace" },
    { "point": "career.contract.decide",       "phase": "replace" },
    { "define": "progressive.season.apply", "version": 1, "header": "api/progressive_api/hooks.hpp" }
  ],

  "services": [
    { "publish": "progressive.history", "version": 2, "header": "api/progressive_api/history.hpp" }
  ],

  "requires": {
    "career.season": "^1",
    "career.driver_ratings": "^1"
  }
}
```

Tails' "what if" addon, which only overrides:

```json
{
  "schema_version": 1,
  "id": "tails.whatif-history",
  "name": "What If History",
  "version": "1.0.0",
  "author": "Tails",
  "kind": ["native"],
  "category": "career",
  "compat": { "game": "f1_25", "launcher": ">=2.0.0" },

  "overrides": [
    { "registry": "career.season",         "scope": "profile", "editable": true },
    { "registry": "career.driver_ratings", "scope": "profile", "editable": true },
    { "registry": "career.season",         "scope": "global",  "file": "layers/examples.json" }
  ],

  "requires": {
    "career.season": "^1",
    "career.driver_ratings": "^1",
    "service:progressive.history": ">=1"
  },

  "launcher": {
    "program": { "windows": "bin/whatif.exe", "linux": "bin/whatif" },
    "sections": [ { "id": "history", "title": "What if history" } ]
  }
}
```

An addon that owns a registry of its own:

```json
{
  "registries": [
    { "name": "tails.whatif.presets", "schema": "schemas/tails.whatif.presets.1.0.json" }
  ],
  "provides": [
    { "registry": "tails.whatif.presets", "file": "registry/presets.json" }
  ]
}
```

## The fields

- **`registries`**: registries this addon owns, `{name, schema}`. The name starts with the addon's id. Each later version adds its schema file; the newest is the current one.
- **`provides`**: base records, `{registry, file}` for a static provider, plus `"dynamic": true` when code provides too. See [Providers](./providers).
- **`overrides`**: layers, `{registry, scope, file?, editable?}`. `file` is a static layer. `editable` lets this addon's launcher pages write a profile layer. See [Layers](./layers) and [Scopes](./scopes).
- **`hooks`**: hook points this addon subscribes to (`{point, phase}`), and points it defines (`{define, version, header}`). The launcher checks that every point exists. See [Hooks](./hooks).
- **`services`**: services this addon publishes, `{publish, version, header}`. See [Services](./services).
- **`requires`**: semver ranges for registries (`"career.season": "^1"`), services (`"service:progressive.history": ">=1"`) and hook points (`"hook:progressive.season.apply": ">=1"`) this addon needs.
  - At install, the launcher warns when nothing in the profile provides them.
  - At Play, it names the missing or too-old one, the same way it names a missing addon requirement today.

`header` paths point at the C++ headers the addon ships for others to include. Its docs page should link them.
