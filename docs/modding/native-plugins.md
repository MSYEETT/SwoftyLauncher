# Native plugins

Native plugins are Windows x86-64 DLLs that run inside F1 25. They are for addon authors who need code changes beyond data edits. [Hooking a game function](/tutorials/hooking) walks through one.

## SDK

Use [`packages/sdk/include/swofty_api.h`](https://github.com/Swofty-Developments/F1ModManager/blob/main/packages/sdk/include/swofty_api.h). The SDK uses the MIT licence.

Export this function:

```c
__declspec(dllexport) int SwoftyAddonInit(const SwoftyApi *api);
```

The loader also calls `SwoftyModInit` when a DLL exports only that name.

Return non-zero on success or 0 on failure. The DLL stays loaded either way, so leave no hooks installed after a failure.

`SwoftyApi` has these members:

| Member | Offered by | Purpose |
|---|---|---|
| `version` | `version.dll` | The API version the loader offers |
| `log(fmt, ...)` | `version.dll` | Write to `swofty/loader.log`. See [Logging](/reference/std/logging) |
| `module_base()` | `version.dll` | The base address of `F1_25.exe` |
| `scan(section, pattern, &out)` | `version.dll` | Find a byte pattern such as `"48 89 5C 24 ?? 57"` in a section such as `".xtls"`. Returns the hit count and sets `out` only when there is exactly one |
| `resolve(id)` | `swofty_core.dll` | The address of a [binding](/reference/std/mixin#bindings), or 0 |
| `register_mixins(mixins, count)` | `swofty_core.dll` | Register [mixins](/reference/std/mixin#c-abi), from `SwoftyAddonInit` only |
| `fields(obj, size)` | `swofty_core.dll` | Per-addon storage on a game object. See [fields](/reference/std/mixin#fields) |
| `setting(id, type, &out)` | `swofty_core.dll` | Read a [setting](/reference/std/settings#c-abi) |
| `watch_setting(id, fn, user)` | `swofty_core.dll` | Call `fn` when a setting changes |

`version.dll` alone offers version 1. `swofty_core.dll` raises it to 3. `SWOFTY_ADDON()` in `swofty/mixin.hpp` also exports `SwoftyAddonApi`, the version the plugin was built against.

The [Game API](/reference/game/) lists the game functions and structs that plugins can use.

## Settings

A plugin reads its addon's [options](/modding/manifest#options) with `swofty/mixin.hpp` from the SDK. Declare each setting with its type and a fallback, using the option `id` as the name:

```cpp
#include <swofty/mixin.hpp>

SWOFTY_SETTING(bool, show_panel, true);
SWOFTY_SETTING(int, ai_offset, 0);
SWOFTY_SETTING(std::string_view, team_livery, "titlesponsor");

SWOFTY_ON_CHANGE(show_panel, bool on)
{
    swofty::log("[my-addon] panel %s", on ? "on" : "off");
}

SWOFTY_ON_LOAD
{
    if (show_panel && team_livery == "replica")
        swofty::log("[my-addon] offset %d", int(ai_offset));
}

SWOFTY_ADDON()
```

| Type | Option type |
|---|---|
| `bool` | `toggle` |
| any number type | `slider` |
| `std::string_view` | `choice` |

Reading a setting returns the profile's value for the session, or the fallback when the option is not declared with that type in `swofty.json`. The plugin log names any mismatch. `SWOFTY_ON_CHANGE` runs when the player changes the value in **ADDON SETTINGS**. Use `swofty::setting<T>{"id", fallback}` for a name that differs from the option id. `SWOFTY_MOD()` is another name for `SWOFTY_ADDON()`.

From C, call `api->setting(id, type, &value)` and `api->watch_setting(id, fn, user)` (API version 3). Both identify the calling addon by its DLL.

## Manifest entry

```json
"operations": {
  "native": [
    { "dll": "native/my_plugin.dll", "capabilities": ["log", "scan", "hook"] }
  ]
}
```

`options` covers the settings members above. The manifest also accepts `read-db` and `on-frame`, which are planned capabilities, not members of the current API.

## Hook rules

- Find addresses by signature, not fixed addresses. Stop if a signature has zero or multiple matches.
- Hook only `.xtls` code. Do not patch `.data1` bodies.
- Keep hook setup out of `DllMain`.
- Keep modded play offline only.

Plugin approval and file-hash checks are planned. The current loader loads DLLs from `swofty/addons/core/`, then `swofty/addons/`. Until the next minor release it also loads `swofty/mods/core/` and `swofty/mods/` with them, in name order.

## Build

Use [the addon template](/modding/template). It builds with `zig cc -target x86_64-windows-gnu`. Other toolchains that produce Windows x86-64 DLLs, including MSVC, also work.
