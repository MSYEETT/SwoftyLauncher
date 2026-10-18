---
title: Events
description: Subscribe to F1 25 lifecycle events and a per-frame tick from a native plugin with swofty/events.hpp or the C ABI.
---

# Events

A native plugin runs code on named lifecycle events and once per rendered frame. The core installs the hooks on F1 25's flow-state machine and session entry points and calls the registered handlers on the game's flow thread, so a handler must return quickly and may be re-entered across frames.

Subscribe at any time. A subscription lasts until `unsubscribe`, and the core drops an addon's subscriptions if its init returns failure. The library needs API version 5 and the `events` or `on-frame` [capability](/reference/addons/native-plugins). It is always on.

## Events

| Name | `SWOFTY_EVENT_*` | Fires |
|---|---|---|
| `session.start` | `SESSION_START` | A session is entered |
| `race.start` | `RACE_START` | A race starts |
| `session.end` | `SESSION_END` | A session ends |
| `lap` | `LAP` | A lap is completed |
| `flag` | `FLAG` | A flag changes |
| `state` | `STATE` | The flow state changes |
| `screen` | `SCREEN` | A menu screen opens |
| `pause` / `resume` | `PAUSE` / `RESUME` | The session is paused or resumed |
| `results` | `RESULTS` | Results are finalised |

A handler receives a `SwoftyEvent`: the event `name`, the `frame` counter, the `time` in seconds since the library attached, two event integers `a` and `b`, and a `data` pointer. The frame tick receives the seconds elapsed since the previous frame.

`session.start`, `race.start` and the frame tick are live. The remaining lifecycle events are being wired up as their engine sources are confirmed.

## C++

```cpp
#include <swofty/events.hpp>
namespace events = swofty::events;
```

| Call | Returns |
|---|---|
| `available()` | `true` when the loader is version 5 or later, the core implements events, and the frame-tick hook is installed |
| `subscribe(event, fn, user, token)` | Registers `fn` for a named event. Fills `*token` |
| `on_frame(fn, user, token)` | Registers `fn` for the per-frame tick. Fills `*token` |
| `unsubscribe(token)` | Drops the subscription with that token |

Each call returns an `events::Result`: `ok`, `unavailable` (the loader is older than version 5, or the core does not implement events) or `invalid`. `events::name(result)` gives the name. `subscribe` is `invalid` for an unknown event name or a null callback; `unsubscribe` is `invalid` for an unknown token.

```cpp
void on_lap(double dt, void *)
{
    swofty::log("[sample] frame %f s", dt);
}

bool init()
{
    events::on_frame(on_lap);
    events::subscribe(SWOFTY_EVENT_SESSION_START,
                      [](const SwoftyEvent *ev, void *) { swofty::log("[sample] %s", ev->name); });
    return true;
}
```

## C ABI

```c
typedef struct SwoftyEvent {
    const char *name;
    uint64_t frame;
    double time;
    int64_t a, b;
    const void *data;
} SwoftyEvent;

typedef void (*SwoftyEventFn)(const SwoftyEvent *ev, void *user);
typedef void (*SwoftyFrameFn)(double dt, void *user);

typedef struct SwoftyEvents {
    int (*available)(void);
    int (*subscribe)(const char *event, SwoftyEventFn fn, void *user, uint32_t *token);
    int (*on_frame)(SwoftyFrameFn fn, void *user, uint32_t *token);
    int (*unsubscribe)(uint32_t token);
} SwoftyEvents;
```

Reach the table through `api->events` when `api->version >= 5`. Each function returns a `SWOFTY_ASSET_*` code: `SWOFTY_ASSET_OK`, `SWOFTY_ASSET_UNAVAILABLE` or `SWOFTY_ASSET_INVALID`.

## Logging

The core logs every subscription to `loader.log`, with the addon, the call and the result:

```
[events] on_frame by sample.my-addon -> ok token 1
[events] subscribe session.start by sample.my-addon -> ok token 2
```
