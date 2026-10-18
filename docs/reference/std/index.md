---
title: Standard library
description: The headers and SwoftyApi tables an engine plugin builds on - mixins, settings, logging, events, overlay, telemetry, database reads and storage.
---

# Standard library

When your [engine plugin](/reference/addons/native-plugins) initializes, it receives a `const SwoftyApi *`. Through that pointer, you can use the mixin system, settings and logging, plus nested tables for events, overlay, telemetry, database reads and storage. These make up the standard library. Each library has one C++ wrapper header and one reference page.

## Libraries

| Library | Header | Feature | API | Thread |
|---|---|---|---|---|
| [Mixin system](./mixin) | `swofty/mixin.hpp` | `hook` | 2 | Game thread |
| [Settings](./settings) | `swofty/mixin.hpp` | `options` | 3 | Flow thread |
| [Logging](./logging) | `swofty/mixin.hpp` | `log` | 1 | Any |
| [Events](./events) | `swofty/events.hpp` | `events` (stable) | 5 | Flow thread |
| [Overlay](./overlay) | `swofty/overlay.hpp` | `overlay` (experimental) | 5 | Render thread |
| [Telemetry](./telemetry) | `swofty/telemetry.hpp` | `telemetry` (experimental) | 5 | Flow thread |
| [Database read](./db) | `swofty/db.hpp` | `db` (stable) | 5 | Caller thread |
| [Storage](./storage) | `swofty/storage.hpp` | `storage` (stable) | 5 | Caller thread |

For screens and home tiles, use an addon operation rather than a `SwoftyApi` table; see [UI](./ui). You can find the merged engine-side contract for the version-5 tables in [the native-libraries design](/design/native-libraries).

## Headers

Include the header for the library you need. The [addon template](/reference/addons/template) ships these headers under `native/include/`.

| Header | Location | Contents |
|---|---|---|
| `swofty_api.h` | `packages/sdk/include/` | The C ABI: `SwoftyApi`, mixin records, setting values, the nested tables. MIT |
| `swofty/mixin.hpp` | `packages/sdk/include/swofty/` | The C++ layer: mixin macros, bindings, settings, `swofty::log`, `SWOFTY_ADDON` |
| `swofty/events.hpp` | `packages/sdk/include/swofty/` | Lifecycle events and the frame tick |
| `swofty/overlay.hpp` | `packages/sdk/include/swofty/` | The 2D overlay |
| `swofty/telemetry.hpp` | `packages/sdk/include/swofty/` | Live car and session reads |
| `swofty/db.hpp` | `packages/sdk/include/swofty/` | Game-database reads |
| `swofty/storage.hpp` | `packages/sdk/include/swofty/` | Addon storage |
| `swofty/assets.hpp` | `packages/sdk/include/swofty/` | Runtime asset loading; see [Asset API](/reference/addons/asset-api) |
| `swofty/game.hpp`, `swofty/game/*.hpp` | `native/include/swofty/` | The [Game API](/reference/game/): the engine's functions, structs and sites |

If you're writing Rust, the `swofty-sdk` crate (`packages/sdk`) mirrors the C ABI.

## Result codes

The version-5 tables (`events`, `overlay`, `telemetry`, `db`, `storage`) share the same result codes, so you can interpret their results the same way. Each wrapper exposes a `Result` enum; use `name(r)` to turn it into a string:

| `Result` | `SWOFTY_ASSET_*` | Meaning |
|---|---|---|
| `ok` | `OK` (0) | The call succeeded |
| `late` | `LATE` (1) | Too late to apply, or a cursor is exhausted |
| `unavailable` | `UNAVAILABLE` (2) | The loader is older than version 5, the feature is off, or the lane is not implemented |
| `invalid` | `INVALID` (3) | A bad argument |

Guard every call with `available()`. A table's `available()` returns non-zero only when its feature is on and the core implements it. If the loader offers a version below 5 or the table pointer is null, the wrapper returns `unavailable`.

## Threads

| Thread | What runs on it |
|---|---|
| Flow thread | `Vm_Update_FlowStateMachine` each frame. Lifecycle events, the `on_frame` tick and telemetry reads fire here |
| Render thread | The swapchain present path. `on_paint` callbacks fire here |
| Caller thread | `db` and `storage` calls run on the thread that calls them |

You can use subscriptions, paint callbacks, and `db` and `storage` calls from any thread. Your callbacks must return at once, without blocking the thread they run on.

## API versions

To see which version the running loader offers, read `SwoftyApi::version`. The header constant, `SWOFTY_API_VERSION` in `swofty_api.h`, is 5.

| Members | Offered by |
|---|---|
| `version`, `log`, `module_base`, `scan` | `version.dll`, as version 1 |
| `resolve`, `register_mixins`, `fields`, `setting`, `watch_setting`, `assets` | `swofty_core.dll`, as version 4 |
| `events`, `overlay`, `telemetry`, `db`, `storage` | `swofty_core.dll`, as version 5 |

The helpers you use determine which version you need: the mixin helpers in `mixin.hpp` need version 2, the settings helpers version 3, `assets.hpp` version 4, and the five table headers version 5.

## Capabilities

Declare the features your [engine plugin](/reference/addons/native-plugins#capabilities) uses in `capabilities` on its `operations.native` entry. Each capability needs the API version shown below:

| Capability | Library | API |
|---|---|---|
| `log` | [Logging](./logging) | 1 |
| `scan` | signature scan (`scan`) | 1 |
| `hook` | [Mixin system](./mixin) | 2 |
| `options` | [Settings](./settings) | 3 |
| `assets` | [Runtime asset loading](/reference/addons/asset-api) | 4 |
| `read-db` | [Database read](./db) | 5 |
| `on-frame` | [Events](./events) frame tick | 5 |
| `events` | [Events](./events) | 5 |
| `overlay` | [Overlay](./overlay) | 5 |
| `telemetry` | [Telemetry](./telemetry) | 5 |
| `storage` | [Storage](./storage) | 5 |
