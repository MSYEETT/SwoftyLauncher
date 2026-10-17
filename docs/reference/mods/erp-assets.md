---
title: Replacing and adding ERP assets
description: Copy resources between archives and edit !pkg resources such as track layers with ERP operations.
---

# Replacing and adding ERP assets

F1 25 stores its art, models, worlds and menus in `.erp` archives under `2025_asset_groups/`. An addon changes them with [ERP operations](/reference/ops/erp) listed in `operations.erp`. When the launcher builds the profile, it writes each archive the operations change to the profile's overlay.

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

## Replace part of a resource

`replace-resource` swaps one fragment of a resource for a raw file from your addon and keeps the other fragments. A texture's fragment 1 holds its pixels:

```json
{
  "op": "replace-resource",
  "erp": "2025_asset_groups/f1_safetycar_package/safety_car_2/wep/safety_car_2.erp",
  "resource": "eaid://f1_safetycar_package/safety_car_2/textures/safety_car_2_decal_da.tif.image",
  "fragment": 1,
  "source": "erp/decal_pixels.bin"
}
```

`add-resource` adds a resource from one raw file per fragment, and `remove-resource` takes one out. A build that removes a resource rewrites the archives instead of loading overlays in game. [ERP operations](/reference/ops/erp#raw-resources) has the rules.

## Add mip files

A texture keeps the rest of its mip chain in a loose `.mipmaps` file whose path is in the texture's fragment 2. `add-mipmaps` copies a mip file, or a folder of them, to a path in the game folder:

```json
{ "op": "add-mipmaps", "source": "mipmaps/", "target": "f1_safetycar_package/safety_car_2/textures/" }
```

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

ERP files take `when` and <span v-pre>`{{id}}`</span> from `db` [settings](/reference/addons/settings#use-a-db-setting), like database operations.

::: warning Not built yet
`patch-pkg` edits other than `clone-entry` and `set-value` are defined but not built. An addon whose ERP files use one has all its ERP files skipped with a warning.
:::

Every operation is in the [ERP operations](/reference/ops/erp) reference.
