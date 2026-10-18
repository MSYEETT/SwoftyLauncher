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
| `swofty/{events,overlay,telemetry,db,storage}.hpp` | `packages/sdk/include/swofty/` | The standard libraries: lifecycle events and a frame tick, the HUD overlay, telemetry reads, game-database reads, addon storage. MIT |
| `swofty/game.hpp` | `native/include/swofty/` | Every subsystem of the [Game API](/reference/game/) |
| `swofty/game/<subsystem>.hpp` | `native/include/swofty/game/` | One subsystem's functions, structs, data and sites |

The [addon template](/reference/addons/template) ships `swofty_api.h` and `swofty/mixin.hpp`. Copy the game headers from `native/include/swofty/` to hook game functions. The Rust crate `swofty-sdk` (`packages/sdk`) mirrors the C ABI.

## APIs

| API | Use it to | Reference |
|---|---|---|
| Addon manifest | Describe the addon, its settings and its operations | [Addon manifest](/reference/addons/manifest) |
| Mixin API | Wrap functions, run code at sites, replace virtual methods, redirect calls, patch bytes | [Mixin API](./mixin) |
| Game API | Call and hook F1 25's functions, read its structs | [Game API](/reference/game/) |
| UI | Add screens and home tiles, drive ADDON SETTINGS pages | [UI](./ui) |
| Settings | Declare options and read them from a plugin | [Settings](./settings) |
| Logging | Write to `swofty/loader.log`, configure the game log | [Logging](./logging) |
| Database read | Read the loaded game database: values, fields, rows, tables | [Database read](./db) |
| Storage | Read and write the addon's own files under `swofty/addons/<id>/` | [Storage](./storage) |
| Native plugins | The plugin ABI: exports, `SwoftyApi`, loading | [Native plugins](/reference/addons/native-plugins) |
| Database operations | Change the game database | [Database operations](/reference/ops/database) |
| ERP operations | Change resources in `.erp` archives | [ERP operations](/reference/ops/erp) |
| UI patches | Change screens, routes and UI databases | [UI patches](/reference/ops/ui-patches) |
| File operations | Copy files into the game folder | [File operations](/reference/ops/files) |

## API versions

`SwoftyApi.version` is the version the running loader offers. `SWOFTY_API_VERSION` in `swofty_api.h` is 5.

| Members | Offered by |
|---|---|
| `version`, `log`, `module_base`, `scan` | `version.dll`, as version 1 |
| `resolve`, `register_mixins`, `fields`, `setting`, `watch_setting`, `assets` | `swofty_core.dll`, as version 4 |
| `events`, `overlay`, `telemetry`, `db`, `storage` | `swofty_core.dll`, as version 5 |

The mixin helpers in `mixin.hpp` need version 2 or later, the settings helpers version 3, `assets.hpp` version 4, and the standard-library headers (`events.hpp`, `overlay.hpp`, `telemetry.hpp`, `db.hpp`, `storage.hpp`) version 5. See the [native libraries design](/design/native-libraries).

Each manifest capability needs a version: `log` and `scan` 1, `hook` 2, `options` 3, `assets` 4, and `read-db`, `on-frame`, `events`, `overlay`, `telemetry` and `storage` 5.
