---
title: Game database
description: Find a key with gamedb, write a database operation, build it, and read the result — with add-row and track renames.
---

# Game database

To change F1 25's teams, cars, drivers, tracks, rules or unlocks, you'll work with `game_package/gamedatabases/gamedatabases.2025.bin`. It's one flat list of hashed keys, each with a type tag and a text value. You supply [database operations](/reference/ops/database) in `operations.database`; the build applies them to a copy, leaving the file in the game folder alone.

## 1. Find the key

Every value has a dotted path: the row, then the field. Pass that path to `tools/gamedb.py get` to hash it and print the record it names. Point the tool at a copy of the game's database:

```sh
python3 tools/gamedb.py get work/game/game_package/gamedatabases/gamedatabases.2025.bin rules.stop_go_penalty.penalty_duration
```

You'll get the key hash, type name and current value, separated by tabs:

```text
43a617d8995bfd8d	int	5
```

To find a different value, start with the [table catalogue](/reference/gamedb/catalogue), which lists every table. Then look up the table's fields and their types in the [key schema](/reference/gamedb/schema). If you'd rather browse the whole database, run `tools/gamedb.py dump <db> --tree -o db.json` to write it as a tree.

## 2. Change a value

Use `set` to write one field of one row. Here, you're changing the penalty duration:

```json [db/changes.json]
{
  "schema_version": 1,
  "ops": [
    { "op": "set", "select": "rules.stop_go_penalty", "field": "penalty_duration", "value": "10" }
  ]
}
```

Give `select` the row you want to change. Row numbers move between game builds, so choose a label over an index:

| Selector | Row |
|---|---|
| `tracks.Track_3` | row 3 of `tracks` |
| `entrants.teams.team[_metadata_=williams_26]` | the team labelled `williams_26` |
| `{446668bc25ba571c}[_metadata_=f1_26]` | a row in a table known only by its hash |

Write values as strings, whatever the field's type. For a bool, use `True` or `False`; for a list, join the entries with commas and no spaces (`0,-0.6,0`). To copy another key's value, use `value_from` instead of `value`:

```json
{ "op": "set", "select": "{446668bc25ba571c}[_metadata_=f1_26]", "field": "worldEri",
  "value_from": "{446668bc25ba571c}[_metadata_=myteam_26].worldEri" }
```

## 3. Add a row

To add a row, use `add-row`: it copies a stock row, changes some fields and raises the table's row count. You tell it which fields to copy in the file's `fields` map, keyed by table:

```json [db/grid.json]
{
  "schema_version": 1,
  "fields": {
    "entrants.teams.team": ["_metadata_", "stat_id", "display_name_code"]
  },
  "ops": [
    {
      "op": "add-row",
      "table": "entrants.teams",
      "from": "entrants.teams.team[_metadata_=williams_26]",
      "set": { "_metadata_": "team12_26", "stat_id": "500", "display_name_code": "T12" }
    }
  ]
}
```

::: danger List every field
Include every field the row has, even nested ones such as `ai_vehicle_config.*` on vehicles: the game reads them all and crashes if one is missing. Give your new row a new `_metadata_` label. For teams and vehicles, you'll also need a `stat_id` above the stock maximum (team 499, vehicle 508). Pass the table's real path to `add-row`, not a `{hash}`.
:::

You can find the full field sets in [Adding a row](/reference/gamedb/recipes#adding-a-row).

## 4. Rename a track

To rename a track in the menus, you need the name they actually read: the `TrackNameData_0` row of the track's `TrackData` row in each dataset under `track_data.game_setup`. They don't read it from `tracks.Track_N`, so setting `tracks.Track_N.title` alone won't change any menu. Set the name to a language key once per dataset:

```json
{ "op": "set", "select": "track_data.game_setup.TrackDataSet_16.TrackData[_metadata_=melbourne]",
  "field": "TrackNameData_0.title", "value": "lng_my_track_title" }
```

Look up each dataset's row and fields in [Renaming a track](/reference/gamedb/recipes#renaming-a-track). You'll supply the string behind the key through a `lang` operation; see [In-game text](./text).

## 5. Drive values from a setting

To let a setting supply the value, use a `db` [setting](/reference/addons/settings#_3-read-a-db-setting). It fills <span v-pre>`{{id}}`</span> placeholders. You can also use `when` to keep an operation only for the values it lists:

```json
{ "op": "set", "select": "rules.stop_go_penalty", "field": "penalty_duration", "value": "{{penalty_seconds}}" }
```

When you change a `db` setting, the profile is marked for a rebuild. Your change takes effect on the next build or launch.

## 6. Build and check

Build your changes:

```sh
swofty build
```

Check the build output for conflicts. Map activations apply first, followed by each addon's operations in load order. If more than one writes to the same key, the later write wins; the build prints each conflict.

The build rewrites the database in the overlay folder. With the in-game loader present, it also carries the changed records to the game through `swofty/assets.json` while the game boots. See [deliveries](/reference/addons/asset-api#deliveries).

For every operation and field, see the [database operations](/reference/ops/database) reference. To understand the file format and how the game reads it, start with [Game database (GVDB)](/reference/gamedb).
