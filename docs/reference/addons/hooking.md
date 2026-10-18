---
title: The mixin system
description: Hook an F1 25 function from a plugin - name a target, write a handler, register it with SwoftyMixin, and call the trampoline.
---

# The mixin system

To hook the F1 25 engine from your [engine plugin](/reference/addons/native-plugins), you give the mixin system a target's binding name and a handler, then call `register_mixins` from `SwoftyAddonInit`. Once every plugin has loaded, swofty_core resolves each target by signature. If several addons hook the same target, it chains their handlers together.

Here you'll work through the C ABI in `swofty_api.h` to see how hooks fit together. For real plugins, use the [C++ Mixin API](/reference/std/mixin): it wraps each of these steps in type-safe macros.

## Targets

You choose a target by its binding: a named place in `F1_25.exe` that swofty_core knows from `swofty/addons/core/bindings.json`. Pass that name as a string, and swofty_core scans the executable for its signature. To find a binding, look in the [Game API](/reference/game/), which lists every binding with its kind and signature.

- `api->resolve("FeHome_Handle_Command")` returns a binding's resolved address, or 0.
- `api->scan(".xtls", "48 89 5C 24 ?? 57 48 83 EC 20", &out)` finds a byte pattern in a section; it returns the hit count and sets `out` only on exactly one match.

A target needs exactly one match to resolve. With zero or multiple matches, swofty_core drops your addon and installs none of its mixins. Check `swofty/loader.log` for the binding's name.

## Wrap a function

Use `SWOFTY_MIXIN_WRAP` to replace a function with your handler. swofty_core writes a trampoline into the `orig` pointer. From your handler, call `orig` to pass control to the next handler in the chain; the game's function sits at the end.

Start by wrapping `FeHome_Handle_Command`, which runs when you press a home-screen button:

```c [native/src/my_plugin.c]
#include <swofty_api.h>

static const SwoftyApi *g_api;
static uint64_t (*orig_handle)(void *vm, void *cmd);

static uint64_t my_handle(void *vm, void *cmd)
{
    g_api->log("[my-addon] home command");
    return orig_handle(vm, cmd);
}

SWOFTY_EXPORT int SwoftyAddonInit(const SwoftyApi *api)
{
    g_api = api;
    static SwoftyMixin mixins[] = {
        { "FeHome_Handle_Command", SWOFTY_MIXIN_WRAP, 0, (void *)my_handle, (void **)&orig_handle },
    };
    return api->register_mixins(mixins, 1);
}
```

- Give `my_handle` the same signature as the game's function. Pointer parameters at the C ABI are `void *`; you'll find their real types in the [Game API](/reference/game/).
- Keep `orig_handle` as `static` so swofty_core's trampoline outlives `SwoftyAddonInit`. Call it when you want the rest of the chain to run. Skip it to replace the game's behaviour.
- Call `register_mixins` only from `SwoftyAddonInit`. Elsewhere, swofty_core refuses the call and logs `register_mixins outside SwoftyAddonInit; refused`. Returning its result gives init a success flag: 1 when the mixins register, 0 when a malformed mixin is rejected.

Next, build the DLL and name it in `operations.native` with the `hook` capability:

```json [swofty.json]
"operations": {
  "native": [{ "dll": "native/my_plugin.dll", "capabilities": ["log", "hook"] }]
}
```

Install your addon, enable it, then launch the game. Look in `swofty/loader.log` to see the installed chain:

```text
[core] wrap FeHome_Handle_Command at 0x1442f1a40 (base+0x2f1a40) installed: swofty_core.dll(0) > my_plugin.dll(0)
```

You'll see both plugins here because swofty_core already wraps `FeHome_Handle_Command` for ADDON SETTINGS.

## Run code at an instruction

To run your code at an address inside a function, use a site binding. `SWOFTY_MIXIN_SITE` runs your handler there and gives it the CPU registers in a `SwoftyContext`, where you can read and write them. Leave `orig` null for a site.

