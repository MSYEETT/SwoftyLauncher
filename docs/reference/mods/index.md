---
title: Make a mod
description: Build a mod with data operations, a swofty.json manifest and one database change, then follow the guide for the content you want to edit.
---

# Make a mod

To change F1 25's content, you can ship a mod: an [addon](/reference/addons/) with no code, DLL or launcher plugin. You describe your changes as `operations`, which edit the game database, in-game text, archive resources and loose files.

You supply the operations; the launcher applies them when it builds a profile, to a copy of the game's files or in memory while the game boots. This keeps the game folder's stock files intact. To remove every change, run `swofty restore`.

## What a mod changes

| Change | Game files | Page |
|---|---|---|
| Teams, cars, drivers, tracks, rules, unlocks | `game_package/gamedatabases/gamedatabases.2025.bin` | [Game database](./database) |
| Textures, models, materials, worlds | `.erp` archives under `2025_asset_groups/` | [Textures and models](./erp-assets) |
| A special livery for one car | A `wep` archive plus the livery and unlock tables | [Adding a livery](./livery) |
| Names and descriptions | Text tags and language strings | [In-game text](./text) |
| A new track | A track package | [Tracks](./tracks) |

## The manifest

Start with a folder containing a `swofty.json` manifest at its root. Alongside it, put the files your operations name:

```
my-mod/
├── swofty.json
└── db/
    └── changes.json
```

In `swofty.json`, give your mod a name and list its `operations` so the launcher knows what to apply:

```json [swofty.json]
{
  "schema_version": 1,
  "id": "yourname.my-mod",
  "name": "My Mod",
  "version": "1.0.0",
  "author": "Your Name",
  "description": "One line shown on the Addons page.",
  "kind": ["database"],
  "compat": { "game": "f1_25", "game_builds": [">=25251167"] },
  "operations": {
    "database": ["db/changes.json"]
  }
}
```

Since your mod ships data only, set `operations` and leave out anything that carries code: the `native` and `launcher` fields. See the [addon manifest](/reference/addons/manifest) reference for every field.

| `operations` field | Contains | Reference |
|---|---|---|
| `database` | database operation files (`set`, `add-row`, `remove-row`, `put-record`, ...) | [Database operations](/reference/ops/database) |
| `erp` | ERP operation files (`copy-resource`, `put-resource`, `replace-resource`, `patch-pkg`, ...) | [ERP operations](/reference/ops/erp) |
| `files` | loose file copies: `[{ "source": "files/x", "target": "game/path" }]` | [File operations](/reference/ops/files) |
| `assets` | one file mixing prefixed ops — `db.set`, `erp.put-resource`, `lang.set`, `group.register` | [Asset API](/reference/addons/asset-api) |

Use `kind` to list the categories shown for your mod on the Addons page: `database`, `erp`, `ui`, `files`.

## Your first mod

For your first mod, make a stop-go penalty last 10 seconds. You only need one database operation.

Create `db/changes.json` in the folder shown above:

```json [db/changes.json]
{
  "schema_version": 1,
  "ops": [
    { "op": "set", "select": "rules.stop_go_penalty", "field": "penalty_duration", "value": "10" }
  ]
}
```

Next, install the folder, enable it on a modded profile, and build:

```sh
swofty addons install ./my-mod
swofty addons enable yourname.my-mod
swofty build
```

`swofty build` applies your operation to a copy of the database and writes the plan to `$DATA/cache/overlay/<profile>/<buildhash>/`. To try the change in-game, press **Play** in the launcher or run `swofty run --apply`. You need a modded profile because the `vanilla` profile takes no addons.

## The path

Choose a guide for the change you want to make:

1. [Adding a livery](./livery) — the full worked example: a special livery as a new picker choice.
2. [Textures and models](./erp-assets) — replace and add resources in `.erp` archives.
3. [Game database](./database) — change values, copy rows, rename tracks.
4. [In-game text](./text) — set names and descriptions with `lang` operations.
5. [Tracks](./tracks) — import and activate a community track.
6. [Packaging a mod](./packaging) and [Testing and publishing](./publishing) — prepare your mod to share.
