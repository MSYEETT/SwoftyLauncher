---
title: Database operations
description: Every database operation and field, selectors, the row-count rule and db settings.
---

# Database operations

Database operations change game values in `game_package/gamedatabases/gamedatabases.2025.bin`. They are one kind of [operation](/reference/data/operations); for a walkthrough, see [Game database](/reference/data/database). A native plugin reads the loaded database back with [`swofty/db.hpp`](/reference/std/db), using the same keys.

## File format

List the operation file in `operations.database` in your [manifest](/reference/addons/manifest#operations), or put the same ops in an `operations.assets` file with a `db.` prefix (`db.set`):

```json
{
  "schema_version": 1,
  "ops": [
    { "op": "set", "select": "tracks.Track_3", "field": "numberOfCorners", "value": "16" }
  ]
}
```

Values are strings. Write arrays as comma-separated values, such as `"0,-0.6,0"`.

## Operations

| Operation | Required fields | Purpose |
|---|---|---|
| `set` | `select`, `field`, `value` or `value_from` | Change a field |
| `add-row` | `table`, `from`, `set` | Copy a row, then change its fields |
| `remove-row` | `select` | Remove a row. Not built yet: a build with it fails |
| `list-append` | `select`, `field`, `value` | Add a value to a list if missing. Not built yet |
| `list-remove` | `select`, `field`, `value` | Remove a value from a list. Not built yet |
| `put-record` | `key`, `type`, `value` | Write one raw record. `key` is a path, which can use [selectors](#selecting-rows), or `0x` and 16 hex digits, `type` the record tag and `value` hex bytes |
| `remove-record` | `key` | Remove one record |

For `set`, use `create: true` and a `type` tag to add a missing field. `value_from` copies a value from another database path. A `set` whose `select` matches no row, or whose field doesn't exist, fails the build.

`add-row` copies the fields listed for the table in the file's `fields` map, keyed by table and row name. The key is `table` as written or its resolved path (`track_data.game_setup.TrackDataSet_16.TrackData`). Every listed field must exist on the source row:

```json
{
  "schema_version": 1,
  "fields": { "entrants.teams.team": ["_metadata_", "stat_id", "display_name_code"] },
  "ops": [
    { "op": "add-row", "table": "entrants.teams", "from": "entrants.teams.team[_metadata_=williams_26]",
      "set": { "_metadata_": "team12_26", "stat_id": "500", "display_name_code": "T12" } }
  ]
}
```

List every field of the row, including nested ones such as `ai_vehicle_config.*` on vehicles. A row missing a field the game reads crashes the game.

An `unlockable_se_livery` row added to an `unlockable_content.unlockable_items.unlockable_item_list_<n>.folder_<m>` table, by `add-row` or by `put-record`, unlocks the new special livery. The build writes the row's ownership key, the djb2 hash of `eSELivery` and the row's `_metadata_`, to the `owned_items` block of `swofty/settings.json`, and `swofty_core.dll` reports that key as owned to the livery picker. A key that any stock database value also gives is left out with a warning.

## Selecting rows

A path names a row by its index or finds it by a field. Any segment of the path can be a selector, `Name[field=value]`:

| Path | Meaning |
|---|---|
| `tracks.Track_3` | Row at index 3 |
| `entrants.teams.team[_metadata_=gm_cadillac_26]` | Team with this label |
| `track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=melbourne]` | Melbourne's row in the `f1_2026` dataset |
| `{446668bc25ba571c}[_metadata_=f1_26]` | Row in a table whose full path is unknown |

Selectors resolve left to right, each one beneath the rows already chosen. A selector reads the table's rows up to its `_instancecount_`, or up to the first row without a `_metadata_` when the table has no count, and must match exactly one row. Otherwise the build fails and names the segment:

```text
track_data.game_setup.TrackDataSet[_metadata_=f9]: no row matches
track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=okayama]: no row matches in track_data.game_setup.TrackDataSet_16.TrackData
```

Values are unquoted and can contain `.` and `[`. A value ends at the first `]` that ends the path or comes before a `.`.

Selectors work in `select`, `field`, `value_from`, `add-row`'s `table` and `from`, and the `key` of `put-record` and `remove-record`. A `{hash}` placeholder goes only at the start of a path; selectors can follow it.

Use labels where possible. Row numbers can change between game builds. The dataset rows under `track_data.game_setup` are listed in [A track with its own name](/reference/data/track-names#_3-the-op-file).

## The row-count rule

Adding a row also updates the table's `_instancecount_`. The game ignores rows beyond this count. A new row needs every key a stock row of the table has; see [Adding a row](/reference/gamedb/recipes#adding-a-row).

`add-row` raises the count of the table `table` resolves to. Adding to `track_data.game_setup.TrackDataSet[_metadata_=f1_2026]` raises `track_data.game_setup.TrackDataSet_16._TrackData_instancecount_`.

`add-row` needs the table's real path, written with row numbers or selectors. A `{hash}` placeholder can be used for `set`, but not for `add-row`.

## Settings

A `db` [option](/reference/addons/manifest#options) changes operations at build time:

- <span v-pre>`{{id}}`</span> in any string is replaced with the option's value: `true` or `false`, the number, or the choice value.
- `"when": { "id": value }` keeps the operation only when every listed option has that value.

```json
{ "op": "set", "select": "rules.stop_go_penalty", "field": "penalty_duration", "value": "{{penalty_seconds}}" },
{ "op": "set", "select": "entrants.teams.team[_metadata_=gm_cadillac_26]", "field": "display_name_code", "value": "CAD",
  "when": { "short_names": true } }
```

Changing a `db` option marks the profile for a rebuild. The next launch, or `swofty build`, applies it.

## Applying changes

Track activation runs first, then operations run in addon load order. A later change to a field replaces the earlier value, and the build prints the conflict ([Order and conflicts](/reference/data/operations#order-and-conflicts)). With the in-game loader, the changed records reach the game through `swofty/assets.json` ([Build route and runtime route](/reference/data/operations#build-route-and-runtime-route)).

See [Game database (GVDB)](/reference/gamedb) for the file format, how the game reads it and the table catalogue. [Modding recipes](/reference/gamedb/recipes) covers adding rows, grid size and tracks.
