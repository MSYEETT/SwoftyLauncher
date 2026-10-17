# Addon manifest

Every addon needs a `swofty.json` file at its root. It describes the addon and lists the changes to apply.

Start with [the addon template](/reference/addons/template).

## Example

```json
{
  "schema_version": 1,
  "id": "author.my-addon",
  "name": "My Addon",
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
| `requires` | Required addons, as `[{"id": "swofty.core", "version": ">=0.1.0"}]` |
| `conflicts` | Conflicting addons, as `[{"id": "author.other-addon"}]` |
| `load_after` | Addon IDs or patterns that should load first |
| `options` | [Settings](#options) shown in the launcher and in game |
| `launcher` | A [helper program](./launcher-api#third-party-addons) with launcher pages, an importer, a build hook and commands |

`compat.game` is `f1_25`. Optional fields are `game_builds` (for example `[">=25251167"]`), `launcher` (a version range) and `seasons` (for example `["f1_2026"]`).

## Options

`options` declares the addon's settings. The launcher shows them in the addon's row, and the game shows them on the addon's page in **ADDON SETTINGS**. Values are stored per profile.

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
| `binds` | `native`: the [plugin](/reference/addons/native-plugins#settings) reads the value while the game runs. `db`: the value is built into the game files, so a change needs a rebuild and a restart |

[Database operations](/reference/ops/database#settings) use `db` options through `{{id}}` placeholders and `when` conditions.

## Operations

| Field | Contents |
|---|---|
| `database` | Paths to [database operation files](/reference/ops/database) |
| `erp` | Paths to [ERP operation files](/reference/ops/erp) |
| `ui` | Paths to [UI patches](/reference/ops/ui-patches) |
| `files` | File copies: `[{"source": "files/example", "target": "game/path"}]` |
| `native` | [Plugin DLLs](/reference/addons/native-plugins), with `dll`, `capabilities`, `early` and the `files` the plugin reads in game |
| `screens` | Game screens registered in game by swofty_core: `[{"name": "my_page", "file": "ui/my_page.ui", "flow": "flow_options", "anchor": "settings_container", "view_model": false}]` |
| `assets` | Paths to asset op files: `{"schema_version": 1, "ops": [...]}` with prefixed ops such as `db.set`, `erp.patch-pkg` and `warn` |

Source paths are relative to the addon folder and use `/`. Target paths are relative to the game folder.

A screen's `file` is the screen's raw `.ui` data. The launcher installs it as `swofty/ui/<name>.ui` and lists it in the `ui` block of `swofty/settings.json`. `flow` defaults to `flow_frontend`. `anchor` names a game screen in the same flow; the screen registers after that screen's resource loads. `view_model` asks swofty_core for a native view model. Names are `[a-z0-9_]` and unique across the profile's addons. `swofty_settings`, `swofty_addon_<n>` and `swofty_map_<n>` belong to ADDON SETTINGS.

The installer checks the manifest and the files it names. Some operation types are still in development; see [Current status](/design/#current-status).
