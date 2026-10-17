---
title: Replacing and adding ERP assets
description: Copy resources between archives and edit !pkg resources such as track layers with ERP operations.
---

# Replacing and adding ERP assets

F1 25 stores its art, models, worlds and menus in `.erp` archives under `2025_asset_groups/`. An addon changes them with [ERP operations](/modding/erp-ops) listed in `operations.erp`. When the launcher builds the profile, it writes each archive the operations change to the profile's overlay.

## Look inside an archive

List an archive's resources with the repository's tool:

```sh
python3 tools/erp.py "2025_asset_groups/environment_package/tracks/monaco/wep/monaco_f1_26_myteam.erp" '*'
```

A resource has a name such as `eaid://environment_package/tracks/monaco/wep/monaco-vehicles_current_myteam.layer` and one or more fragments. See [ERP archives](/reference/erp) for the format.

## Add a resource

`copy-resource` appends a resource to an archive, byte for byte. It reads the resource from `from_erp`, an archive the game ships, or from `source`, an archive in your addon folder. `as` gives the copy a new name:

```json [erp/car.json]
{
  "schema_version": 1,
  "ops": [
    {
      "op": "copy-resource",
      "source": "erp/car.erp",
      "erp": "2025_asset_groups/f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso_barcelona_26.erp",
      "resource": "eaid://f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso.wep",
      "as": "eaid://f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso_barcelona_26.wep"
    }
  ]
}
```

`erp` paths are relative to the game folder. `source` paths are relative to the addon folder. The copy fails when `erp` already has a resource with the new name.

## Edit a `!pkg` resource

Worlds, layers and UI databases are `!pkg` resources: trees of named entries. `patch-pkg` edits entries inside one and keeps the rest of the archive unchanged. Custom Grid Size adds pit garages to each track this way. From `addons/custom-grid-size/tracks/monaco.json`:

```json [tracks/monaco.json]
{
  "op": "patch-pkg",
  "when": { "cars": 26 },
  "erp": "2025_asset_groups/environment_package/tracks/monaco/wep/monaco_f1_26_myteam.erp",
  "resource": "eaid://environment_package/tracks/monaco/wep/monaco-vehicles_current_myteam.layer",
  "edits": [
    {
      "op": "clone-entry",
      "at": "instances",
      "src": "[name=vehicles_current_myteam$PitGarage23]",
      "as": "vehicles_current_myteam$PitGarage25",
      "set": {
        "uniqueid": "a5256e66c3b302404d61d63301e06514",
        "overrides.WorldObject.transform": [-0.3303549885749817, 0.0, -0.9438560009002686, 0.0, 0.0, 1.0, 0.0, 0.0, 0.9438560009002686, 0.0, -0.3303549885749817, 0.0, -334.4569396972656, -14.192349433898926, 255.0829315185547, 1.0],
        "overrides.VehPlacement.Id": 25
      }
    }
  ]
}
```

| Edit | Fields | Effect |
|---|---|---|
| `clone-entry` | `at`, `src`, `as`, optional `set` and `from` | Copies the entry `src` in the container `at` and names the copy `as`. In an array, `src` is a selector such as `[name=...]` and `as` replaces the selected field. `from` takes the entry from another resource, given as `erp`, `resource` and `fragment` |
| `set-value` | `path`, `value` | Sets one value |

Paths use dots and `[field=value]` selectors. A value is written in the type of the value it replaces: numbers for integers and floats, 16 numbers for a `mat4`, a hex string for a `woid`.

Run `patch-pkg` after a `copy-resource` to change the copy, such as the layer names a renamed world lists.

## Warn the player

`warn` prints a message when the addon is built. Custom Grid Size names the tracks that lack room for the chosen grid:

```json
{ "op": "warn", "when": { "cars": 30 },
  "message": "Monaco has room for 28 cars because grid slot 29 would be only 6 m past the last corner. A 30-car race there leaves 2 cars without a grid slot or garage." }
```

## Settings

ERP files take `when` and <span v-pre>`{{id}}`</span> from `db` [settings](/tutorials/addon-settings#use-a-db-setting), like database operations.

::: warning Not built yet
The build applies `copy-resource`, `put-resource`, `patch-pkg` with `clone-entry` and `set-value` edits, and `warn`. `replace-resource`, `add-resource`, `remove-resource` and `add-mipmaps` are defined, and an addon whose ERP files use any of them has all its ERP files skipped with a warning.
:::

Every operation is in the [ERP operations](/modding/erp-ops) reference.
