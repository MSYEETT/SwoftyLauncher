---
title: Mixin API
description: The C++ mixin layer in swofty/mixin.hpp - bindings, the mixin macros, chains, init and per-object fields.
---

# Mixin API

```cpp
#include <swofty/mixin.hpp>
#include <swofty/game.hpp>
```

Use these macros to declare your plugin's mixins through the C++ layer over the [mixin system](/reference/addons/hooking). `SWOFTY_ADDON()` resolves every binding you name and registers all your mixins with swofty_core. Installation waits until every plugin has loaded.

| | |
|---|---|
| Header | `swofty/mixin.hpp` |
| Member | `SwoftyApi::register_mixins`, `SwoftyApi::resolve` |
| Feature | `hook` |
| API version | 2 |
| Thread | The game thread that reaches the hooked instruction |

## Bindings

To choose where your mixin runs, you name a binding: a place in `F1_25.exe`. You'll find the available bindings in the [Game API](/reference/game/). The game headers declare each one as an `inline constexpr` object in `namespace swofty::game`; swofty_core resolves the ones your plugin uses by signature before registering your mixins.

| Template | Kind | Members |
|---|---|---|
| `fn<"Name", R(A...)>` | `func` | `get()` returns the function pointer; `operator()(A...)` calls it. Variadic signatures work |
| `address<"Name">` | `func` | |
| `site<"Name">` | `site` | |
| `call<"Name">` | `call` | |
| `data<"Name", T>` | `data` | `get()` returns `T *`; `operator*` returns `T &` |
| `vtable<"Name", Slots>` | `vtable` | `get()` returns `void **`; `slots` is the slot count |

Every binding gives you `id` (the name string), `binding_kind`, and `address()` (the resolved address, or 0). For a function binding's signature, use its `Name_t` type alias:

```cpp
using FeHome_Handle_Command_t = uint64_t(FEHomeViewModel *vm, NeStr **cmd);
inline constexpr fn<"FeHome_Handle_Command", FeHome_Handle_Command_t> FeHome_Handle_Command{};
```

## Macros

Each macro takes a binding name from `swofty::game` and opens your handler's body. Inside it, call `orig` to pass control to the next handler in the chain. At the end of the chain, `orig` calls the game's code.

| Macro | Binding | Handler |
|---|---|---|
| `SWOFTY_WRAP(target, R, params...)` | `func` | Wraps the function. `R` and `params` match `target_t` |
| `SWOFTY_WRAP_PRIORITY(prio, target, R, params...)` | `func` | The same, with a priority |
| `SWOFTY_OVERWRITE(prio, target, R, params...)` | `func` | A wrap that replaces the function and does not call `orig` |
| `SWOFTY_SITE(target, param)` | `site` | Runs at the instruction with the registers, as `swofty::Context &` |
| `SWOFTY_SITE_PRIORITY(prio, target, param)` | `site` | The same, with a priority |
| `SWOFTY_VTABLE(target, slot, R, params...)` | `vtable` | Replaces one slot of a virtual table. `slot` is below the binding's `slots` |
| `SWOFTY_VTABLE_PRIORITY(prio, target, slot, R, params...)` | `vtable` | The same, with a priority |
| `SWOFTY_REDIRECT(target, R, params...)` | `call` | Replaces the callee of one call instruction |
| `SWOFTY_PATCH(target, bytes...)` | `site` | Writes the bytes at the site. No handler |
| `SWOFTY_ON_LOAD` | | Runs once, after every mixin is installed |

```cpp
SWOFTY_WRAP(FeHome_Handle_Command, uint64_t, FEHomeViewModel *vm, NeStr **cmd)
{
    return orig(vm, cmd) * 10 + 3;
}

SWOFTY_VTABLE(vt_Fake_Other, 8, uintptr_t)
{
    return orig() + 5;
}

SWOFTY_REDIRECT(call_Fake_Callee, int)
{
    return orig() + 100;
}

SWOFTY_PATCH(site_Fake_Patch, 0xB8, 0x02, 0x00, 0x00, 0x00)
```

Match your handler's signature to its binding, and choose a macro for that binding's kind. Get either wrong and it won't compile.

### Context

