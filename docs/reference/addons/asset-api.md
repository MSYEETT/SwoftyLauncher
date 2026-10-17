---
title: Asset API
description: Asset ops, how the build routes them to build or runtime delivery, and the parts still being built.
---

# Asset API

An asset op is one change to game content: an asset group, an archive resource, a database record, a language string or a loose file. Addons declare asset ops in their manifest, and built-in addons return them from a [build hook](./launcher-api#build-hooks). The types are in the `swofty-addon` crate (`packages/addon-api`), module `swofty_addon::asset`.

## Op files

`operations.assets` in `swofty.json` lists op files. Each op has a prefix that names its kind.

```json [assets/livery.json]
{
  "schema_version": 1,
  "ops": [
    { "op": "erp.put-resource", "erp": "2025_asset_groups/f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso.erp",
      "resource": "eaid://f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso.wep", "source": "erp/car.erp" },
    { "op": "db.set", "select": "entrants.teams.team[_metadata_=toro_rosso_26]", "field": "display_name_code", "value": "AT" },
    { "op": "lang.set", "language": "*", "key": "lng_team_tor_hashtag", "value": "AlphaTauri" },
    { "op": "file.put", "source": "files/sponsor.dds", "target": "livery_package_shared/sponsor.dds" },
    { "op": "warn", "message": "Racing Bulls livery" }
  ]
}
```

`operations.database` and `operations.erp` files load into the same model, with `db.` and `erp.` added to each op.

| Op | Fields | Change |
|---|---|---|
| `group.register` | `group`, `erp` or `erps`, optional `deps`, `template` | Add an asset group to the engine's manifest |
| `erp.*` | As in [ERP operations](/reference/ops/erp) | Change an archive |
| `db.*` | As in [database operations](/reference/ops/database) | Change the game database |
| `lang.set` | `language` (code or `*`), `key`, `value` | Set one string |
| `lang.table` | `language`, `source` | Load a whole `.lng` file |
| `file.put` | `source`, `target` | Replace or add a loose game file |
| `warn` | `message` | Print a message at build time |

A `source` is a path inside the addon, or `{ "root": "addon" | "data" | "cache", "path": "..." }` for a file in the addon's folder, its `addon-data` folder or its cache folder.

## Routing

Each op kind (`group`, `erp`, `db`, `lang`, `file`) is delivered one of three ways:

| Route | Meaning |
|---|---|
| Build | The build rewrites the game file and installs it through the journal |
| Runtime | swofty_core applies the op in memory while the game boots |
| Waiting | No delivery exists yet. The build lists the op |

`erp`, `db` and `file` ops are built. `group` and `lang` ops wait. The installed loader states the kinds it delivers at runtime in `swofty/addons/core/features.json`, as `stable` or `experimental`.

When two addons write the same target, the later addon in the profile's load order wins, and the build lists the conflict.

## Runtime delivery

<Badge type="warning" text="in progress" /> Runtime delivery of `group`, `lang`, `erp`, `db` and `file` ops through `swofty/assets.json`, and the native `swofty::assets` calls for plugins (API version 4, `swofty/assets.hpp`, the `assets` capability), are being built. This section is filled when they land. See the [design](/design/addons-and-mods).
