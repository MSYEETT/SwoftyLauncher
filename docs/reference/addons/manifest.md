---
title: Addon manifest
description: Every swofty.json field, with types and real examples for launcher, options, compat, requires and operations.
---

# Addon manifest

Your addon needs a `swofty.json` at its root. This is where you give it a name, declare its settings and list what it ships. Set `schema_version` to `1` and stick to the fields below: unknown fields are rejected.

```json [swofty.json]
{
  "schema_version": 1,
  "id": "author.my-addon",
  "name": "My Addon",
  "version": "1.0.0",
  "author": "Your name",
  "compat": { "game": "f1_25" },
  "operations": { "database": ["db/example.json"] }
}
```

## Identity

| Field | Type | Value |
|---|---|---|
| `schema_version` | integer | `1`. |
| `id` | string | Choose a lowercase dotted id with at least two parts, each part `[a-z0-9_-]`, such as `author.my-addon`. The `swofty.` prefix is reserved. |
| `name` | string | Display name. |
| `version` | string | Semver, such as `1.0.0`. |
| `author` | string | Display author. |
| `description` | string? | One-line summary, shown on the Addons page. |
| `license` | string? | SPDX identifier, such as `MIT`. |
| `homepage` | string? | A URL. |
| `icon` | string? | A path inside your addon, shown on the Addons page and your addon's sidebar pages. Use an `.svg` for a one-colour glyph in the launcher's icon style, or a square `.png`, `.jpg` or `.webp` to show the image as it is. |
| `kind` | string[] | Optional categories: `database`, `erp`, `ui`, `files`, `native`. |

## compat

```json
"compat": {
  "game": "f1_25",
  "game_builds": [">=25251167"],
  "launcher": ">=0.1.0",
  "seasons": ["f1_2026"]
}
```

| Field | Type | Value |
|---|---|---|
| `game` | string | `f1_25`. Any other value is rejected. |
| `game_builds` | string[] | Semver ranges matched against the build id as `<id>.0.0`. Leave empty to accept every build. |
| `launcher` | string? | A semver range the launcher must satisfy. |
| `seasons` | string[] | Season tags, such as `f1_2026`. |

## requires, conflicts, load_after

```json
"requires": [{ "id": "swofty.core", "version": ">=0.1.0" }],
"conflicts": [{ "id": "*", "touches": "db:entrants.entrant_lists.entrant_list_34.*" }],
"load_after": ["swofty.track.*"]
```

| Field | Type | Value |
|---|---|---|
| `requires` | `[{ id, version }]` | List the addons that must be installed. `version` is a semver range and defaults to `*`. |
| `conflicts` | `[{ id, touches? }]` | List the addons that must not be enabled together. Use `touches` to narrow the conflict to a shared target; `id` may be `*`. |
| `load_after` | string[] | Addon ids or `*` patterns to build before your addon. |

## options

To give players settings they can change, declare them in `options`. They'll appear in your addon's row on the launcher's **Addons** page and on its page in **ADDON SETTINGS** in game. Values are stored per profile. Follow [Settings](./settings) for the full procedure.

```json
"options": [
  { "id": "show_panel", "type": "toggle", "label": "Show panel", "default": true, "binds": "native" },
  { "id": "ai_offset", "type": "slider", "label": "AI offset", "min": -10, "max": 10, "step": 1,
    "default": 0, "binds": "native" },
  { "id": "team_livery", "type": "choice", "label": "Team livery", "default": "titlesponsor",
    "values": ["titlesponsor", { "value": "tier01", "label": "Tier 1 sponsor" }, "replica"], "binds": "db" }
]
```

