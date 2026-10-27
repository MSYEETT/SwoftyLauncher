---
title: Your own systems
description: How an addon defines a system of its own (its registries, scope, projection onto core records, scoped hooks and a service) so other addons can build on it, with a six-compound tyre system as the worked example.
---

# Your own systems

An addon can bring a whole system the game doesn't have: a six-compound tyre range, a points system with dropped scores, five R&D departments, a two-qualifying weekend. It builds the system out of the same pieces the core uses, so other addons can extend it the way they extend anything else.

A system has six parts:

```text
1. registries   the core's, where the game already has the concept; your own (<addon>.<name>) for the rest
2. a scope      where it applies: a data group, a setup mode, a content package, a car package
3. a projection your records turned into core records at build, by your launcher plugin
4. hooks        for what data can't do, filtered to your scope
5. a service    for calls other addons make
6. registry.json  declaring all of it, with its dependencies
```

```text
swofty.f1-1990.tyre_compound ──projects──▶ tyre.visual, tyre.class, tyre.event, tyre.allocation ──applier──▶ game rows, icons
swofty.f1-1990.tyre_rules    ──projects──▶ tyre.rules                                            ──core────▶ scoped hooks
        ▲ layers and keys from other addons (a seventh compound, wear)                             ▲ service swofty.f1-1990.tyres
```

## A six-compound tyre system

F1 1990 races on Goodyear A (hardest) to E (softest, qualifying only), plus W for the wet. The game has nothing like this, so F1 1990 owns two registries.

```json
// swofty.f1-1990/schemas/tyre_compound.1.0.json (abridged)
{
  "x-registry": "swofty.f1-1990.tyre_compound", "x-version": "1.0", "x-resolve": "build",
  "required": ["key", "letter", "physical"],
  "properties": {
    "key":             { "type": "string", "pattern": "^[a-z0-9_]+$" },
    "letter":          { "type": "string", "maxLength": 2 },
    "softness":        { "type": "integer" },
    "physical":        { "type": "string", "x-ref": "tyre.physical" },
    "colour":          { "type": "string" },
    "qualifying_only": { "type": "boolean" },
    "wet":             { "type": "boolean" }
  }
}
```

```json
// swofty.f1-1990/registry/tyres.json
{
  "swofty.f1-1990.tyre_compound": {
    "swofty.f1-1990/a": { "key": "a", "letter": "A", "softness": 1, "physical": "swofty.f1-1990/goodyear_a", "colour": "#FFFFFF" },
    "swofty.f1-1990/b": { "key": "b", "letter": "B", "softness": 2, "physical": "swofty.f1-1990/goodyear_b", "colour": "#F6C21C" },
    "swofty.f1-1990/c": { "key": "c", "letter": "C", "softness": 3, "physical": "swofty.f1-1990/goodyear_c", "colour": "#E2231A" },
    "swofty.f1-1990/d": { "key": "d", "letter": "D", "softness": 4, "physical": "swofty.f1-1990/goodyear_d", "colour": "#9B2FAE" },
    "swofty.f1-1990/e": { "key": "e", "letter": "E", "softness": 5, "physical": "swofty.f1-1990/goodyear_e", "colour": "#2F74E8", "qualifying_only": true },
    "swofty.f1-1990/w": { "key": "w", "letter": "W", "softness": 0, "physical": "swofty.f1-1990/goodyear_w", "colour": "#22A45D", "wet": true }
  },
  "swofty.f1-1990.tyre_rules": {
    "swofty.f1-1990/season": {
      "default": { "race": ["b", "c", "d"], "qualifying": ["e"], "wet": "w", "sets": { "race": 7, "qualifying": 3, "wet": 3 } },
      "rounds": [
        { "round": 4,  "venue": "venue/monaco", "race": ["c", "d"] },
        { "round": 11, "venue": "venue/spa",    "race": ["a", "b", "c"] }
      ]
    }
  },
  "tyre.physical": {
    "swofty.f1-1990/goodyear_b": { "extends": "game/c3", "vcmpd": "tyres/goodyear_b.xml" }
  }
}
```

`rounds` is a keyed array (`x-key: round`), so another addon can change one round with a layer.

### The scope

The whole system applies only inside 1990, so Grand Prix and every other mode stay stock:

```json
// swofty.f1-1990/registry.json
{ "systems": [ { "id": "swofty.f1-1990.tyres", "scope": { "setup_mode": "swofty.f1-1990/f1_1990" } } ] }
```

Every record of the system inherits that scope unless it sets its own. A scope is one value the core understands everywhere: on records, on layers, on hook subscriptions and on [events](/std/events). It can name:

