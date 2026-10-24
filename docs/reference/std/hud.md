---
title: Race HUD
description: Show your text in the race HUD's own notification from native code with swofty/hud.hpp.
---

# Race HUD

`swofty::hud::toast(text, detail, seconds)` shows your text in the race HUD's central notification, the one the game uses for DRS enabled, a fastest lap or a penalty. The game's UI draws it in the game's style, so it follows the HUD's layout, scale and language settings, and needs no overlay drawing.

| | |
|---|---|
| Header | `swofty/hud.hpp` |
| Namespace | `swofty::hud` |
| Table | `SwoftyHud`, from the core's `SwoftyCoreHud` export |
| Feature | `popup` (stable) |
| API version | 8, feature-detected |
| Thread | `toast`: any. The toast is posted on the flow thread |

## Calls

| Call | Returns |
|---|---|
| `available()` | `true` when the running core exports `SwoftyCoreHud` and found the game's message system on this build |
| `toast(text, detail = {}, seconds = 4)` | `ok` once queued; `invalid` for empty `text`; `unavailable` on an older core or when the message system did not resolve |
| `name(Result)` | `"ok"`, `"unavailable"` and so on |

`text` is the main line and `detail` an optional second line. `seconds` is rounded to the nearest of 2, 3, 4, 5, 6, 8, 10, 12, 15, 20 and 30. Text is UTF-8 and each string is capped at 200 bytes. The queue holds at most 8 toasts (the oldest goes first); the HUD shows them one after another by priority, as it does its own notifications.

The HUD draws a toast only while it is on screen: during a session on track. A toast posted in the menus is not drawn.

## How it works

The race HUD's central notification (`hud_notifications_dynamic` in `templates/hud_dynamic_templates.ui`) draws the message at the front of the game's `hud_main_queue`, the same message system the [popup](./popup) uses. Its labels read the message's `message` and `sub_message` attributes and honour each attribute's `localise` flag, and its timer reads `msg_duration`.

The launcher's UI build adds `swofty_toast_<seconds>` to `message_data/hud_messages.uimsg` (in `flow_persistent.erp`), one per duration: type `hud_main`, queue `hud_main_queue`, priority 2, literal (`localise: false`) `message` and `sub_message`, the fixed `msg_duration`, the generic race flag and `enabled_in_spectator`. `toast` queues the request; on the next flow tick the core constructs the message for the rounded duration, sets `message` and `sub_message` to your text and releases the handle. The core finds the message system from the first message the game constructs, or, when the game has constructed none yet (a benchmark race starts without one), from the UI system. The core log says what it posted:

```
[popup] message system 00000004A9D4A330 from the UI system (no message constructed yet)
[hud] toast shown for my-addon as 'swofty_toast_4': "Pit lane open"
```

Without `swofty_toast_*` in the game's data (a launcher older than this core), the toast is dropped with one log line.

## Example

```cpp [native/src/my_plugin.cpp]
#include <swofty/hud.hpp>

void on_pit_entry()
{
    auto r = swofty::hud::toast("Pit limiter on", "Manual pit lane", 3);
    if (r != swofty::hud::Result::ok)
        swofty::log("[my-addon] no toast: %s", swofty::hud::name(r));
}
```

## Testing

`SWOFTY_TOAST_TEST=<seconds>` in the game's environment makes the core post a test toast (`SWOFTY TOAST <n>`) every that many seconds, with a `[hud]` log line each time.
