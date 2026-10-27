---
title: The C ABI
description: The SwoftyRegistry, SwoftyHooks and SwoftyServices tables under the standard library 2.0 C++ wrappers.
---

# The C ABI

The C++ wrappers on the other pages sit on three C tables. Like every 1.x table, they're appended to `SwoftyApi` and also exported for feature detection. So an addon built against the 2.0 headers still loads on a 1.x core, and sees the libraries as unavailable.

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

/* exports, for an addon that must also load on a 1.x core */
SWOFTY_EXPORT const SwoftyRegistry *SwoftyCoreRegistry(void);
SWOFTY_EXPORT const SwoftyHooks *SwoftyCoreHooks(void);
SWOFTY_EXPORT const SwoftyServices *SwoftyCoreServices(void);
```

## Rules

- **Copies:** the core copies every string and JSON text it's given. Values come back in the caller's buffer. When the buffer is too small, the call returns `SWOFTY_TOO_SMALL` with the size needed, and the C++ wrapper retries.
- **Results:** the shared result codes plus `SWOFTY_TOO_SMALL`. A refusal leaves its reason in `last_error` for the calling thread.
- **Timing:** `declare`, `provide`, `provide_fn`, `define`, `attach`, `publish` and `when` are accepted from `SwoftyAddonInit` on. A call before the core's setup queues and is applied in load order, the same as 1.x event subscriptions.
- **Threads:**
  - registry reads, layers, `publish` and `when` work from any thread;
  - providers run on the reading thread;
  - watchers and `when` callbacks run on the flow thread;
  - game hook points run on the thread the game calls the function on.
- **Growth:** fields are only appended. A table's `version` names the last layout the core fills (see [Versioning](./versioning#abi-and-table-versions)).
