---
title: Versioning
description: How registries, hook payloads, services and the C ABI change over time in standard library 2.0 without breaking the addons that use them.
---

# Versioning

Every registry, hook payload, service and C table carries a version, and they all follow one rule:

**The standard library only ever grows. Adding is a minor change. Renaming or changing what something means is a major change, and a major change is added next to the old version, which keeps working in every later release. Nothing is ever removed.**

## A minor change

<Split>

`career.season` 1.1 gives each team an optional reserve driver. The struct gains one field. Code built against 1.0 keeps working without a rebuild, and an addon built against 1.1 can set the new field.

The struct's `version` is the schema it follows. A header follows the newest minor of its major.

<template #example>
<Example root="tails.whatif-history" code-file="native/src/whatif.cpp" data-file="registry/layers.json" files="swofty.json, native/build.sh">
<template #code>

```cpp
// tails.whatif-history: an addon built against 1.1 sets the new field
#include <swofty/registry.hpp>
#include <swofty/systems/career.hpp>
#include <swofty/systems/drivers.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    career::seasons.layer(career::seasons["swofty.progressive/2017"], [](career::Season &s) {
        s.teams.at(4).reserve = drivers::identities["game/jenson_button"];
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/layers.json
{
  "career.season": {
    "swofty.progressive/2017": { "teams": [ { "slot": 4, "reserve": "game/jenson_button" } ] }
  }
}
```

</template>
</Example>
</template>
</Split>

The header change, in `packages/sdk/std/registries/career.season/season.1.hpp`:

```diff
 struct SeasonTeam {
     int slot = 0;                                   // 1-11, the career team that carries this season's team
     Ref<teams::Identity> team;                      // empty: an empty slot
     std::array<Ref<drivers::Identity>, 2> seats;
+    Ref<drivers::Identity> reserve;                 // since 1.1: optional, empty when unset
 };
-SWOFTY_RECORD(SeasonTeam, slot, team, seats)
+SWOFTY_RECORD(SeasonTeam, slot, team, seats, reserve)

 struct Season {
-    static constexpr Version version{1, 0};
+    static constexpr Version version{1, 1};
     std::string data_group;
     int year = 0;
     Keyed<SeasonTeam, &SeasonTeam::slot> teams;
     Keyed<SeasonChange, &SeasonChange::round, &SeasonChange::slot, &SeasonChange::seat> changes;
 };
```

A DLL built against 1.0 that edits the same season never sees `reserve`, and never loses it: the wrapper writes the struct's fields back over the record it read (see [the tolerant reader rule](#the-tolerant-reader-rule)).

Minor changes are:

- a new optional field;
- a new `enum` value, which readers must handle as "unknown" (see the rule below);
- looser validation: a larger range in a `check`, an optional field that used to be required;
- a new registry, hook point, service function or core table field.

## What carries a version

- **Launcher:** semver, `2.0.0`. A major launcher version marks a large addition. It never removes anything.
- **C ABI:** `SWOFTY_API_VERSION` (an integer, 9 in 2.0). `SwoftyApi` and every core table only ever grow.
- **Core tables:** each table's `version` field names its last layout.
- **Registries:** `major.minor`, in each record struct's `static constexpr Version version`.
- **Hook payloads:** a size-prefixed struct with a payload version.
- **Services:** an integer version per service.
- **Snapshots:** each one stores the version it was written with.

## A major change

<Split>

`career.season` 2.0 turns each seat from a driver into a driver plus a car number. Anything that can break existing code is major:

- removing or renaming a field;
- changing a field's type or meaning;
- making validation stricter;
- adding or changing a key of a `Keyed` list;
- changing an id format.

A major is a second handle on the same registry name, with its own struct. The owner registers an **adapter** that converts in both directions, so the old major keeps working. The core registers this one for `career.season`; an addon that owns a registry does the same in its `SWOFTY_REGISTER`.

<template #example>
<Example root="packages/sdk/std/registries/career.season" code-file="season.2.hpp" files="season.1.hpp, 1.0.json, 1.1.json, 2.0.json, status.json, CHANGES.md, examples/2016.json">
<template #code>

