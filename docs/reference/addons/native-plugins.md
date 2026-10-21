---
title: Engine plugins
description: An engine plugin is a native DLL an addon names in operations.native. The in-game loader loads it into F1 25 and gives it SwoftyApi. The steps to make one, and the SwoftyApi and operations.native reference.
---

# Engine plugins

An engine plugin is your own code running inside F1 25: a prebuilt Windows x86-64 DLL that your addon names in `operations.native` in `swofty.json`. The build copies it into the game folder. The in-game loader loads it and hands it a `const SwoftyApi *`, your way into the engine. Through it you hook the engine with [the mixin system](/reference/addons/hooking) and call the [standard library](/reference/std/) for events, the running career, overlay, telemetry, database reads, storage, settings and logging.

Write your plugin in C or C++ against [`swofty_api.h`](https://github.com/Swofty-Developments/SwoftyLauncher/blob/main/packages/sdk/include/swofty_api.h) (MIT).

## Make an engine plugin

1. **Start from the template.** Copy [`templates/addon`](./template#templates-addon) and edit `native/src/sample_plugin.cpp` ([Write the plugin](#_1-write-the-plugin)).
2. **Build the DLL.** The template's `native/build.sh` builds it with `zig c++`; any toolchain that makes a Windows x86-64 DLL works ([Build the DLL](#_2-build-the-dll)).
3. **Declare it in `native`.** Name the DLL, and the files it reads in game, in `operations.native` ([Name it in the manifest](#_3-name-it-in-the-manifest)).
4. **Hook the game with mixins.** Wrap an engine function or run code at an instruction ([The mixin system](./hooking)).
5. **Use the standard library.** Subscribe to events, draw on the overlay, read telemetry and the database, store data, read settings and log ([Standard library](/reference/std/)).
6. **Test it.** Build the profile, play, and find your lines in `swofty/loader.log` ([Launch and read the log](#_5-launch-and-read-the-log), [Packaging and publishing](./publishing)).

## What's in this section

| Page | What it covers |
|---|---|
| [The mixin system](./hooking) | Wrapping engine functions and running code at an instruction, with chains and rules |
| [In-game UI](./in-game-ui) | Adding a screen and a home tile, and changing existing screens |
| [Standard library](/reference/std/) | The headers and `SwoftyApi` tables: mixins, settings, logging, events, overlay, telemetry, database reads, storage, UI |
| [Engine API](/reference/game/) | The engine functions, structs and sites you can call and hook |
| [Changes at runtime](./runtime-assets#from-a-native-plugin) | Changing textures, text and database records from a plugin while the game boots |

## Build and load one

### 1. Write the plugin

Copy [the addon template](/reference/addons/template) and edit `native/src/sample_plugin.cpp`, or start from this:

```cpp [native/src/my_plugin.cpp]
#include <swofty/mixin.hpp>

SWOFTY_ON_LOAD
{
    swofty::log("[my-addon] loaded, game base 0x%llx", (unsigned long long)swofty::api->module_base());
}

SWOFTY_ADDON()
```

The loader calls into your plugin through `SwoftyAddonInit`. `SWOFTY_ADDON()` exports that function for you:

```c
__declspec(dllexport) int SwoftyAddonInit(const SwoftyApi *api);
```

Return non-zero on success, 0 on failure. The DLL stays loaded either way, so leave no hooks installed if initialization fails. If your DLL exports only `SwoftyModInit`, the loader accepts that name too.

### 2. Build the DLL

The template's `native/build.sh` runs:

```sh
zig c++ -target x86_64-windows-gnu -std=c++23 -O2 -Wall -Wextra -Iinclude -shared -s \
    -o my_plugin.dll src/my_plugin.cpp
```

```text
built /path/to/addon/native/my_plugin.dll
```

You can use MSVC or any other toolchain that produces a Windows x86-64 DLL.

### 3. Name it in the manifest

Tell the launcher which DLL to load:

```json [swofty.json]
"operations": {
  "native": [
    { "dll": "native/my_plugin.dll" }
  ]
}
```

### 4. Build the profile

Install and enable the addon, then build:

```sh
swofty addons install .
swofty addons enable example.my-addon
swofty build
```

This copies `native/my_plugin.dll` to `swofty/addons/example.my-addon/native/` in the game folder and lists it in `swofty/settings.json` for the loader.

### 5. Launch and read the log

Press **Play**, or run `swofty run`. Open `swofty/loader.log` in the game folder to see the loader load your plugin, followed by your plugin's own line:

```text
addon my_plugin.dll: loaded at 0x7ffcd8e10000, calling SwoftyAddonInit (API v5)
[my-addon] loaded, game base 0x7ff6a4120000
```

## SwoftyApi

The `const SwoftyApi *` you receive in `SwoftyAddonInit` gives you these members:

| Member | Offered by | Purpose |
|---|---|---|
| `version` | `version.dll` (v1) | The API version the loader offers |
| `log(fmt, ...)` | `version.dll` (v1) | Write to `swofty/loader.log`. See [Logging](/reference/std/logging) |
| `module_base()` | `version.dll` (v1) | The base address of `F1_25.exe` |
| `scan(section, pattern, &out)` | `version.dll` (v1) | Find a byte pattern such as `"48 89 5C 24 ?? 57"` in a section such as `".xtls"`; returns the hit count, sets `out` only on exactly one |
| `resolve(id)` | `swofty_core.dll` (v4) | A [binding](/reference/std/mixin#bindings)'s address, or 0 |
| `register_mixins(mixins, count)` | `swofty_core.dll` (v4) | Register [mixins](/reference/addons/hooking), from `SwoftyAddonInit` only |
| `fields(obj, size)` | `swofty_core.dll` (v4) | Per-addon storage on a game object. See [fields](/reference/std/mixin#fields) |
| `setting(id, type, &out)` | `swofty_core.dll` (v4) | Read a [setting](/reference/std/settings#read-from-c) |
| `watch_setting(id, fn, user)` | `swofty_core.dll` (v4) | Call `fn` when a setting changes |
| `assets` | `swofty_core.dll` (v4) | Change assets while the game boots. See [Asset API](/reference/addons/asset-api#native-plugins) |
| `events` | `swofty_core.dll` (v5) | Lifecycle events and the frame tick. See [Events](/reference/std/events) |
| `overlay` | `swofty_core.dll` (v5) | The 2D overlay. See [Overlay](/reference/std/overlay) |
| `telemetry` | `swofty_core.dll` (v5) | Live car and session reads. See [Telemetry](/reference/std/telemetry) |
| `db` | `swofty_core.dll` (v5) | Game-database reads. See [Database read](/reference/std/db) |
| `storage` | `swofty_core.dll` (v5) | Addon storage. See [Storage](/reference/std/storage) |
| `career` | `swofty_core.dll` (v6) | Flow states, the running career, its components and the game bus; career-mode blocks (v7). See [Career](/reference/std/career) |
| `popup` | `swofty_core.dll` (v7) | The game's message popup with your text. See [Popup](/reference/std/popup) |
| `lang` | `swofty_core.dll` (v8) | Runtime string overrides. See [Strings](/reference/std/strings) |

For guarded reads of game memory (`read`, `in_image`, `object_like`, `c_string`), include the header-only [`swofty/mem.hpp`](/reference/std/memory); it needs no table.

The API version depends on what's loaded: `version.dll` alone offers version 1; `swofty_core.dll` raises it to 8 (7 before string overrides, 6 before core 0.2.12, 5 before core 0.2.11). Your plugin declares the version it was built against through `SwoftyAddonApi`, which `SWOFTY_ADDON()` also exports with the value of `SWOFTY_API_VERSION` (8). For the engine functions and structs you can call and hook, see the [Game API](/reference/game/).

## Settings

To read your addon's [options](/reference/addons/manifest#options), look them up by option `id`. You can also react when the player changes them in-game. Use `swofty/mixin.hpp` to read each setting:

```cpp
SWOFTY_SETTING(bool, show_panel, true);

SWOFTY_ON_CHANGE(show_panel, bool on)
{
    swofty::log("[my-addon] panel %s", on ? "on" : "off");
}
```

If you write in C, call `api->setting(id, type, &out)` and `api->watch_setting(id, fn, user)`. See [Settings](/reference/std/settings) for the full C++ and C reference.

## The `operations.native` entry

```json [swofty.json]
"operations": {
  "native": [
    {
      "dll": "native/my_plugin.dll",
      "early": false,
      "files": ["lang/", "erp/frag.bin"]
    }
  ]
}
```

| Field | Meaning |
|---|---|
| `dll` | The plugin DLL, relative to the addon folder |
| `early` | `true` loads the plugin at the [early stage](/reference/addons/asset-api#the-early-stage); default `false` waits for the window stage |
| `files` | Files and folders the plugin reads while the game runs, relative to the addon folder. Default empty |

## Files in the game folder

If your plugin needs files while the game runs, list them in `files`. With the addon enabled in the profile, the build copies those entries and your `dll` to `swofty/addons/<id>/` in the game folder, preserving their paths within the addon. Listed folders include their subfolders, except entries whose names start with `.`. Nothing else from your addon folder gets copied. The entry above produces:

```text
swofty/addons/example.my-addon/native/my_plugin.dll
swofty/addons/example.my-addon/lang/...
swofty/addons/example.my-addon/erp/frag.bin
```

When you pass a file path to the [Asset API](/reference/addons/asset-api#native-plugins) or [Storage](/reference/std/storage), it is relative to `swofty/addons/<id>/`. So `lang_table("eng", "lang/extra.lng")` reads the file you shipped as `lang/extra.lng` in your addon folder.

These files install through the journal, just like the loader. Turn the addon off, uninstall it, or switch to a profile without it, and the next build removes them. You can also remove them with `swofty restore`. A missing listed file makes the build fail, as does having two plugins in the profile with the same file name.

## Load order

Your profile's load order carries through to the `plugins` list the build writes in `swofty/settings.json`. The loader starts with `swofty/addons/core/`, then loads each listed plugin from its addon folder, followed by any other DLL directly in `swofty/addons/` in name order.

Set `"early": true` to load your plugin at the early stage. Otherwise it waits for the window stage, with the same order preserved. To find which addon a plugin belongs to, swofty_core looks up its file name through `dlls` in `swofty/settings.json`.

## Rules

- Find addresses by signature, not fixed addresses. Stop when a signature has zero or multiple matches.
- Hook only `.xtls` code. Never patch `.data1` bodies.
- Keep hook setup out of `DllMain`.
- Keep modded play [offline](/reference/safety).
