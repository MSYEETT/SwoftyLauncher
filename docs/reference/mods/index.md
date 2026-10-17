---
title: Creating mods
description: What a mod is made of, how it reaches the game, and where each kind of change is documented.
---

# Creating mods

A mod changes F1 25's content and carries no code: textures, models, liveries, database rows, text and tracks.

A mod is packaged as a folder with a [`swofty.json`](/reference/addons/manifest) manifest that lists its operations and no plugins. The player installs it like an addon ([Packaging a mod](./packaging)), and the launcher applies the operations to copies of the game's files when it builds the profile. The game folder keeps its original files.

```
my-mod/
├── swofty.json
├── db/
│   └── changes.json
└── erp/
    ├── changes.json
    └── source.erp
```

## What to change

| Change | Game files | Page |
|---|---|---|
| Textures, models, materials and worlds | `.erp` archives under `2025_asset_groups/` | [Textures and models](./erp-assets) |
| Teams, cars, drivers, tracks, rules and unlocks | `game_package/gamedatabases/gamedatabases.2025.bin` | [Game database](./database) |
| Track layers and menu scenes | `!pkg` resources inside `.erp` archives | [Textures and models](./erp-assets#edit-a-pkg-resource) |
| A special livery for a car | A `wep` archive, the livery and unlock tables | [Adding a livery](./livery) |
| Names and descriptions | Text tags and language files | [In-game text](./text) |
| New tracks | A track package | [Tracks](./tracks) |

## Operations

| Manifest field | Operations | Reference |
|---|---|---|
| `operations.database` | `set`, `add-row`, `remove-row`, `list-append`, `list-remove`, `put-record`, `remove-record` | [Database operations](/reference/ops/database) |
| `operations.erp` | `copy-resource`, `put-resource`, `patch-pkg`, `warn` and others | [ERP operations](/reference/ops/erp) |
| `operations.files` | Loose files copied into the game folder | [File operations](/reference/ops/files) |
| `operations.assets` | Prefixed ops such as `db.set` and `erp.put-resource`, in one file | [Addon manifest](/reference/addons/manifest#operations) |

A mod can declare [settings](/reference/addons/settings) whose values feed its operations, such as a choice of livery name.

## Tools

| Tool | Use |
|---|---|
| `tools/erp.py` | List and extract an archive's resources |
| `swofty build` | Build the profile and print each operation's warnings and conflicts. See [the command line](/reference/cli#swofty-build) |
| [Game database tools](/reference/gamedb/tools) | Read, write and inspect the game database |
