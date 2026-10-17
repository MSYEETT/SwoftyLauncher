# Mod manifest

Every mod needs a `swofty.json` file at its root. It describes the mod and lists the changes to apply.

Start with [the mod template](/modding/template).

## Example

```json
{
  "schema_version": 1,
  "id": "author.my-mod",
  "name": "My Mod",
  "version": "1.0.0",
  "author": "Your name",
  "compat": { "game": "f1_25" },
  "operations": {
    "database": ["db/example.json"]
  }
}
```

## Main fields

| Field | Meaning |
|---|---|
| `schema_version` | Use `1` |
| `id` | Unique ID using lowercase letters, digits, `.`, `_` or `-`. The `swofty.` prefix is reserved |
| `name`, `author` | Display name and author |
| `version` | Version such as `1.0.0` |
| `description`, `license`, `homepage` | Optional description, licence identifier and link |
| `kind` | Optional categories: `database`, `erp`, `ui`, `files`, `native` |
| `compat` | Game and version requirements |
| `requires` | Required mods, as `[{"id": "swofty.core", "version": ">=0.1.0"}]` |
| `conflicts` | Conflicting mods, as `[{"id": "author.other-mod"}]` |
| `load_after` | Mod IDs or patterns that should load first |

`compat.game` is `f1_25`. Optional fields are `game_builds` (for example `[">=25251167"]`), `launcher` (a version range) and `seasons` (for example `["f1_2026"]`).

## Options

Each entry in `options` has an `id`, `type`, `default` and `binds`:

- `type`: `bool`, `enum`, `int`, `float` or `string`.
- `label` and `description`: text shown to players.
- `values`: choices for an `enum`.
- `min` and `max`: limits for numbers.
- `binds`: `db` for a setting that needs a rebuild, or `native` for a plugin setting.

## Operations

| Field | Contents |
|---|---|
| `database` | Paths to [database operation files](/modding/database-ops) |
| `erp` | Paths to [ERP operation files](/modding/erp-ops) |
| `ui` | Paths to [UI patches](/modding/ui-patches) |
| `files` | File copies: `[{"source": "files/example", "target": "game/path"}]` |
| `native` | [Plugin DLLs](/modding/native-plugins), with `dll` and `capabilities` |

Source paths are relative to the mod folder and use `/`. Target paths are relative to the game folder.

The installer checks the manifest and the files it names. Some operation types are still in development; see [Current status](/design/#current-status).
