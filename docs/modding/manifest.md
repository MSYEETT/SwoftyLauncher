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
| `options` | [Settings](#options) shown in the launcher and in game |

`compat.game` is `f1_25`. Optional fields are `game_builds` (for example `[">=25251167"]`), `launcher` (a version range) and `seasons` (for example `["f1_2026"]`).

## Options

`options` declares the mod's settings. The launcher shows them in the mod's row, and the game shows them in the mod's section of **MOD SETTINGS**. Values are stored per profile.

```json
"options": [
  { "id": "show_panel", "type": "toggle", "label": "Show panel", "default": true, "binds": "native" },
  { "id": "ai_offset", "type": "slider", "label": "AI offset", "min": -10, "max": 10, "step": 1,
    "default": 0, "binds": "native" },
  { "id": "team_livery", "type": "choice", "label": "Team livery", "default": "titlesponsor",
    "values": ["titlesponsor", { "value": "tier01", "label": "Tier 1 sponsor" }, "replica"], "binds": "db" }
]
```

| Field | Meaning |
|---|---|
| `id` | Lowercase letters, digits and `_`, starting with a letter, at most 40 characters |
| `type` | `toggle`, `slider` or `choice` |
| `label`, `description` | Text shown to players; `label` defaults to the `id` |
| `default` | `true` or `false` for a toggle, a number on the slider's steps, or one of the choice values |
| `min`, `max`, `step` | Slider range. All three are required, `max - min` must be a whole number of steps, and a slider has at most 100 steps. Whole-number ranges hold whole numbers |
| `values` | Choice values, 1 to 16. Each is a string, or `{ "value", "label" }` to show a different name |
| `binds` | `native`: the [plugin](/modding/native-plugins#settings) reads the value while the game runs. `db`: the value is built into the game files, so a change needs a rebuild and a restart |

[Database operations](/modding/database-ops#settings) use `db` options through `{{id}}` placeholders and `when` conditions.

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