```cpp
swofty::where{ .data_group = "f1_career_1990" }               // one career's data
swofty::where{ .setup_mode = "swofty.f1-1990/f1_1990" }       // every session run under that mode
swofty::where{ .content = "swofty.classic-seasons-2017/f1_2017" }   // one GP/TT package
swofty::where{ .vehicle_package = "f1_1990" }                 // cars of one package
```

### The projection

The game knows compound classes, visual compounds, events and allocations, not "Goodyear letters". F1 1990's launcher plugin gets its effective records in the `build` call and returns **core records**. The core's appliers turn those into game rows and icons, and the core allocates the stat ids:

```text
build    swofty.f1-1990.tyre_compound  6 records, plus layers from other addons
         swofty.f1-1990.tyre_rules     1 record, 2 round overrides
emit     tyre.visual      swofty.f1-1990/goodyear_{a,b,c,d,e,w}     ids from the allocator
         tyre.class       one class per distinct round shape (e.g. {B,C,D,E,W}, {C,D,E,W})
         tyre.event       swofty.f1-1990/f1_1990/<track> for 16 rounds: the class and presets per round
         tyre.allocation  one preset per shape
refuse   a round that names more than 4 dry compounds: a class has 5 options, and W takes one
```

```cpp
// in F1 1990's launcher plugin
std::optional<ClassShape> shape_of(const Round &r, const Pool &pool, std::string *why)
{
    std::vector<Compound> dry = r.race + r.qualifying;          // letters, deduplicated
    std::sort(dry.begin(), dry.end(), softest_first);
    if (dry.size() > 4) {
        *why = "round " + std::to_string(r.round) + " names " + std::to_string(dry.size()) + " dry compounds; 4 fit";
        return std::nullopt;
    }
    ClassShape s;
    for (const Compound &c : dry)
        s.options.push_back({ c.letter_name(), c.physical, visual_id(c) });
    s.options.push_back({ "Wet", pool.wet.physical, visual_id(pool.wet) });
    return s;                                                    // rounds with the same shape share one tyre.class
}
```

What the game allows, which the projection works within:

- **Physics is not the limit:** there are 9 physics codes, tuned per car package in its own compound file.
- **A class has at most 5 option slots.**
- **The badge follows the slot, not the compound.** One class per round shape keeps the right letter on screen.
- **The car and its allocation each pick their class in a different function** (`Sess_TyreClass_FromName`, `setPresetChoice`). The core answers both from `tyre.event` through the `session.tyres.class_of` hook point.

### Scoped hooks

What data can't do, a scoped handler does. It never runs outside 1990:

```cpp
#include <swofty/hooks.hpp>
#include <swofty/session/tyres.hpp>

using swofty::session::ChooseSet;

SWOFTY_ON_LOAD
{
    // 1990: when a car runs out of fresh sets of a compound, borrow unused ones from another session
    swofty::hooks::before<ChooseSet>(swofty::where{ .setup_mode = "swofty.f1-1990/f1_1990" }, [](ChooseSet &c) {
        if (c.candidates == 0)
            c.chosen = f1990::tyres::borrow_unused(c.car, c.wanted_slot);
    });
}
```

### The service

Other addons ask in letters, and never learn how the letters map onto slots:

```cpp
// swofty_f1_1990_api/tyres.hpp
struct F1990Tyres {
    static constexpr const char *name = "swofty.f1-1990.tyres";
    static constexpr uint32_t version = 1;
    uint32_t size;
    int (*current)(uint32_t car, char *out, size_t cap);              // "swofty.f1-1990/c"; 0 outside 1990
    int (*event_compounds)(char *out, size_t cap);                    // "b,c,d,e,w", softest first
    int (*letter_of_option)(uint8_t option, char *out, size_t cap);   // "D"
};
```

A strategy overlay or a telemetry dashboard reads `current()` and shows "C", in F1 1990 and in any addon that extends it.

## Another addon extends it

`community.goodyear-1990-plus` adds a seventh compound, F (a softer qualifying tyre), and makes C wear faster. It doesn't change any of F1 1990's files.

```json
// community.goodyear-1990-plus/registry/tyres.json: its own key in F1 1990's registry
{
  "swofty.f1-1990.tyre_compound": {
    "community.goodyear-1990-plus/f": {
      "key": "f", "letter": "F", "softness": 6, "qualifying_only": true,
      "physical": "community.goodyear-1990-plus/goodyear_f", "colour": "#FF7800"
    }
  },
  "tyre.physical": {
    "community.goodyear-1990-plus/goodyear_f": { "extends": "swofty.f1-1990/goodyear_e", "grip": 1.04, "wear": 1.6 }
  }
}
```

```json
// a layer of swofty.f1-1990.tyre_rules "swofty.f1-1990/season": Monaco and Hungary qualify on F
{ "rounds": [ { "round": 4, "qualifying": ["f"] }, { "round": 10, "qualifying": ["f"] } ] }
```

