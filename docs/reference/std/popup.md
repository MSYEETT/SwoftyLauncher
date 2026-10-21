---
title: Popup
description: Show the game's own message popup with your title and body from native code with swofty/popup.hpp.
---

# Popup

`swofty::popup::show(title, body)` raises the game's own message popup with your text and one OK button. It is the persistent multichoice popup the game uses for its errors and confirmations, drawn over the current screen in the game's style.

| | |
|---|---|
| Header | `swofty/popup.hpp` |
| Namespace | `swofty::popup` |
| Table | `SwoftyApi::popup` (`SwoftyPopup`) |
| Feature | `popup` — stable, always on |
| API version | 7 (core 0.2.12) |
| Thread | `show`: any. The popup opens on the flow thread |

Every call that returns a `swofty::popup::Result` uses the shared [result codes](./#result-codes).

## Calls

| Call | Returns |
|---|---|
| `available()` | `true` when the core found the game's message system on this build |
| `show(title, body)` | `ok` once the popup is queued; `invalid` when title and body are both empty; `unavailable` on a core older than API 7 or when the message system did not resolve |
| `name(Result)` | `"ok"`, `"unavailable"` and so on |

Text is UTF-8; `\n` breaks a line. The core caps each string at 4000 bytes. A popup with the same title and body as one still waiting in the queue is not queued twice, and the queue holds at most 8 popups (the oldest goes first).

The popup closes when the player presses OK. Nothing reports the press back.

## How it works

The game raises every popup the same way: it constructs a message by its id in the message data (`Ui_Message_Construct`), sets the message's user attributes through the returned handle (`Ui_Message_SetAttribute`) and drops the handle (`Ui_Message_ReleaseHandle`). The message system queues the message and the persistent screen draws it.

`show` queues the request. On the next flow tick the core constructs a `swofty_message`, sets its `title` and `message` attributes to your text and releases the handle. The launcher's UI build adds `swofty_message` to `onlineservices_messages.uimsg`: title and body are `lng_value_tag` with a literal (`localise: false`) token that reads the `title` and `message` attributes, and one option, `lng_msg_ok`. The core learns the message system's address from the first message the game constructs after boot (offline, the online-services error); a popup queued before that waits for it.

Without `swofty_message` in the game's data, the core uses the stock `beta_motd` message, whose title is the fixed `lng_message_of_the_day_title`; your title then leads the body. The core log says which message it used:

```
[popup] shown for my-addon as 'swofty_message': "My title"
```

## Example

```cpp [native/src/my_plugin.cpp]
#include <swofty/popup.hpp>

SWOFTY_ON_LOAD
{
    auto r = swofty::popup::show("My addon", "Loaded. Press OK to continue.");
    if (r != swofty::popup::Result::ok)
        swofty::log("[my-addon] no popup: %s", swofty::popup::name(r));
}

SWOFTY_ADDON()
```

## Availability

The core finds the message functions through one signature over the game's `beta_motd` poster and one over the handle release. On a build where either does not match, `available()` returns `false` and `show` returns `unavailable`; the core log's `[popup] setup:` line says what resolved.

Requires `swofty.core` 0.2.12 or later:

```json [swofty.json]
"requires": [{ "id": "swofty.core", "version": ">=0.2.12" }]
```
