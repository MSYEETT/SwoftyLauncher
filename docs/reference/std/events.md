---
title: Events
description: Subscribe to F1 25 lifecycle events and run a callback every rendered frame with swofty/events.hpp.
---

# Events

When you need to react to a lifecycle event (session start, a front-end state change, a career loading, lap, flag, results), subscribe a callback to its name. You can also subscribe to every frame for a per-frame tick. The core calls your callback on the flow thread.

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
| `available()` | `true` when the core implements events and its setup has not found the frame tick missing on this build; `true` from `SwoftyAddonInit` on, also before the setup runs |
| `subscribe(event, fn, user = nullptr, token = nullptr)` | `ok` and fills `token`; `invalid` for an unknown event name or a null `fn` |
| `on_frame(fn, user = nullptr, token = nullptr)` | `ok` and fills `token`; `unavailable` once the setup found no frame tick on this build |
| `unsubscribe(token)` | `ok`, or `invalid` for an unknown token |

Give `fn` for `subscribe` the signature `void(const SwoftyEvent *ev, void *user)`. For `fn` with `on_frame`, use `void(double dt, void *user)`: `dt` tells you how many seconds have passed since the previous frame.

`on_frame` callbacks run on the first flow-thread update after each rendered frame, and at least every 33 ms when no frame is rendered, as on loading screens. Only one thread runs them at a time.

Registration is accepted at any time from `SWOFTY_ON_LOAD` on, and already in `SwoftyAddonInit`. When the addons load in one batch (no early stage), every `SWOFTY_ON_LOAD` runs before the core's events setup: a `subscribe` or `on_frame` made then returns `ok`, the core logs it as `queued until the events setup` and arms it when the setup completes (`[events] N registration(s) queued before setup are armed`). If the setup finds no frame tick, the core logs each queued `on_frame` that will not fire.

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
| `SWOFTY_EVENT_FLOW_STATE` | `flow.state` |
| `SWOFTY_EVENT_CAREER_LOADED` | `career.loaded` |
| `SWOFTY_EVENT_CAREER_UNLOADED` | `career.unloaded` |
| `SWOFTY_EVENT_CAREER_HUB` | `career.hub` |
| `SWOFTY_EVENT_CAREER_STARTING` | `career.starting` |

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

## Flow and career events

These five come from the [career library](./career) (API version 6, feature `career`; `career.starting` API version 7). The payload pointer is valid only during the callback; copy what you keep. To ask for the current state instead of waiting for an event, use `swofty::career::state()` and `swofty::career::flow()`.

| Event | `a` | `b` | `data` | When |
|---|---|---|---|---|
| `flow.state` | new status | old status | `const SwoftyFlowState *` | Every front-end flow state changes status. Statuses are `SWOFTY_FLOW_NONE` (0), `ENTERING` (1), `UPDATING` (2), `EXITING` (3). `data->name` is the state name (`StateTeamHQUpdate`), or `#` and the hash in hex when the core cannot read it; `data->hash` is the djb2 of the name |
| `career.loaded` | mode | 0 | `const SwoftyCareerState *` | A career is loaded and its mode is known. Mode is `SWOFTY_CAREER_MY_TEAM` (1) or `SWOFTY_CAREER_DRIVER` (2); `data->key` is the career key once the player's entrant is known |
| `career.unloaded` | the mode it had | 0 | `const SwoftyCareerState *` | The career is gone, or another career replaced it (then `career.loaded` follows) |
| `career.hub` | 1 entered, 0 left | mode | `const SwoftyCareerState *` | Team HQ is idle for the mode: `StateTeamHQUpdate` is updating with no time advance, session, end of season or season break running. Fires in My Team and Driver Career alike |
| `career.starting` | mode | `SWOFTY_CAREER_START_*` | `const SwoftyCareerStart *` | The player chose to start or load a career in that mode on the Career menu, before the game builds it. `data->blocked` is 1 when a [block](./career#blocking-a-career-mode) cancelled it |

`career.hub` left always comes before `career.unloaded`. `flow.state` runs on the thread that changes the state, which is the flow thread.

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

Name your plugin in `operations.native`:

```json [swofty.json]
"operations": {
  "native": [{ "dll": "native/my_plugin.dll" }]
}
```

## Availability

The core implements `on_frame`, `session.start`, `race.start` and, from core 0.2.11 (API version 6), the four flow and career events above, and from core 0.2.12 (API version 7) `career.starting`. The other names are accepted but do not fire yet. A build where a hook does not resolve leaves only that event silent; the core log says which (`[events] setup:` and `[career] setup:` lines). You can check the engine status in [the native-libraries design](/design/native-libraries#_1-events-frame-scheduler).
