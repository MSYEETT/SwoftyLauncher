# ERP operations

ERP operations change resources inside the game's `.erp` archives. The format is described in [ERP archives](/reference/erp). The patched archive is built into the profile's cache and reaches the game through the journal, never by editing the game's copy in place.

## File format

An ERP operation file is listed in `operations.erp`:

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

`erp` paths are relative to the game folder. `source` paths are relative to the mod root. `resource` is the resource name as the ERP index lists it. The resource names on this page are illustrative; `python3 tools/erp.py <archive> '*'` lists a real archive's names.

## Operations

| Op | Fields | Effect |
|---|---|---|
| `replace-resource` | `erp`, `resource`, `fragment?`, `source` | Replaces one fragment of a resource (fragment 0 if absent) |
| `add-resource` | `erp`, `resource`, `type`, `sources` | Adds a resource with the given type and one source file per fragment |
| `copy-resource` | `from_erp`, `erp`, `resource` | Copies a resource and all its fragments from one game ERP into another |
| `remove-resource` | `erp`, `resource` | Removes a resource |
| `add-mipmaps` | `source`, `target` | Adds mipmap files from the mod at a game-relative `target` |

[UI patches](/modding/ui-patches) are a separate operation list, because they transform `!pkg` resources rather than replace them.

## `foreach`

`foreach` expands every operation in the file once per item of a game-profile enumerator, such as every 2026 track. `{track}` is replaced by each item. This example is the per-track route to a 24-slot 2026 world: it copies the My Team vehicle layer into each normal 2026 world archive.

```json
{
  "schema_version": 1,
  "foreach": "@game.tracks.f1_2026",
  "ops": [
    { "op": "copy-resource",
      "from_erp": "2025_asset_groups/environment_package/tracks/{track}/wep/{track}_f1_26_myteam.erp",
      "erp": "2025_asset_groups/environment_package/tracks/{track}/wep/{track}_f1_26.erp",
      "resource": "eaid://environment_package/tracks/{track}/wep/{track}-vehicles_current_myteam.layer" }
  ]
}
```

## What the builder guarantees

- A fragment that is not changed keeps its stored bytes, compressed or not.
- A changed fragment is recompressed with zstd if the original was zstd, and stored otherwise.
- Each resource's MD5, the hash of its concatenated decompressed fragments, is recomputed.
- Rebuilding an archive with no operations reproduces it byte for byte.
