# Manifest

Every mod has a `swofty.json` at its root. A mod is a directory, or a `.swoftymod` zip of that directory:

```
<mod-id>/
  swofty.json          manifest
  db/*.json            database operation files
  erp/...              ERP operation files and the payloads they reference
  ui/*.pkgpatch.json   UI patches
  files/...            loose files
  native/*.dll         native plugins
  icon.png, README.md
```

## Example

```json
{
  "schema_version": 1,
  "id": "swofty.f1-2026-grid24",
  "name": "F1 2026: 24-Car Grid",
  "version": "1.0.0",
  "author": "swofty",
  "description": "A twelfth team on the 2026 grid.",
  "license": "MIT",
  "kind": ["database"],

  "compat": {
    "game": "f1_25",
    "game_builds": [">=25251167"],
    "launcher": ">=0.1.0",
    "seasons": ["f1_2026"]
  },

  "requires": [{ "id": "swofty.core", "version": ">=0.1.0" }],
  "conflicts": [{ "id": "*", "touches": "db:entrants.entrant_lists.entrant_list_34.*" }],
  "load_after": ["swofty.track.*"],

  "options": [
    {
      "id": "team_livery",
      "type": "enum",
      "label": "Team livery",
      "default": "titlesponsor",
      "values": ["titlesponsor", "tier01", "replica"],
      "binds": "db"
    }
  ],

  "operations": {
    "database": ["db/grid24.json"],
    "erp": [],
    "ui": [],
    "files": [],
    "native": []
  }
}
```

## Fields

### Identity

| Field | Type | Required | Meaning |
|---|---|---|---|
| `schema_version` | integer | yes | `1` |
| `id` | string | yes | Lowercase, dotted, characters `[a-z0-9._-]`, for example `author.mod-name`. The `swofty.` prefix is reserved for the launcher's own mods |
| `name` | string | yes | Display name |
| `version` | string | yes | Semantic version, `1.2.0` |
| `author` | string | yes | |
| `description` | string | no | One or two sentences for the library |
| `license` | string | no | SPDX identifier |
| `homepage` | string | no | URL |
| `kind` | array | no | Any of `database`, `erp`, `ui`, `files`, `native`. A filter for the library; `operations` is authoritative |

### `compat`

| Field | Type | Meaning |
|---|---|---|
| `game` | string | `f1_25` |
| `game_builds` | array of ranges | Ranges over the Steam `buildid`, such as `">=25251167"`. Absent means any build |
| `launcher` | semver range | The launcher versions the mod works with |
| `seasons` | array | Season content the mod needs, such as `f1_2026` |

A mod whose `game_builds` excludes the installed build is flagged and left out of builds until the user overrides it.

### Dependencies and order

| Field | Shape | Meaning |
|---|---|---|
| `requires` | `[{id, version}]` | Mods that must be enabled, with a semver range |
| `conflicts` | `[{id, touches?}]` | Declared conflicts. `id` may be `*`. `touches` narrows it to a target, such as `db:<path glob>` |
| `load_after` | `[glob]` | Mod ids this mod loads after when both are enabled |

The resolver orders mods by `requires` and `load_after`, then by the profile's order. A cycle fails the build.

### `options`

| Field | Type | Meaning |
|---|---|---|
| `id` | string | Unique within the mod |
| `type` | string | `bool`, `enum`, `int`, `float` or `string` |
| `label`, `description` | string | Shown in the launcher and in game |
| `default` | value | The value when the profile sets none |
| `values` | array | The choices, for `enum` |
| `min`, `max` | number | Bounds, for `int` and `float` |
| `binds` | string | `db`: the value feeds the build, and changing it needs a rebuild and restart. `native`: a plugin reads it live |

### `operations`

| Field | Shape | Meaning |
|---|---|---|
| `database` | `[path]` | [Database operation files](/modding/database-ops), applied in order |
| `erp` | `[path]` | [ERP operation files](/modding/erp-ops) |
| `ui` | `[path]` | [UI patches](/modding/ui-patches), `*.pkgpatch.json` |
| `files` | `[{source, target}]` | Loose files. `source` is inside the mod, `target` is relative to the game folder |
| `native` | `[{dll, capabilities}]` | [Native plugins](/modding/native-plugins). `capabilities` is any of `log`, `scan`, `hook`, `read-db`, `options`, `on-frame` |

Paths are relative to the mod root and use `/`.

## Validation

`swofty mods install` and the launcher check the manifest when a mod is installed: the schema version, the id format, a parseable version and ranges, unique option ids, enum defaults that are among `values`, and that every file named in `operations` exists.
