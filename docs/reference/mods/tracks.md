---
title: Importing and packaging a track
description: The track package format the launcher imports, its manifest.json fields, and how a package becomes a playable track.
---

# Importing and packaging a track

A track is a package zip in the format made for the EGO GameDatabases Editor. The Team Simplified addon imports it unchanged as a mod, stores it in `$DATA/addon-data/teamsimplified.mods/library/<track_code>/track/`, and the build installs it when a profile enables it. Players import packages as described in [Mods and tracks](/using/mods#tracks).

## Package layout

| File | Purpose |
|---|---|
| `manifest.json` | Track code, name and setup |
| `<code>.erp` | Track data |
| `mipmaps/` | Optional textures |
| `ui/images.json` | Track artwork |
| `ui/minimap.json` | Minimap placement |

`manifest.json` must be at the root of the zip. A zip that holds exactly one inner zip with `manifest.json` at its root is unwrapped.

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
| `track_code` | Letters, digits, `_` and `-`. Names the track everywhere |
| `track_name`, `version`, `author_name` | Optional display fields |
| `erp` | The track archive, relative to the package |
| `activation.copy_from_track` | The base track whose `TrackData` rows are cloned in every dataset |
| `activation.copy_slot` | The track whose `Track` row and resource row are cloned |
| `activation.overrides` | `path` and `value` set on the cloned `TrackData` rows, limited to `datasets` when given |
| `activation.track_overrides` | `path` and `value` set on the cloned `Track` row |
| `ers` | Per-track ERS values written into every 2026 car handling file |
| `copy_directories` | `source` folders in the package and their `destination` in the game |
| `ui.images_file`, `ui.minimap_file` | Menu artwork and minimap files |

Every path in the package must stay inside it. [How maps are installed](/reference/maps) lists every table, archive and menu change activation makes.

## Track names

Menus read the track's name from `TrackDataSet_<n>.TrackData_<m>.TrackNameData_0`, cloned from the base track. An `activation.overrides` entry with the path `TrackNameData_0.title` and a language key as its value sets it. Without one, menus show the base track's name. See [Rename a track](./database#rename-a-track).

## Check a package

```sh
swofty mods import testring.zip
swofty mods enable testring
swofty build
python3 tools/map_check.py <overlay folder> --game <game folder>
```

`map_check.py` checks that every key the game reads on a source row exists on the track's row. It reads the track packages from the Team Simplified library. See [Checking a build](/reference/maps#checking-a-build).