```cpp
// packages/sdk/std/registries/career.season/season.2.hpp
namespace swofty::career::v2 {

struct Seat {
    Ref<drivers::Identity> driver;
    std::optional<int> number;                     // the car number; empty: the game's own
};
SWOFTY_RECORD(Seat, driver, number)

struct SeasonTeam {
    int slot = 0;
    Ref<teams::Identity> team;
    std::array<Seat, 2> seats;                     // was std::array<Ref<drivers::Identity>, 2>
    Ref<drivers::Identity> reserve;
};
SWOFTY_RECORD(SeasonTeam, slot, team, seats, reserve)

struct Season {
    static constexpr Version version{2, 0};
    std::string data_group;
    int year = 0;
    Keyed<SeasonTeam, &SeasonTeam::slot> teams;
    Keyed<career::SeasonChange, &career::SeasonChange::round, &career::SeasonChange::slot,
          &career::SeasonChange::seat> changes;
};
SWOFTY_RECORD(Season, data_group, year, teams, changes)

inline Registry<Season> seasons{"career.season", { .applies = Applies::read, .snapshot = true, .snapshot_budget = 65536 }};

}

// the core's adapter between the two majors, registered in the core's own SWOFTY_REGISTER
using namespace swofty;

SWOFTY_REGISTER
{
    swofty::adapter(career::seasons, career::v2::seasons,
        [](career::Season s) {                     // 1 -> 2: upgrade
            career::v2::Season out{ .data_group = s.data_group, .year = s.year, .changes = s.changes };
            for (const career::SeasonTeam &t : s.teams)
                out.teams.push_back({ .slot = t.slot, .team = t.team,
                                      .seats = { { { .driver = t.seats[0] }, { .driver = t.seats[1] } } },
                                      .reserve = t.reserve });
            return out;
        },
        [](career::v2::Season s) {                 // 2 -> 1: downgrade for old readers
            career::Season out{ .data_group = s.data_group, .year = s.year, .changes = s.changes };
            for (const career::v2::SeasonTeam &t : s.teams)
                out.teams.push_back({ .slot = t.slot, .team = t.team,
                                      .seats = { t.seats[0].driver, t.seats[1].driver },
                                      .reserve = t.reserve });
            return out;
        });
}
```

</template>
</Example>
</template>
</Split>

A reader asks for the major it was built against, through the handle it uses:

```cpp
auto id = "swofty.progressive/2017";

// built against 1.x: career::seasons is the major-1 handle, so the core hands it 1.x values
std::optional<career::Season> s1 = career::seasons.get(career::seasons[id]);

// built against 2.x
std::optional<career::v2::Season> s2 = career::v2::seasons.get(career::v2::seasons[id]);
```

A data file names its major after the registry name, `"career.season@1"`. A name without `@` means the registry's first major, so a data file written for 2.0.0 keeps its meaning.

With the adapter in place:

- **Old readers:** a reader built against 1.x gets the effective value downgraded to 1.x.
- **Old layers:** a 1.x addon's edit runs on the value downgraded to 1.x. Only the fields it changed carry over to the 2.0 value, so it still overrides.
- **Old snapshots:** a career saved with 1.x is upgraded on load, by the owner's `swofty::migrate` when it has one, and otherwise by the adapter's `up`.
- **Every major is kept**, with its adapter, in every later release. A 2.0 addon that reads `career::seasons` (major 1) still works on launcher 3.x and later.

## Where the versions live

The headers, schemas and examples for the core's registries live in the SDK, one folder per registry. The schema files are generated from the headers' `SWOFTY_RECORD` lists at release. Data files and the core's validation in game use them. A released schema file is never edited again:

```text
packages/sdk/std/
  registries/
    career.season/
      season.1.hpp          major 1 (career::Season), follows the newest minor
      season.2.hpp          major 2 (career::v2::Season)
      1.0.json              generated, released with 2.0.0, never edited again
      1.1.json              generated: adds reserve
      2.0.json
      status.json
      examples/
        2016.json           a record, validated against every schema of its major in CI
        whatif-layer.cpp    an edit, built and run against every minor of its major in CI
        whatif-layer.json   the same edit as a data file
      CHANGES.md            one line per version: what changed and in which launcher
    career.driver_ratings/
      ...
  hooks/
    career.contract.decide.hpp   the payload struct and its version history
  services/
    README.md                    services are owned by addons; this lists the known ones
```

An addon that owns a registry ships one header per major for other addons to include (`api/<addon>/<name>.hpp`), with the version in each struct. The core generates its schema in game from `SWOFTY_RECORD` and checks data files against it.

## The tolerant reader rule

Every reader must ignore what it doesn't know. That is what makes minor changes safe:

