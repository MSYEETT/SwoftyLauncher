---
title: Standard library
description: The headers, APIs and file formats an addon builds on.
---

# Standard library

Everything an addon can use, by area. Data operations are JSON files listed in `swofty.json`. Native plugins use the C and C++ headers of the SDK and the generated game headers.

## Headers

| Header | Location | Contents |
|---|---|---|
| `swofty_api.h` | `packages/sdk/include/` | The C ABI: `SwoftyApi`, mixin records, setting values. MIT |
| `swofty/mixin.hpp` | `packages/sdk/include/swofty/` | The C++ layer: mixin macros, bindings, settings, `swofty::log`, `SWOFTY_ADDON`. MIT |
| `swofty/game.hpp` | `native/include/swofty/` | Every subsystem of the [Game API](/reference/game/) |
| `swofty/game/<subsystem>.hpp` | `native/include/swofty/game/` | One subsystem's functions, structs, data and sites |

The [addon template](/modding/template) ships `swofty_api.h` and `swofty/mixin.hpp`. Copy the game headers from `native/include/swofty/` to hook game functions. The Rust crate `swofty-sdk` (`packages/sdk`) mirrors the C ABI.

## APIs

| API | Use it to | Reference |
|---|---|---|
| Addon manifest | Describe the addon, its settings and its operations | [Addon manifest](/modding/manifest) |
| Mixin API | Wrap functions, run code at sites, replace virtual methods, redirect calls, patch bytes | [Mixin API](./mixin) |
| Game API | Call and hook F1 25's functions, read its structs | [Game API](/reference/game/) |
| UI | Add screens and home tiles, drive ADDON SETTINGS pages | [UI](./ui) |
| Settings | Declare options and read them from a plugin | [Settings](./settings) |
| Logging | Write to `swofty/loader.log`, configure the game log | [Logging](./logging) |
| Native plugins | The plugin ABI: exports, `SwoftyApi`, loading | [Native plugins](/modding/native-plugins) |
| Database operations | Change the game database | [Database operations](/modding/database-ops) |
| ERP operations | Change resources in `.erp` archives | [ERP operations](/modding/erp-ops) |
| UI patches | Change screens, routes and UI databases | [UI patches](/modding/ui-patches) |
| File operations | Copy files into the game folder | [File operations](./files) |

## API versions

`SwoftyApi.version` is the version the running loader offers. `SWOFTY_API_VERSION` in `swofty_api.h` is 3.

| Members | Offered by |
|---|---|
| `version`, `log`, `module_base`, `scan` | `version.dll`, as version 1 |
| `resolve`, `register_mixins`, `fields`, `setting`, `watch_setting` | `swofty_core.dll`, as version 3 |

The mixin helpers in `mixin.hpp` need version 2 or later, and the settings helpers version 3.

Each manifest capability needs a version: `log` and `scan` 1, `hook` 2, `options` 3. `read-db` and `on-frame` are accepted in manifests and not offered by any version.
