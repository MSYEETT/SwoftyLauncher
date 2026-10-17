---
title: Loading mods at runtime
description: Replace a texture, add strings, edit the game database and register an asset group from an addon, with swofty_core applying each change in memory while the game boots.
---

# Loading mods at runtime

An addon's [asset ops](./asset-api) of the kinds `group`, `erp`, `db` and `lang` reach the game through swofty_core: the build writes `swofty/assets.json`, and the core applies it in memory while the game boots. The game folder keeps its stock files. Each example below is one op file listed in `operations.assets`:

```json [swofty.json]
{
  "schema_version": 1,
  "id": "example.runtime",
  "name": "Runtime example",
  "version": "1.0.0",
  "author": "Your Name",
  "compat": { "game": "f1_25" },
  "operations": {
    "assets": ["assets/texture.json", "assets/text.json", "assets/database.json", "assets/group.json"]
  }
}
```

Build the profile and check what it planned:

```sh
swofty addons install example-runtime
swofty addons enable example.runtime
swofty build
```

Then start the game and read the `[assets]` lines in `swofty/loader.log`.

## Replace a texture

Put the new texture in an `.erp` archive in the addon, under the same resource name as the stock one, and copy it over the stock resource with `erp.put-resource`:

```json [assets/texture.json]
{
  "schema_version": 1,
  "ops": [
    { "op": "erp.put-resource",
      "erp": "2025_asset_groups/ui_package/flow_boot.erp",
      "resource": "eaid://ui_package/art/textures/press_start/ps_logo.tif.image",
      "source": "erp/logo.erp" }
  ]
}
```

- `erp` is the game archive the resource lives in. `python3 tools/erp.py <archive> '*'` lists an archive's resources ([Textures and models](/reference/mods/erp-assets)).
- `source` is the archive in the addon that holds the resource. `"as"` names it differently in the game archive. A resource name the game archive does not have is added.
- The build writes the changed resources into an overlay archive under `swofty/assets/resources/` and lists it in `resource_erps`. Every asset group that lists `flow_boot.erp` reads the overlay archive first.
- A replacement keeps the stock resource's type, argument and fragment layout and sizes. The build leaves out a resource that changes them and reports a warning.

```text
[assets] resources: overlay ERP swofty/assets/resources/0.erp prebuilt by the launcher (... bytes, 1 resource(s): 1 replaced, 0 added) for 1 target ERP(s): 2025_asset_groups/ui_package/flow_boot.erp
```

## Add or change strings

`lang.set` sets one string. `"*"` sets it in every language, and a code such as `eng` in one:

```json [assets/text.json]
{
  "schema_version": 1,
  "ops": [
    { "op": "lang.set", "language": "*", "key": "lng_team_tor_hashtag", "value": "AlphaTauri" },
    { "op": "lang.set", "language": "eng", "key": "lng_example_hint", "value": "Runtime example loaded" },
    { "op": "lang.table", "language": "eng", "source": "lang/extra_eng.lng" }
  ]
}
```

- `lang.table` loads every string of an `.lng` file in the addon. `swofty_formats::lng` writes one.
- A key the game does not have is added, and a database field such as `name_loc_tag` can then name it ([In-game text](/reference/mods/text)).
- The build merges the strings of every addon into one table per language under `swofty/assets/lang/`. The core adds each table after the stock table when the game loads that language, and its strings win over the stock ones.

```text
[assets] lang: added 2 string(s) from swofty/assets/lang/eng.lng for 'eng' at priority ...
```

## Edit the database

Database ops take the same form as in [database operations](/reference/ops/database), with the `db.` prefix:

```json [assets/database.json]
{
  "schema_version": 1,
  "ops": [
    { "op": "db.set", "select": "entrants.teams.team[_metadata_=toro_rosso_26]", "field": "display_name_code", "value": "AT" },
    { "op": "db.put-record", "key": "0xf01d03c7a1b2c3d4", "type": 0, "value": "01000000" }
  ]
}
```

- `db.put-record` writes one raw record by path or hash. `type` is its tag and `value` its bytes in hex.
- The build applies every op to a copy of the database, then writes each record that differs from stock to `db_edits` and sets `early_stage`. The game's database file stays stock.
- With the early stage the loader loads addons before the game builds its database, and the core applies every edit as it loads.

```text
[assets] gamedb: 2 overwritten, 0 created, 0 retyped, 0 skipped; ...
```

A build that removes records, or writes a record of tag 10, 15 or 16 or one whose value is not text, writes the database file instead, with a warning.

## Register an asset group

The engine loads content by asset group. `group.register` adds a group to the engine's asset manifest, pointing at archives in the game folder:

```json [assets/group.json]
{
  "schema_version": 1,
  "ops": [
    { "op": "file.put", "source": "erp/showcar.erp", "target": "2025_asset_groups/example_package/showcar.erp" },
    { "op": "group.register",
      "group": "asset_groups/example_package/showcar.assetgroup",
      "erp": "2025_asset_groups/example_package/showcar.erp" }
  ]
}
```

- `file.put` ships the archive. Loose files are copied into the game folder at build, or served from `swofty/overlay/` when the profile turns on the experimental `file` feature.
- `erps` lists several archives. `deps` names groups that load first, and `template` names a stock group to copy settings from.
- The core skips a group that the stock manifest already has. A database field that points at a stock group whose archive is missing hangs the session load.
- Database fields such as a track's `worldAssetGroup` name the group. Tracks use this route: each track's archive goes to `2025_asset_groups/environment_package/tracks/<code>/<code>.erp` with one group per world, bake and probe set ([How maps are installed](/reference/maps)).

## From a build hook

A built-in addon returns the same ops from [`Addon::build`](./launcher-api#build-hooks), and a third-party addon from its helper's `build` method. They take the same routes, after the addon's declared ops. Team Simplified merges `.serps` mods this way: `db.put-record`, `erp.put-resource` and `lang.set` per enabled mod, with sources in its `data` root.