| Field | Type | Value |
|---|---|---|
| `id` | string | Use `[a-z][a-z0-9_]*`, at most 40 characters, and keep it unique within your addon. |
| `type` | string | `toggle`, `slider` or `choice`. |
| `label` | string? | Shown to players. Defaults to `id`. |
| `description` | string? | Shown under the label. |
| `default` | any | Use `true`/`false` for a toggle, a number on the slider's grid, or one of the choice values. |
| `min`, `max`, `step` | number | Set all three to define a slider's range. `max - min` must be a whole number of steps, at most 100. |
| `values` | array | Supply 1 to 16 values for a choice. Each is a string; use `{ "value", "label" }` to show a different name. |
| `binds` | string | `native` — the [engine plugin](./native-plugins#settings) reads the value while the game runs; `db` — the value is built into the game files, so a change needs a rebuild. |

## operations

Put your data edits in `operations`: the build applies them to the game's files, just as it does for a [mod](/reference/mods/). Write source paths relative to your addon folder, using `/`, and target paths relative to the game folder.

```json
"operations": {
  "database": ["db/grid24.json"],
  "erp": ["erp/livery.json"],
  "ui": ["ui/tooltip.pkgpatch.json"],
  "files": [{ "source": "files/sponsor.dds", "target": "livery_package_shared/sponsor.dds" }],
  "native": [{ "dll": "native/plugin.dll", "early": false, "files": ["lang/"] }],
  "screens": [{ "name": "grid_page", "file": "ui/grid_page.ui", "flow": "flow_options", "anchor": "settings_container", "view_model": false }],
  "assets": ["assets/ops.json"]
}
```

| Field | Type | Contents |
|---|---|---|
| `database` | string[] | [Database operation files](/reference/ops/database). |
| `erp` | string[] | [ERP operation files](/reference/ops/erp). |
| `ui` | string[] | [UI patches](/reference/ops/ui-patches). |
| `files` | `[{ source, target }]` | Verbatim file copies into the game folder. |
| `native` | `[NativePlugin]` | [Engine plugins](./native-plugins). |
| `screens` | `[ScreenOp]` | Screens to register in game. |
| `assets` | string[] | [Asset op files](./asset-api#op-files) — `{ "schema_version": 1, "ops": [...] }` with prefixed ops such as `db.set`, `erp.patch-pkg` and `warn`. |

A **NativePlugin** is `{ dll, early, files }`. Give it the DLL path, use `early` to load it before the menu, and list the `files` it reads in game.

A **ScreenOp** is `{ name, file, flow, anchor?, view_model }`. Choose `name` and `flow` values that use `[a-z0-9_]` and are unique across the profile's addons. Supply raw `.ui` data in `file`; it gets installed as `swofty/ui/<name>.ui`.

The `flow` defaults to `flow_frontend`. To register after another screen in the same flow, name that screen in `anchor`. Use `view_model` to ask core for a native view model. The names `swofty_settings`, `swofty_addon_<n>` and `swofty_map_<n>` belong to ADDON SETTINGS.

## launcher

To add sidebar pages, an importer, a build hook and commands, name a [launcher plugin](./launcher-api) in the `launcher` block. You supply a prebuilt program; the launcher runs it as a subprocess.

```json
"launcher": {
  "program": { "windows": "bin/example-helper.exe", "linux": "bin/example-helper" },
  "sections": [{ "id": "items", "title": "Example", "icon": "package" }],
  "imports": ["exmod"],
  "build": true,
  "commands": [{ "name": "example", "about": "List the example items" }]
}
```

| Field | Type | Value |
|---|---|---|
| `program` | `{ os: path }` | Give the plugin's path inside your addon for each OS. Keys are `linux`, `windows`, `macos`; supply at least one. |
| `sections` | `[{ id, title, icon }]` | Each entry adds a sidebar page. Choose an `icon` from the launcher's [icon set](./launcher-api#views). |
| `imports` | string[] | List file extensions without the dot. When someone drops a file with one of these extensions, the launcher routes it to your plugin. |
| `build` | boolean | Set to `true` when your plugin answers the `build` call. Defaults to `false`. |
| `commands` | `[CommandSpec]` | `swofty` subcommands — `{ name, about, args, subcommands }`. |

## Paths and validation

When you install the addon, the installer checks your manifest and the files it names:

- Keep every path relative and inside the addon — no leading `/`, no `\`, no `..`.
- Use a lowercase dotted `id`; keep option ids `[a-z][a-z0-9_]*` and unique, and screen names `[a-z0-9_]` and unique.
- Name at least one program in `launcher.program`; each `imports` entry must be non-empty and have no dot.

Some operation types are still in development. Check [Current status](/design/#current-status) and use [the template](./template) as your starting point.
