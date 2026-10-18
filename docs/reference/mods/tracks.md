---
title: Tracks
description: Import a community track package, enable it on a profile, build it, and check it with map_check.py.
---

# Tracks

You ship a track as a package zip in the format made for the EGO GameDatabases Editor. The built-in Team Simplified addon imports that zip unchanged into its mod library. To use the track, enable it in a profile and build: activation clones a base track's database rows, installs your track's archive and adds its menu entries.

## Import and activate

```sh
swofty mods import testring.zip
swofty mods enable testring
swofty build
```

1. `swofty mods import` stores your package under `$DATA/addon-data/teamsimplified.mods/library/<track_code>/track/`. The track code becomes its mod id.
2. `swofty mods enable` turns it on in your profile.
3. `swofty build` activates every enabled track before running the other operations.

You can also do this from the [Mods page](/using/mods#tracks).

The importer recognises a track package by the `manifest.json` at the root of its zip. If you give it a zip containing exactly one such zip, it unwraps it. It refuses 7z and RAR.

## Package layout

| File | Purpose |
|---|---|
| `manifest.json` | Track code, name and activation, at the root of the zip |
| `<code>.erp` | The track archive |
| `mipmaps/` | Optional textures |
| `ui/images.json` | Track artwork |
| `ui/minimap.json` | Minimap placement |

## manifest.json

```json [manifest.json]
{
  "track_code": "testring",
  "track_name": "Test Ring",
  "version": "0.1.0",
  "author_name": "tester",
  "erp": "testring.erp",
  "activation": {
    "copy_from_track": "melbourne",
    "copy_slot": "melbourne",
    "overrides": [{ "datasets": ["f1_2025"], "path": "TyreData_0.compound_harder", "value": "C3" }],
    "track_overrides": [{ "path": "allow_night_start", "value": "False" }]
  },
  "ers": { "HotLapDeployFactor": 0.7 },
  "copy_directories": [
    { "source": "mipmaps/", "destination": "asset_groups/environment_package/tracks/testring/mipmaps/" }
  ],
  "ui": { "schema_version": 1, "images_file": "ui/images.json", "minimap_file": "ui/minimap.json" }
}
```

| Field | Meaning |
|---|---|
| `track_code` | Names your track everywhere; use letters, digits, `_` and `-` |
| `track_name`, `version`, `author_name` | Optional display fields |
| `erp` | Path to your track archive, relative to the package |
| `activation.copy_from_track` | Base track to clone `TrackData` rows from in every dataset |
| `activation.copy_slot` | Track to clone the `Track` row and resource row from |
| `activation.overrides` | Set `path` to `value` on the cloned `TrackData` rows; give `datasets` to limit which datasets you change |
| `activation.track_overrides` | Set `path` to `value` on the cloned `Track` row |
| `ers` | Per-track ERS values written into every 2026 car handling file |
| `copy_directories` | `source` folders in the package and their `destination` in the game |
| `ui.images_file`, `ui.minimap_file` | Menu artwork and minimap files |

Keep every path in the package inside the package. For every table, archive and menu change made during activation, see [How tracks are installed](/reference/maps).

## The track's name

Menus read `TrackNameData_0.title` on the cloned `TrackData` rows, so they'll show the base track's name until you override it:

1. Add an `activation.overrides` entry with a language key as its `value`:

```json
{ "path": "TrackNameData_0.title", "value": "lng_my_track_title" }
```

2. Give that key a string with a `lang` operation — see [In-game text](./text).

You're setting the same field a database mod uses in [Rename a track](./database#_4-rename-a-track).

## Check the build

```sh
python3 tools/map_check.py <overlay folder> --game <game folder>
```

Use the folder `swofty build` printed under `$DATA/cache/overlay/<profile>/<buildhash>/` for `<overlay folder>`. `map_check.py` reads that folder's `plan.json` and the track packages in the Team Simplified library. It checks that:

- Every key the game reads on a base row also exists on your track's new row.
- Each rewritten archive and database re-serialises correctly.
- Every ERS setting is present.

See [Checking a build](/reference/maps#checking-a-build).
