---
title: Track packages
description: Make a community track package in the EGO GameDatabases Editor format, import it on the Mods page, build it, and check it with map_check.py.
---

# Track packages

A community track is a [mod](./): a package zip in the format made for the EGO GameDatabases Editor. The Simplified for Swofty addon imports that zip unchanged into its mod library. To use the track, turn it on in a profile and build: activation clones a base track's database rows, installs your track's archive and adds its menu entries.

## Import and activate

```sh
swofty mods import testring.zip
swofty mods enable testring
swofty build
```

1. `swofty mods import` stores your package under `$DATA/addon-data/teamsimplified.mods/library/<track_code>/track/`. The track code becomes its mod id.
2. `swofty mods enable` turns it on in your profile.
3. `swofty build` activates every enabled track before the other operations run: the addon's build hook emits a `track.add` [operation](/reference/data/operations) for each.

You can also do this from the [Mods page](/using/mods#tracks).

The importer recognises a track package by a `manifest.json` with a `track_code` at the root of its zip. If you give it a download zip that holds exactly one such zip, it unwraps it. The track code becomes the mod's id, or `<code>-track` when a mod in the library already has that id.

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
| `activation.copy_from_track` | Base track to clone `TrackData` rows from in every dataset; without it, `copy_slot` is the base track |
| `activation.copy_slot` | Track to clone the `Track` row and resource row from; defaults to `copy_from_track` |
| `activation.overrides` | Set `path` to `value` on the cloned `TrackData` rows; give `datasets` to limit which datasets you change |
| `activation.track_overrides` | Set `path` to `value` on the cloned `Track` row |
| `ers` | Per-track ERS values written into every 2026 car handling file |
| `copy_directories` | `source` folders in the package and their `destination` in the game |
| `ui.images_file`, `ui.minimap_file` | Menu artwork and minimap files |

`activation` needs `copy_from_track`, `copy_slot` or both, each naming a stock track. A build leaves out a package with neither, or with a `manifest.json` that doesn't read, and builds the rest of the profile. It warns:

```
Portimao (mod.portimao): track Portimao is left out; its manifest.json doesn't say which track it replaces or copies. Import a fixed version, or ask its author for one
```

A race weekend set to a track the build leaves out races at its stock track again, with a warning. Importing or installing such a package shows the same warning, and the Addons page marks its addon **Track left out**.

An override `path` is relative to the row. A segment of it can name a sub-row by number (`TyreData_0`), every sub-row (`TyreData_*`) or the one sub-row whose field matches (`Name[field=value]`, as in [Selecting rows](/reference/ops/database#selecting-rows)). A selector that matches more than one sub-row fails the build.

An override `value` is a string, such as `"1.55"` or `"64,64,64"`. A JSON number is also accepted and written as its text: `1.55` sets `"1.55"`.

Keep every path in the package inside the package. For every table, archive and menu change made during activation, see [How tracks are installed](/reference/installing-tracks).

## The track's name

Menus read `TrackNameData_0.title` and the other `TrackNameData_0` fields on the cloned `TrackData` rows, so they show the base track's names until you override them. A track package carries no text, so point each field at a stock language key with an `activation.overrides` entry:

```json
{ "path": "TrackNameData_0.title", "value": "lng_track_jpn_gp_title_short" }
```

To give the track a name of its own, with new keys and strings, ship it in a data addon with `track.add`, `lang.set` and `db.set` ([A track with its own name](/reference/data/track-names)).

## Using the package elsewhere

The package is the EGO GameDatabases Editor format, so the same zip also installs with the editor. `copy_slot` needs editor 1.6.1 or later. Neither way replaces a stock track: `copy_slot` adds a new track id, and a package with the track code `bahrain24` fills the game's empty `bahrain24` slot.

The editor adds no text, so a new language key such as `lng_my_track_title` shows no name there. For a package that shows a name in both, set `TrackNameData_0.title` to a stock key, such as the base track's own.

## Racing line and other binary XML

The track archive's AI racing line, AI brake settings and splines hold binary XML with string stats beside it ([Binary XML with stats](/reference/ops/erp#binary-xml-with-stats)). When those stats don't match the data, the build writes new stats into the track archive it installs and prints:

```text
testring: fixed the size headers of 1 AI racing line(s) in Test Ring so F1 25 reads them safely
```

When the data starts as binary XML but doesn't parse, the build stops, and Play doesn't start:

```text
Test Ring would crash F1 25: its testring_ai_track.aispline doesn't match the size header the game reads with it, and the launcher can't read the file to fix the header. Turn Test Ring off in this profile, and set any race weekend that uses it back to another track.
```

Export the archive from a tool that writes each resource's stats from its own data, so the package works with other tools too.

F1 25 crashes when an AI car drives a route of the racing line, often the pit lane, on which the passing space never changes from one gate to the next. The build moves one white line on such a route and prints:

```text
testring: fixed the AI pit lane of Test Ring so F1 25 doesn't crash when an AI car drives it
```

A pit lane whose white lines vary along its length, as at the stock tracks, needs no change.

## Check the build

```sh
python3 tools/map_check.py <overlay folder> --game <game folder>
```

Use the folder `swofty build` printed under `$DATA/cache/overlay/<profile>/<buildhash>/` for `<overlay folder>`. `map_check.py` reads that folder's `plan.json` and the track packages in the Team Simplified library. It checks that:

- Every key the game reads on a base row also exists on your track's new row.
- Each rewritten archive and database re-serialises correctly.
- Every ERS setting is present.

See [Checking a build](/reference/installing-tracks#checking-a-build).
