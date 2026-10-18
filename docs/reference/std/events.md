---
title: Events
description: Subscribe to F1 25 lifecycle events and run a callback every rendered frame with swofty/events.hpp.
---

# Events

When you need to react to a lifecycle event (session start, lap, flag, results), subscribe a callback to its name. You can also subscribe to every frame for a per-frame tick. The core calls your callback on the flow thread.

| | |
|---|---|
| Header | `swofty/events.hpp` |
| Namespace | `swofty::events` |
| Table | `SwoftyApi::events` (`SwoftyEvents`) |
| Feature | `events` — stable, always on |
| API version | 5 |
| Thread | Flow thread (`Vm_Update_FlowStateMachine` each frame) |

Every call returns `swofty::events::Result` (`ok`, `late`, `unavailable`, `invalid`). To turn a result into a string, use `swofty::events::name(r)`; for what each one means, see the [result codes](./#result-codes).

## Calls

| Call | Returns |
|---|---|
| `available()` | `true` once the core implements events |
| `subscribe(event, fn, user = nullptr, token = nullptr)` | `ok` and fills `token`; `invalid` for an unknown event name or a null `fn` |
| `on_frame(fn, user = nullptr, token = nullptr)` | `ok` and fills `token` |
| `unsubscribe(token)` | `ok`, or `invalid` for an unknown token |

Give `fn` for `subscribe` the signature `void(const SwoftyEvent *ev, void *user)`. For `fn` with `on_frame`, use `void(double dt, void *user)`: `dt` tells you how many seconds have passed since the previous frame.

Your subscription lasts until you call `unsubscribe`. If your addon's `SwoftyAddonInit` returns failure, the core drops its subscriptions. Callbacks run on the flow thread, so yours must return at once and never block.

## Event names

Choose the event you want to subscribe to with a `SWOFTY_EVENT_*` constant:

| Constant | String |
|---|---|
| `SWOFTY_EVENT_SESSION_START` | `session.start` |
| `SWOFTY_EVENT_SESSION_END` | `session.end` |
| `SWOFTY_EVENT_RACE_START` | `race.start` |
| `SWOFTY_EVENT_LAP` | `lap` |
| `SWOFTY_EVENT_FLAG` | `flag` |
| `SWOFTY_EVENT_STATE` | `state` |
| `SWOFTY_EVENT_SCREEN` | `screen` |
| `SWOFTY_EVENT_PAUSE` | `pause` |
| `SWOFTY_EVENT_RESUME` | `resume` |
| `SWOFTY_EVENT_RESULTS` | `results` |

Your callback receives the event details in a `SwoftyEvent`:

```c
typedef struct SwoftyEvent {
    const char *name;   // the event name
    uint64_t frame;     // frame counter at the event
    double time;        // seconds since the subscription attached
    int64_t a, b;       // event-specific integers
    const void *data;   // event-specific pointer, or null
} SwoftyEvent;
```

## Example

To log every lap and the frame rate, register a callback for each:

```cpp [native/src/my_plugin.cpp]
#include <swofty/events.hpp>

static void on_lap(const SwoftyEvent *ev, void *)
{
    swofty::log("[my-addon] lap on frame %llu, %.1f s in", (unsigned long long)ev->frame, ev->time);
}

static void on_frame(double dt, void *)
{
    swofty::log("[my-addon] %.0f fps", dt > 0 ? 1.0 / dt : 0.0);
}

SWOFTY_ON_LOAD
{
    if (swofty::events::available()) {
        swofty::events::subscribe(SWOFTY_EVENT_LAP, on_lap);
        swofty::events::on_frame(on_frame);
    }
}

SWOFTY_ADDON()
```

Declare the capability on your plugin's manifest entry:

```json [swofty.json]
"operations": {
  "native": [{ "dll": "native/my_plugin.dll", "capabilities": ["log", "events", "on-frame"] }]
}
```

## Availability

The engine doesn't implement events yet. On current builds, every call returns `unavailable` and `available()` returns `false`, so guard every call with `available()`. You can check the engine status in [the native-libraries design](/design/native-libraries#_1-events-frame-scheduler).
