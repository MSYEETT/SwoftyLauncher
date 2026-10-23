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
| `add-row` | `table`, `from`, `set`; optional `copy`, `stat_id` | Copy a row with every key, then change its fields |
| `remove-row` | `select` | Remove a row. Not built yet: a build with it fails |
| `list-append` | `select`, `field`, `value` | Add each comma-separated value to a list field if missing |
| `list-remove` | `select`, `field`, `value` | Remove each comma-separated value from a list field |
| `put-record` | `key`, `type`, `value` | Write one raw record. `key` is a path, which can use [selectors](#selecting-rows), or `0x` and 16 hex digits, `type` the record tag and `value` hex bytes |
| `remove-record` | `key`; optional `allow_stock_removal` | Remove one record |

For `set`, use `create: true` and a `type` tag to add a missing field. `value_from` copies a value from another database path. A `set` whose `select` matches no row, or whose field doesn't exist, fails the build. A `remove-record` whose key is already absent is a no-op, logged.

`list-append` and `list-remove` treat the field as a comma-separated list and keep its order. Several addons can append to the same list without a conflict. A missing field fails the build.

### Removing stock records

A `remove-record` of a key in the game's stock database is skipped. The build warns once per addon with the count, such as `My Mod: 12 removal(s) of stock database records skipped; the game crashes when a record it reads is missing`. The addon's row on the **Addons** page shows the count after a build or a conflict check. Keys that addons add, earlier in the same addon or in another one, are removed as before. Set `"allow_stock_removal": true` on the op to remove a stock record:

```json
{ "op": "remove-record", "key": "tracks.Track_0.PoleOnRight", "allow_stock_removal": true }
```

The game crashes on any key it reads that is missing; see [Missing keys](/reference/gamedb/reading#missing-keys). A build that removes a stock record writes the database into `gamedatabases.2025.bin` instead of editing it at runtime. **Export game files (.zip)** in the DB Editor skips the same removals and lists them.

`add-row` copies every key of the source row: its fields, its sub-object keys such as `ai_vehicle_config.*` on vehicles, and its nested tables with their `_instancecount_` keys and rows, at any depth. `set` then changes keys of the copy. Every `set` key must be a key the copy has:

```json
{
  "schema_version": 1,
  "ops": [
    { "op": "add-row", "table": "entrants.teams", "from": "entrants.teams.team[_metadata_=williams_26]",
      "set": { "_metadata_": "team12_26", "display_name_code": "T12" } }
  ]
}
```

| Field | Values | Default |
|---|---|---|
| `copy` | `all`: every key of the source row and every key in the file's `fields` list. `listed`: only the keys in the `fields` list | `all` |
| `stat_id` | `fresh`: the next free id above the table's highest `stat_id`. `keep`: the source row's id | `fresh` in a top-level table whose rows all have different ids, `keep` elsewhere |

### Key names

The database stores a hash of each key's path, not the path, so the build can't list a row's keys. It probes the source row with the launcher's key catalogue, `row_keys.txt.gz`: every key path found under each row shape of the stock database, generated from the identifier strings in `F1_25.exe`, `tools/gamedb/names.txt`, the key catalogue and the run-time compositions in `tools/gamedb/compositions.txt`, such as `player.driver.preloaded` on entrant setup modes. A row shape is a row path with its row numbers written `_N`: `entrants.entrant_setup_modes.entrant_setup_mode_N`.

The file's `fields` map, keyed by `table` as written or its resolved path and the row name, adds names the catalogue lacks. Every listed field must exist on the source row:

```json
"fields": { "entrants.teams.team": ["_metadata_", "stat_id", "display_name_code"] }
```

`copy: listed` keeps the behaviour of launcher 1.2.1 and earlier: the copy has only the listed fields, and the `fields` list is required.

### The completeness check

The build prints how many keys each `add-row` copied, and checks the copy against the catalogue:

| Finding | Default | `copy` |
|---|---|---|
| A key of the source row that the `fields` list doesn't name | Line: `copied 9 key(s) the fields list for … doesn't name: …` | `all` |
| A key of the source row that the `fields` list doesn't name, so the copy lacks it | Warning: `the fields list for … misses 9 key(s) of the source row, not copied: …` | `listed` |
| A row shape, of the row or a nested row, that has no catalogue entry | Warning: `the key catalogue has no entry for …` | both |

Rows past a row count that `set` lowers don't count as missing. With `"strict": true` at the top of the op file, each warning fails the build.

The build can't count the keys it can't name. An FNV-1a key gives no sign of the path it hashes, and the file holds records in hash order, so a key under the source row and a key elsewhere look alike. A key that only the game's code builds at run time, from names the exe holds in pieces, is missing from the catalogue until `tools/gamedb/compositions.txt` lists it. Regenerate the catalogue after changing that file:

```sh
cargo run --release -p swofty-core --example row_keys -- --game <F1 25 folder>
```

`cargo run -p swofty-core --example check_add_rows -- --game <F1 25 folder> --addon <addon folder>` applies an addon's database files to the stock database and prints the check. `--listed` checks the files as launcher 1.2.1 copied them; `--strict` stops at the first finding.

### Stat ids

Tables indexed by `stat_id` find a row by its id, so a copy that keeps its source's id hides the source or another copy. Every stock top-level table with a `stat_id` on each row gives each row its own id, except `f1_world.map.markers.TrackMapMarker_N`, `schedule.events.fixed_event_N` and `track_data.game_setup.TrackDataSet_N`. Among them: `entrants.entrant_lists.entrant_list_N`, `entrants.entrant_setup_modes.entrant_setup_mode_N`, `entrants.vehicles.vehicle_packages.vehicle_package_N`, `entrants.teams.team_N`, `entrants.drivers.driver_N`, `entrants.vehicles.vehicle_N` and `game_setup.game_setup_modes.game_setup_mode_N`.

A copy into such a table gets the next free id above the table's highest, and the build prints it:

```text
f1-1990: add-row entrants.entrant_lists.entrant_list_68 (from entrants.entrant_lists.entrant_list[_metadata_=f1_2026]): stat_id 105 (the source's is 67)
```

A `stat_id` in `set` wins. A copy into a nested table, such as an entrant of an entrant list, keeps its source's id; the build prints a line when the nested table's ids were all different.

An `unlockable_se_livery` row added to an `unlockable_content.unlockable_items.unlockable_item_list_<n>.folder_<m>` table, by `add-row` or by `put-record`, unlocks the new special livery. The build writes the row's ownership key, the djb2 hash of `eSELivery` and the row's `_metadata_`, to the `owned_items` block of `swofty/settings.json`, and `swofty_core.dll` reports that key as owned to the livery picker. A key that any stock database value also gives is left out with a warning.

## What a copy does not bring along

`add-row` copies the source row's keys. The game also finds data for a row by its label, by a name built from the label or from a field, or by its `stat_id`, in other tables and in game files. A copy under a new label gets none of that data. The game then reads stock defaults, a missing row or a missing resource for it, often without an error: the car runs, but on the wrong setup.

| What the copy lacks | How the game finds it | Known case |
|---|---|---|
| Rows of another table labelled with the source's label | Its own label | Teams: `showroom.teams_history.team_history`, `f1_world.fan_zone.fz_choices_stings.FZChoicesStings` (`williams_26`). Vehicles: `animation.team_data.teamData`, `audio.audio_bundle_settings.audio_bundle_settings` (`mercedes_2025`). Game setup modes: `game_setup.branding.game_setup_branding` (`f1`) |
| Rows labelled with a name built from the source | A folder name, the label plus a suffix, a field value | Vehicle packages: `car_setup.presets.vehicle_package_setups_N`, labelled with the package folder `f1_2025_vehicle_package`, the first folder of `resource_path` |
| Values elsewhere that name the source | Other rows hold the source's label or `stat_id` | Vehicles: `entrants.entrant_lists.entrant_list_N.entrant_N.vehicle`, `showroom.vehicle_offsets.vehicle_offset_N.vehicle`, `showroom.vehicles_history.vehicle_history_N.vehicle`. Vehicle packages: `entrants.vehicles.vehicle_N.vehicle_package` |
| Game files named after the source | Paths built from the label or a field | Vehicle packages: the car template's `CarPhysics2.baseVtfAsset` and `Tyres` defaults name `eaid://f1_2025_vehicle_package/...`. Vehicles: per-track AI data at `environment_package/tracks/<track>/ai/<ai_package_folder>/<asset_filename>/` |
| Keys of the source the catalogue does not name | Paths the exe builds at run time | Car setup presets: 420 keys per row. Game setup modes: 11 keys under `championship_won_cutscene` and `tyre_regulations`, which the `swofty_f1` copy lacked until `addons/swoftymod/db/core.json` listed them in a `fields` list. Both are in the catalogue from launcher 1.3.8 |

### Car setups of a vehicle package

On session start the game finds a vehicle package's car setups with the row lookup `car_setup.presets.vehicle_package_setups[_metadata_=<package folder>]` (reader `0x140c04fd0`) and reads `<row>.<ai|player>.<variant>.<slider>`:

- variants: `max_downforce`, `med_downforce`, `balanced`, `med_speed`, `max_speed`;
- sliders: the 42 labels of `car_setup.sliders.slider_N`, such as `ballast`, `brake_bias`, `front_suspension_height` and `rear_suspension_height`.

Each stock row holds these 420 keys and its `_metadata_`. A copy of `f1_2025` with the folder `cs_2019_vehicle_package` needs a preset row of its own:

```json
{ "op": "add-row", "table": "car_setup.presets",
  "from": "car_setup.presets.vehicle_package_setups[_metadata_=f1_2025_vehicle_package]",
  "set": { "_metadata_": "cs_2019_vehicle_package" } }
```

| Preset row for the copy | In game |
|---|---|
| None | The AI drives setups that ground the car: plank contacts, sparks, running wide |
| `_metadata_` only | Loading the session crashes (a read at index -1) |
| All 421 keys | The copy drives its source's setups |

Launchers before 1.3.8 copy only the catalogued `_metadata_`; give the op a `fields` list of the 421 keys for them.

### The side-data check

After every addon's operations ran, the build checks each `add-row` whose copy has a new label, against the built database:

1. It lists the names the game can derive from the source row: the label, the label plus the table's row word (`f1_2025_vehicle_package`), identifier values of the row's fields (`asset_filename`, `ai_package_folder`), and the folders of path values that hold the label or a digit (`resource_path`). It keeps the names the copy changed.
2. For each such name, it finds rows labelled with it in other tables, and warns for each table that has no row labelled with the copy's name.
3. It finds values elsewhere that name the source by its label, or by its `stat_id` in an id field named after the source's table (`team_id`), and warns for each field that never names the copy. When rows of other tables share the source's label, it counts only fields named after the source's table or whose catalogue meaning names it.

It leaves out the table the copy went into, rows under the source and the copy, and the copies SwoftyMod adds. It groups the copies of one source, lists derived names before plain labels, and shows at most 8 table warnings per addon. The warnings do not fail the build, even with `"strict": true`:

```text
Classic Seasons: car_setup.presets.vehicle_package_setups has a row labelled f1_2025_vehicle_package and none labelled cs_2019_vehicle_package; f1_2025_vehicle_package is folder 1 of resource_path and the label + _vehicle_package of the add-row source entrants.vehicles.vehicle_packages.vehicle_package[_metadata_=f1_2025]. Add a row per copy, such as an add-row into car_setup.presets.vehicle_package_setups from car_setup.presets.vehicle_package_setups[_metadata_=f1_2025_vehicle_package] that sets _metadata_
My Team: showroom.teams_history.team_history has a row labelled williams_26 and none labelled team12_26; williams_26 is the label of the add-row source entrants.teams.team[_metadata_=williams_26]. Add a row per copy, such as an add-row into showroom.teams_history.team_history from showroom.teams_history.team_history[_metadata_=williams_26] that sets _metadata_
My Team: 4 value(s) pick the add-row source entrants.vehicles.vehicle[_metadata_=mercedes_2025] by its label and none pick the copy team12_2025: entrants.entrant_lists.entrant_list_N.entrant_N.vehicle (2), showroom.vehicle_offsets.vehicle_offset_N.vehicle (1), showroom.vehicles_history.vehicle_history_N.vehicle (1)
```

A table that holds a row for every season, such as `entrants.entrant_lists.entrant_list` for a vehicle package labelled `f1_2025`, also shows up when the source's label is a season name. The check reads names only: a table the game never reads by that name, and game files, are outside it.

`swofty db deps <row>` prints the same data for a stock row before you copy it: the derived names, the rows of other tables labelled with each, and the fields whose values pick the row:

```sh
swofty db deps "entrants.vehicles.vehicle_packages.vehicle_package[_metadata_=f1_2025]"
```

`cargo run -p swofty-core --example check_add_rows -- --game <F1 25 folder> --addon <addon folder>` prints the check for one addon with the completeness check.

### Keys the catalogue does not name

A key whose path only the exe builds stays out of the catalogue until `tools/gamedb/compositions.txt` lists it, and `add-row` then copies it without a `fields` list. `tools/gamedb/deepkeys.py` finds such keys under a row; see [Finding unnamed keys](/reference/gamedb/tools#finding-unnamed-keys).

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

Adding a row also updates the table's `_instancecount_`. The game ignores rows beyond this count. A new row needs every key a stock row of the table has, which `add-row` copies; see [Key names](#key-names) and [Adding a row](/reference/gamedb/recipes#adding-a-row).

`add-row` raises the count of the table `table` resolves to. Adding to `track_data.game_setup.TrackDataSet[_metadata_=f1_2026]` raises `track_data.game_setup.TrackDataSet_16._TrackData_instancecount_`.

`add-row` needs the table's real path, written with row numbers or selectors. A `{hash}` placeholder can be used for `set`, but not for `add-row`.

A mod made for an earlier game version can lower a count and leave stock rows above it. `add-row` writes the new row over those stock keys and the build prints `reuses N stock key(s) of a row above the table's count`. A key another addon wrote at the new row's number still fails the op with `already exists`.

## Rows SwoftyMod adds

SwoftyMod adds two rows labelled `swofty_f1`, copies of the stock `f1` rows, before any addon's operations run:

- a game setup mode in `game_setup.game_setup_modes`, with `stat_id` 20 and the `swofty_f1` entrant setup mode;
- an entrant setup mode in `entrants.entrant_setup_modes`, with empty `entrant_lists` and `selectable_entrant_lists`.

Every stock list of game setup modes that holds `f1` also holds `swofty_f1`. A SwoftyMod change that does not apply to the database is skipped with a warning. An addon adds its entrant lists to the mode with `list-append`:

```json
{ "op": "list-append", "select": "entrants.entrant_setup_modes.entrant_setup_mode[_metadata_=swofty_f1]",
  "field": "entrant_lists", "value": "my_entrant_list" }
```

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

Track activation runs first, then operations run in addon load order. [DB Editor](/using/db-editor) packages are addons and run in the same order; a package change that no longer applies is skipped with a warning instead of failing the build. A later change to a field replaces the earlier value, and the build prints the conflict ([Order and conflicts](/reference/data/operations#order-and-conflicts)). With the in-game loader, the changed records reach the game through `swofty/assets.json` ([Build route and runtime route](/reference/data/operations#build-route-and-runtime-route)).

See [Game database (GVDB)](/reference/gamedb) for the file format, how the game reads it and the table catalogue. [Modding recipes](/reference/gamedb/recipes) covers adding rows, grid size and tracks.
