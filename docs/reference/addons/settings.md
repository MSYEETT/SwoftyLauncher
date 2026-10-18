---
title: Addon settings
description: Declare a toggle, choice and slider in options, pick db or native binding, and read the value from an op file, an engine plugin and a launcher plugin.
---

# Addon settings

Declare your settings once, in `options` in `swofty.json`, and they appear in both places: your addon's row on the launcher's **Addons** page and its page in the game's **ADDON SETTINGS**. Each profile stores its own values.

## 1. Declare the settings

Start with the [addon template](./template)'s four settings. Its engine plugin reads three of them; the fourth changes a value in the game database.

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
    "description": "Length of a stop-go penalty in seconds. Written into the game database, so a change needs a rebuild.",
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
| `toggle` | `true` or `false` | No `values`, `min`, `max` or `step`. |
| `slider` | A number on the grid | Supply `min`, `max`, `step`. `max - min` must be a whole number of steps, at most 100. Choose a `default` on the grid. |
| `choice` | One of `values` | Supply 1 to 16 values, each a string or `{ "value", "label" }`. Pick one as the `default`. |

Give each option an `id` that matches `[a-z][a-z0-9_]*`, is at most 40 characters and is unique within your addon. You'll find the full field list in [the manifest reference](./manifest#options).

## 2. Choose where the value goes

Choose `binds` based on where you need the value. This also determines when the player's changes take effect:

| `binds` | Read by | A change applies |
|---|---|---|
| `native` | the addon's [engine plugin](./native-plugins#settings), while the game runs | at once |
| `db` | the build, through <span v-pre>`{{id}}`</span> and `when` in op files | after a rebuild and a restart |

If you have a [launcher plugin](./launcher-api), its build hook receives **every** declared option's value, whatever its `binds`, in its `build` call.

## 3. Read a `db` setting

Put a <span v-pre>`{{id}}`</span> placeholder in your op file to use a setting's value. The build replaces it with that value:

```json [db/example.json]
{
  "schema_version": 1,
  "ops": [
    { "op": "set", "select": "rules.stop_go_penalty", "field": "penalty_duration", "value": "{{penalty_seconds}}" }
  ]
}
```

To apply an op only when each listed setting matches its specified value, use `when`. Custom Grid Size uses this to run each track's edits for the car count they were made for:

```json
{ "op": "patch-pkg", "when": { "cars": 26 }, "erp": "…", "resource": "…", "edits": [] }
```

Because a `db` setting goes into the build, changing it marks the profile for a rebuild. The next launch applies it. In game, a `db` setting's description ends with *Applies after a restart.*

## 4. Read a `native` setting

In your engine plugin, declare each setting with the option's `id`, its C++ type and a fallback. Here's how the template does it in `native/src/sample_plugin.cpp`:

```cpp [native/src/sample_plugin.cpp]
#include <swofty/mixin.hpp>

SWOFTY_SETTING(bool, verbose, false);
SWOFTY_SETTING(int, greeting_count, 1);
SWOFTY_SETTING(std::string_view, greeting, "hello");

SWOFTY_ON_CHANGE(verbose, bool on)
{
    swofty::log("[sample] verbose logging %s", on ? "on" : "off");
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

Read a setting to get the profile's current value. To react when the player changes it in ADDON SETTINGS, use `SWOFTY_ON_CHANGE`.

If `swofty.json` has no option with that `id` and type, you get the fallback instead. Check `swofty/loader.log` for the mismatch.

List the `options` capability in your plugin's `operations.native` entry:

```json [swofty.json]
"native": [{ "dll": "native/sample_plugin.dll", "capabilities": ["log", "scan", "options"] }]
```

## 5. Read a setting in a launcher plugin

Your [launcher plugin](./launcher-api) receives every option's value in `params.options` when its `build` call arrives:

```json
{ "method": "build", "params": { "options": { "greeting_count": 1, "penalty_seconds": 10 }, "features": { "stable": ["file"], "experimental": [] } } }
```

You can read the value straight from the map:

```rust [src/main.rs]
fn build(&self, options: &Value) -> Result<Value, String> {
    let seconds = options["penalty_seconds"].as_u64().unwrap_or(5);
    // … build ops from `seconds`
}
```

## How a value persists and travels

Look under `options` in a profile to find the saved values, keyed by addon id. You'll only see values the player changed from the default:

```json [$DATA/profiles/default.json]
"options": {
  "example.sample-mod": { "penalty_seconds": 10 }
}
```

1. When you build, the launcher writes the profile's values into `swofty/settings.json` in the game folder.
2. At launch, SwoftyMod reads that file and builds each addon's page in ADDON SETTINGS. It also answers the plugin's `setting` calls.
3. Change a value in game and the plugin updates at once: its `SWOFTY_ON_CHANGE` handlers run, and the change is written to the requests file.
4. The launcher merges the requests file into the profile, so you'll see the new value on the **Addons** page.

To list, change or reset settings from the command line:

```sh
swofty settings list example.sample-mod
swofty settings set example.sample-mod penalty_seconds 10
swofty settings reset example.sample-mod
```

## See it

Build the profile and start the game:

```sh
swofty run --apply
```

Open **Career → ADDON SETTINGS**. Your addon's tile shows a **Stop-go penalty** slider at 10, a **Greetings** slider, a **Greeting** choice and a **Verbose log** toggle. Try changing a `native` setting: it takes effect the same lap. The `db` **Stop-go penalty** reads *Applies after a restart.*

You'll find the C ABI and every helper in [Settings](/reference/std/settings).
