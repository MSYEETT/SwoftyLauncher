# Native plugins

Native plugins are Windows x86-64 DLLs that run inside F1 25. They are for addon authors who need code changes beyond data edits. [Hooking a game function](/reference/addons/hooking) walks through one.

## SDK

Use [`packages/sdk/include/swofty_api.h`](https://github.com/Swofty-Developments/SwoftyLauncher/blob/main/packages/sdk/include/swofty_api.h). The SDK uses the MIT licence.

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
| `assets` | `swofty_core.dll` | Load mods at runtime. See [Asset API](/reference/addons/asset-api#native-plugins) |

`version.dll` alone offers version 1. `swofty_core.dll` raises it to 4. `SWOFTY_ADDON()` in `swofty/mixin.hpp` also exports `SwoftyAddonApi`, the version the plugin was built against.

The [Game API](/reference/game/) lists the game functions and structs that plugins can use.

## Settings

A plugin reads its addon's [options](/reference/addons/manifest#options) with `swofty/mixin.hpp` from the SDK. Declare each setting with its type and a fallback, using the option `id` as the name:

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
    { "dll": "native/my_plugin.dll", "capabilities": ["log", "scan", "hook"], "files": ["lang/", "erp/frag.bin"] }
  ]
}
```

`options` covers the settings members above, `assets` covers `swofty/assets.hpp`, and `storage` covers `swofty/storage.hpp`. `"early": true` loads the plugin at the [early stage](/reference/addons/asset-api#the-early-stage). `files` lists the files and folders the plugin reads while the game runs, as paths in the addon's folder. The manifest also accepts `read-db` and `on-frame`, which are planned capabilities, not members of the current API.

## Files in the game folder

When the addon is on in the profile, the build copies each plugin's `dll` and everything its `files` lists to `swofty/addons/<id>/` in the game folder, at the same paths as in the addon's folder. A listed folder is copied with its subfolders, except entries whose names start with `.`. Nothing else from the addon's folder is copied. For the entry above:

```text
swofty/addons/example.my-addon/native/my_plugin.dll
swofty/addons/example.my-addon/lang/...
swofty/addons/example.my-addon/erp/frag.bin
```

`swofty/addons/<id>/` is the folder that [Asset API](/reference/addons/asset-api#native-plugins) file arguments are relative to, so `lang_table("eng", "lang/extra.lng")` reads the file that is `lang/extra.lng` in the addon's folder.

The files are installed through the journal like the loader. Turning the addon off, uninstalling it or switching to a profile without it removes them at the next build, and `swofty restore` removes them. The build fails when a listed file is missing, or when two plugins in the profile have the same file name.

The build lists the plugins in `plugins` in `swofty/settings.json`, in the profile's load order. The loader loads `swofty/addons/core/` first, then each listed plugin from its addon folder, then any other DLL directly in `swofty/addons/` in name order. A plugin with `"early": true` loads at the early stage and the rest wait for the window stage, in the same order. swofty_core finds a plugin's addon by its file name through `dlls` in `swofty/settings.json`.

## Hook rules

- Find addresses by signature, not fixed addresses. Stop if a signature has zero or multiple matches.
- Hook only `.xtls` code. Do not patch `.data1` bodies.
- Keep hook setup out of `DllMain`.
- Keep modded play offline only.

Plugin approval and file-hash checks are planned. Until the next minor release the loader also loads `swofty/mods/core/` and `swofty/mods/`, in name order with `swofty/addons/core/` and `swofty/addons/`.

## Build

Use [the addon template](/reference/addons/template). It builds with `zig cc -target x86_64-windows-gnu`. Other toolchains that produce Windows x86-64 DLLs, including MSVC, also work.
