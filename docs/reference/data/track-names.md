---
title: A track with its own name
description: Ship a community track inside a data addon with track.add, and give it a name of its own with lang.set and db.set on its TrackNameData_0 rows.
---

# A track with its own name

A community track activates with names copied from its base track, or with the stock keys its package sets. To ship a track with a name of its own, put the track in a [data addon](./) and combine three ops in one `assets` file:

- `track.add` adds the track from a folder in your addon;
- `lang.set` adds your strings;
- `db.set` points the track's `TrackNameData_0` rows at your keys.

This page uses the Okayama package (track code `okayama`, base track `melbourne`). Players who only want the track import its zip on the **Mods** page instead ([Tracks](/reference/mods/tracks)).

## 1. Lay out the addon

```
okayama-named/
├── swofty.json
├── assets/
│   └── track.json
└── track/
    ├── manifest.json
    ├── okayama.erp
    ├── mipmaps/
    └── ui/
```

Unpack the track package into `track/`: the zip with `manifest.json` at its root, not a download zip that wraps it. Ship a track only with its author's permission, and credit them.

## 2. The manifest

```json [swofty.json]
{
  "schema_version": 1,
  "id": "yourname.okayama-named",
  "name": "Okayama with its own name",
  "version": "1.0.0",
  "author": "Your Name",
  "compat": { "game": "f1_25" },
  "operations": {
    "assets": ["assets/track.json"]
  }
}
```

## 3. The op file

```json [assets/track.json]
{
  "schema_version": 1,
  "ops": [
    { "op": "track.add", "source": "track" },
    { "op": "lang.set", "language": "*", "key": "lng_okayama_title", "value": "Okayama" },
    { "op": "lang.set", "language": "*", "key": "lng_okayama_gp", "value": "Okayama Grand Prix" },
    { "op": "db.set", "select": "track_data.game_setup.TrackDataSet[_metadata_=f1].TrackData[_metadata_=okayama]",
      "field": "TrackNameData_0.title", "value": "lng_okayama_title" },
    { "op": "db.set", "select": "track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=okayama]",
      "field": "TrackNameData_0.title", "value": "lng_okayama_title" },
    { "op": "db.set", "select": "track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=okayama]",
      "field": "TrackNameData_0.gp_long_name", "value": "lng_okayama_gp" }
  ]
}
```

- `track.add` takes `source`, a folder that holds `manifest.json`. The build activates the track before any database op runs.
- Activation copies the base track's `TrackData` row into every dataset that has the base track and labels the copy with the track code: its `_metadata_`, `track` and `TrackNameData_0.file_name` all become `okayama`. Select the copy with `[_metadata_=okayama]` or `[track=okayama]`.
- `TrackDataSet[_metadata_=f1_2026]` selects a dataset by its label. The datasets and their row numbers are below.
- Your `db.set` ops run after activation, so they win over the package's own `activation.overrides`.
- Add one `db.set` per dataset and field you want named. `TrackNameData_0` holds `title`, `country_name`, `circuit`, `city_name`, `city_name_short`, `gp_long_name`, `gp_long_name_multiline`, `gp_long_name_multiline_previous`, `gp_short_name` and `acronym_name` ([Renaming a track](/reference/gamedb/recipes#renaming-a-track)).

The datasets under `track_data.game_setup` in the shipping database:

| Row | Dataset | Row | Dataset |
|---|---|---|---|
| `TrackDataSet_0` | `f1` | `TrackDataSet_10` | `sprint_f1_2025` |
| `TrackDataSet_1` | `f2` | `TrackDataSet_11` | `f2_2024` |
| `TrackDataSet_2` | `classic` | `TrackDataSet_12` | `multiclass` |
| `TrackDataSet_3` | `short` | `TrackDataSet_13` | `sprint_multiclass` |
| `TrackDataSet_4` | `f1_2023` | `TrackDataSet_14` | `f2_2025_sprint` |
| `TrackDataSet_5` | `sprint_f1_2023` | `TrackDataSet_15` | `f2_2025` |
| `TrackDataSet_6` | `f1_2024` | `TrackDataSet_16` | `f1_2026` |
| `TrackDataSet_7` | `sprint_f1_2024` | `TrackDataSet_17` | `sprint_f1_2026` |
| `TrackDataSet_8` | `f2_2023` | `TrackDataSet_18` | `f2_2026_sprint` |
| `TrackDataSet_9` | `f1_2025` | `TrackDataSet_19` | `f2_2026` |

A dataset without the base track gets no copy, and a `db.set` there fails the build with `no row matches`. Melbourne is in every dataset but `short`.

## 4. Install and build

```sh
swofty addons install ./okayama-named
swofty addons enable yourname.okayama-named
swofty build
```

The output names the datasets that got a copy and counts your edits:

```text
okayama: track data: okayama cloned from melbourne in 19 dataset(s): f1, f2, classic, f1_2023, sprint_f1_2023, f1_2024, sprint_f1_2024, f2_2023, f1_2025, sprint_f1_2025, f2_2024, multiclass, sprint_multiclass, f2_2025_sprint, f2_2025, f1_2026, sprint_f1_2026, f2_2026_sprint, f2_2026
runtime: swofty/assets.json with 77 asset group(s), 0 resource(s), 3 database edit(s), 1 language table(s), 0 overlay file(s)
```

With the in-game loader, your three edits go to the game in `db_edits` and your strings in `swofty/assets/lang/all.lng`.

## 5. Check the rows

To read the edits in the built database, put the database on the build route for one build: add `"build_assets": ["db"]` to `$DATA/config.json` and run `swofty build` again. The copy is the last row of each dataset, so read the row count first:

```sh
DB="$DATA/cache/overlay/<profile>/<buildhash>/game_package/gamedatabases/gamedatabases.2025.bin"
python3 tools/gamedb.py get "$DB" track_data.game_setup.TrackDataSet_16._TrackData_instancecount_
python3 tools/gamedb.py get "$DB" track_data.game_setup.TrackDataSet_16.TrackData_29.TrackNameData_0.title
```

```text
956bcd940ef95eb6	int	30
fae8a9633c226280	string	lng_okayama_title
```

Row 29 is the copy, and its `title` holds your key. Take `"build_assets"` out again afterwards.

## Notes

- Don't also turn the same track on in the **Mods** page: two `track.add` ops with one track code fail the build with `track okayama is added twice`.
- Without the in-game loader, the track takes an invitational slot (at most eight tracks), and the `lang.set` ops wait: the game has no string for your key.
- The EGO GameDatabases Editor adds no text, so the zip alone can't name the track with a new key ([The track's name](/reference/mods/tracks#the-track-s-name)).

## Reference

- [Operations](./operations#asset-op-files): `track.add` and the other prefixed kinds.
- [In-game text](./text) and [Game database](./database): `lang.set` and `db.set`.
- [How tracks are installed](/reference/installing-tracks): everything activation changes.