Here you'll read `r14` at `site_FeHome_AfterCareerCall`, the instruction immediately after the game adds the Career tile. At that point, `r14` holds the `TileList *`:

```c [native/src/my_plugin.c]
static void after_career(SwoftyContext *c)
{
    void *tile_list = (void *)c->r14;
    g_api->log("[my-addon] tile list at %p", tile_list);
    c->r12 += 1;
}
```

Register it alongside the wrap:

```c
static SwoftyMixin mixins[] = {
    { "FeHome_Handle_Command",       SWOFTY_MIXIN_WRAP, 0, (void *)my_handle,    (void **)&orig_handle },
    { "site_FeHome_AfterCareerCall", SWOFTY_MIXIN_SITE, 0, (void *)after_career, NULL },
};
```

`SwoftyContext` holds `xmm0`–`xmm15`, then `rflags`, `r15` down to `r8`, `rdi`, `rsi`, `rdx`, `rcx`, `rbx`, `rax`, `rbp`, `rsp`, `trampoline_rsp` and `rip`. Write to any of them, and your change reaches the game's register when the handler returns.

## The SwoftyMixin record

```c
typedef struct SwoftyMixin {
    const char *target;   // the binding name, or null for SWOFTY_MIXIN_ON_LOAD
    uint32_t kind;        // a kind below, optionally OR'd with SWOFTY_MIXIN_OVERWRITE
    int32_t priority;     // higher runs first; default 0
    void *handler;        // the handler function
    void **orig;          // swofty_core writes the trampoline here (WRAP, VTABLE, REDIRECT); null otherwise
} SwoftyMixin;
```

| Kind | Value | Handler |
|---|---|---|
| `SWOFTY_MIXIN_WRAP` | 1 | Wraps a `func` binding; calls `orig` |
| `SWOFTY_MIXIN_SITE` | 2 | `void(SwoftyContext *)` at a `site` binding |
| `SWOFTY_MIXIN_VTABLE` | 3 | Replaces one slot of a `vtable` binding |
| `SWOFTY_MIXIN_REDIRECT` | 4 | Replaces the callee of a `call` binding |
| `SWOFTY_MIXIN_PATCH` | 5 | Writes bytes at a `site` binding; `handler` points to the bytes |
| `SWOFTY_MIXIN_ON_LOAD` | 6 | `void(void)`, after every mixin installs; `target` null |

Use `SWOFTY_MIXIN_OVERWRITE` (`0x100`), OR'd onto a wrap's `kind`, to mark a handler that doesn't call `orig`. For a vtable slot or patch length, use `SWOFTY_MIXIN_WITH_ARG(kind, arg)`; it stores the value in the top 16 bits of `kind`.

## Chains

swofty_core groups every addon's mixins by target. When other addons hook your target, here's how their handlers fit together:

- Higher priorities run first and wrap the lower ones. The default is 0.
- Equal priorities run in load order: `swofty/addons/core/` first, then `swofty/addons/`, each in file-name order.
- Site handlers all run, in the same order.

| Case | Result |
|---|---|
| Two overwrites on one target | Conflict |
| Hooks whose patched bytes overlap | Conflict |
| Handlers below an overwrite | Warning |
| A binding that does not resolve | The addon is dropped |

If there's a conflict, every addon involved is dropped and none of their mixins are installed. You'll find each addon's name in `swofty/loader.log`.

## Rules

- Hook bindings, not fixed addresses.
- Hook only `.xtls` code. Never patch `.data1` bodies.
- Keep hook setup out of `DllMain`.

When you use the [C++ Mixin API](/reference/std/mixin), you also get `SWOFTY_WRAP`, `SWOFTY_SITE`, `SWOFTY_VTABLE`, `SWOFTY_REDIRECT`, `SWOFTY_PATCH`, `SWOFTY_ON_LOAD`, typed bindings from the [Game API](/reference/game/), per-object [fields](/reference/std/mixin#fields) and [`subclass`](/reference/std/mixin#subclass).
