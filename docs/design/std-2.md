---
title: Standard library 2.0
description: The design for Swofty Launcher 2.0 - registries, providers, layers and hooks as the shape of the whole standard library, so addons can feed, read and override each other's data and share one hook per game function, without breaking any 1.x addon.
---

# Standard library 2.0: registries, providers and hooks

**Status:** proposal. **Ships as:** Swofty Launcher 2.0.0, `SWOFTY_API_VERSION` 9, registry table version 1.

Standard library 2.0 gives every extension point the same four parts:

- a **registry**: a named, typed collection of records;
- **providers**, which fill a registry;
- **layers**, through which other addons override a provider's records;
- **hooks**, through which the core runs each game function's handlers in one ordered chain.

Over these sit **operations**, the core's verified verbs that apply data to the game, and **services**, through which one addon calls another without `GetProcAddress`. The launcher sees the same registries as the game, so a launcher page edits exactly what the game reads.

Every 1.x addon keeps working: the 1.x tables, mixins and manifest fields stay and keep their meaning (see [Compatibility](#compatibility)).

## Contents

| Section | What it covers |
|---|---|
| [Where 1.x stands](#where-1-x-stands) | The current extension points and how they differ |
| [Goals](#goals) | What 2.0 has to do, and what it leaves out |
| [Concepts](#concepts) | Registries, records, providers, layers, scopes, snapshots, hooks, operations, services |
| [The ABI](#the-abi) | The new tables, their calls, threads and ownership |
| [Manifest](#manifest) | The new `swofty.json` fields |
| [The launcher side](#the-launcher-side) | Registry files in the build, host calls, conflicts, Send Logs |
| [First schemas](#first-schemas) | `career.season` and `career.driver_ratings` |
| [The 1.x modules in 2.0](#the-1-x-modules-in-2-0) | Each current module as a registry or hook |
| [Compatibility](#compatibility) | Why 1.x addons keep working, and how the release proves it |
| [Worked example](#worked-example-a-what-if-history-addon) | A "what if" history addon on top of Progressive Career |
| [Rollout](#rollout) | The releases, what each ships and its exit criteria |
| [Open questions](#open-questions) | Decisions still to make |

## Where 1.x stands

The 1.x standard library is a set of separate libraries, each with its own rules:

| Library | How an addon extends it | Scope | Conflicts |
|---|---|---|---|
| [Career contents](/reference/std/career-content) | Register a tile before the mixin commit | Per data group | First registration of a group wins |
| [Era picker boxes](/reference/std/era-picker) | Register a box before the mixin commit | Global | Every box shows |
| [Team HQ tabs](/reference/addons/manifest#hub-tab) (`hub_tab`) | A manifest field | Per data group | One tab per addon |
| [Career wizard](/reference/std/career-wizard) | Calls for one data group | Per data group | Last call wins |
| [Characters](/reference/std/characters) | Calls for one data group | Per data group | Last call wins |
| [Strings](/reference/std/strings) | Runtime overrides per key | Global | Last call wins |
| [Events](/reference/std/events) | Subscribe to a named event | Global | Every subscriber runs |
| [Mixins](/reference/std/mixin) | Wrap, site, redirect or vtable a game function | Global | Chains by priority; two overwrites drop both addons |
| [Storage](/reference/std/storage) | Files in the addon's own folder | The addon | None: no other addon can read them |

Addons share data in three ways today, and each one is private:

- **`GetProcAddress` exports.** Progressive Career's DLLs call `ProgressiveRealTeam`, `ProgressiveFollowTeam` and `ProgressiveNamesYear` in `progressive.dll`. Nothing outside Progressive knows these exist.
- **Storage files.** A storage file is readable only by the addon that owns it. A companion addon can't read or change another addon's `data/years.json`.
- **Separate mixins on the same game functions.** Progressive Career and Career Editor both reverse-engineer and wrap the contract, seat, rating and logo paths on their own.

## Goals

1. **One model for every extension point:** registration, scope, precedence, conflict reporting, versioning and feature detection work the same way in every library.
2. **Addons build on each other:** an addon can provide data, read the effective data of another addon's registry, override it in a layer, and call another addon's services, all through declared, versioned contracts.
3. **One hook per game function:** the core wraps each game function it exposes once, and runs every subscriber in a defined order with a defined result policy.
4. **Scopes with lifetimes:** a layer applies globally, to one profile, or to one career. A career keeps the values it started with.
5. **The launcher and the game agree:** a launcher page reads and writes the same records that the game reads.
6. **Verbs, not addresses:** the common changes (seats, ratings, numbers, logos, fuel) become core operations built on verified bindings, so an addon doesn't need its own reverse engineering for them.
7. **No 1.x addon stops working** in any 2.x release.

Out of scope for 2.0:

- Replacing mixins. Raw mixins stay for everything the standard library doesn't cover.
- A scripting language. Providers, layers and hooks are native code and JSON data.
- Changing the in-game loader's DLL loading, the build pipeline or the archive formats.

## Concepts

### Registries and records

A **registry** is a named collection of **records** of one schema. Names are dotted and lowercase: `career.season`, `career.driver_ratings`, `era.box`, `hub.tab`, `lang.string`.

- **Record id:** a string unique in its registry, such as `f1_career_progressive/2017` or `max_verstappen/2017`. A schema can define the id as a composite of fields.
- **Value:** a JSON object that matches the registry's schema. The C ABI passes values as UTF-8 JSON text. The C++ and Rust wrappers give typed views generated from the schema.
- **Schema:** a JSON Schema (draft 2020-12 subset) with two additions:
  - `x-key` on an array of objects names the field that identifies an element, so a layer can change one element without restating the array;
  - `x-snapshot: "career"` marks a registry whose effective values are frozen into each career (see [Snapshots](#snapshots)).
- **Owner:** the addon that declares the registry. The core owns the registries of its own libraries. The owner publishes the schema and decides its versions.
- **Schema version:** `major.minor`. A minor version only adds optional fields. A major version is a new contract, and the core can hold two majors of one registry side by side while consumers move.

### Providers

A **provider** fills a registry's **base layer**.

- **Static provider:** a JSON file in the addon, named in the manifest's `provides`. The launcher reads it too, so launcher pages see the base values without the game running.
- **Dynamic provider:** a callback registered from native code. The core calls it when a record is read and caches the result until the provider calls `invalidate`. A dynamic provider can also publish a static file, so the launcher sees its values.

One provider owns each record id. When two providers offer the same id, the later one in load order wins, and the core logs the clash as a conflict, which the launcher's conflicts view also lists.

### Layers

A **layer** is one addon's set of changes to one registry, at one scope.

- **A patch is a JSON Merge Patch** (RFC 7396) on the record. In an array marked `x-key`, elements are matched by key: a patch element with an existing key merges into that element, a new key appends, and `null` with a key removes the element.
- **Validation:** the core checks each patched result against the schema. A layer that fails is dropped as a whole, with one log line that names the addon, the record and the failing path. The launcher's conflicts view shows it. Nothing pops up in game.
- **Where layers come from:**
  - files named in the manifest's `overrides` (static layers);
  - native code, through `layer` and `clear_layer` (dynamic layers);
  - the launcher, through a plugin's host calls (profile layers, see [The launcher side](#the-launcher-side)).
- **Precedence:** base, then each addon's layers in load order, the same order that decides files and mixin chains today. Within one addon, its global layer comes before its profile layer. A later layer wins where two change the same field.

**Effective value** is the result of applying every layer to the base. Consumers read effective values. They can also ask for the origin of each field (which addon set it), for logs and launcher pages.

### Scopes

| Scope | Lifetime | Written by |
|---|---|---|
| `global` | Every profile, as long as the addon is on | The addon's static files and code |
| `profile` | One launcher profile | The addon's launcher pages, through host calls |
| `career` | One career, frozen at its creation | The core's snapshot, and in-game code that changes the career itself |

### Snapshots

A registry with `x-snapshot: "career"` keeps, in each career, the effective values it had when that career was created.

- **When:** at `career.starting` for a new career, the core computes the effective records for the career's data group and writes them into a core-owned save container in the profile save (`swofty_registry`, beside the containers that career contents already keep).
- **Reading:** while that career is loaded, reads return the snapshot plus any `career`-scope layers written during the career. Editing a profile layer later changes new careers, not the running one.
- **Migration:** a snapshot stores its schema version. On load, a snapshot from an older minor version reads as is. One from an older major version goes to the owner's `migrate` callback; with no callback, the career keeps the old values and logs that it did.
- **Size:** the game's save heap is small (a full `SAVE_DATA` heap crashed a 1.5 test build). Snapshots are compressed, budgeted per registry in the schema (`x-snapshot-budget`, default 64 KB), and a snapshot over budget is refused for that career with a log line.

### Hooks

A **hook point** is a named place where the core calls subscribers in one ordered chain.

- **Game hook points:** the core owns the mixin on the game function and exposes it as a point, such as `career.contract.decide`, `career.calendar.add_activity` or `session.fuel.load`. Each point has a versioned, size-prefixed C payload struct.
- **Addon hook points:** an addon can define a point and fire it from its own code, so other addons extend it. For example, Progressive Career's `progressive.season.apply` would let a companion adjust a season before Progressive applies it.
- **Phases:** each point has up to three.
  - `before` handlers run in order and may change the payload.
  - Then the first `replace` handler that returns handled replaces the game's code. With none, the game's code runs.
  - `after` handlers all run and see the result.
- **Order:** higher priority first, then load order, the same rule as mixin chains.
- **Mixins stay:** a hook point is one entry in that function's mixin chain, at the core's priority. A 1.x addon's raw `SWOFTY_WRAP` on the same function still chains around it. When a 1.x addon holds an overwrite on the function, the core's hook set is the one dropped, which is the 1.x rule today. The point then reports `unavailable`, and the log and the launcher's conflicts view name the addon holding the overwrite.
- **Events are hook points with only an `after` phase.** The 1.x events table stays as it is and fires from the same points.

### Operations

An **operation** is a core verb that changes the game, built on L2 bindings with independent verifies and tested in the core:

- `career.apply_lineup(season)`, `career.set_seat(team, slot, driver)`;
- `career.set_driver_ratings(driver, ratings)`, `career.set_car_number(driver, number)`;
- `team.set_logo(team, logo)`, `team.set_name(team, name)`;
- `session.set_track_fuel(track, fuel)`.

Operations take the same values as the registries, so a consumer reads an effective record and hands it to an operation. They move into the core one at a time, each when a second addon needs it. Their first versions come from the code that already does the job in Progressive Career (seats, ratings, numbers, logos, fuel) and Career Editor (the live career API).

### Services

A **service** is a versioned function table one addon publishes for others: `publish("progressive.history", 1, &table, sizeof table)` and `lookup("progressive.history", 1, &table, &size)`. It replaces `GetProcAddress` between addons:

- the table is versioned and size-checked;
- the manifest declares it, so the launcher can tell a consumer that its provider is missing or too old;
- a lookup before the provider has loaded is queued and answered once it does, so DLL load order stops mattering.

## The ABI

2.0 appends two tables to `SwoftyApi` and exports them for feature detection, the same pattern as `SwoftyCoreHub` and `SwoftyCoreHud`:

```c
#define SWOFTY_API_VERSION 9
#define SWOFTY_API_REGISTRY 1
#define SWOFTY_REGISTRY_VERSION 1
#define SWOFTY_HOOKS_VERSION 1

enum { SWOFTY_SCOPE_GLOBAL = 0, SWOFTY_SCOPE_PROFILE = 1, SWOFTY_SCOPE_CAREER = 2 };
enum { SWOFTY_PHASE_BEFORE = 0, SWOFTY_PHASE_REPLACE = 1, SWOFTY_PHASE_AFTER = 2 };

typedef struct SwoftyText { const char *data; size_t size; } SwoftyText;           /* UTF-8, not owned */
typedef int (*SwoftyProvideFn)(void *user, const char *registry, const char *id, char *out, size_t cap, size_t *len);
typedef void (*SwoftyRecordFn)(void *user, const char *registry, const char *id);
typedef int (*SwoftyMigrateFn)(void *user, const char *registry, SwoftyText old_value, uint32_t old_major,
                               char *out, size_t cap, size_t *len);

typedef struct SwoftyRegistry {
    uint32_t version;
    int (*available)(void);
    /* owners */
    int (*declare)(const char *registry, SwoftyText schema_json, SwoftyMigrateFn migrate, void *user);
    int (*schema)(const char *registry, uint32_t *major, uint32_t *minor);
    /* providers */
    int (*provide)(const char *registry, const char *id, SwoftyText value_json);
    int (*provide_fn)(const char *registry, SwoftyProvideFn fn, void *user);
    int (*invalidate)(const char *registry, const char *id);          /* id NULL: the whole registry */
    /* layers */
    int (*layer)(const char *registry, const char *id, SwoftyText patch_json, uint32_t scope);
    int (*clear_layer)(const char *registry, const char *id, uint32_t scope);
    /* reads */
    int (*get)(const char *registry, const char *id, char *out, size_t cap, size_t *len);
    int (*origin)(const char *registry, const char *id, const char *json_pointer, char *addon, size_t cap);
    int (*first)(const char *registry, char *id, size_t cap);
    int (*next)(const char *registry, const char *after, char *id, size_t cap);
    /* changes */
    int (*watch)(const char *registry, SwoftyRecordFn fn, void *user, uint32_t *token);
    int (*unwatch)(uint32_t token);
    /* services */
    int (*publish)(const char *service, uint32_t version, const void *table, size_t size);
    int (*lookup)(const char *service, uint32_t min_version, const void **table, size_t *size);
} SwoftyRegistry;

typedef struct SwoftyHookCall {
    uint32_t size;          /* sizeof(SwoftyHookCall) */
    uint32_t phase;
    const char *point;
    void *payload;          /* the point's payload struct, size-prefixed */
    int handled;            /* a replace handler sets 1 */
} SwoftyHookCall;
typedef void (*SwoftyHookFn)(void *user, SwoftyHookCall *call);

typedef struct SwoftyHooks {
    uint32_t version;
    int (*available)(void);
    int (*point)(const char *point, uint32_t *payload_version);     /* ok when the point is live */
    int (*attach)(const char *point, uint32_t phase, int priority, SwoftyHookFn fn, void *user, uint32_t *token);
    int (*detach)(uint32_t token);
    int (*define)(const char *point, uint32_t payload_version);     /* an addon's own point */
    int (*fire)(const char *point, void *payload);                  /* runs the before/replace/after chain */
} SwoftyHooks;

/* appended to SwoftyApi at version 9 */
    const SwoftyRegistry *registry;
    const SwoftyHooks *hooks;
```

Rules every call follows, the same as the 1.x tables:

- **Results:** every call returns the shared result codes (`ok`, `late`, `unavailable`, `invalid`), plus `SWOFTY_ASSET_TOO_SMALL` (4) when `cap` is too small, with `*len` set to the size needed.
- **Feature detection:** a wrapper whose loader offers a version below 9, or a null table, returns `unavailable`. `SwoftyCoreRegistry` and `SwoftyCoreHooks` are also exported, so an addon built against the 2.0 headers loads on a 1.x core and sees the libraries absent.
- **Append-only tables:** fields are only ever appended, and `version` names the last layout the core fills.
- **Ownership:** the core copies every string and JSON text it's given. Values it returns are written into the caller's buffer. The C++ wrapper retries with the size the core reports.
- **Threads:** registry reads, layers and service lookups work from any thread. Dynamic providers and `watch` callbacks run on the flow thread. Game hook points run on the thread the game calls the function on, which each point's reference page names. Callbacks must return quickly.
- **Timing:** `declare`, `provide`, `provide_fn`, `define` and `attach` are accepted from `SwoftyAddonInit` on. Like 1.x event subscriptions, a call made before the core's setup is queued and applied in load order.

C++ wrappers: `swofty/registry.hpp` (typed records generated from schemas by `tools/schemagen.py`), `swofty/hooks.hpp` (typed payloads per point) and `swofty/services.hpp`. Rust: the `swofty-sdk` crate mirrors them.

## Manifest

New `swofty.json` fields. Each one is optional, and an addon that uses any of them sets `"compat": { "launcher": ">=2.0.0" }`: a 1.x launcher rejects unknown fields, and the Addon Browser shows that version only to launchers inside the range.

| Field | Type | Meaning |
|---|---|---|
| `registries` | object[] | Registries this addon owns: `{name, schema}`, with `schema` a path to the JSON Schema |
| `provides` | object[] | Base records: `{registry, file}` for a static provider, `{registry, dynamic: true}` for one filled from code (optionally with `file` too) |
| `overrides` | object[] | Layers: `{registry, file?, scope, editable?}`. `editable` lets this addon's launcher pages write a profile layer of that registry |
| `hooks` | object[] | Hook points this addon attaches to: `{point, phase}`. The launcher shows them and checks they exist |
| `services` | object[] | Services published (`{name, version}`) and needed (`{name, min_version}`) |
| `requires` | object | Registry and service contracts this addon needs, as semver ranges: `{"career.season": "^1", "service:progressive.history": ">=1"}`. The launcher names a missing or too-old provider before Play |

## The launcher side

- **Registry files in the build:** at build, the launcher copies every enabled addon's static provider and layer files, and the active profile's layers, into `swofty/registry/` in the game folder, with an index in load order. The core reads the index at start. Profile layers live in `$DATA/registry/<profile>/<addon>/<registry>.json`.
- **Host calls for launcher plugins:**

  | Call | Result |
  |---|---|
  | `registry_schemas` | The registries in the profile, their owners and versions |
  | `registry_get` `{registry, id}` | The effective static value: base plus static and profile layers. Dynamic providers aren't running outside the game, so only their static files count |
  | `registry_list` `{registry}` | The record ids |
  | `registry_layer` `{registry, id}` | This plugin's own profile layer for a record |
  | `set_registry_layer` `{registry, id, patch}` | Validates and writes this plugin's profile layer; needs `editable` in the manifest. Errors name the failing path |

- **Conflicts:** the profile's conflicts view (`conflicts.json` today) also lists providers that clash, layers that failed validation, and hook points held by a 1.x overwrite.
- **Send Logs:** the bundle adds `registry/effective.json`, which holds every registry's effective records with each field's origin. The core writes it at career load and on a crash.
- **Required providers:** Play checks `requires` and names the missing or too-old addon, the same way it names a missing addon requirement today.

## First schemas

The first slice is the one two addons need now: Progressive Career, and a "what if" history editor (see the [worked example](#worked-example-a-what-if-history-addon)).

### `career.season` 1.0

Owner: the core. Snapshot: `career`. Id: `<data_group>/<year>`.

```json
{
  "data_group": "f1_career_progressive",
  "year": 2017,
  "teams": [
    { "slot": 1, "team": "mercedes", "seats": ["lewis_hamilton", "valtteri_bottas"] },
    { "slot": 8, "team": "toro_rosso", "seats": ["carlos_sainz-jr", "daniil_kvyat"] }
  ],
  "changes": [
    { "round": 1, "slot": 9, "seat": 1, "out": "pascal_wehrlein", "in": "antonio_giovinazzi", "kind": "stand_in", "until_round": 3 },
    { "round": 15, "slot": 8, "seat": 1, "out": "daniil_kvyat", "in": "pierre_gasly", "kind": "swap" }
  ]
}
```

- `teams` is keyed by `slot` (`x-key`): the career's team slot, 1 to 11, the career team that carries that season's team. `team` is the season's team key, the one Classic Seasons uses (`toro_rosso`, `force_india`, `lotus`). A slot with no team that season has `"team": null` and no seats.
- `seats` holds the two race seats in order, seat 0 then seat 1. A driver is in at most one seat of a season.
- `changes` is keyed by `round` + `slot` + `seat`. `kind` is `swap` (the new driver keeps the seat) or `stand_in` (the seat goes back after `until_round`). `round` counts from 1 and must be within the season's calendar.
- Driver ids are the game database labels the career already uses (`carlos_sainz-jr`). Unknown driver ids and team keys fail validation.

### `career.driver_ratings` 1.0

Owner: the core. Snapshot: `career`. Id: `<data_group>/<year>/<driver>`.

```json
{ "data_group": "f1_career_progressive", "year": 2017, "driver": "max_verstappen",
  "overall": 88, "experience": 74, "racecraft": 90, "awareness": 81, "pace": 91 }
```

Every rating is an integer from 0 to 100. A record sets the driver's ratings at the start of that season. The player's own driver then develops as in the stock game.

Progressive Career 2.0 provides both registries for its data group from `data/years.json`, `data/official_ratings.json` and `data/ratings_model.json`. Its static provider files are generated at build by `tools/registry.py`. Its DLLs read the effective values instead of `data/years.json`, so Choose Driver, Team Select, the grid, mid-season swaps, follow-your-driver and season ratings all use one merged view.

## The 1.x modules in 2.0

Each 1.x library keeps its table and header. In 2.0 its implementation becomes a registry or hook point, and its 1.x calls write into it:

| 1.x library | 2.0 registry or hook | What the 1.x call becomes |
|---|---|---|
| Career contents | `career.content` | `register` provides a record in the calling addon's name |
| Era picker boxes | `era.box` | `register` provides a record |
| Team HQ tabs (`hub_tab`) | `hub.tab` | The manifest field becomes a static provider |
| Career wizard | `career.wizard` (per data group) | Each call writes the calling addon's global layer of its data group's record |
| Characters | `career.character` | Each call writes a layer of the character's record |
| Strings | `lang.string` | `set` writes the calling addon's global layer of the key |
| Events | hook points with an `after` phase | `subscribe` attaches an `after` handler |
| Settings (`options`) | `addon.option` | Options are records the launcher already edits; the in-game ADDON SETTINGS reads them |
| Storage | unchanged | An addon's own files stay private; shared data moves to registries |
| Mixins | unchanged | Raw mixins chain with the core's hook points |

Where 1.x behaviour was "last call wins" (wizard, characters, strings), 2.0 keeps that result for a single addon. Across addons, load order decides. When two addons write the same field, that's now listed as a conflict instead of passing silently.

## Compatibility

**Nothing an existing addon uses is removed or changes meaning in any 2.x release.**

- **ABI:**
  - `SwoftyApi` is append-only. Version 9 adds `registry` and `hooks` after `era`. An addon that checks `api->version >= 8` still passes.
  - Every exported core table (`SwoftyCoreHub`, `SwoftyCoreHud`, `SwoftyCoreEra`, `SwoftyCoreCareerContent` and the rest) keeps its name, layout and version.
  - The result codes, feature gates and `available()` semantics don't change.
- **Behaviour:**
  - Each 1.x table becomes a shim over its registry (see [the module mapping](#the-1-x-modules-in-2-0)). For one addon, the shim produces the same visible result as 1.x: the same tile, box, tab, wizard route, string or character change.
  - Registration timing stays as permissive as 1.x: calls before the mixin commit queue as they do today.
- **Mixins:**
  - Chains, priorities and load order are unchanged.
  - A hook point is one more entry in a chain, and the core's set is the one dropped in a conflict with an addon, as in 1.x. So adding a hook point can't drop a 1.x addon.
  - The core installs a game hook point's mixin only once some addon attaches to that point. With no 2.0 consumer, a 2.0 core patches the same game bytes as 1.5.
- **Manifest:**
  - Every 1.x field keeps its meaning, and `schema_version` stays 1.
  - New fields are optional and need `compat.launcher >=2.0.0`.
  - A 1.x addon's database, ERP, text, UI, file and shader operations build to the same bytes, so the build pipeline's hash doesn't change for profiles that use no 2.0 feature.
- **Addon-to-addon calls:** Progressive Career keeps its `GetProcAddress` exports through 2.x, beside the new `progressive.history` service.

**How the release proves it:** the 2.0.0 release is gated on a compatibility suite in CI and in the Wine test:

1. **Every addon on the Addon Browser:** every published version that declares a launcher range including 1.5 is downloaded, built into a test profile with each 1.5 and 2.0 core, and compared:
   - the built game files hash the same;
   - the same native DLLs load, with no new dropped DLL;
   - the mixin chains print the same, apart from the core's own hook points, which show only when a 2.0 consumer attaches;
   - the `loaded`/`ok` lines each addon logs at start are the same.
2. **Shim tests:** for each 1.x table, a fixture addon makes every call, and the effective registry records are compared with the 1.x state the call used to produce.
3. **In game:** the existing in-game checks (a Progressive career start, an era picker box, a Team HQ tab, a wizard route) run on the 2.0 core with 1.x builds of those addons.

**Deprecation:** 1.x tables stay supported for all of 2.x. Removing one needs a 3.0 release, with the replacement documented, and at least one 2.x release that logs a note when a DLL calls it.

## Worked example: a "what if" history addon

`tails.whatif-history` lets players change each Progressive season's grid and each driver's ratings per season, from a launcher page.

```json
{
  "schema_version": 1,
  "id": "tails.whatif-history",
  "name": "What If History",
  "version": "1.0.0",
  "author": "Tails",
  "kind": ["native"],
  "category": "career",
  "compat": { "game": "f1_25", "launcher": ">=2.0.0" },
  "requires": { "career.season": "^1", "career.driver_ratings": "^1" },
  "overrides": [
    { "registry": "career.season", "scope": "profile", "editable": true },
    { "registry": "career.driver_ratings", "scope": "profile", "editable": true }
  ],
  "launcher": { "program": { "windows": "launcher/whatif.exe", "linux": "launcher/whatif" }, "sections": ["history"] }
}
```

1. **The launcher page lists seasons:** `registry_list career.season`, then `registry_get` for each year, which returns Progressive's base merged with any layers.
2. **The player swaps Hamilton and Alonso for 2017:** the page sends `set_registry_layer` for `f1_career_progressive/2017` with `{"teams": [{"slot": 1, "seats": ["fernando_alonso", "valtteri_bottas"]}, {"slot": 4, "seats": ["lewis_hamilton", "stoffel_vandoorne"]}]}`. The launcher validates the result: a patch that put Alonso at Mercedes without changing McLaren's seats would fail, because he'd hold two seats.
3. **Play:** the build copies the layer into `swofty/registry/`.
4. **A new 2017 Progressive career:** the core snapshots the effective 2017 season into the career. Choose Driver, the grid, follow-your-driver and the season's ratings all have Alonso at Mercedes and Hamilton at McLaren.
5. **Later edits:** changing the page again affects careers started after the change. The running career keeps its snapshot.

The addon needs no DLL. Progressive needs no knowledge of it.

## Rollout

| Release | Ships | Exit criteria |
|---|---|---|
| **2.0.0** | The core registry, layers, scopes, snapshots and services; the hooks table with addon-defined points; the launcher's registry files, host calls, conflicts and Send Logs dump; `career.season` and `career.driver_ratings`; Progressive Career 2.0 as provider and consumer, with the `progressive.history` service; the `templates/registry-override` example addon; the compatibility suite | The suite is green on every Addon Browser addon. Progressive 2.0 matches 1.2 in game (same seats, swaps, ratings, logos, fuel and follow contracts in a 2016 and a 2017 career). The example addon's 2017 grid change shows on Choose Driver, on the grid and in follow mode. The reference pages are written |
| **2.1.0** | The 1.x modules moved onto registries internally (hub, era, career contents, wizard, characters, strings, options); the first game hook points: `career.contract.decide`, `career.calendar.add_activity`, `session.fuel.load` | Shim tests are green. Progressive and Career Editor use the hook points instead of their own wraps of those functions. No 1.x addon's chain changes |
| **2.2.0** | `team.identity` (names, logos, numbers) and `driver.identity`; the operations `apply_lineup`, `set_seat`, `set_driver_ratings`, `set_car_number`, `set_team_logo`, `set_track_fuel` | Progressive and Career Editor call the operations instead of their own copies |
| **2.x** | More registries and points as second consumers appear | Each new registry has two addons using it before it's documented as stable |

Each release goes through the usual release flow: a prep commit on main, the main CI, then the tag. Each needs swofty's go-ahead in the chat where it's prepared.

## Open questions

- **JSON or typed binary records:** JSON keeps the ABI small and launcher-readable. Hot paths (anything read every frame) would need typed caches. The proposal is JSON at the ABI with generated typed views, revisited if profiling shows a cost.
- **Layer permissions:** today any addon can override any registry it declares. Should a registry owner be able to mark fields read-only, or require a manifest declaration per field?
- **Career-scope writes:** which in-game changes count as career-scope layers (an in-game editor, a contract the player signs) and which go straight to the game through operations?
- **Schema ownership for game data:** the core owns `career.*` because it's game data, not Progressive's. Should a provider be able to declare its own `career.*` registry, or only provide records into core ones?
- **Snapshot budget:** 64 KB per registry per career is a guess. It needs measuring against the save heap with Progressive's full 2016-2026 history.
- **Launcher range filtering:** confirm that the Addon Browser hides versions whose `compat.launcher` excludes the running launcher, and add it if it doesn't, before the first 2.0-only addon is published.
