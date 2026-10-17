# Sample mod

A starting point for an F1 Mod Manager mod. Copy this directory, rename it, and change `id` in `swofty.json`.

## Contents

| Path | What it is |
|---|---|
| `swofty.json` | The manifest: id, version, compatibility, settings and the list of operations |
| `db/example.json` | Database operations on `gamedatabases.2025.bin`: a `set` whose value comes from the `penalty_seconds` setting, and a `set` through a row selector |
| `ui/example.pkgpatch.json` | A `!pkg` patch that clones a tooltip entry in `tool_tips.uidb` |
| `native/src/sample_plugin.cpp` | A native plugin that reads its settings, reacts when they change in game and runs one signature scan |
| `native/include/swofty_api.h` | The plugin C ABI, version 3 |
| `native/include/swofty/mixin.hpp` | The C++ layer: settings, mixins and `SWOFTY_MOD` |
| `native/build.sh` | Builds `native/sample_plugin.dll` with `zig c++` for `x86_64-windows-gnu` |

## Settings

`options` in `swofty.json` declares four settings: a toggle, a choice and a slider read by the plugin (`binds: native`), and a slider written into the database (`binds: db`). The launcher shows them in the mod's row and the game shows them under **MOD SETTINGS**. The plugin declares the same ids with `SWOFTY_SETTING` and reacts to in-game changes with `SWOFTY_ON_CHANGE`.

## Build and install

```sh
native/build.sh
swofty mods install .
swofty mods enable example.sample-mod
```

Remove the `native` entry from `swofty.json` and the `native/` directory for a data-only mod. A native plugin loads only after the player consents to its exact file hash.

## Rules

* Ship operations, not game files. Never include ERPs, `gamedatabases` copies or extracted assets.
* `add-row` appends at the table's `_instancecount_` and bumps the count. Rows past the count are never read.
* Hook only `.xtls` code. `.data1` bodies may be called but never hooked.

The manifest reference, the database and UI operations and the plugin API are documented under Modding in the F1 Mod Manager documentation.

## Licence

This template is under the MIT licence, see [LICENSE](LICENSE), so mods built from it can use any licence.
