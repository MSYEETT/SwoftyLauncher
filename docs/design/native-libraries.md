# Native standard libraries

This page is for contributors. It is the contract for five native standard-library additions that
addon plugins reach through `SwoftyApi`. The SDK surface, feature gates and stubs ship now; the
engine-side implementation is built per library in its own lane. A line says where a library is a
stub.

The additions raise `SWOFTY_API_VERSION` to 5. Each library is a nested table on `SwoftyApi`
(`events`, `overlay`, `telemetry`, `db`, `storage`), reached from a C++ wrapper header
(`swofty/events.hpp`, `swofty/overlay.hpp`, `swofty/telemetry.hpp`, `swofty/db.hpp`,
`swofty/storage.hpp`). Every call returns one of the shared result codes: `ok`, `late`,
`unavailable` (older core, or the feature is off, or not yet implemented), `invalid` (bad
argument). A wrapper returns `unavailable` when the loader offers a version below 5 or the table
pointer is null. Each table's `available()` returns non-zero only when its feature is on and the
core implements it.

Engine addresses below are VAs for the catalogued `F1_25.exe` build; they are in `re/db/functions.tsv`.
Confirm a hook by decompiling before relying on it.

## Feature gates

The feature table is in `native/src/core/stage.cpp` (`FEATURES`); `native/build.sh` writes it to
`swofty/addons/core/features.json`. A stable feature is always on; an experimental feature is on only
when `"features"` in `swofty/assets.json` lists it.

| Feature | Default | Library |
|---|---|---|
| `events` | stable | Events + frame scheduler |
| `storage` | stable | Storage |
| `db` | stable | Database read (shares the gate with database write) |
| `overlay` | experimental | Overlay / draw |
| `telemetry` | experimental | Telemetry |

## Threads

| Thread | What runs on it |
|---|---|
| Flow thread | The thread that calls `Vm_Update_FlowStateMachine` each frame. Lifecycle events, the `on_frame` tick, and all telemetry reads fire here. |
| Render thread | The swapchain present path. `on_paint` callbacks fire here. |
| Caller thread | `db` reads and `storage` calls run on the thread that calls them. Returned `db` string pointers are valid until the next `db` call on that thread. |

Subscriptions, paint callbacks and `storage` and `db` calls are valid from any thread. A callback
must return quickly and must not block the thread it runs on.

## 1. Events + frame scheduler

`swofty/events.hpp`, table `SwoftyEvents`, feature `events` (stable).

| Call | Result |
|---|---|
| `available()` | non-zero when the feature is on and implemented |
| `subscribe(event, fn, user, token)` | `ok` and fills `token`; `invalid` for an unknown event name or null `fn` |
| `on_frame(fn, user, token)` | `ok` and fills `token` |
| `unsubscribe(token)` | `ok`, or `invalid` for an unknown token |

Event names are strings (`SWOFTY_EVENT_*`): `session.start`, `session.end`, `race.start`, `lap`,
`flag`, `state`, `screen`, `pause`, `resume`, `results`. A subscriber receives a `SwoftyEvent`
(`name`, `frame`, `time` seconds since attach, `a`/`b` event integers, `data` pointer). `on_frame`
receives the seconds since the previous frame. Callbacks run on the flow thread and last until
`unsubscribe`; the core drops an addon's subscriptions if the addon's init returns failure.

Accept point: `subscribe`, `on_frame` and `unsubscribe` are accepted at any time. Callbacks fire
once a session or the flow machine is running.

Hooks: the per-frame tick is `Vm_Update_FlowStateMachine` 0x144423ce0. Lifecycle events come from
`Game_GameModule_OnSessionEnter` 0x14080c300 (`session.start`), `Sess_Weekend_Start` 0x140af0460,
`Game_ChampionshipFormationLapComponent_OnRaceStarted` 0x140b808a0 (`race.start`),
`Game_SessionStateFinaliseResultsSync_OnResultsFinalised` 0x1409d2b20 (`results`), and the
`*_OnStateUpdate` functions (`Game_StateCareerDataSync_OnStateUpdate` 0x1436b0700,
`Game_SessionStateFinaliseResultsSync_OnStateUpdate` 0x142d5c9b0) for `state`, `screen` and
`session.end`. The `lap`, `flag`, `pause` and `resume` sources are found in the events lane.

