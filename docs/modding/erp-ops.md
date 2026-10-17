# ERP operations

ERP operations describe changes to artwork and other resources in `.erp` archives. List these files in `operations.erp` in your [manifest](/modding/manifest).

The current builder skips ERP operations. This is the format for the planned build step.

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
| `copy-resource` | `from_erp`, `erp`, `resource` | Copy a resource between archives |
| `remove-resource` | `erp`, `resource` | Remove a resource |
| `add-mipmaps` | `source`, `target` | Copy texture files to a game-relative path |

For menu changes, see [UI patches](/modding/ui-patches). For the archive format, see [ERP archives](/reference/erp).