```json
// a layer of tyre.physical "swofty.f1-1990/goodyear_c": C wears 15% faster wherever it's used
{ "wear": 1.15 }
```

```json
// community.goodyear-1990-plus/registry.json
{
  "requires": [
    { "registry": "swofty.f1-1990.tyre_compound", "version": "^1" },
    { "registry": "swofty.f1-1990.tyre_rules",    "version": "^1" }
  ],
  "overrides": [ { "registry": "swofty.f1-1990.tyre_rules", "file": "layers/rounds.json", "scope": "global" } ]
}
```

At build:

- F1 1990's projection sees seven compounds. Rounds 4 and 10 get a new class shape, `{F, D, C, W}`.
- If the allocator runs out of visual compound ids, the build stops and names `tyre.visual` and the limit, instead of the game crashing later.
- A layer that makes a round name five dry compounds is dropped, and the conflicts view names its addon.
- With F1 1990 not installed, the launcher says "requires swofty.f1-1990" before Play, and no rows are written.

## More systems, the same shape

**Points with dropped scores (1990: 9-6-4-3-2-1, best 11 results):**

```json
{
  "session.points": { "swofty.f1-1990/f1_1990_race": { "extends": "game/standard_race", "points": [9, 6, 4, 3, 2, 1], "fastest_lap": null } },
  "swofty.f1-1990.championship_rules": { "swofty.f1-1990/drivers": { "scope": { "data_group": "f1_career_1990" }, "best_results": 11 } }
}
```

```cpp
swofty::hooks::after<swofty::career::StandingsRebuild>(swofty::where{ .data_group = "f1_career_1990" },
    [](const swofty::career::StandingsRebuild &s, const swofty::hooks::Outcome &) {
        auto rules = swofty::registry::get("swofty.f1-1990.championship_rules", "swofty.f1-1990/drivers");
        swofty::career::apply_dropped_scores(s, (*rules)["best_results"].as_int());   // keeps gross totals for the UI
    });
```

**Five R&D departments (Aero, Chassis & Suspension, Engine, Tyres, Reliability & Gearbox):**

```json
{
  "career.rnd.department": {
    "swofty.f1-1990/tyres": { "scope": { "data_group": "f1_career_1990" }, "name": "lng_f1990_rnd_tyres", "icon": "rnd/tyres", "economy": { "cost_scale": 0.8 } }
  },
  "career.rnd.tree": {
    "swofty.f1-1990/ferrari": { "scope": { "data_group": "f1_career_1990" }, "extends": "game/rnd_tree_ferrari", "departments": ["aero", "chassis", "engine", "tyres", "reliability"] }
  }
}
```

The game draws four departments natively. A fifth needs the native layer R&D Tyres built, which moves into the core as the `career.rnd.department` applier.

**Two qualifying sessions:**

```json
{ "session.weekend_format": { "swofty.f1-1990/two_qualifying": {
    "scope": { "setup_mode": "swofty.f1-1990/f1_1990" },
    "sessions": ["practice_1", "qualifying_1", "practice_2", "qualifying_2", "warm_up", "race"],
    "grid": { "from": ["qualifying_1", "qualifying_2"], "rule": "best_of" } } } }
```

`best_of` is a grid rule the core adds to `session.grid.build`, so it's shared rather than reimplemented by every addon that needs it.

**A character (Elena becomes Flavio):**

```json
{ "career.character": { "swofty.f1-1990/agent": {
    "scope": { "data_group": "f1_career_1990" }, "replaces": "game/agent_elena",
    "name": "lng_f1990_flavio", "portrait": "ui/flavio.png", "voice": "mute" } } }
```

## What the core provides for systems

The platform pieces systems need, in order of priority:

- **Now:**
  - **scope as a core value**, on records, layers, hooks and events;
  - **a stable id allocator** with a per-profile lockfile, so ids never collide between addons and never move between builds;
  - **whole-row clones** that copy every key of the donor row (no more boot crashes from a missing unnamed key);
  - **career tiles and save partitions** as records;
  - **hook filters by scope** (`swofty::where`).
- **Next:**
  - **UI extension points:** texture-array slices, icon glyphs, scoped screens and scoped strings;
  - **capacity checks before Play:** row counts, grid size, ids;
  - **services;**
  - **dependencies** (see [Dependencies and guards](/std/dependencies)).
- **Later:**
  - **option references in records** (<span v-pre>`"sets": "{{option:race_sets}}"`</span>);
  - **a core career id;**
  - **extending game enums.**

F1 1990 is the test: today it's 16 DLLs that check "am I inside 1990" six different ways. On these pieces, 13 of the 16 become records, core-answered hook points or fixes in the core. One small DLL is left, with three handlers: the best-of grid, a qualifying engine line, and dropped scores.
