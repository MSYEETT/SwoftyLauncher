---
title: The C ABI
description: The SwoftyAddonRegister export and the SwoftyRegistry, SwoftyHooks and SwoftyServices tables under the standard library 2.0 C++ wrappers.
---

# The C ABI

The typed C++ API sits on one new export, `SwoftyAddonRegister`, and three C tables appended to `SwoftyApi`.

## The registration export

<Split>

`SWOFTY_REGISTER` blocks collect into a list in the DLL. When the DLL has at least one, `SWOFTY_ADDON()` also exports `SwoftyAddonRegister`.

- The loader calls `SwoftyAddonRegister` at the early stage, before the game loads its database, on the loader thread, addon by addon in load order.
- Then `SwoftyAddonInit` and `SWOFTY_ON_LOAD` run as in 1.x.
- The build turns on `"early": true` for any DLL that exports `SwoftyAddonRegister`. The addon's `swofty.json` doesn't change.
- A DLL without `SWOFTY_REGISTER` exports only what 1.x exported.

<template #example>
<Example root="mod.esp" code-file="native/src/esp.cpp" files="swofty.json, native/build.sh, sounds/v10_screamer.bdl">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/sounds.hpp>
#include <swofty/systems/cars.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto v10 = sounds::engine_sounds.add("v10_screamer", {
        .bundle = file("sounds/v10_screamer.bdl"),
        .settings_from = sounds::engine_sounds["game/ferrari_2025"],
    });
    cars::vehicles.layer(cars::vehicles["swofty.classic-seasons-2017/ferrari"], [=](cars::Vehicle &v) {
        v.engine_sound = v10;
    });
}

SWOFTY_ADDON()

// what SWOFTY_ADDON() expands to in this DLL, trimmed
extern "C" __declspec(dllexport) const uint32_t SwoftyAddonApi = SWOFTY_API_VERSION;   // 9
extern "C" __declspec(dllexport) int SwoftyAddonRegister(const SwoftyApi *a)           // early stage
{
    return ::swofty::detail::run_register(a);   // every SWOFTY_REGISTER block, in source order
}
extern "C" __declspec(dllexport) int SwoftyAddonInit(const SwoftyApi *a)               // as in 1.x
{
    return ::swofty::detail::init(a, nullptr);
}

// what the two calls above send to the registry table
//   a->registry->add("engine_sound", "v10_screamer", {"bundle":"sounds/v10_screamer.bdl",...}, id, cap)
//                                                    -> id "mod.esp/v10_screamer"
//   a->registry->layer_fn("vehicle", "swofty.classic-seasons-2017/ferrari", edit, user, SWOFTY_SCOPE_GLOBAL)
```

</template>
</Example>
</template>
</Split>

```text
addon esp.dll: calling SwoftyAddonRegister (API v9) at the early stage
[registry] register: mod.esp (esp.dll) 1 engine_sound, 1 layer on vehicle (early stage, 0.4 ms)
addon esp.dll: calling SwoftyAddonInit (API v9)
```

## The tables

Like every 1.x table, the three tables are appended to `SwoftyApi` and also exported for feature detection. So an addon built against the 2.0 headers still loads on a 1.x core, and sees the libraries as unavailable.

```c
/* swofty_api.h */
#define SWOFTY_API_VERSION 9
#define SWOFTY_REGISTRY_VERSION 1
#define SWOFTY_HOOKS_VERSION 1
#define SWOFTY_SERVICES_VERSION 1

enum { SWOFTY_SCOPE_GLOBAL = 0, SWOFTY_SCOPE_PROFILE = 1, SWOFTY_SCOPE_CAREER = 2 };
enum { SWOFTY_PHASE_BEFORE = 0, SWOFTY_PHASE_REPLACE = 1, SWOFTY_PHASE_AFTER = 2 };
enum { SWOFTY_TOO_SMALL = 4 };   /* joins ok/late/unavailable/invalid: *len holds the size needed */

typedef struct SwoftyText { const char *data; size_t size; } SwoftyText;   /* UTF-8, borrowed for the call */