In a site handler, you change the game's registers by writing to the context. `swofty::Context` is `SwoftyContext`, with these fields in order: `xmm0` to `xmm15` as `SwoftyXmm` (`u8[16]`, `u32[4]`, `u64[2]`, `f32[4]`, `f64[2]`), then `rflags`, `r15` down to `r8`, `rdi`, `rsi`, `rdx`, `rcx`, `rbx`, `rax`, `rbp`, `rsp`, `trampoline_rsp` and `rip`.

## Chains

Your mixin may share a target with other addons. swofty_core groups every addon's mixins by target and orders each group like this:

- Higher priorities run first and wrap the lower ones. The default is 0.
- Equal priorities run in load order: the plugins in `swofty/addons/core/`, then those in `swofty/addons/`, each in file-name order.
- Site handlers all run, in the same order.

To see the chain, look in `swofty/loader.log`, which prints each one as `swofty_core.dll(0) > my_plugin.dll(0)`.

| Case | Result |
|---|---|
| Two overwrites on one target | Conflict |
| Hooks on different targets whose patched bytes overlap | Conflict |
| Handlers below an overwrite | Warning |
| A binding that does not resolve | The plugin registers no mixins |
| A mixin not declared in `swofty/session.json`, when that file exists | The registration is refused |

A conflict drops every addon involved, so you lose all of their mixins, not just the conflicting ones. In a conflict in one mixin commit between an addon and one of swofty_core's own hook sets (`swofty_core.dll (...)` in the log), only the core's set is dropped.

## Init

```cpp
SWOFTY_ADDON()
SWOFTY_ADDON(check)
```

Use `SWOFTY_ADDON` to export `SwoftyAddonInit` and `SwoftyAddonApi`. You can pass `check` to run your own check during init. Init proceeds in this order:

1. Stores the API in `swofty::api`.
2. Resolves every binding.
3. Runs `check`, if you supplied it.
4. Registers every mixin.
5. Connects your plugin's [settings](./settings).

If the API is older than version 2, a binding does not resolve, or `check` returns `false`, init returns 0 and registers nothing. `SWOFTY_MOD` is another name for `SWOFTY_ADDON`.

## Objects

### subclass

```cpp
void **swofty::subclass(void *obj, std::initializer_list<std::pair<size_t, void *>> slots, size_t count = 32);
```

Use `subclass` to replace virtual table slots for one object. It copies the object's table, replaces the slots you list, and returns the copy; `count` controls how many slots it copies. Objects with the same table and the same replacements share one copy.

### fields

```cpp
template <class T> T *swofty::fields(const void *obj);
void swofty::release_fields(const void *obj);
```

To attach your addon's data to a game object, call `fields`: it gives you a zeroed `T`. Calls for the same object and addon return the same storage until `release_fields`. Use a trivially constructible and destructible `T`. If you ask for a different size on an object your addon already holds, you get `nullptr`.

## C ABI

To register mixins through the C ABI, pass an array of `SwoftyMixin` to `register_mixins`. You can only call it from `SwoftyAddonInit`:

```c
typedef struct SwoftyMixin {
    const char *target;   // a binding name swofty_core knows
    uint32_t kind;        // SWOFTY_MIXIN_* kind, optionally OR'd with SWOFTY_MIXIN_OVERWRITE
    int32_t priority;
    void *handler;
    void **orig;          // swofty_core writes the trampoline here (WRAP/VTABLE/REDIRECT)
} SwoftyMixin;
```

| Constant | Value |
|---|---|
| `SWOFTY_MIXIN_WRAP` | 1 |
| `SWOFTY_MIXIN_SITE` | 2 |
| `SWOFTY_MIXIN_VTABLE` | 3 |
| `SWOFTY_MIXIN_REDIRECT` | 4 |
| `SWOFTY_MIXIN_PATCH` | 5 |
| `SWOFTY_MIXIN_ON_LOAD` | 6 |
| `SWOFTY_MIXIN_OVERWRITE` | `0x100`, a flag on `kind` |

Use `SWOFTY_MIXIN_WITH_ARG(kind, arg)` to store the vtable slot or patch length in the top 16 bits of `kind`. `SWOFTY_MIXIN_KIND` and `SWOFTY_MIXIN_ARG` read them back. To get a binding's address, call `resolve(id)`; it returns 0 if the binding does not resolve. For an end-to-end walkthrough of the C ABI, see [Hooking a game function](/reference/addons/hooking).
