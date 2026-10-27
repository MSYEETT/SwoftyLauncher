---
title: Versioning
description: How registries, hook payloads, services and the C ABI change over time in standard library 2.0 without breaking the addons that use them.
---

# Versioning

Everything other addons depend on has a version. They all follow one rule:

**Adding is a minor change. Removing, renaming or changing what something means is a major change, and a major change keeps the old version working beside the new one.**

What carries a version:

- **Launcher:** semver, `2.0.0`. A major launcher version is the only place an old interface may be removed.
- **C ABI:** `SWOFTY_API_VERSION` (an integer, 9 in 2.0). `SwoftyApi` and every core table only ever grow.
- **Core tables:** each table's `version` field names its last layout.
- **Registry schemas:** `major.minor`, in `x-version`.
- **Hook payloads:** a size-prefixed struct with a payload version.
- **Services:** an integer version per service.
- **Snapshots:** each one stores the schema version it was written with.

## Where the versions live

The schemas, payload structs and examples for the core's registries live in the SDK, one folder per registry. A released file is never edited again:

```text
packages/sdk/std/
  registries/
    career.season/
      1.0.json              schema, released with 2.0.0, never edited again
      1.1.json              schema, adds an optional field
      season.hpp            the typed struct, follows the newest minor
      examples/
        2016.json           validated against every schema of this major in CI
        whatif-layer.json
      CHANGES.md            one line per version: what changed and in which launcher
    career.driver_ratings/
      1.0.json
      ...
  hooks/
    career.contract.decide.hpp   the payload struct and its version history
  services/
    README.md                    services are owned by addons; this lists the known ones
```

An addon that owns registries keeps the same layout in its own folder: `schemas/<name>.<major>.<minor>.json`.

## Minor changes

A minor change adds something optional. Code written for the older minor keeps working without a rebuild.

```diff
 // career.season 1.0.json -> 1.1.json
-  "x-version": "1.0",
+  "x-version": "1.1",
   "$defs": {
     "team": {
       "required": ["slot", "seats"],
       "properties": {
         "slot":  { "type": "integer", "minimum": 1, "maximum": 11 },
         "team":  { "type": ["string", "null"] },
-        "seats": { "type": "array", "items": { "type": "string" }, "maxItems": 2 }
+        "seats": { "type": "array", "items": { "type": "string" }, "maxItems": 2 },
+        "reserve": { "type": "string", "x-ref": "db:drivers", "x-since": "1.1" }
       }
     }
   }
```

Minor changes are:

- a new optional field;
- a new `enum` value, which readers must handle as "unknown" (see the rule below);
- looser validation: a larger range, an optional field that used to be required;
- a new registry, hook point, service function or core table field.

## Major changes

Anything that can break existing code is major:

- removing or renaming a field;
- changing a field's type or meaning;
- making validation stricter;
- adding or changing an `x-key`;
- changing an id format.

A major version is a new schema file next to the old one. The owner registers an **adapter** that converts in both directions, so the old major keeps working:

```cpp
// 2.0 turns each seat from a driver id into an object with the car number
swofty::registry::adapter("career.season", /* from */ 1, /* to */ 2,
    [](swofty::json::Value v) {                          // 1 -> 2: upgrade
        for (auto &team : v["teams"].items()) {
            swofty::json::Value seats = swofty::json::array();
            for (auto &driver : team["seats"].items())
                seats.push(swofty::json::object({ { "driver", driver }, { "number", nullptr } }));
            team["seats"] = seats;
        }
        return v;
    },
    [](swofty::json::Value v) {                          // 2 -> 1: downgrade for old readers
        for (auto &team : v["teams"].items()) {
            swofty::json::Value seats = swofty::json::array();
            for (auto &seat : team["seats"].items())
                seats.push(seat["driver"]);
            team["seats"] = seats;
        }
        return v;
    });
```

With the adapter in place:

- **Old readers:** a 1.x reader that calls `get<Season>` (built against 1.x) gets the effective value downgraded to 1.x.
- **Old layers:** a 1.x addon's layer is upgraded to 2.0 before it's applied, so it still overrides.
- **Old snapshots:** a career saved with 1.x is upgraded on load.
- **Old majors are kept** for the rest of the launcher major they shipped in, and dropped only in the next launcher major.