/* provider: write the record's JSON into out; return SWOFTY_ASSET_OK, or SWOFTY_ASSET_INVALID for "not mine" */
typedef int (*SwoftyProvideFn)(void *user, const char *registry, const char *id, char *out, size_t cap, size_t *len);
typedef void (*SwoftyListFn)(void *user, const char *registry, void (*add)(void *ctx, const char *id), void *ctx);
typedef void (*SwoftyRecordFn)(void *user, const char *registry, const char *id);   /* id "" ends a batch */
typedef int (*SwoftyConvertFn)(void *user, SwoftyText in, char *out, size_t cap, size_t *len);
typedef void (*SwoftyCheckFn)(void *user, SwoftyText value, void (*error)(void *ctx, const char *path, const char *msg), void *ctx);
/* edit: in is the effective value so far; write the edited value into out */
typedef int (*SwoftyEditFn)(void *user, SwoftyText in, char *out, size_t cap, size_t *len);
/* selector: non-zero when the record matches; base is the record's base value, before layers */
typedef int (*SwoftyMatchFn)(void *user, SwoftyText base);

typedef struct SwoftyRegistry {
    uint32_t version;
    int (*available)(void);
    int (*declare)(const char *registry, SwoftyText schema_json);
    int (*check)(const char *registry, SwoftyCheckFn fn, void *user);
    int (*adapter)(const char *registry, uint32_t from_major, uint32_t to_major,
                   SwoftyConvertFn up, SwoftyConvertFn down, void *user);
    int (*provide)(const char *registry, const char *id, SwoftyText value_json);
    int (*provide_fn)(const char *registry, SwoftyProvideFn fn, SwoftyListFn list, void *user);
    int (*invalidate)(const char *registry, const char *id);        /* id NULL: every record */
    int (*layer)(const char *registry, const char *id, SwoftyText patch_json, uint32_t scope);
    int (*clear_layer)(const char *registry, const char *id, uint32_t scope);
    int (*get)(const char *registry_at_major, const char *id, char *out, size_t cap, size_t *len);
    int (*status)(const char *registry, const char *id);
    int (*origin)(const char *registry, const char *id, const char *pointer, char *addon, size_t cap);
    int (*first)(const char *registry, const char *prefix, char *id, size_t cap);
    int (*next)(const char *registry, const char *prefix, const char *after, char *id, size_t cap);
    int (*watch)(const char *registry, SwoftyRecordFn fn, void *user, uint32_t *token);
    int (*unwatch)(uint32_t token);
    int (*last_error)(char *out, size_t cap);                       /* this thread's last refusal */

    /* the typed API: what Registry<T> calls */
    int (*add)(const char *registry, const char *key, SwoftyText value_json, char *id, size_t cap);
    int (*variant)(const char *registry, const char *key, const char *base_id,
                   SwoftyEditFn edit, void *user, char *id, size_t cap);
    int (*layer_fn)(const char *registry, const char *id, SwoftyEditFn edit, void *user, uint32_t scope);
    int (*layer_select)(const char *registry, SwoftyMatchFn match, const char *prefix,
                        const char *const *except, size_t except_count,
                        SwoftyEditFn edit, void *user, uint32_t scope);
    int (*option)(const char *id, uint32_t type, SwoftySettingValue *out);
    int (*migrate)(const char *registry, uint32_t from_major, uint32_t to_major, SwoftyConvertFn fn, void *user);
} SwoftyRegistry;

typedef struct SwoftyHookCall {
    uint32_t size;
    uint32_t phase;
    const char *point;
    void *payload;           /* the point's payload struct: uint32_t size, uint32_t payload_version, ... */
    int replaced;            /* set by the core after the replace phase */
} SwoftyHookCall;
typedef int (*SwoftyHookFn)(void *user, SwoftyHookCall *call);  /* replace phase: non-zero = handled */

typedef struct SwoftyHooks {
    uint32_t version;
    int (*available)(void);
    int (*live)(const char *point, uint32_t *payload_version);
    int (*attach)(const char *point, uint32_t phase, int32_t priority, SwoftyHookFn fn, void *user, uint32_t *token);
    int (*detach)(uint32_t token);
    int (*define)(const char *point, uint32_t payload_version);
    int (*fire)(const char *point, void *payload, int *replaced);
} SwoftyHooks;

typedef void (*SwoftyServiceFn)(void *user, const void *table, size_t size, uint32_t version);

