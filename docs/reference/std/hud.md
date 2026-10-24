---
title: Race HUD
description: Show your text in the race HUD's own notification from native code with swofty/hud.hpp.
---

# Race HUD

`swofty::hud::toast(text, detail, seconds, icon)` shows your text in the race HUD's central notification, the one the game uses for DRS enabled, a fastest lap or a penalty. The game's UI draws it in the game's style, so it follows the HUD's layout, scale and language settings, and needs no overlay drawing. `swofty::hud::set(path, value)` writes one of the HUD's own data leaves, such as the lap counter's description.

| | |
|---|---|
| Header | `swofty/hud.hpp` |
| Namespace | `swofty::hud` |
| Table | `SwoftyHud` version 1, from the core's `SwoftyCoreHud` export; later fields are appended |
| Feature | `popup` (stable) |
| API version | 8, feature-detected |
| Thread | `toast`: any. The toast is posted on the flow thread |

## Calls

| Call | Returns |
|---|---|
| `available()` | `true` when the running core exports `SwoftyCoreHud` and found the game's message system on this build |
| `toast(text, detail = {}, seconds = 4, icon = {})` | `ok` once queued; `invalid` for empty `text` or an unknown `icon`; `unavailable` on an older core or when the message system did not resolve |
| `set(path, text)`, `set(path, bool)`, `set(path, int32_t)` | `ok` once queued; `invalid` for a path outside `ui_menu_data.hud_*` and `view_model_data.hud*`; `unavailable` as for `toast` |
| `name(Result)` | `"ok"`, `"unavailable"` and so on |

`text` is the main line and `detail` an optional second line. `seconds` is rounded to the nearest of 2, 3, 4, 5, 6, 8, 10, 12, 15, 20 and 30. Text is UTF-8 and each string is capped at 200 bytes. The queue holds at most 8 toasts (the oldest goes first); the HUD shows them one after another by priority, as it does its own notifications.

The HUD draws a toast only while it is on screen: during a session on track. A toast posted in the menus is not drawn.

`icon` is empty for the default icon or one of the game's race flag icons: `generic`, `diagonal`, `black`, `checkpoint`, `chequered`, `fast_lap`, `formation_lap`, `pp_icon`, `red`, `safety_car`, `virtual_safety_car`, `virtual_safety_car_green`, `flag_rival_won`.

## HUD data

`set` writes one leaf of the HUD's data on the flow thread. A later write to the same path replaces a queued one. The game also writes many of these leaves, so a value can be overwritten by the game's own update; write it again when it matters. Leaves the race HUD reads from `ui_menu_data`:

| Leaf | Type | Shown by |
|---|---|---|
| `ui_menu_data.hud_lap_description_string` | text | the lap counter, while `hud_lap_number_visible` is false |
| `ui_menu_data.hud_lap_description_loc_tag` | bool | true: the description is a `lng_*` key; false: literal text |
| `ui_menu_data.hud_lap_number_visible` | bool | false shows the description instead of the lap number |
| `ui_menu_data.hud_total_lap_count_visible` | bool | the lap total |

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

- `SWOFTY_TOAST_TEST=<seconds>` makes the core post a test toast (`SWOFTY TOAST <n>`) every that many seconds, cycling through the icons, with a `[hud]` log line each time.
- `SWOFTY_HUD_SET_TEST=<text>` writes `<text>` to the lap counter description and hides the lap number and lap total, every 5 s through `set`. The lap counter widget is not on screen in a benchmark race, so the writes show only in the `[hud] set applied` log lines there.
- `SWOFTY_HUD_DEV_SET=b:<path>=1;s:<path>=<text>;i:<path>=<n>` sets any registry leaf every 5 s. It is a development switch and skips the path check.
