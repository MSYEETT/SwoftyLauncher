# Sample mod

A starting point for an F1 Mod Manager mod. Copy this directory, rename it, and change `id` in `swofty.json`.

## Contents

| Path | What it is |
|---|---|
| `swofty.json` | The manifest: id, version, compatibility, options and the list of operations |
| `db/example.json` | Database operations on `gamedatabases.2025.bin`: a plain `set` and a `set` through a row selector |
| `ui/example.pkgpatch.json` | A `!pkg` patch that clones a tooltip entry in `tool_tips.uidb` |
| `native/src/sample_plugin.c` | A native plugin that logs and runs one signature scan |
| `native/include/swofty_api.h` | The plugin C ABI, version 1 |
| `native/build.sh` | Builds `native/sample_plugin.dll` with `zig cc` for `x86_64-windows-gnu` |

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
