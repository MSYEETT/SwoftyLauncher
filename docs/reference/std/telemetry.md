---
title: Telemetry
description: Read live F1 25 car and session state, or open the shared-memory feed, with swofty/telemetry.hpp.
---

# Telemetry

You can read live car and session state through typed structs, or turn on a shared-memory feed for external tools. Each read gives you the most recent frame.

| | |
|---|---|
| Header | `swofty/telemetry.hpp` |
| Namespace | `swofty::telemetry` |
| Table | `SwoftyApi::telemetry` (`SwoftyTelemetry`) |
| Feature | `telemetry` — experimental, off by default |
| API version | 5 |
| Thread | Flow thread |

Every call returns `swofty::telemetry::Result` (`ok`, `late`, `unavailable`, `invalid`). To see what each means, read the [result codes](./#result-codes).

## Calls

| Call | Returns |
|---|---|
| `available()` | `true` when the feature is on, implemented, and a session is live |
| `session(out)` | `ok` and fills `SwoftySessionState`; `unavailable` outside a session |
| `car(index, out)` | `ok` and fills `SwoftyCarState`; `invalid` for an out-of-range index |
| `participant(index, out)` | `ok` and fills `SwoftyParticipant` |
| `shared_memory(&name)` | turns the shared feed on and writes the mapping name |

To read a particular participant, pass their participant index as `index`. For your player's index, use `SwoftySessionState::player_index`. The structs carry `version` = `SWOFTY_TELEMETRY_VERSION` (1). Each ends in a `reserved` block so fields can be added without changing the struct's size.

- `SwoftyCarState` — speed (m/s), rpm, gear, throttle/brake/clutch/steer, per-wheel tyre temp, tyre wear and brake temp, fuel, position, lap, sector, lap and sector times, lap distance, world position and velocity.
- `SwoftySessionState` — session type, track, flag, weather, total laps, participant count, player index, session time, time left, track and air temperature.
- `SwoftyParticipant` — name, team, position, lap, lap distance, best lap time.

## Example

To log your player's speed every frame, read the session first: it gives you the index to use for the car read.

```cpp [native/src/my_plugin.cpp]
#include <swofty/events.hpp>
#include <swofty/telemetry.hpp>

namespace tel = swofty::telemetry;

static void frame(double, void *)
{
    if (!tel::available())
        return;
    tel::SessionState s{};
    tel::CarState car{};
    if (tel::session(&s) == tel::Result::ok && tel::car(s.player_index, &car) == tel::Result::ok)
        swofty::log("[my-addon] %.0f km/h, gear %d", car.speed * 3.6f, car.gear);
}

SWOFTY_ON_LOAD
{
    swofty::events::on_frame(frame);
}

SWOFTY_ADDON()
```

Declare the capability in your plugin's manifest entry:

```json [swofty.json]
"operations": {
  "native": [{ "dll": "native/my_plugin.dll", "capabilities": ["telemetry", "on-frame"] }]
}
```

## Shared-memory feed

To turn on the feed for external tools, call `shared_memory(&name)`. It opens a named file mapping (`Local\SwoftyTelemetry`) and writes its name. Inside the mapping is a `SwoftyTelemetryFeed`:

| Field | Type | Notes |
|---|---|---|
| `version` | `uint32` | `SWOFTY_TELEMETRY_VERSION` |
| `size` | `uint32` | `sizeof(SwoftyTelemetryFeed)` |
| `seq` | `uint64` | seqlock; odd while a frame is being written |
| `frame` | `uint64` | frame counter |
| `session` | `SwoftySessionState` | current session |
| `cars` | `SwoftyCarState[24]` | per participant |
| `participants` | `SwoftyParticipant[24]` | per participant |

Use the sequence counter to check whether a frame was written while you were copying the data:

1. Check that `seq` is even.
2. Copy the fields you need.
3. Check that `seq` hasn't changed. If it has, retry.

The name stays valid for the lifetime of the process.

## Availability

`telemetry` is experimental and stays off until you list `telemetry` under `"features"` in `swofty/assets.json`; see [feature gates](/design/native-libraries#feature-gates). The telemetry engine lane hasn't landed yet, so every call on current builds returns `unavailable`. See [the native-libraries design](/design/native-libraries#_3-telemetry).
