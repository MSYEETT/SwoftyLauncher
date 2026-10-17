# Database operations

Database operations change `game_package/gamedatabases/gamedatabases.2025.bin`, the game value database (GVDB). They work on records by name, never on bytes. The format is described in [Game database (GVDB)](/reference/gvdb).

## File format

A database operation file is listed in `operations.database`:

```json
{
  "schema_version": 1,
  "ops": [
    { "op": "set", "select": "tracks.Track_3", "field": "numberOfCorners", "value": "16" }
  ]
}
```

Values are strings, as the database stores them. Arrays are comma-joined: `"0,-0.6,0"`.

## Operations

| Op | Fields | Effect |
|---|---|---|
| `set` | `select`, `field`, `value` or `value_from`, `datasets?`, `create?`, `type?` | Writes one field of every row the selector matches. Fails if the key does not exist, unless `create` is true, in which case `type` gives the type tag |
| `add-row` | `table`, `from`, `set` | Appends a row to `table`, cloned field by field from the row `from`, then applies `set` |
| `remove-row` | `select` | Removes a row and keeps the table contiguous |
| `list-append` | `select`, `field`, `value` | Adds a token to a list value (type 13) if it is not there |
| `list-remove` | `select`, `field`, `value` | Removes a token from a list value |

## Selectors

A selector names a container, then optionally filters rows with `[field=value]`:

| Selector | Matches |
|---|---|
| `tracks.Track_3` | One row by its index |
| `entrants.teams.team[_metadata_=gm_cadillac_26]` | The row of `entrants.teams` labelled `gm_cadillac_26` |
| `entrants.entrant_setup_modes.entrant_setup_mode[_metadata_=f1_26]` | Rows of a nested table by label |
| `{446668bc25ba571c}[_metadata_=f1_26]` | A table whose path is still unknown, by its hash placeholder |

`_metadata_` is the row label every table row carries. Any other field works too. Selectors resolve to concrete 64-bit keys when the operation is applied, against the database as merged so far, so a game update that renumbers rows does not break a mod.

## `value_from`

`value_from` copies a value from another resolved path instead of a literal:

```json
{ "op": "set", "select": "{446668bc25ba571c}[_metadata_=f1_26]", "field": "worldEri",
  "value_from": "{446668bc25ba571c}[_metadata_=myteam_26].worldEri" }
```

## `datasets`

`datasets` scopes a `set` to some datasets (`f1`, `f1_2025`, `f1_2026`, `sprint_f1_2025`, `sprint_f1_2026`), the way track package overrides do. Absent means every dataset.

## The row-count rule

Every table stores its row count in a key `<container>._<row>_instancecount_`, for example `entrants.teams._team_instancecount_`. The game reads rows `0..count-1` only. A row past the count is silently ignored.

So:

- `add-row` appends. The new row takes the index equal to the table's current count, and the same operation increments the count.
- `remove-row` moves later rows down one index and decrements the count.
- `add-row` and `remove-row` need the table's real path to compute the count key. A table known only by its `{hash}` placeholder accepts `set`, not row operations.
- After every build, each touched table's `_instancecount_` must equal its contiguous row count. The build fails otherwise.

## Example: a twelfth team on the 2026 grid

These operations give the F1 2026 grid a twelfth team. On the pristine database they produce `team_358`, `vehicle_413` and `entrant_22`/`entrant_23`, and set the three counts to 359, 414 and 24.

```json
{
  "schema_version": 1,
  "ops": [
    { "op": "set", "select": "entrants.entrant_setup_modes.entrant_setup_mode[_metadata_=f1_26]",
      "field": "max_entrants", "value": "24" },
    { "op": "set", "select": "entrants.entrant_setup_modes.entrant_setup_mode[_metadata_=f1_generic_2026]",
      "field": "max_entrants", "value": "24" },

    { "op": "set", "select": "{446668bc25ba571c}[_metadata_=f1_26]", "field": "worldEri",
      "value_from": "{446668bc25ba571c}[_metadata_=myteam_26].worldEri" },
    { "op": "set", "select": "{446668bc25ba571c}[_metadata_=f1_26]", "field": "worldAssetGroup",
      "value_from": "{446668bc25ba571c}[_metadata_=myteam_26].worldAssetGroup" },

    { "op": "add-row", "table": "entrants.teams",
      "from": "entrants.teams.team[_metadata_=gm_cadillac_26]",
      "set": { "_metadata_": "team12_26", "display_name": "lng_custom_team_26",
               "display_name_code": "T12", "stat_id": "500", "primary_colour": "-16735845" } },
    { "op": "add-row", "table": "entrants.vehicles",
      "from": "entrants.vehicles.vehicle[_metadata_=myteam_honda_2026]",
      "set": { "_metadata_": "team12_2026", "stat_id": "509" } },

    { "op": "add-row", "table": "entrants.entrant_lists.entrant_list_34",
      "from": "entrants.entrant_lists.entrant_list_34.entrant_20",
      "set": { "_metadata_": "team12_driver_01", "team": "team12_26", "vehicle": "team12_2026",
               "entrant_reference": "jack_tremblay", "car_number": "66",
               "vehicle_customisation": "jack_tremblay_2026" } },
    { "op": "add-row", "table": "entrants.entrant_lists.entrant_list_34",
      "from": "entrants.entrant_lists.entrant_list_34.entrant_21",
      "set": { "_metadata_": "team12_driver_02", "team": "team12_26", "vehicle": "team12_2026",
               "entrant_reference": "rashid_nair", "car_number": "65",
               "vehicle_customisation": "jack_tremblay_2026" } }
  ]
}
```

The first two operations raise `max_entrants`. The next two move the 2026 world to the My Team world, which has 24 grid slots and 12 garages. The `add-row`s add the team, its car and two entrants. Without the count bumps, the game would never read the new rows.

## Merging

All mods' database operations run over one index in load order:

- The last writer wins per key, and the build records which mod wrote what. Two mods writing different final values to one key is a conflict, reported with both values.
- The same value from two mods is a harmless duplicate.
- Two mods adding rows with one `_metadata_` label to one table is a conflict.
- List operations on the same list merge as an ordered union.
- A profile can resolve a conflict explicitly by naming the winner.

The same baseline and the same ordered operations always produce the same bytes.