Status: stub. `native/src/core/events.cpp` returns `unavailable` from every call.

## 2. Overlay / draw

`swofty/overlay.hpp`, table `SwoftyOverlay`, feature `overlay` (experimental).

| Call | Result |
|---|---|
| `available()` | non-zero when the feature is on and implemented |
| `on_paint(fn, user, token)` | `ok` and fills `token` |
| `remove(token)` | `ok`, or `invalid` for an unknown token |
| `size(paint, w, h)` | drawable size in pixels |
| `rect` / `rect_outline` / `line` / `text` | draw into the current frame |
| `measure(paint, size, utf8, w, h)` | text bounds |
| `load_image(id, file)` | `ok`, or `invalid` for a bad id or missing file |
| `image(paint, id, ...)` | `ok`, or `invalid` for an unknown id |

Immediate mode: register a paint callback with `on_paint`; the core invokes it once per presented
frame with a `SwoftyPaint` handle and issues the draw calls made through that handle. Coordinates
are framebuffer pixels, origin top-left; query `size` each frame. Colours are `0xRRGGBBAA`. Text
uses the built-in font at a pixel size. Textures load once from the addon folder (`load_image`,
outside paint) and draw by id, multiplied by a tint (`0xffffffff` for none).

Accept point: `on_paint`, `remove` and `load_image` are accepted at any time. Draw calls are valid
only inside a paint callback, with that callback's `paint` handle.

Hooks: the lane finds and hooks the Direct3D 12 swapchain present path (`d3d12core.dll`); there is
no present hook in the knowledge base yet.

Status: stub. `native/src/core/hud.cpp` returns `unavailable` and the draw calls do nothing.

## 3. Telemetry

`swofty/telemetry.hpp`, table `SwoftyTelemetry`, feature `telemetry` (experimental).

| Call | Result |
|---|---|
| `available()` | non-zero when the feature is on, implemented, and a session is live |
| `session(out)` | `ok` and fills `SwoftySessionState`; `unavailable` outside a session |
| `car(index, out)` | `ok` and fills `SwoftyCarState`; `invalid` for an out-of-range index |
| `participant(index, out)` | `ok` and fills `SwoftyParticipant` |
| `shared_memory(name)` | turns the feed on and names the mapping |

Structs are versioned (`version` = `SWOFTY_TELEMETRY_VERSION`, currently 1). `SwoftyCarState` holds
speed (m/s), rpm, gear, throttle/brake/clutch/steer, per-wheel tyre temp/wear and brake temp, fuel,
position, lap, sector, lap and sector times, lap distance, world position and velocity.
`SwoftySessionState` holds session type, track, flag, weather, lap count, participant count, player
index, session time and temperatures. `SwoftyParticipant` holds name, team, position, lap and best
lap. Each struct ends in a `reserved` block so fields can be added without changing the size.

Shared feed: `shared_memory` opens a named file mapping (`Local\SwoftyTelemetry`) laid out as
`SwoftyTelemetryFeed`:

| Field | Type | Notes |
|---|---|---|
| `version` | `uint32` | `SWOFTY_TELEMETRY_VERSION` |
| `size` | `uint32` | `sizeof(SwoftyTelemetryFeed)` |
| `seq` | `uint64` | seqlock; odd while a frame is being written |
| `frame` | `uint64` | frame counter |
| `session` | `SwoftySessionState` | current session |
| `cars` | `SwoftyCarState[24]` | per participant |
| `participants` | `SwoftyParticipant[24]` | per participant |

A reader checks that `seq` is even, copies, then checks that `seq` did not change; it retries
otherwise.

Accept point: reads return the most recent frame's values and run on the flow thread.
`shared_memory` is accepted at any time and stays on for the lifetime of the process.

Hooks: session and participant data come from the session witnesses `Sess_Veh_UpdateWheelMap`
0x142e90560, `Sess_ParticipantVec_PushBack` 0x140af2ab0, `Sess_Select_Entrants` 0x1424a2350,
`Sess_Startup_VehicleAiLoop` 0x142505f30 and `Game_GameModule_OnSessionEnter` 0x14080c300. The
per-car physics/telemetry struct is not catalogued yet; the lane finds it.

