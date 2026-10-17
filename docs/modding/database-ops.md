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

## Selecting rows

A selector names a row or finds it by a field:

| Selector | Meaning |
|---|---|
| `tracks.Track_3` | Row at index 3 |
| `entrants.teams.team[_metadata_=gm_cadillac_26]` | Team with this label |
| `{446668bc25ba571c}[_metadata_=f1_26]` | Row in a table whose full path is unknown |

Use labels where possible. Row numbers can change between game builds.

## The row-count rule

Adding or removing rows also updates the table's `_instancecount_`. The game ignores rows beyond this count.

Row additions and removals need the table's real path. A `{hash}` placeholder can be used for `set`, but not row changes.

## Applying changes

Operations run in mod load order. Later changes to a field replace earlier values.

See [Game database (GVDB)](/reference/gvdb) for the file format.
