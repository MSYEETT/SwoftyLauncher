---
title: Layers
description: Edits, selectors, option-bound changes, precedence, validation and conflicts - how addons change each other's registry records in code in standard library 2.0.
---

# Layers

A **layer** is one addon's change to a record it doesn't own, at one [scope](/std/scopes). In code, a layer is an **edit**: a function that gets the record's effective value and changes it. The provider's records are never touched; your change sits on top.

<Split>

Tails' "what if" addon gives Alonso the 2017 Mercedes, fills the empty eleventh slot with Manor, and removes Gasly's round 15 debut.

The edit gets Progressive's 2017 season as a `career::Season`, with every earlier layer already applied. The core runs it in load order and records which fields it changed.

<template #example>
<Example root="tails.whatif-history" code-file="native/src/whatif.cpp" data-file="registry/layers.json" files="swofty.json, native/whatif.dll">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/career.hpp>
#include <swofty/systems/drivers.hpp>
#include <swofty/systems/teams.hpp>

using namespace swofty;

static Ref<drivers::Identity> d(const char *key) { return drivers::identities[std::string("game/") + key]; }

SWOFTY_REGISTER
{
    career::seasons.layer(career::seasons["swofty.progressive/2017"], [](career::Season &s) {
        s.teams.at(1).seats = { d("fernando_alonso"), d("valtteri_bottas") };
        s.teams.at(4).seats = { d("lewis_hamilton"), d("stoffel_vandoorne") };
        s.teams.at(11) = { .slot = 11, .team = teams::identities["game/manor"],
                           .seats = { d("pascal_wehrlein"), d("esteban_ocon") } };
        s.changes.erase(15, 8, 1);       // round 15, slot 8, seat 1: Kvyat keeps his seat
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
{
  "career.season": {
    "swofty.progressive/2017": {
      "teams": [
        { "slot": 1, "seats": ["game/fernando_alonso", "game/valtteri_bottas"] },
        { "slot": 4, "seats": ["game/lewis_hamilton", "game/stoffel_vandoorne"] },
        { "slot": 11, "team": "game/manor", "seats": ["game/pascal_wehrlein", "game/esteban_ocon"] }
      ],
      "changes": [
        { "round": 15, "slot": 8, "seat": 1, "$delete": true }
      ]
    }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: tails.whatif-history (whatif.dll) 1 layer on career.season (early stage, 0.2 ms)
[registry] career.season swofty.progressive/2017: edit by tails.whatif-history changed
           /teams/slot=1/seats, /teams/slot=4/seats, /teams/slot=11, /changes/round=15,slot=8,seat=1 (removed)
```

## Edits

The base, from Progressive, and the effective value after Tails' edit:

```text
                       base (swofty.progressive)                 effective
teams/slot=1           mercedes  hamilton, bottas                mercedes  alonso, bottas
teams/slot=4           mclaren   alonso, vandoorne               mclaren   hamilton, vandoorne
teams/slot=11          -         (empty)                         manor     wehrlein, ocon
changes                round 15 slot 8 seat 1: kvyat -> gasly    (none)
```

- **An edit changes only what it touches.** `team` in slots 1 and 4 is kept. Every other team, and every field the edit doesn't set, comes through as it was.
- **`Keyed` lists are matched by key.** `teams.at(11)` finds slot 11, or appends it. `changes.erase(15, 8, 1)` removes the element with that key.
- **The core diffs each edit.** It compares the value before and after, by field and by key, and records each changed field's origin (see [Origins](/std/reading#origins)).
- **Edits are code.** For code addons they replace JSON Merge Patch. Data files still use merge patches, with the same meaning (see [Without code](#without-code)).

## Writing a layer

In `SWOFTY_REGISTER`, for every game start. Or at any time while the game runs, from `SWOFTY_ON_LOAD` or later:

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/career.hpp>

using namespace swofty;

void give_alonso_the_mercedes(int year)
{
    auto id = career::seasons["swofty.progressive/" + std::to_string(year)];
    career::seasons.layer(id, [](career::Season &s) {
        s.teams.at(1).seats[0] = drivers::identities["game/fernando_alonso"];
    });
}

void undo(int year)
{
    career::seasons.clear_layer(career::seasons["swofty.progressive/" + std::to_string(year)]);
}
```

```text
[registry] layer from tails.whatif-history on career.season swofty.progressive/2017 registered at run time;
           applies from the next session load
```

- **One layer per record per scope.** Calling `layer` again on the same record and scope replaces your edit; it doesn't stack on it. Put every change to one record in one edit.
- **A late layer takes effect the next time the game reads that data.** The core says when (see [When changes apply](/std/build)).
- **From a launcher page,** the page saves the player's choice in the addon's storage, and the addon's code applies it as a profile layer. See [The launcher side](/std/launcher#editing-a-layer-from-a-launcher-page).

## Selectors

<Split>

A layer can target every record that matches a **selector**, instead of one id. It applies to matching records from every provider, including ones added by addons the layer's author has never heard of, and ones added later.

`where` runs on each record's base value, before any layers. The edit then runs on each match.

<template #example>
<Example root="kizi.ai-tweaks" code-file="native/src/ai_tweaks.cpp" data-file="registry/layers.json" files="swofty.json, native/ai_tweaks.dll">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/ai.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    // more aggressive AI in every F1 car, stock or added
    auto f1 = where<ai::VehicleConfig>([](const ai::VehicleConfig &c) { return c.cls == ai::Class::f1; });

    ai::vehicle_configs.layer(f1, [](ai::VehicleConfig &c) {
        c.racecraft.overtake_aggression = 0.85f;
        c.racecraft.defend_aggression = 0.8f;
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
{
  "ai.vehicle_config": {
    "$select": [
      {
        "where": { "class": "f1" },
        "patch": { "racecraft": { "overtake_aggression": 0.85, "defend_aggression": 0.8 } }
      }
    ]
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] ai.vehicle_config: kizi.ai-tweaks selector (ai_tweaks.cpp:10) matched 612 records
           (519 game/, 80 swofty.classic-seasons-*, 13 swofty.f1-1990/), edit applied to each
```

The forms a selector takes:

```cpp
where<ai::VehicleConfig>([](const ai::VehicleConfig &c) { return c.cls == ai::Class::f1; })      // a field equals a value
where<ai::VehicleConfig>([](const ai::VehicleConfig &c) {
    return c.cls == ai::Class::f1 || c.cls == ai::Class::f1_26; })                                  // one of several values
where<career::Season>([](const career::Season &s) { return s.year >= 2016 && s.year <= 2019; })  // a numeric range
under<content::Package>("swofty.classic-seasons-2017/")                                           // ids under a prefix
under<ai::VehicleConfig>("").except({ ai::vehicle_configs["game/safety_car"] })                   // everything but some ids
```

Within one addon, its selector layers apply first, then its layers on single ids. So an addon can set a default for every car and then a different value for one.

## Option-bound layers

<Split>

An edit that calls `swofty::option<T>()` depends on that option. When the player changes it in ADDON SETTINGS or the launcher, the core runs the edit again with the new value.

One layer replaces 16 pre-built copies of every car, one per damage level. The option itself stays in `swofty.json`, where ADDON SETTINGS and the launcher find it:

```json
{ "options": [ { "id": "damage", "type": "choice", "values": ["realistic", "heavy", "brutal"], "default": "realistic" } ] }
```

<template #example>
<Example root="william09leo.heavier-damage-realism" code-file="native/src/damage.cpp" data-file="registry/layers.json" files="swofty.json, native/damage.dll">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto f1 = where<cars::Handling>([](const cars::Handling &h) { return h.cls == cars::Class::f1; });

    cars::handling.layer(f1, [](cars::Handling &h) {
        std::string level = swofty::option<std::string>("damage");
        if (level == "heavy")
            h.damage_scale = 1.6f;
        else if (level == "brutal")
            h.damage_scale = 2.4f;
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
{
  "vehicle.handling": {
    "$select": [
      { "where": { "class": "f1" }, "when": { "option": "damage", "is": "heavy" },  "patch": { "damage_scale": 1.6 } },
      { "where": { "class": "f1" }, "when": { "option": "damage", "is": "brutal" }, "patch": { "damage_scale": 2.4 } }
    ]
  }
}
```

</template>
</Example>
</template>
</Split>

The new value takes effect the next time the game reads that data: at once for registries read on demand, at the next session load for session data, at the next game start for boot data such as handling.

```text
[registry] option damage of william09leo.heavier-damage-realism changed: heavy -> brutal;
           1 selector layer on vehicle.handling re-runs, applies from the next game start
```

## Precedence

The base comes first. Then each addon's layers, in **load order**, the same order that decides which file wins and how mixins chain. Within one addon, its `global` layer comes before its `profile` layer.

```text
load order: swofty.progressive (provider), tails.whatif-history, someone.realism

  base                    slot 1 seats: hamilton, bottas         (swofty.progressive)
  + tails.whatif-history  slot 1 seats: alonso, bottas           (global)
  + tails.whatif-history  slot 1 seats: alonso, jenson_button    (profile)
  + someone.realism       slot 1 seats: alonso, bottas           (global)
  = effective             slot 1 seats: alonso, bottas           someone.realism wins /teams/slot=1/seats
```

The player reorders addons in the launcher to change who wins, the same way they reorder addons that change the same file today.

## Validation

The core validates the **result** of each edit. If it breaks the record's shape, a `Ref` that doesn't resolve, or the owner's `check`, that layer is dropped as a whole: every record it touches loses its change. The base and the other addons' layers still apply.

```text
[registry] career.season swofty.progressive/2017: layer of tails.whatif-history (global) dropped:
           /teams/slot=1/seats/0 game/fernando_alonso is also in /teams/slot=4/seats/0 (a driver holds one seat a season)
```

The owner adds its rules with `check`. The core's rule for `career.season`:

```cpp
career::seasons.check([](const career::Season &s, Errors &err) {
    std::set<std::string_view> seen;
    for (const career::SeasonTeam &t : s.teams)
        for (const Ref<drivers::Identity> &d : t.seats)
            if (!d.id().empty() && !seen.insert(d.id()).second)
                err.add("teams/slot=" + std::to_string(t.slot) + "/seats", std::string(d.id()) + " already holds a seat");
});
```

## Conflicts

A conflict is any case where an addon's change doesn't end up in the effective value:

- **Field contest:** two addons' edits set the same field to different values. The later one wins.
- **Dropped layer:** a layer failed validation.
- **Provider clash:** one id provided twice (see [Providers](/std/providers#one-provider-per-record)).

The core logs each conflict once per session with both addons and the field, and writes the conflicts report at each session load. Send Logs includes it, and the launcher can show the last one. Nothing pops up in game.

```text
[registry] conflict: career.season swofty.progressive/2017 /teams/slot=1/seats:
           tails.whatif-history sets [game/fernando_alonso, game/jenson_button], someone.realism sets [game/fernando_alonso, game/valtteri_bottas];
           someone.realism wins (later in load order)
```

## Without code

A data file's layer is a [JSON Merge Patch](https://www.rfc-editor.org/rfc/rfc7396) in `registry/layers.json`, keyed by registry name, then the full id of the record it changes. Name the fields to change, leave the rest out, and use `null` to remove a field.

The base:

```json
{
  "data_group": "f1_career_progressive",
  "year": 2017,
  "teams": [
    { "slot": 1, "team": "game/mercedes", "seats": ["game/lewis_hamilton", "game/valtteri_bottas"] },
    { "slot": 4, "team": "game/mclaren",  "seats": ["game/fernando_alonso", "game/stoffel_vandoorne"] },
    { "slot": 11, "team": null, "seats": [] }
  ],
  "changes": [
    { "round": 15, "slot": 8, "seat": 1, "out": "game/daniil_kvyat", "in": "game/pierre_gasly", "kind": "swap" }
  ]
}
```

The layer, from the data tab above, gives the same effective value as the edit:

```json
{
  "data_group": "f1_career_progressive",
  "year": 2017,
  "teams": [
    { "slot": 1, "team": "game/mercedes", "seats": ["game/fernando_alonso", "game/valtteri_bottas"] },
    { "slot": 4, "team": "game/mclaren",  "seats": ["game/lewis_hamilton", "game/stoffel_vandoorne"] },
    { "slot": 11, "team": "game/manor", "seats": ["game/pascal_wehrlein", "game/esteban_ocon"] }
  ],
  "changes": []
}
```

- **Keyed lists** (`Keyed` in code, `x-key` in the schema) are matched by key. Slots 1 and 4 merge: their `seats` are replaced and `team` is kept. Slot 11 merges too and gains a team.
- **Lists that aren't keyed** (`seats`) are replaced whole, as in RFC 7396.
- **`"$delete": true`** with an element's key fields removes that element from a keyed list.
- **A key the base doesn't have** is appended.
- **`"$select"`** holds selector layers: `where` matches fields (`{ "class": "f1" }`), one of several values (`{ "class": ["f1", "f1_26"] }`) or a numeric range (`{ "year": { "min": 2016, "max": 2019 } }`); `ids` matches a prefix (`"swofty.classic-seasons-2017/*"`, or `"*"` with `"except": ["game/safety_car"]`). An entry with both `ids` and `where` matches records that pass both.
- **`"when"`** binds a layer to an option: `is` for one value, `in` for several, `on`/`off` for a toggle.
