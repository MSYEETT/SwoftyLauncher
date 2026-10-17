---
title: Your first addon
description: Write a swofty.json, change one game value, then install and enable the addon.
---

# Your first addon

This tutorial builds a data-only addon that makes a stop-go penalty last 10 seconds. It needs no compiler.

## 1. Make the folder

```
my-first-addon/
├── swofty.json
└── db/
    └── penalty.json
```

## 2. Write the manifest

`swofty.json` names the addon and lists what it changes:

```json [swofty.json]
{
  "schema_version": 1,
  "id": "yourname.my-first-addon",
  "name": "My First Addon",
  "version": "0.1.0",
  "author": "Your Name",
  "description": "Stop-go penalties last 10 seconds.",
  "kind": ["database"],
  "compat": { "game": "f1_25", "game_builds": [">=25251167"] },
  "requires": [{ "id": "swofty.core", "version": ">=0.1.0" }],
  "operations": {
    "database": ["db/penalty.json"]
  }
}
```

The `id` uses lowercase letters, digits, `.`, `_` and `-`. The `swofty.` prefix is reserved. Every field is listed in the [addon manifest](/modding/manifest) reference.

## 3. Change a value

`db/penalty.json` holds [database operations](/modding/database-ops). This one sets one field of one row:

```json [db/penalty.json]
{
  "schema_version": 1,
  "ops": [
    {
      "op": "set",
      "select": "rules.stop_go_penalty",
      "field": "penalty_duration",
      "value": "10"
    }
  ]
}
```

Values are strings, whatever the field's type.

## 4. Install

::: code-group

```sh [Command line]
swofty addons install ./my-first-addon
```

```text [Launcher]
Addons → Install addon… → the folder that contains swofty.json
```

:::

The launcher copies the folder to `$DATA/addons/yourname.my-first-addon/`. Installing again replaces that copy.

## 5. Enable

Pick a modded profile, then turn the addon on with its switch on the **Addons** page, or:

```sh
swofty addons enable yourname.my-first-addon
```

The `vanilla` profile takes no addons.

## 6. Play

Press **Play** in the launcher, or run `swofty run`. The launcher builds the profile, applies `db/penalty.json` to the game database and starts the game. To build without starting the game, run `swofty build` and read its warnings.

In game, **ADDON SETTINGS** under **Career** has a tile for **My First Addon** with its author, version and description.

::: tip Next
Give the penalty a slider in [Addon settings](/tutorials/addon-settings).
:::

## Share it

Share the folder. Include your operations, never copies of the game's archives, database or executable.

The [addon template](/modding/template) has every kind of operation and a native plugin to start from.
