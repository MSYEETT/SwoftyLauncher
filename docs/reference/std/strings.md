---
title: Strings
description: Change the text of lng_* keys while the game runs, for every language or one, with swofty/lang.hpp.
---

# Strings

Use string overrides to change what a `lng_*` key shows while the game runs: a name in one career, a label while a feature is on. They differ from `lang` operations and `lang_set` in the [asset API](/reference/addons/asset-api), which are built into the game's language data while it loads and can't change afterwards.

| | |
|---|---|
| Header | `swofty/lang.hpp` |
| Namespace | `swofty::lang` |
| Table | `SwoftyApi::lang` (`SwoftyLang`) |
| Feature | `lang` — stable |
| API version | 8 |
| Thread | `set`, `clear`, `clear_all`: any thread. `refresh`: the game thread |

Check the `swofty::lang::Result` each call returns: `ok`, `unavailable` or `invalid`. The [result codes](./#result-codes) explain what each means. A core older than API v8 returns `unavailable` from every call.

## Calls

| Call | Does |
|---|---|
| `available()` | `true` when the core implements overrides and found the game's string lookup |
| `set(key, text, language = "*")` | makes `key` show `text`; `"*"` applies to every language, a language code (`eng`, `fre`, ...) only while it is the game's language and wins over `"*"` |
| `clear(key, language = "*")` | drops the override for that language, or every override of `key` with `"*"` |
| `clear_all()` | drops every override, of every addon |
| `refresh()` | sends the current language to the game's language listeners, the notification a language change sends, so labels already on screen read their text again |

`text` is UTF-8 and is copied. An override also works for a key the game's tables lack.

## How it applies

Every `lng_*` lookup goes through the language set's find (`Loc_LangSet_Find`). The core hooks it when an addon sets its first override; until then the game's lookup runs untouched. While overrides exist, a key with an override returns the override text and every other key reads the game's tables.

A change applies the next time the game looks the key up. Screens and labels opened afterwards show it; text already on screen keeps its old value until it is looked up again, which `refresh()` triggers. Call `refresh()` once after a batch of changes, on the game thread (for example from a core [event](./events) handler).

The core keeps every text it was given until the game exits, because the game may still hold a returned string after its override is cleared.

## Example

Here's how you name a character only while one kind of career is loaded:

```cpp [native/src/my_plugin.cpp]
#include <swofty/events.hpp>
#include <swofty/lang.hpp>

namespace lg = swofty::lang;

bool my_career_loaded();

void on_career(const SwoftyEvent *ev, void *)
{
    if (!lg::available())
        return;
    if (!std::strcmp(ev->name, SWOFTY_EVENT_CAREER_LOADED) && my_career_loaded()) {
        lg::set("lng_elena_name", "Flavio");
        lg::set("lng_elena_name", "Flavio", "ita");
    } else {
        lg::clear("lng_elena_name");
    }
    lg::refresh();
}

SWOFTY_ON_LOAD
{
    uint32_t token = 0;
    swofty::events::subscribe(SWOFTY_EVENT_CAREER_LOADED, on_career, nullptr, &token);
    swofty::events::subscribe(SWOFTY_EVENT_CAREER_UNLOADED, on_career, nullptr, &token);
}

SWOFTY_ADDON()
```

To build one plugin against a core with and without the API, guard the calls with `#if SWOFTY_API_VERSION >= 8` and check `lg::available()` at runtime.