- **Unknown fields are kept, not rejected.** `SWOFTY_RECORD` reads the fields the struct lists. The wrapper keeps the rest of the record and writes the struct's fields back over it, so an edit or a round trip never drops a newer field.
- **Unknown `enum` values mean "something newer".** Handle them like the safest known value, and don't fail.
- **An unknown hook payload field is never read.** A payload struct only grows, and `size` says how much of it the core filled.

```cpp
for (const career::SeasonChange &change : season.changes) {
    switch (change.kind) {
    case career::ChangeKind::swap:     apply_swap(change); break;
    case career::ChangeKind::stand_in: apply_stand_in(change); break;
    default: swofty::log("[grid] change kind %d is newer than this addon; skipped", int(change.kind));
    }
}
```

The SDK's `SWOFTY_RECORD` conversion already follows this rule, so an addon that sticks to the typed structs gets it for free.

## Status

Every registry, hook point, service and table field has a status. The SDK keeps it next to the registry:

```json
// packages/sdk/std/registries/career.season/status.json
{ "status": "stable", "since": "2.0.0" }

// a newer one
{ "status": "experimental", "since": "2.2.0", "feature": "registry_team_identity" }

// a newer major exists; this one still works, and always will
{ "status": "superseded", "since": "2.0.0", "superseded": "2.4.0", "by": "career.season@2" }
```

- **experimental:** on for an addon that lists the feature in `experimental_features` in `swofty.json`. Its shape can still change before it's stable, and the docs mark it. Once stable, it follows every rule on this page.
- **stable:** the rules on this page apply.
- **superseded:** a newer version or a better way exists. The old one keeps working, with no warning to players. The docs point to the newer one.

To go from experimental to stable, a registry or hook point needs:

- two addons using it, at least one of them not the owner;
- tests and examples in the SDK;
- written docs;
- one release as experimental.

## ABI and table versions

The C ABI follows the same rule. A field is only ever appended, and the C++ wrapper checks the version before it touches a newer field:

```cpp
// how Registry<T>::origin checks for a call added in registry table version 2
template <class T>
std::string Registry<T>::origin(Ref<T> id, std::string_view field) const
{
    const SwoftyRegistry *t = detail::table();
    if (!t || t->version < 2 || !t->origin)        // origin arrived in table version 2
        return {};                                 // unavailable: no origin to report
    ...
}
```

- An addon built against the 2.0 headers loads on a 1.x core and sees every 2.0 library as unavailable, through `SwoftyCoreRegistry` and `SwoftyCoreHooks`. A 1.x loader never calls its `SwoftyAddonRegister`, so its `SWOFTY_REGISTER` doesn't run there.
- An addon built against 1.x headers loads on a 2.0 core unchanged (see [Compatibility](/std/compatibility)).
- `SWOFTY_API_VERSION` goes up by one whenever `SwoftyApi` gains a field. A table's own `version` goes up whenever that table gains a field. See [The C ABI](/std/abi).

## Making a change

Every change to a core registry, hook point, operation or table goes through the same steps, in one pull request:

1. **Write it in the header first:** the new field in the struct and its `SWOFTY_RECORD` list, the new major's struct and handle, or the payload struct with its new version.
2. **Add examples:** at least one valid record or edit, and one that must fail.
3. **Bump the version:** minor or major by the rules above. A major change adds its adapter, and a migration when snapshots need one.
4. **Implement and test it in the core.**
5. **Write the docs page section,** with a full example (see [Rollout and docs](/design/std-2/rollout#the-docs)).
6. **Add a line to `CHANGES.md`,** which the release notes quote.

CI enforces the mechanical parts:

- a released schema file that changed fails the build;
- `tools/std/schemacheck.py` compares each newly generated schema with the last released one and fails when the change is major but the version only went up a minor;
- every example is validated against its schema, and every example edit is built and run;
- every snapshot fixture in `packages/sdk/std/fixtures/` (saved careers from older versions) has to load and migrate;
- the [compatibility suite](/std/compatibility#the-release-gate) runs on every release.

## Saves across launcher versions

- **A newer launcher reading an older career:** snapshots migrate as above.
- **An older 2.x launcher reading a career saved by a newer one:** an unknown minor is read as is, by the tolerant reader rule. An unknown major is left untouched in the save, and that career reads those registries live, with a log line.
- **A 1.x launcher reading a 2.x career:** the `swofty_registry` container is kept by the save splice and otherwise ignored. The career plays as it would have in 1.x.
