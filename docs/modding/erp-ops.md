# Archive (ERP) edits

ERP operations describe changes to artwork and other resources in `.erp` archives. List these files in `operations.erp` in your [manifest](/modding/manifest).

`swofty build` applies `patch-pkg`, `copy-resource` and `warn`. A mod whose ERP files use any other operation has its ERP files skipped.

## Example

```json
{
  "schema_version": 1,
  "ops": [
    {
      "op": "replace-resource",
      "erp": "2025_asset_groups/ui_package/screens/fe_loading.erp",
      "resource": "eaid://ui_package/art/textures/loading/background.tif",
      "fragment": 0,
      "source": "erp/background.tif"
    }
  ]
}
```

The resource name above is an example. List the real names with:

```sh
python3 tools/erp.py <archive> '*'
```

`erp` paths are relative to the game folder. `source` paths are relative to your mod folder.

## Operations

| Operation | Fields | Purpose |
|---|---|---|
| `replace-resource` | `erp`, `resource`, `fragment`, `source` | Replace one part of a resource; `fragment` defaults to 0 |
| `add-resource` | `erp`, `resource`, `type`, `sources` | Add a resource, with one source file per part |
| `copy-resource` | `from_erp` or `source`, `erp`, `resource`, optional `as` | Copy a resource between archives |
| `remove-resource` | `erp`, `resource` | Remove a resource |
| `add-mipmaps` | `source`, `target` | Copy texture files to a game-relative path |
| `patch-pkg` | `erp`, `resource`, `fragment`, `edits` | Run `clone-entry` and `set-value` edits on a `!pkg` resource |
| `warn` | `message` | Print a message when the mod is built |

## Copying resources

`copy-resource` appends a resource to the archive `erp`, copying its parts byte for byte. It reads the resource from the game archive `from_erp`, or from `source`, an archive in your mod folder. `as` gives the copy a new name. The copy fails when `erp` already has a resource with that name.

```json
{
  "op": "copy-resource",
  "source": "erp/car.erp",
  "erp": "2025_asset_groups/f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso_barcelona_26.erp",
  "resource": "eaid://f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso.wep",
  "as": "eaid://f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso_barcelona_26.wep"
}
```

Run `patch-pkg` after the copy to change the copy's contents, such as the layer names a renamed world lists.

## Editing `!pkg` resources

`patch-pkg` changes entries inside a `!pkg` resource, such as a track layer, and keeps the rest of the archive unchanged. Its `edits` work as in [UI patches](/modding/ui-patches#routes-and-ui-databases):

- `clone-entry` copies the entry `src` in the container at `at` and names the copy `as`. In an array, `src` is a selector such as `[name=vehicles_current_myteam$PitBox12]`, and `as` replaces the selected field.
- `set` and `set-value` paths use dots and `[field=value]` selectors. A value is written in the type of the value it replaces: numbers for integers and floats, 16 numbers for a `mat4`, a hex string for a `woid`.
- `from` on `clone-entry` takes the entry from another resource, given as `erp`, `resource` and `fragment`, as shipped with the game.

```json
{
  "op": "patch-pkg",
  "when": { "cars": 26 },
  "erp": "2025_asset_groups/environment_package/tracks/bahrain/wep/bahrain_f1_26_myteam.erp",
  "resource": "eaid://environment_package/tracks/bahrain/wep/bahrain-vehicles_current_myteam.layer",
  "edits": [
    {
      "op": "clone-entry", "at": "instances",
      "src": "[name=vehicles_current_myteam$PitBox12]", "as": "vehicles_current_myteam$PitBox13",
      "set": {
        "uniqueid": "8631efce0fbce521eb5b71c1d07dd616",
        "overrides.VehPlacement.Id": 13,
        "overrides.WorldObject.transform": [-0.9988, 0.0, -0.048, 0.0, 0.0, 1.0, 0.0, 0.0, 0.048, 0.0, -0.9988, 0.0, -350.3214, -15.4045, -190.421, 1.0]
      }
    }
  ]
}
```

ERP files take `when` and `{{id}}` settings like [database operations](/modding/database-ops). Only the archives a build changes are written to the overlay.

For menu changes, see [UI patches](/modding/ui-patches). For the archive format, see [ERP archives](/reference/erp).
