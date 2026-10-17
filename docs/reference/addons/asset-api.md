---
title: Asset API
description: Asset ops in swofty.json and build hooks, how each kind reaches the game, the order they apply in, conflicts, and what swofty_core does with them while the game boots.
---

# Asset API

An asset op is one change to game content: an asset group, an archive resource, a database record, a language string or a loose file. An addon declares asset ops in its manifest, or returns them from a [build hook](./launcher-api#build-hooks). The launcher plans every op of a profile in one place, the asset planner, and delivers each kind either by rewriting game files at build or through swofty_core while the game boots.

The types are in the `swofty-addon` crate (`packages/addon-api`), module `swofty_addon::asset`. [Loading mods at runtime](./runtime-assets) walks through each kind.

## Op files

`operations.assets` in `swofty.json` lists op files:

```json [swofty.json]
"operations": {
  "assets": ["assets/livery.json"]
}
```

Each op names its kind in a prefix:

```json [assets/livery.json]
{
  "schema_version": 1,
  "ops": [
    { "op": "erp.put-resource", "erp": "2025_asset_groups/f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso.erp",
      "resource": "eaid://f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso.wep", "source": "erp/car.erp" },
    { "op": "db.set", "select": "entrants.teams.team[_metadata_=toro_rosso_26]", "field": "display_name_code", "value": "AT" },
    { "op": "lang.set", "language": "*", "key": "lng_team_tor_hashtag", "value": "AlphaTauri" },
    { "op": "file.put", "source": "files/sponsor.dds", "target": "livery_package_shared/sponsor.dds" },
    { "op": "warn", "message": "Racing Bulls livery" }
  ]
}
```

| Op | Fields | Change |
|---|---|---|
| `group.register` | `group`, `erp` or `erps`, optional `deps`, `template` | Add an asset group to the engine's asset manifest |
| `erp.put-resource` | `erp`, `resource`, `source`, optional `as` | Copy a resource from an archive in the addon into a game archive, replacing or adding it |
| `erp.copy-resource`, `erp.patch-pkg` | As in [ERP operations](/reference/ops/erp) | Copy a resource, or edit a `!pkg` resource in place |
| `db.set`, `db.add-row`, `db.remove-row`, `db.list-append`, `db.list-remove` | As in [database operations](/reference/ops/database) | Change rows and fields |
| `db.put-record` | `key` (a path, or `0x` and 16 hex digits), `type` (the tag), `value` (hex bytes) | Write one raw record |
| `db.remove-record` | `key` | Remove one record |
| `lang.set` | `language` (a code, or `*` for every language), `key`, `value` | Set one string |
| `lang.table` | `language`, `source` (an `.lng` file) | Set every string in a table |
| `file.put` | `source`, `target` (a path in the game folder) | Replace or add a loose game file |
| `warn` | `message` | Print a message at build |

- Files in `operations.database` and `operations.erp` load into the same model, with `db.` and `erp.` added to each op. Each `operations.files` entry is a `file.put`.
- Every op takes `when` and <span v-pre>`{{id}}`</span> from the addon's [`db` settings](/reference/addons/settings#use-a-db-setting), as database operations do.
- `erp.replace-resource`, `erp.add-resource`, `erp.remove-resource` and `erp.add-mipmaps` are defined but not built. An addon that uses one has all its `erp` ops skipped, with a warning.

### Sources

A `source` is a path in the addon's folder, or an object that names another root:

```json
"source": "erp/car.erp"
"source": { "root": "data", "path": "library/my-mod/files/car.erp" }
```

| `root` | Folder |
|---|---|
| `addon` (a plain string) | `$DATA/addons/<id>/` |
| `data` | `$DATA/addon-data/<id>/` |
| `cache` | `$DATA/cache/addons/<id>/` |

A path that leaves its folder fails the build. The build hash covers every op and the size and modification time of every source an op names.

## Deliveries

Each kind is delivered one of three ways:

| Route | Meaning |
|---|---|
| Build | The build rewrites the game file and installs it through the journal |
| Runtime | The build writes `swofty/assets.json`, and swofty_core applies it in memory while the game boots. Game files stay stock. |
| Waiting | No delivery exists for the kind. The build lists the op and its origin |

The installed loader lists its runtime features in `swofty/addons/core/features.json`:

```json
{ "schema": 1, "features": { "group": "stable", "erp": "stable", "db": "stable", "lang": "stable", "file": "experimental" } }
```

| Kind | Build delivery | Runtime feature | Route |
|---|---|---|---|
| `group` | none | stable | Runtime |
| `erp` | the archive is rewritten, unchanged resources copied byte for byte | stable | Runtime |
| `db` | `gamedatabases.2025.bin` is rewritten | stable | Runtime |
| `lang` | none | stable | Runtime |
| `file` | the file is copied into the game folder | experimental | Build, or Runtime when turned on |

- A `stable` kind takes the runtime route.
- An `experimental` kind takes the runtime route only when `"runtime_assets"` in `$DATA/config.json` names it, for example `"runtime_assets": ["file"]`. The build then lists it in `features` in `swofty/assets.json`, which turns it on in swofty_core.
- `"build_assets"` in `$DATA/config.json` keeps the kinds it names that have a build delivery (`erp`, `db`, `file`) on the build route even when they are `stable`, for example `"build_assets": ["erp", "db"]`.
- Otherwise a kind with a build delivery is built, and a kind without one waits. The build prints `<origin>: <n> lang op(s) wait for lang support in swofty_core`, or says the feature is experimental and not turned on.
- A loader without `features.json` has no runtime features. `swofty build` without the loader builds `erp`, `db` and `file` and lists `group` and `lang` as waiting.
- Every op of one kind takes the same route in a build.
- A `db` build that removes records, or writes a record of tag 10, 15 or 16 or one whose value is not text, is built into the game file instead, with a warning.

## Order and conflicts

Ops apply in this order:

1. Tracks the profile enables (`track.add`), whose database rows and archives come first.
2. Addons in the profile's load order.
3. Within an addon, its op files in `operations` order (`database`, then `erp`, then `assets`), then `operations.files`, then the ops its build hook returns.

A later write wins. The planner records the target of each write:

| Target | Written by |
|---|---|
| An asset group | `group.register` |
| A resource in an archive | `erp.put-resource`, `erp.copy-resource`, `erp.patch-pkg` |
| A database record | `db.set`, `db.put-record`, `db.remove-record` |
| A string in a language | `lang.set` |
| A game path | `file.put` |

A write that replaces a target (`erp.put-resource`, `db.set`, `db.put-record`, `db.remove-record`, `lang.set`, `file.put`) after another origin wrote it is a conflict. `erp.copy-resource`, `erp.patch-pkg`, `group.register` and added rows compose. The build prints each conflict with the winner and the origins it overrides:

```text
conflict: text lng_team_tor_hashtag (eng): teamsimplified.mods (rm-designs.scuderia-alphatauri-orlen) overrides example.livery
```

An origin is the addon id, with the item inside the addon in brackets, such as a mod in the Team Simplified library.

## In the game

The build writes `swofty/assets.json` and its files into the loader folder `swofty/` in the game folder. File paths in it are relative to `swofty/`.

| Kind | `swofty/assets.json` | Files |
|---|---|---|
| `group` | `asset_groups`: `group`, `erps`, `deps`, `template`, `origin` | |
| `erp` | `resource_erps`: one overlay archive per set of target archives, as `file` and target `erps` | `assets/resources/<n>.erp` |
| `db` | `db_edits`: each record that differs from the database after track activation, as `hash`, `type` and `value`, and `"early_stage": true` | |
| `lang` | `lang.<code>.table`, and `lang."*".table` for every language | `assets/lang/<code>.lng`, `assets/lang/all.lng` |
| `file` | `"overlay": true`, and `"file"` in `features` | `overlay/<target>` |

swofty_core applies each kind at a fixed point of the boot:

| Kind | Applied | Core part |
|---|---|---|
| `group` | when the engine parses its asset manifest. A group the stock manifest already has is skipped. | `assets.cpp` |
| `erp` | when the engine parses its asset manifest: every group listing a target archive reads the overlay archive first | `resources.cpp` |
| `db` | when the game database loads (`Gdb_Load_GameDatabase`) | `gamedb.cpp` |
| `lang` | when the game loads a language file (`Loc_Load_LanguageFile`), as a table added after the stock table | `overlay.cpp` |
| `file` | each time the game resolves a path under `swofty/overlay/` | `overlay.cpp` |

### The early stage

The game builds its database before the window opens. With `"early_stage": true`, the loader loads swofty_core inside the game's `CreateMutexW("Codemasters F1 25")` call, before the database loads, and swofty_core runs its window-stage handlers at the window trigger. The build sets `early_stage` whenever there are database edits. `SWOFTY_EARLY_STAGE=1` or `0` overrides it.

- Plugins with `"early": true` on their `operations.native` entry load with swofty_core at the early stage. The build lists them in `early_plugins` in `swofty/settings.json`, and a list that is not empty turns the early stage on.
- Every other plugin loads at the window trigger, with a second mixin commit. A window-stage mixin that overlaps one installed at the early stage drops its addon.

Without the early stage the database is already built when addons load. Only in-place int, float and bool overwrites of existing keys apply then. New keys, strings and type changes are skipped and logged.

### Experimental features

An experimental feature applies only when `features` in `swofty/assets.json` lists it. Otherwise swofty_core skips it and logs:

```text
[assets] overlay skipped: the runtime file feature is experimental and "features" in swofty/assets.json does not turn it on
```

### Logs

swofty_core logs every runtime asset step to `swofty/loader.log` with the `[assets]` prefix: each group, each resource with its fragments and stock archives, each database edit (`overwrote`, `created`, `retyped` or `skipped` with the reason), each string table and each overlaid file. Without `swofty/assets.json` it logs nothing and installs no hooks.

## Native plugins

`SwoftyApi` version 4 adds the `assets` table, and `swofty/assets.hpp` in the SDK wraps it. A plugin's calls feed the same lists in swofty_core as `swofty/assets.json`: entries from `assets.json` apply first, then calls in plugin load order. [From a native plugin](./runtime-assets#from-a-native-plugin) has an example.

```cpp
namespace swofty::assets {

enum class Result { ok, late, unavailable, invalid };
struct Fragment {
    std::string_view file;
    std::string_view fourcc = {};
    uint32_t flags = 0;
};

bool available(std::string_view feature);
Result register_group(std::string_view group, std::span<const std::string_view> erps,
                      std::span<const std::string_view> deps = {}, std::string_view templ = {});
Result put_resource(std::string_view uri, std::span<const Fragment> fragments,
                    std::span<const std::string_view> erps = {});
Result db_set(std::string_view key, std::string_view type, std::string_view value);
Result db_set(uint64_t hash, uint8_t tag, std::span<const std::byte> value);
Result lang_set(std::string_view language, std::string_view key, std::string_view text);
Result lang_table(std::string_view language, std::string_view file);
Result overlay_file(std::string_view game_path, std::string_view file);

}
```

| Call | Kind | Same as | Accepted until |
|---|---|---|---|
| `register_group` | `group` | `group.register` | the engine parses its asset manifest |
| `put_resource` | `erp` | a `resources` entry: the core builds the overlay archive | swofty_core writes the resource overlay, at the end of plugin init |
| `db_set` | `db` | a `db_edits` entry | the game database loads. Only plugins with `"early": true` load before it |
| `lang_set`, `lang_table` | `lang` | `lang.set`, `lang.table` | the game loads its first language file |
| `overlay_file` | `file` | `file.put` | the game first reads that path |

| Result | Meaning |
|---|---|
| `ok` | The entry joined the list |
| `late` | The call came after its point |
| `unavailable` | swofty_core lacks the feature, or it is experimental and `features` in `swofty/assets.json` does not turn it on, or the core offers an API version below 4 |
| `invalid` | An argument is empty or malformed, or a file is missing |

- Plugin init is every plugin's `SwoftyAddonInit` and `SWOFTY_ON_LOAD`. The core reads the lists after the last of them.
- After plugin init, `register_group`, `db_set`, `lang_set`, `lang_table` and `overlay_file` join the core's lists directly while their point has not passed and that kind already has something to apply. Otherwise they return `late`. `put_resource` returns `late`.
- At the window stage the game database is already built, so `db_set` returns `late`.
- `available(feature)` takes `group`, `erp`, `db`, `lang` or `file`.
- `db_set` takes a key path, or `0x` and 16 hex digits. `type` is a `db_edits` type name (`int`, `float`, `string`, `enum`, `id`, `bool`, ...) or a tag number. The raw form takes the hash and the tag, and its value bytes are the record's text.
- `put_resource` fragments are files with `fourcc` and `flags` as in a `resources` entry; empty and 0 take the stock resource's. `erps` names the target archives, and without it the core finds the archives that hold the uri.
- File arguments are paths relative to the addon's folder, `swofty/addons/<id>/`. A path that leaves the folder, or a file that is missing, returns `invalid`.
- The core finds the addon from the calling DLL through `dlls` in `swofty/settings.json`. A DLL that no addon lists gets `invalid` for every call that takes a file.
- Every call is logged with the addon id and the DLL:

```text
[assets] api: example.livery (livery.dll) register_group asset_groups/example_package/showcar.assetgroup -> ok
[assets] api: example.livery (livery.dll) db_set entrants.teams.team.x = 1 -> late: the core loaded at the window stage, after the game database loaded
```

A plugin that calls the API declares the `assets` capability, and `"early": true` loads it at the early stage:

```json [swofty.json]
"operations": {
  "native": [{ "dll": "native/livery.dll", "capabilities": ["log", "assets"], "early": true }]
}
```
