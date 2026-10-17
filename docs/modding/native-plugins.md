# Hooking game functions

Native plugins are Windows x86-64 DLLs that run inside F1 25. They are for mod authors who need code changes beyond data edits.

## SDK

Use [`packages/sdk/include/swofty_api.h`](https://github.com/Swofty-Developments/F1ModManager/blob/main/packages/sdk/include/swofty_api.h). The SDK uses the MIT licence.

Export this function:

```c
__declspec(dllexport) int SwoftyModInit(const SwoftyApi *api);
```

Return non-zero on success or 0 on failure. The DLL stays loaded either way, so leave no hooks installed after a failure.

The current API provides:

| Member | Purpose |
|---|---|
| `version` | Check the API version |
| `log` | Write to `swofty/loader.log` |
| `module_base` | Get the base address of `F1_25.exe` |
| `scan` | Find a code pattern; use the result only when exactly one match is found |

The [Game API](/reference/game/) lists the game functions and structs that plugins can use.

## Settings

A plugin reads its mod's [options](/modding/manifest#options) with `swofty/mixin.hpp` from the SDK. Declare each setting with its type and a fallback, using the option `id` as the name:

```cpp
#include <swofty/mixin.hpp>

SWOFTY_SETTING(bool, show_panel, true);
SWOFTY_SETTING(int, ai_offset, 0);
SWOFTY_SETTING(std::string_view, team_livery, "titlesponsor");

SWOFTY_ON_CHANGE(show_panel, bool on)
{
    swofty::log("[my-mod] panel %s", on ? "on" : "off");
}

SWOFTY_ON_LOAD
{
    if (show_panel && team_livery == "replica")
        swofty::log("[my-mod] offset %d", int(ai_offset));
}

SWOFTY_MOD()
```

| Type | Option type |
|---|---|
| `bool` | `toggle` |
| any number type | `slider` |
| `std::string_view` | `choice` |

Reading a setting returns the profile's value for the session, or the fallback when the option is not declared with that type in `swofty.json`. The plugin log names any mismatch. `SWOFTY_ON_CHANGE` runs when the player changes the value in **MOD SETTINGS**. Use `swofty::setting<T>{"id", fallback}` for a name that differs from the option id.

From C, call `api->setting(id, type, &value)` and `api->watch_setting(id, fn, user)` (API version 3). Both identify the calling mod by its DLL.

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

Plugin approval and file-hash checks are planned. The current loader loads DLLs from `swofty/mods/core/`, then `swofty/mods/`.

## Build

Use [the mod template](/modding/template). It builds with `zig cc -target x86_64-windows-gnu`. Other toolchains that produce Windows x86-64 DLLs, including MSVC, also work.
