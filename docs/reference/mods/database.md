---
title: Editing the game database
description: Change values, copy rows and rename tracks in gamedatabases.2025.bin with database operations.
---

# Editing the game database

F1 25 keeps its teams, cars, drivers, tracks and rules in `game_package/gamedatabases/gamedatabases.2025.bin`. An addon changes it with [database operations](/reference/ops/database) listed in `operations.database`. The launcher applies them to a copy of the stock file when it builds the profile.

## Find the key

Every value has a path such as `rules.stop_go_penalty.penalty_duration`: the row, then the field. The [table catalogue](/reference/gamedb/catalogue) lists the tables, and the [key schema](/reference/gamedb/schema) lists each table's fields with their types.

## Change a value

```json [db/example.json]
{
  "schema_version": 1,
  "ops": [
    {
      "op": "set",
      "select": "entrants.entrant_setup_modes.entrant_setup_mode[_metadata_=f1_26]",
      "field": "max_entrants",
      "value": "22"
    }
  ]
}
```

`select` finds the row:

| Selector | Row |
|---|---|
| `tracks.Track_3` | Row 3 of `tracks` |
| `entrants.teams.team[_metadata_=williams_26]` | The team labelled `williams_26` |
| `{446668bc25ba571c}[_metadata_=f1_26]` | A row in a table known only by its hash |

Select by label: row numbers can change between game builds. Write values in the stock form: `True` or `False` for bools, comma-joined lists with no spaces.

## Copy a value

`value_from` copies another key's value. Custom Grid Size points the F1 2026 world at the My Team world:

```json
{ "op": "set", "when": { "cars": 24 },
  "select": "{446668bc25ba571c}[_metadata_=f1_26]", "field": "worldEri",
  "value_from": "{446668bc25ba571c}[_metadata_=myteam_26].worldEri" }
```

## Add a row

`add-row` copies a stock row, changes some of its fields and raises the table's row count. The file's `fields` map lists every field to copy, by table:

```json [db/grid.json]
{
  "schema_version": 1,
  "fields": {
    "entrants.entrant_lists.entrant_list_34.entrant": [
      "_metadata_", "badge_customisation", "car_number", "driver", "driver_customisation",
      "entrant_reference", "team", "team_position", "vehicle", "vehicle_customisation"
    ]
  },
  "ops": [
    {
      "op": "add-row", "when": { "cars": 24 },
      "table": "entrants.entrant_lists.entrant_list_34",
      "from": "entrants.entrant_lists.entrant_list_34.entrant[_metadata_=gm_cadillac_01]",
      "set": {
        "_metadata_": "team12_driver_01", "entrant_reference": "jack_tremblay", "car_number": "66",
        "team": "team12_26", "vehicle": "team12_2026", "vehicle_customisation": "jack_tremblay_2026"
      }
    }
  ]
}
```

::: danger Every field
List every field of the row, including nested ones such as `ai_vehicle_config.*` on vehicles. The game reads them all and crashes on a missing one. `addons/custom-grid-size/db/grid.json` lists every field of team, vehicle and entrant rows.
:::

A new row needs a new `_metadata_` label, and a new `stat_id` above the stock maximum for teams (499) and vehicles (508). `add-row` needs the table's real path, not a `{hash}`. See [Adding a row](/reference/gamedb/recipes#adding-a-row).

## Rename a track

The menus read a track's names from the `TrackNameData_0` row of its `TrackData` row in each dataset under `track_data.game_setup`, not from `tracks.Track_N`. An edit to `tracks.Track_N.title` alone changes no menu. Set the name once per dataset:

```json
{ "op": "set", "select": "track_data.game_setup.TrackDataSet_16.TrackData[_metadata_=melbourne]",
  "field": "TrackNameData_0.title", "value": "lng_my_track_title" }
```

The values are language keys. See [Renaming a track](/reference/gamedb/recipes#renaming-a-track) for each dataset's row and fields.

## Drive values from settings

A `db` [setting](/reference/addons/settings#use-a-db-setting) fills <span v-pre>`{{id}}`</span> placeholders, and `when` keeps an operation only for the listed values:

```json
{ "op": "set", "select": "rules.stop_go_penalty", "field": "penalty_duration", "value": "{{penalty_seconds}}" }
```

## Order

The launcher applies map activations first, then each addon's operations in load order. A later write to the same key wins. Check a build with `swofty build` before starting the game.

Every operation and field is in the [database operations](/reference/ops/database) reference. The [Game database](/reference/gamedb) section covers the file format and how the game reads it.