typedef struct SwoftyServices {
    uint32_t version;
    int (*available)(void);
    int (*publish)(const char *name, uint32_t version, const void *table, size_t size);
    int (*when)(const char *name, uint32_t min_version, size_t min_size, SwoftyServiceFn fn, void *user);
} SwoftyServices;

/* SwoftyApi, version 9: appended after era */
    const SwoftyRegistry *registry;
    const SwoftyHooks *hooks;
    const SwoftyServices *services;

/* core exports, for an addon that must also load on a 1.x core */
SWOFTY_EXPORT const SwoftyRegistry *SwoftyCoreRegistry(void);
SWOFTY_EXPORT const SwoftyHooks *SwoftyCoreHooks(void);
SWOFTY_EXPORT const SwoftyServices *SwoftyCoreServices(void);

/* addon export, emitted by SWOFTY_ADDON() when the DLL has a SWOFTY_REGISTER */
SWOFTY_EXPORT int SwoftyAddonRegister(const SwoftyApi *api);
```

## What the C++ wrapper calls

```text
Registry<T>{name, options}       declare        the schema the wrapper generates from SWOFTY_RECORD and RegistryOptions
                                                (x-key from Keyed, x-applies, x-snapshot, x-snapshot-budget, x-version)
add(key, value)                  add            id comes back as "<addon id>/<key>"
variant(key, base, edit)         variant
provide(fn, list)                provide_fn
invalidate(id)                   invalidate
layer(target, edit, scope)       layer_fn
layer(select, edit, scope)       layer_select   where(): match; under(): prefix; .except(): except
clear_layer(target, scope)       clear_layer
check(fn)                        check
get(id)                          get            "<name>@<T::version.major>", e.g. "career.season@1"
status(id)                       status
origin(id, field)                origin
all(prefix)                      first, next
watch(fn)                        watch          the Token's destructor and reset() call unwatch
swofty::option<V>(id)            option
swofty::adapter(from, to, ...)   adapter
swofty::migrate(from, to, fn)    migrate
data files, C callers            provide (a JSON value), layer (a JSON merge patch)
```

An edit crosses the ABI as JSON:

- The core calls the `SwoftyEditFn` with the effective value so far.
- The wrapper reads it into `T` through `SWOFTY_RECORD`, runs the edit, and writes the struct's fields back over the value it got. Fields `T` doesn't list come through unchanged.
- The core diffs `in` and `out`. The changed fields become the edit's origins and its conflicts. It then validates the result. A failing edit is dropped, with a log line naming the addon.
- `option` called during an edit records that the edit depends on that option, and the core runs the edit again when the player changes it. Outside an edit, it's a plain read of the calling addon's option, like 1.x `setting`.

## Rules

- **Copies:** the core copies every string and JSON text it's given. Values come back in the caller's buffer. When the buffer is too small, the call returns `SWOFTY_TOO_SMALL` with the size needed, and the C++ wrapper retries.
- **Results:** the shared result codes plus `SWOFTY_TOO_SMALL`. A refusal leaves its reason in `last_error` for the calling thread.
- **Timing:**
  - the registry calls (`declare`, `add`, `variant`, `provide`, `provide_fn`, `layer`, `layer_fn`, `layer_select`) are accepted from `SwoftyAddonRegister` on, and at any time after it while the game runs. A change made after the game read that data takes effect at the next moment it reads it, and the core logs when ("applies from the next session load", "applies from the next game start");
  - `define`, `attach`, `publish` and `when` are accepted from `SwoftyAddonInit` on;
  - a call before the core's setup queues and is applied in load order, the same as 1.x event subscriptions.
- **Threads:**
  - `SwoftyAddonRegister` runs on the loader thread;
  - registry reads, layers, `publish` and `when` work from any thread;
  - providers, edits and selector matches run on the thread that resolves the record. They must return quickly and not read the registry they belong to (a re-entrant read returns `late`);
  - watchers and `when` callbacks run on the flow thread;
  - game hook points run on the thread the game calls the function on.
- **Growth:** fields are only appended. A table's `version` names the last layout the core fills (see [Versioning](/std/versioning#abi-and-table-versions)). On a 1.x core, `SwoftyAddonRegister` is never called, and the 2.0 libraries read as unavailable.
