# Database edits

Database operations change game values in `game_package/gamedatabases/gamedatabases.2025.bin`.

## File format

List the operation file in `operations.database` in your [manifest](/modding/manifest):

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
| `remove-row` | `select` | Remove a row |
| `list-append` | `select`, `field`, `value` | Add a value to a list if missing |
| `list-remove` | `select`, `field`, `value` | Remove a value from a list |

For `set`, use `create: true` and a `type` tag to add a missing field. `value_from` copies a value from another database path.

`add-row` copies the fields listed for the table in the file's `fields` map, keyed by table and row name. Every listed field must exist on the source row:

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

An `add-row` of an `unlockable_se_livery` row into an `unlockable_content.unlockable_items.unlockable_item_list_<n>.folder_<m>` table unlocks the new special livery. The build writes the row's ownership key, the djb2 hash of `eSELivery` and the row's `_metadata_`, to the `owned_items` block of `swofty/settings.json`, and `swofty_core.dll` reports that key as owned to the livery picker. A key that any stock database value also gives is left out with a warning.

## Selecting rows

A selector names a row or finds it by a field:

| Selector | Meaning |
|---|---|
| `tracks.Track_3` | Row at index 3 |
| `entrants.teams.team[_metadata_=gm_cadillac_26]` | Team with this label |
| `{446668bc25ba571c}[_metadata_=f1_26]` | Row in a table whose full path is unknown |

Use labels where possible. Row numbers can change between game builds.

## The row-count rule

Adding or removing rows also updates the table's `_instancecount_`. The game ignores rows beyond this count. A new row needs every key a stock row of the table has; see [Adding a row](/reference/gamedb/recipes#adding-a-row).

Row additions and removals need the table's real path. A `{hash}` placeholder can be used for `set`, but not row changes.

## Settings

A `db` [option](/modding/manifest#options) changes operations at build time:

- <span v-pre>`{{id}}`</span> in any string is replaced with the option's value: `true` or `false`, the number, or the choice value.
- `"when": { "id": value }` keeps the operation only when every listed option has that value.

```json
{ "op": "set", "select": "rules.stop_go_penalty", "field": "penalty_duration", "value": "{{penalty_seconds}}" },
{ "op": "set", "select": "entrants.teams.team[_metadata_=gm_cadillac_26]", "field": "display_name_code", "value": "CAD",
  "when": { "short_names": true } }
```

Changing a `db` option marks the profile for a rebuild. The next launch, or `swofty build`, applies it.

## Applying changes

Operations run in addon load order. Later changes to a field replace earlier values.

See [Game database (GVDB)](/reference/gamedb) for the file format, how the game reads it and the table catalogue. [Modding recipes](/reference/gamedb/recipes) covers adding rows, grid size and tracks.
