---
title: Hooking a game function
description: Use the Advanced Mod Mixin System to wrap an F1 25 function, run code at an instruction, and order your hook among other addons.
---

# Hooking a game function

The **Advanced Mod Mixin System** lets a native plugin edit the F1 25 engine directly. A plugin names the game function it wants from the [Game API](/reference/game/), writes a handler, and SwoftyMod installs it when the game starts. Several addons can hook the same function: SwoftyMod chains them by priority.

This tutorial wraps `FeHome_Handle_Command`, the function that runs when a home-screen button is pressed. SwoftyMod hooks the same function for ADDON SETTINGS.

## 1. Get the headers

A hooking plugin needs three headers from the repository:

| Header | From | Gives |
|---|---|---|
| `swofty_api.h` | `packages/sdk/include/` | The C ABI |
| `swofty/mixin.hpp` | `packages/sdk/include/swofty/` | The mixin macros, settings and logging |
| `swofty/game.hpp`, `swofty/game/*.hpp` | `native/include/swofty/` | The game's functions, structs and sites |

Copy `native/include/swofty/game.hpp` and `native/include/swofty/game/` into the template's `native/include/swofty/`.

## 2. Wrap the function

```cpp [native/src/my_plugin.cpp]
#include <swofty/game.hpp>

using namespace swofty::game;

SWOFTY_WRAP(FeHome_Handle_Command, uint64_t, FEHomeViewModel *vm, NeStr **cmd)
{
    swofty::log("[my-addon] home command");
    return orig(vm, cmd);
}

SWOFTY_ADDON()
```

- The first argument is the binding's name. The rest is its signature: the return type, then the parameters. The header declares the binding as `FeHome_Handle_Command_t = uint64_t(FEHomeViewModel *vm, NeStr **cmd)`, and a handler with a different signature fails to compile.
- `orig` calls the next handler in the chain, or the game's own function at the end of it. Skip `orig` to replace the game's behaviour.
- `SWOFTY_ADDON()` exports `SwoftyAddonInit`. It resolves every binding the plugin uses and registers every mixin in the file.

## 3. Build and declare it

```sh
zig c++ -target x86_64-windows-gnu -std=c++23 -O2 -Wall -Wextra -Iinclude -shared -s \
    -o my_plugin.dll src/my_plugin.cpp
```

```json [swofty.json]
"operations": {
  "native": [{ "dll": "native/my_plugin.dll", "capabilities": ["log", "hook"] }]
}
```

Install and enable the addon, then launch. `swofty/loader.log` in the game folder shows the chain:

```text
wrap FeHome_Handle_Command at 0x... installed: swofty_core.dll(0) > my_plugin.dll(0)
```

## 4. Order the chain

Every handler has a priority. Higher priorities run first, so they wrap the ones below. Equal priorities run in load order: SwoftyMod first, then addon plugins in file name order.

```cpp
SWOFTY_WRAP_PRIORITY(10, FeHome_Handle_Command, uint64_t, FEHomeViewModel *vm, NeStr **cmd)
{
    return orig(vm, cmd);
}
```

With a second addon at the default priority 0, the chain is `my_plugin.dll(10) > swofty_core.dll(0) > other.dll(0)`, then the game.

`SWOFTY_OVERWRITE` marks a handler that replaces the function and does not call `orig`. SwoftyMod logs a warning when handlers sit below an overwrite. The game log addon replaces the engine's log function this way:

```cpp
SWOFTY_OVERWRITE(0, Ne_Log_Thunk, void, const char *channel, const char *fmt, ...)
{
}
```

::: warning Conflicts
Two overwrites of one binding conflict, and so do hooks on different bindings whose patched bytes overlap. SwoftyMod logs each conflict and drops every addon involved, so none of its mixins install.
:::

## 5. Run code at an instruction

A site binding is an address inside a function. `SWOFTY_SITE` runs a handler there with the CPU registers, and the handler can change them. SwoftyMod adds the ADDON SETTINGS tile this way:

```cpp
SWOFTY_SITE(site_FeHome_AfterCareerCall, swofty::Context &c)
{
    auto list = reinterpret_cast<TileList *>(c.r14);
    uint8_t index = uint8_t(c.r12 + 1);
    FeHome_Add_CareerTile(list, index);
    c.r12 += 1;
    c.r15 += 1;
}
```

Bindings are callable: `FeHome_Add_CareerTile(list, index)` calls the game's function.

## 6. Check before installing

`SWOFTY_ADDON(check)` runs `check` after every binding resolves and before any mixin installs. Return `false` to install nothing:

```cpp
bool check()
{
    uintptr_t site = site_FeHome_AfterCareerCall.address(), builder = FeTile_Build_List.address();
    return site > builder && site - builder <= 0x8000;
}

SWOFTY_ADDON(check)
```

## Bindings and signatures

A binding is a name for one place in `F1_25.exe`. `swofty/addons/core/bindings.json` gives each one a byte pattern, and SwoftyMod scans the executable for it when the game starts. A binding that finds no match, or more than one, does not resolve, and a plugin that uses it registers no mixins. The log names the binding.

The [Game API](/reference/game/) lists every binding with its signature, its status and its address in the current build. **stable** bindings are confirmed by a passing trace test.

::: tip Rules
- Hook bindings, not fixed addresses.
- Hook only `.xtls` code. Never patch `.data1` bodies.
- Keep hook setup out of `DllMain`.
:::

The [Mixin API](/reference/std/mixin) reference covers every macro: vtables, call redirects, byte patches, `SWOFTY_ON_LOAD` and per-object fields.