Status: stub. `native/src/core/telemetry.cpp` returns `unavailable` from every call.

## 4. Database read

`swofty/db.hpp`, table `SwoftyDb`, feature `db` (stable, shared with the database write operations).

| Call | Result |
|---|---|
| `available()` | non-zero when the database has loaded |
| `get(key, out)` / `get(hash, out)` | whole typed value by dotted key or pre-hashed key |
| `int_at` / `float_at` / `string_at` / `bool_at` | one field of a known type |
| `open(table, cursor)` | positions a cursor at the first row |
| `row(table, label, cursor)` | positions a cursor at a labelled row |
| `step(cursor)` | advances; `late` when the table is exhausted |
| `cell(cursor, field, out)` | reads one field of the cursor's current row |
| `row_name(cursor)` | the current row's label, or null |

Keys are the dotted path hashed with fnv1a64, the same keys the write operations use; see the
[game database reference](/reference/gamedb). A `SwoftyDbValue` carries a `tag` (`SWOFTY_DB_*`) and
the integer, float or string. A `SwoftyDbCursor` is caller-owned. String pointers are valid until
the next `db` call on the same thread.

Accept point: reads are available once the database has loaded; they return `unavailable` before
then.

Hooks: `Gdb_Lookup_Key` 0x1445a22f0 (fnv1a64 key lookup), `Gdb_Init_TableCursor` 0x1445a25c0,
`Gdb_Get_RowByIndex` 0x1445a28f0, `Gdb_Find_RowByLabel` 0x1445a2bd0, `Gdb_Get_FieldInt`
0x1445a2cf0 and `Gdb_Get_RowName` 0x1445a3900, over the database `Gdb_Load_GameDatabase`
0x1445a0350 resolves. `native/src/core/gamedb.cpp` already resolves this database for writes; the
read path reuses the pointer.

Status: stub. `native/src/core/dbread.cpp` returns `unavailable` from every call.

## 5. Storage

`swofty/storage.hpp`, table `SwoftyStorage`, feature `storage` (stable).

| Call | Result |
|---|---|
| `available()` | non-zero when the feature is on and implemented |
| `read(path, buf, cap, len)` | `ok` and fills `buf`; `invalid` when the file exceeds `cap` |
| `write(path, data, len)` | atomic replace |
| `append(path, data, len)` | append, creating the file when absent |
| `remove(path)` | delete |
| `size(path, out)` | file size in bytes |
| `list(dir, buf, cap, len)` | newline-separated names into `buf` |

An addon reads and writes under `swofty/addons/<id>/`. Paths are relative to that folder; the core
rejects an absolute path, a drive letter and any path that climbs above the folder. The calling DLL
is identified the way the assets API identifies it (`settings_addon` in
`native/src/core/settings.cpp`), so an addon reaches only its own folder. Writes go to a temp file
and rename.

Accept point: calls are accepted at any time.

`write` and `append` create any missing parent folders inside the addon folder. `list` names a
sub-folder with a trailing `/`. The core rejects a bad or escaping path with `invalid`, a missing
file for `read`, `size` or `remove` with `invalid`, and a file or listing larger than the buffer
with `invalid` after setting `*len` to the size needed.

Status: implemented. `native/src/core/storage.cpp` identifies the caller with `settings_addon` and
reads and writes under `swofty/addons/<id>/`. The [storage reference](/reference/std/storage) is the
addon-facing page.

## Files

| Path | Role |
|---|---|
| `packages/sdk/include/swofty_api.h` | C ABI: version 5, the five nested tables, shared structs |
| `packages/sdk/include/swofty/{events,overlay,telemetry,db,storage}.hpp` | C++ wrappers |
| `packages/sdk/src/lib.rs` | Rust mirror and the layout test |
| `templates/addon/native/include/` | the copy addons compile against |
| `native/src/core/{events,hud,telemetry,dbread,storage}.cpp` | per-library stubs wired into `manager.cpp` |
| `native/src/core/stage.cpp` | the feature table |

The wrapper headers and the mixin, settings and logging helpers are the native half of the
[standard library](/reference/std/).
