---
title: Mixin API
description: The Advanced Mod Mixin System in swofty/mixin.hpp - bindings, mixin macros, chains and per-object fields.
---

# Mixin API

```cpp
#include <swofty/mixin.hpp>
#include <swofty/game.hpp>
```

The **Advanced Mod Mixin System**. A plugin declares mixins with the macros below, and `SWOFTY_ADDON()` registers them all with SwoftyMod, which installs them when every plugin has loaded. The [tutorial](/tutorials/hooking) walks through one.

## Bindings

A binding is a named place in `F1_25.exe`. The game headers declare one `inline constexpr` object per binding in `namespace swofty::game`, and the [Game API](/reference/game/) lists them. SwoftyMod resolves each binding a plugin uses by signature before the plugin's mixins register.

| Template | Kind | Members |
|---|---|---|
| `fn<"Name", R(A...)>` | `func` | `get()` returns the function pointer; `operator()(A...)` calls it. Variadic signatures work |
| `address<"Name">` | `func` | |
| `site<"Name">` | `site` | |
| `call<"Name">` | `call` | |
| `data<"Name", T>` | `data` | `get()` returns `T *`; `operator*` returns `T &` |
| `vtable<"Name", Slots>` | `vtable` | `get()` returns `void **`; `slots` is the slot count |

Every binding has `id`, the name string, `binding_kind`, and `address()`, the resolved address or 0. Each function binding also has a type alias `Name_t` for its signature:

```cpp
using FeHome_Handle_Command_t = uint64_t(FEHomeViewModel *vm, NeStr **cmd);
inline constexpr fn<"FeHome_Handle_Command", FeHome_Handle_Command_t> FeHome_Handle_Command{};
```

## Mixins

Each macro takes a binding name from `swofty::game` and opens the handler's body. Inside a handler, `orig` calls the next handler in the chain, and the game's code at its end.

| Macro | Binding | Handler |
|---|---|---|
| `SWOFTY_WRAP(target, R, params...)` | `func` | Replaces the function. `R` and `params` must match `target_t` |
| `SWOFTY_WRAP_PRIORITY(prio, target, R, params...)` | `func` | The same, with a priority |
| `SWOFTY_OVERWRITE(prio, target, R, params...)` | `func` | A wrap that replaces the function and does not call `orig` |
| `SWOFTY_SITE(target, param)` | `site` | Runs at the instruction with the registers, as `swofty::Context &` |
| `SWOFTY_SITE_PRIORITY(prio, target, param)` | `site` | The same, with a priority |
| `SWOFTY_VTABLE(target, slot, R, params...)` | `vtable` | Replaces one slot of a virtual table. `slot` must be below the binding's `slots` |
| `SWOFTY_VTABLE_PRIORITY(prio, target, slot, R, params...)` | `vtable` | The same, with a priority |
| `SWOFTY_REDIRECT(target, R, params...)` | `call` | Replaces the callee of one call instruction |
| `SWOFTY_PATCH(target, bytes...)` | `site` | Writes the bytes at the site. No handler |
| `SWOFTY_ON_LOAD` | | Runs once, after every mixin is installed |

These are from the Wine test's plugins in `native/test/addons/`. The vtable, call and patch bindings belong to its stand-in executable:

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

A handler whose signature differs from its binding, or a macro used on the wrong kind of binding, fails to compile.

### Context

`swofty::Context` is `SwoftyContext`: `xmm0` to `xmm15` as `SwoftyXmm` (`u8[16]`, `u32[4]`, `u64[2]`, `f32[4]`, `f64[2]`), then `rflags`, `r15` to `r8`, `rdi`, `rsi`, `rdx`, `rcx`, `rbx`, `rax`, `rbp`, `rsp`, `trampoline_rsp` and `rip`. A site handler's writes to it reach the game's registers.

## Chains

SwoftyMod groups every addon's mixins by target. In a group:

- Higher priorities run first and wrap the lower ones. The default is 0.
- Equal priorities run in load order: the plugins in `swofty/addons/core/`, then those in `swofty/addons/`, each in file name order.
- Site handlers all run, in the same order.

`swofty/loader.log` prints each chain as `swofty_core.dll(0) > my_plugin.dll(0)`.

| Case | Result |
|---|---|
| Two overwrites on one target | Conflict |
| Hooks on different targets whose patched bytes overlap | Conflict |
| Handlers below an overwrite | Warning |
| A binding that does not resolve | The plugin registers no mixins |
| A mixin not declared in `swofty/session.json`, when that file exists | The registration is refused |

A conflict drops every addon involved: none of their mixins install.

## Init

```cpp
SWOFTY_ADDON()
SWOFTY_ADDON(check)
```

`SWOFTY_ADDON` exports `SwoftyAddonInit` and `SwoftyAddonApi`. Init stores the API in `swofty::api`, resolves every binding, runs `check` when given, registers every mixin, then connects the plugin's [settings](./settings). It returns 0, and registers nothing, when the API is older than version 2, a binding does not resolve or `check` returns `false`. `SWOFTY_MOD` is another name for it.

## Objects

### subclass

```cpp
void **swofty::subclass(void *obj, std::initializer_list<std::pair<size_t, void *>> slots, size_t count = 32);
```

Gives one object a copy of its virtual table with the listed slots replaced, and returns the copy. `count` is the number of slots copied. Objects with the same table and the same replacements share one copy.

### fields

```cpp
template <class T> T *swofty::fields(const void *obj);
void swofty::release_fields(const void *obj);
```

Attaches a zeroed `T` to a game object, per addon. The same object and addon return the same storage until `release_fields`. `T` must be trivially constructible and destructible. Asking for a different size on an object the addon already holds returns `nullptr`.

## C ABI

`register_mixins` takes an array of `SwoftyMixin`, from `SwoftyAddonInit` only:

```c
typedef struct SwoftyMixin {
    const char *target;
    uint32_t kind;
    int32_t priority;
    void *handler;
    void **orig;
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

`SWOFTY_MIXIN_WITH_ARG(kind, arg)` stores the vtable slot or the patch length in the top 16 bits of `kind`. `SWOFTY_MIXIN_KIND` and `SWOFTY_MIXIN_ARG` read them back. `resolve(id)` returns a binding's address, or 0.
