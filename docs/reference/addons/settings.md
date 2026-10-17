---
title: Addon settings
description: Declare a slider, a toggle and a choice, show them in ADDON SETTINGS and read them from the database and from a plugin.
---

# Addon settings

Settings are declared once, in `options` in `swofty.json`. The launcher shows them in the addon's row on the **Addons** page, and the game shows them on the addon's page in **ADDON SETTINGS**. Values are stored per profile.

## Declare the settings

These are the [addon template](/reference/addons/template)'s settings: three read by a plugin and one built into the database.

```json [swofty.json]
"options": [
  {
    "id": "verbose",
    "type": "toggle",
    "label": "Verbose log",
    "description": "Log setting changes and the signature scan to swofty/loader.log.",
    "default": false,
    "binds": "native"
  },
  {
    "id": "greeting",
    "type": "choice",
    "label": "Greeting",
    "description": "What the plugin writes to swofty/loader.log when it loads.",
    "values": ["hello", { "value": "good morning", "label": "Good morning" }, "hi"],
    "default": "hello",
    "binds": "native"
  },
  {
    "id": "greeting_count",
    "type": "slider",
    "label": "Greetings",
    "description": "How many times the plugin writes the greeting.",
    "min": 0,
    "max": 5,
    "step": 1,
    "default": 1,
    "binds": "native"
  },
  {
    "id": "penalty_seconds",
    "type": "slider",
    "label": "Stop-go penalty",
    "description": "Length of a stop-go penalty in seconds. It is written into the game database, so a change needs a rebuild.",
    "min": 5,
    "max": 20,
    "step": 5,
    "default": 5,
    "binds": "db"
  }
]
```

| Type | Value | Rules |
|---|---|---|
| `toggle` | `true` or `false` | |
| `slider` | A number on the slider's steps | `min`, `max` and `step` are required. `max - min` is a whole number of steps, at most 100 |
| `choice` | One of `values` | 1 to 16 values. Each is a string, or `{ "value", "label" }` to show a different name |

The full field list is in [Options](/reference/addons/manifest#options).

## Choose where the value goes

`binds` decides how a value reaches the addon.

| `binds` | Read by | When a change applies |
|---|---|---|
| `native` | The addon's [native plugin](/reference/addons/native-plugins), while the game runs | At once |
| `db` | The build, through `{{id}}` and `when` in operation files | After a rebuild and a restart |

## Use a `db` setting

<span v-pre>`{{penalty_seconds}}`</span> in an operation file is replaced with the setting's value when the profile is built:

```json [db/example.json]
{
  "schema_version": 1,
  "ops": [
    { "op": "set", "select": "rules.stop_go_penalty", "field": "penalty_duration", "value": "{{penalty_seconds}}" }
  ]
}
```

`when` keeps an operation only when every listed setting has that value. Custom Grid Size runs each track's edits only for the car count they were made for:

```json
{ "op": "patch-pkg", "when": { "cars": 26 }, "erp": "...", "resource": "...", "edits": [] }
```

Changing a `db` setting marks the profile for a rebuild, and the next launch applies it. In game, a `db` setting's description ends with *Applies after a restart.*

## Read a `native` setting

The plugin declares each setting with the option's `id`, its C++ type and a fallback. This is `native/src/sample_plugin.cpp` from the template:

```cpp [native/src/sample_plugin.cpp]
#include <swofty/mixin.hpp>

SWOFTY_SETTING(bool, verbose, false);
SWOFTY_SETTING(int, greeting_count, 1);
SWOFTY_SETTING(std::string_view, greeting, "hello");

SWOFTY_ON_CHANGE(verbose, bool on)
{
    swofty::log("[sample] verbose logging %s", on ? "on" : "off");
}

SWOFTY_ON_CHANGE(greeting, std::string_view text)
{
    if (verbose)
        swofty::log("[sample] greeting is now \"%.*s\"", int(text.size()), text.data());
}

SWOFTY_ON_LOAD
{
    std::string_view text = greeting;
    for (int i = 0; i < greeting_count; i++)
        swofty::log("[sample] %.*s from the sample mod", int(text.size()), text.data());
}

SWOFTY_ADDON()
```

| C++ type | Option type |
|---|---|
| `bool` | `toggle` |
| Any number type | `slider` |
| `std::string_view` | `choice` |

- Reading a setting returns the profile's current value.
- `SWOFTY_ON_CHANGE` runs when the player changes the value in ADDON SETTINGS.
- The fallback is used when `swofty.json` has no option with that `id` and type. `swofty/loader.log` names the mismatch.

The manifest entry for the plugin lists the `options` capability:

```json
"native": [{ "dll": "native/sample_plugin.dll", "capabilities": ["log", "scan", "options"] }]
```

## How a value travels

1. The launcher writes the profile's values into `swofty/settings.json` in the game folder when it builds.
2. SwoftyMod reads that file when the game starts. It builds each addon's page in ADDON SETTINGS and answers the plugin's `setting` calls.
3. A change in game updates the plugin at once, calls its `SWOFTY_ON_CHANGE` handlers and is written to the requests file.
4. The launcher merges the requests file into the profile, so its **Addons** page shows the new value.

From the command line:

```sh
swofty settings list yourname.my-first-addon
swofty settings set yourname.my-first-addon penalty_seconds 10
swofty settings reset yourname.my-first-addon
```

See [Settings](/reference/std/settings) for the C ABI and every helper.
