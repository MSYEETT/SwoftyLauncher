# Archive (ERP) edits

ERP operations describe changes to artwork and other resources in `.erp` archives. List these files in `operations.erp` in your [manifest](/reference/addons/manifest).

`swofty build` applies every operation below. A `patch-pkg` edit other than `clone-entry` and `set-value` is not built, and an addon whose ERP files use one has its ERP files skipped with a warning.

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

`erp` paths are relative to the game folder. `source` paths are relative to your addon folder.

## Operations

| Operation | Fields | Purpose |
|---|---|---|
| `replace-resource` | `erp`, `resource`, `fragment`, `source` | Replace one fragment of a resource with a raw file; `fragment` defaults to 0 |
| `add-resource` | `erp`, `resource`, `type`, `sources` | Add a resource, with one raw source file per fragment |
| `copy-resource` | `from_erp` or `source`, `erp`, `resource`, optional `as` | Copy a resource between archives |
| `put-resource` | `erp`, `resource`, `source`, optional `as` | Copy a resource from an archive in the addon, replacing a resource of the same name or adding it |
| `remove-resource` | `erp`, `resource` | Remove a resource |
| `add-mipmaps` | `source`, `target` | Copy a mip file, or a folder of them, to a game-relative path |
| `patch-pkg` | `erp`, `resource`, `fragment`, `edits` | Run `clone-entry` and `set-value` edits on a `!pkg` resource |
| `warn` | `message` | Print a message when the addon is built |

## Raw resources

`replace-resource` and `add-resource` take raw fragment data: the bytes a fragment holds after decompression. The build compresses each fragment the way the archive stores it and updates its size and the resource hash.

- `replace-resource` replaces fragment `fragment` of `resource` in `erp` with the file `source` and keeps the other fragments byte for byte. It fails when the archive has no such resource or the resource has no such fragment.
- `add-resource` appends `resource` to `erp`. `type` is the resource type, such as `GfxSurfaceRes`, at most 15 characters. `sources` lists one file per fragment, in order. The new resource takes its type argument, fragment names, flags and compression from the first resource in `erp` of the same type with the same number of fragments, and the operation fails when `erp` has none. It also fails when `erp` already has a resource with that name; `put-resource` replaces or adds.

```json
{
  "op": "add-resource",
  "erp": "2025_asset_groups/ui_package/screens/fe_loading.erp",
  "resource": "eaid://ui_package/art/textures/loading/sponsor.tif.image",
  "type": "GfxSurfaceRes",
  "sources": ["erp/sponsor.head.bin", "erp/sponsor.pixels.bin"]
}
```

## Removing resources

`remove-resource` takes `resource` out of `erp` and fails when `erp` has no such resource. The in-game loader cannot take a resource out of a stock archive, so a build with any `remove-resource` delivers every ERP operation by rewriting the archives, and prints:

```text
erp: built into the game files; 1 resource removal(s) cannot apply at runtime
```

## Mip files

A texture (`GfxSurfaceRes`) with three fragments keeps the rest of its mip chain in a loose file. Fragment 2 starts with a length byte and that file's path, relative to the game folder, such as `f1_safetycar_package/safety_car_2/textures/safety_car_2_decal_da.tif.mipmaps`.

`add-mipmaps` copies `source` to `target` in the game folder. A folder copies every file under it, keeping their paths below `target`. The files replace or add game files through the journal, on both routes. Point a texture at a new mip file by replacing fragment 2 with `replace-resource`, or by adding the texture with `add-resource`.

```json
{ "op": "add-mipmaps", "source": "mipmaps/", "target": "f1_safetycar_package/safety_car_2/textures/" }
```

## Copying resources

`copy-resource` appends a resource to the archive `erp`, copying its parts byte for byte. It reads the resource from the game archive `from_erp`, or from `source`, an archive in your addon folder. `as` gives the copy a new name. The copy fails when `erp` already has a resource with that name.

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

`patch-pkg` changes entries inside a `!pkg` resource, such as a track layer, and keeps the rest of the archive unchanged. Its `edits` work as in [UI patches](/reference/ops/ui-patches#routes-and-ui-databases):

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

ERP files take `when` and `{{id}}` settings like [database operations](/reference/ops/database). Only the archives a build changes are written to the overlay.

With the in-game loader, changed and added resources reach the game as overlay archives instead of rewritten archives ([deliveries](/reference/addons/asset-api#deliveries)). Both routes produce the same resource bytes. The overlay leaves out a replaced resource whose type, argument or fragment layout, fragment sizes included, differs from the stock one, and the build warns.

For menu changes, see [UI patches](/reference/ops/ui-patches). For the archive format, see [ERP archives](/reference/erp).