A reader asks for the major it was built against:

```cpp
// built against career.season 1.x: Season::version is {1, 1}, so get<Season> asks for major 1
std::optional<Season> s = swofty::registry::get<Season>(id);

// untyped readers name the major
std::optional<swofty::json::Value> v = swofty::registry::get("career.season@1", id);
```

## The tolerant reader rule

Every reader must ignore what it doesn't know. That is what makes minor changes safe:

- **Unknown fields are kept, not rejected.** The typed structs put them in `extra` and write them back unchanged.
- **Unknown `enum` values mean "something newer".** Handle them like the safest known value, and don't fail.
- **An unknown hook payload field is never read.** A payload struct only grows, and `size` says how much of it the core filled.

```cpp
switch (change.kind) {
case ChangeKind::swap:     apply_swap(change); break;
case ChangeKind::stand_in: apply_stand_in(change); break;
default:                   swofty::log("[grid] change kind %d is newer than this addon; skipped", int(change.kind));
}
```

The SDK's `from_json` already follows this rule, so a hand-written addon that sticks to the typed structs gets it for free.

## Lifecycle

Every registry, hook point, service and table field has a status:

```json
// packages/sdk/std/registries/career.season/status.json
{ "status": "stable", "since": "2.0.0" }

// a newer one
{ "status": "experimental", "since": "2.2.0", "feature": "registry_team_identity" }

// on its way out
{ "status": "deprecated", "since": "2.0.0", "deprecated": "2.4.0", "replacement": "career.season@2" }
```

- **experimental:** off unless an addon lists the feature in `experimental_features`. It may change in any release, minor rules don't apply yet. The docs mark it.
- **stable:** the rules on this page apply.
- **deprecated:** still works. The core logs once per session when an addon uses it, naming the replacement. Removed only in the next launcher major.

To go from experimental to stable, a registry or hook point needs:

- two addons using it, at least one of them not the owner;
- tests and examples in the SDK;
- written docs;
- one release as experimental.

## ABI and table versions

The C ABI follows the same rule. A field is only ever appended, and code checks the version before touching a newer field:

```cpp
// how the C++ wrapper checks for a field added in registry table version 2
inline Result origin(std::string_view registry, std::string_view id, std::string_view pointer, std::string &out)
{
    const SwoftyRegistry *t = detail::table();
    if (!t || t->version < 2 || !t->origin)        // origin arrived in table version 2
        return Result::unavailable;
    ...
}
```

- An addon built against the 2.0 headers loads on a 1.x core and sees every 2.0 library as unavailable, through `SwoftyCoreRegistry` and `SwoftyCoreHooks`.
- An addon built against 1.x headers loads on a 2.0 core unchanged (see [Compatibility](./compatibility)).
- `SWOFTY_API_VERSION` goes up by one whenever `SwoftyApi` gains a field. A table's own `version` goes up whenever that table gains a field.

## Making a change

Every change to a core registry, hook point, operation or table goes through the same steps, in one pull request:

1. **Write it in the schema or header first:** the new schema file, or the payload struct with its new version.
2. **Add examples:** at least one valid record or call, and one that must fail.
3. **Bump the version:** minor or major by the rules above. A major change adds its adapter and migration.
4. **Implement and test it in the core.**
5. **Write the docs page section,** with a full example (see [Rollout and docs](./rollout#the-docs)).
6. **Add a line to `CHANGES.md`,** which the release notes quote.

CI enforces the mechanical parts:

- a released schema file that changed fails the build;
- `tools/std/schemacheck.py` compares each new schema with the last released one and fails when the change is major but the version only went up a minor;
- every example is validated against its schema;
- every snapshot fixture in `packages/sdk/std/fixtures/` (saved careers from older versions) has to load and migrate;
- the [compatibility suite](./compatibility#the-release-gate) runs on every release.

## Saves across launcher versions

- **A newer launcher reading an older career:** snapshots migrate as above.
- **An older 2.x launcher reading a career saved by a newer one:** an unknown minor is read as is, by the tolerant reader rule. An unknown major is left untouched in the save, and that career reads those registries live, with a log line.
- **A 1.x launcher reading a 2.x career:** the `swofty_registry` container is kept by the save splice and otherwise ignored. The career plays as it would have in 1.x.
