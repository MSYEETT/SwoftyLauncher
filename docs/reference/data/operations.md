---
title: Operations
description: What an operation is, every field of swofty.json operations, the prefixed kinds in assets files, the build and runtime routes, and the order and conflicts between addons.
---

# Operations

An **operation** (op) is one data change your addon asks for: set a database field, replace a resource in an archive, add a string, register an asset group, copy a loose file, add a track. You don't write code for it. You list op files in the `operations` block of `swofty.json`, and every `swofty build` (and every **Play**) applies the ops of each enabled addon to a copy of the game's files, or hands them to swofty_core to apply in memory while the game boots. The game folder keeps its stock files.

An addon with only operations is a [data addon](./). An addon with code can list operations too.

## An operation file

An op file is JSON with a `schema_version` and a list of `ops`. Each op names its kind in `op`:

```json [db/changes.json]
{
  "schema_version": 1,
  "ops": [
    { "op": "set", "select": "rules.stop_go_penalty", "field": "penalty_duration", "value": "10" }
  ]
}
```

List the file under the field for its kind:

```json [swofty.json]
"operations": {
  "database": ["db/changes.json"]
}
```

| Field in an op file | Meaning |
|---|---|
| `schema_version` | `1` |
| `ops` | The ops, applied in order |
| `fields` | Optional. For `add-row`: extra key names to copy, keyed by `<table>.<row name>`, or the only ones with `copy: listed` ([Key names](/reference/ops/database#key-names)) |
| `strict` | Optional. `true` fails the build on an `add-row` completeness warning ([The completeness check](/reference/ops/database#the-completeness-check)) |

Every op takes two extras from your addon's [`db` settings](/reference/addons/settings#_3-read-a-db-setting): <span v-pre>`{{id}}`</span> in any string is replaced with the setting's value, and `"when": { "id": value }` keeps the op only when each listed setting has that value.

## The `operations` fields

| Field | Holds | Changes | Reference |
|---|---|---|---|
| `database` | Op files of database ops, no prefix: `set`, `add-row`, `put-record`, `remove-record` | Records in `gamedatabases.2025.bin` | [Database operations](/reference/ops/database) |
| `erp` | Op files of ERP ops, no prefix: `replace-resource`, `add-resource`, `copy-resource`, `put-resource`, `remove-resource`, `add-mipmaps`, `patch-pkg`, `warn` | Resources in `.erp` archives | [ERP operations](/reference/ops/erp) |
| `assets` | Op files that mix every kind, each op with a prefix: `db.set`, `erp.put-resource`, `lang.set`, `group.register`, `file.put`, `track.add`, ... | Everything above, plus text, asset groups, loose files and tracks | [Asset op files](#asset-op-files), [Asset API](/reference/addons/asset-api) |
| `files` | `[{ "source", "target" }]` entries, no op file | Loose files in the game folder | [File operations](/reference/ops/files) |
| `screens` | `[{ "name", "file", "flow", "anchor", "view_model" }]` entries | New in-game screens, installed as `swofty/ui/<name>.ui` | [In-game UI](/reference/addons/in-game-ui#add-a-screen) |
| `ui` | `*.pkgpatch.json` patch files | Menus. Not built yet: the build skips them with a warning | [UI patches](/reference/ops/ui-patches) |

A `database` file and an `assets` file with `db.` ops are the same model: the build adds `db.` to every op in a `database` file and `erp.` to every op in an `erp` file, then plans them together. Each `files` entry becomes a `file.put`. Choose `assets` when one file should change several kinds at once.

`native` sits in the same block but is not an operation: it names an [engine plugin](/reference/addons/native-plugins), a DLL with code that runs in the game. `launcher`, outside `operations`, names a [launcher plugin](/reference/addons/launcher-api), whose build hook can return ops of its own. An addon with either is a code addon.

## Asset op files

In an `assets` file, the prefix before the dot names the kind:

```json [assets/livery.json]
{
  "schema_version": 1,
  "ops": [
    { "op": "erp.put-resource", "erp": "2025_asset_groups/f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso.erp",
      "resource": "eaid://f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso.wep", "source": "erp/car.erp" },
    { "op": "db.set", "select": "entrants.teams.team[_metadata_=toro_rosso_26]", "field": "display_name_code", "value": "AT" },
    { "op": "lang.set", "language": "*", "key": "lng_team_tor_hashtag", "value": "AlphaTauri" },
    { "op": "warn", "message": "Racing Bulls livery" }
  ]
}
```

| Kind | Ops | Changes | Reference |
|---|---|---|---|
| `db.*` | `db.set`, `db.add-row`, `db.put-record`, `db.remove-record` | Game database records | [Database operations](/reference/ops/database) |
| `erp.*` | `erp.replace-resource`, `erp.add-resource`, `erp.copy-resource`, `erp.put-resource`, `erp.remove-resource`, `erp.add-mipmaps`, `erp.patch-pkg` | Archive resources, and mip files in the game folder | [ERP operations](/reference/ops/erp) |
| `lang.*` | `lang.set`, `lang.table` | In-game strings, in one language or all (`*`); new keys are added | [In-game text](./text) |
| `group.*` | `group.register` | An asset group in the engine's asset manifest | [Asset API](/reference/addons/asset-api#op-files) |
| `file.*` | `file.put` | A loose file in the game folder | [File operations](/reference/ops/files) |
| `track.add` | `source`: a folder holding an unpacked track package | A whole track: database rows, archive, menus, ERS | [A track with its own name](./track-names) |
| `item.show` | `id`, `name`, optional `author`, `version`, `description` | A read-only tile in the in-game **ADDON SETTINGS** screen | [Asset API](/reference/addons/asset-api#op-files) |
| `warn` | `message` | Nothing; prints the message at build | |

`db.remove-row`, `db.list-append` and `db.list-remove` are defined but not built: a build with one fails. A `patch-pkg` edit other than `clone-entry` and `set-value` makes the build skip every ERP op of that addon, with a warning.

An unknown op name or field fails the build and names the file.

### Sources

A `source` is a path in your addon folder. To read from the addon's data or cache folder, give an object:

```json
"source": "erp/car.erp"
"source": { "root": "data", "path": "library/my-mod/files/car.erp" }
```

`root` is `addon` (the default), `data` (`$DATA/addon-data/<id>/`) or `cache` (`$DATA/cache/addons/<id>/`). A path that leaves its folder fails the build. See [Sources](/reference/addons/asset-api#sources).

## Build route and runtime route

Each kind reaches the game one of two ways:

- **Build route.** `swofty build` writes a changed copy of the game file into the overlay folder, and **Play** swaps it into the game folder for the session through the journal.
- **Runtime route.** `swofty build` writes the changes to `swofty/assets.json`, and swofty_core, the in-game loader, applies them in memory while the game boots. The game's files stay stock.

| Kind | Build route | Runtime route | With the in-game loader |
|---|---|---|---|
| `group` | none | Registered when the engine parses its asset manifest | Runtime |
| `erp` | The archive is rewritten | An overlay archive the game reads first | Runtime |
| `db` | `gamedatabases.2025.bin` is rewritten | `db_edits`, applied as the database loads | Runtime |
| `lang` | none | A string table added after the stock one | Runtime |
| `file` | The file is copied into the game folder | Served from `swofty/overlay/` | Build (runtime is experimental) |

- Without the in-game loader, `erp`, `db` and `file` take the build route, and `group` and `lang` ops wait: the build lists them and the game doesn't see them.
- `track.add`, `item.show` and `warn` have no route: the build handles them itself.
- Some changes force the build route: an `erp.remove-resource`, a replaced resource whose layout differs from the stock one, and database edits the loader can't apply.
- To force a kind onto the build route, add it to `"build_assets"` in `$DATA/config.json`, for example `"build_assets": ["db"]`. To turn on the experimental runtime `file` route, add `"runtime_assets": ["file"]`.

The full rules are in [Deliveries](/reference/addons/asset-api#deliveries).

## Order and conflicts

A build applies ops in this order:

1. `track.add` ops from every addon, in load order. Track activation clones the base track's database rows before any other database op runs.
2. Addons in the profile's load order.
3. Within one addon: its `database` files, then `erp` files, then `assets` files, each in the order listed, then `files` entries, then the ops its build hook returns.

A later write to the same target wins. Replacing what another addon wrote counts as a conflict, and the build prints the winner and the addons it overrides:

```text
warning: conflict: text lng_team_tor_hashtag (eng): teamsimplified.mods (rm-designs.scuderia-alphatauri-orlen) overrides example.livery
```

| Target | Replaced by | Composes with |
|---|---|---|
| A database record | `db.set`, `db.put-record`, `db.remove-record` | `db.add-row` |
| A resource in an archive | `erp.put-resource`, `erp.replace-resource`, `erp.remove-resource` | `erp.add-resource`, `erp.copy-resource`, `erp.patch-pkg` |
| A string in a language | `lang.set` | |
| A game path | `file.put`, `erp.add-mipmaps` | |
| An asset group | | `group.register` |

To make your addon win, move it later in the load order, or name the other addon in `load_after` ([manifest](/reference/addons/manifest#requires-conflicts-load-after)). See [Order and conflicts](/reference/addons/asset-api#order-and-conflicts).

## Check what a build did

```sh
swofty build
```

| Output | Meaning |
|---|---|
| `runtime: swofty/assets.json with <n> asset group(s), <n> resource(s), <n> database edit(s), <n> language table(s), <n> overlay file(s)` | What goes the runtime route |
| `replace <path>`, `add <n> file(s) under <folder>` | Game files the build route replaces or adds |
| `warning: <addon>: <message>` | A `warn` op |
| `warning: <addon>: <n> lang op(s) wait for lang support in swofty_core` | Ops with no route: the in-game loader is missing |
| `warning: conflict: ...` | A later addon replaced a target an earlier one wrote |
| `overlay <buildhash>: <folder>` | The build's folder, with `plan.json` and `swofty/assets.json` |

A build that can't apply an op fails and names the addon and the cause, such as `<select>: no row matches`.

## Reference

- [Database operations](/reference/ops/database), [ERP operations](/reference/ops/erp), [File operations](/reference/ops/files), [UI patches](/reference/ops/ui-patches): each kind's fields.
- [Asset API](/reference/addons/asset-api): asset op files, deliveries, what swofty_core does in game, and the native API.
- [Changes at runtime](/reference/addons/runtime-assets): one worked example per runtime kind.
- [Addon manifest](/reference/addons/manifest#operations): the `operations` block among every other `swofty.json` field.
