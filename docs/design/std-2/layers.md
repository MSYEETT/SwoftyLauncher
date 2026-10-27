---
title: Layers
description: Patches, precedence, validation and conflicts - how addons override each other's registry records in standard library 2.0.
---

# Layers

A **layer** is one addon's change to one record, at one [scope](./scopes). Layers are how an addon overrides data it doesn't own: it never edits the provider's files, it puts its change on top.

## Patches

A layer is a [JSON Merge Patch](https://www.rfc-editor.org/rfc/rfc7396): name the fields to change, leave the rest out, and use `null` to remove a field.

The base, from Progressive:

```json
{
  "data_group": "f1_career_progressive",
  "year": 2017,
  "teams": [
    { "slot": 1, "team": "mercedes", "seats": ["lewis_hamilton", "valtteri_bottas"] },
    { "slot": 4, "team": "mclaren",  "seats": ["fernando_alonso", "stoffel_vandoorne"] },
    { "slot": 11, "team": null, "seats": [] }
  ],
  "changes": [
    { "round": 15, "slot": 8, "seat": 1, "out": "daniil_kvyat", "in": "pierre_gasly", "kind": "swap" }
  ]
}
```

The layer:

```json
{
  "teams": [
    { "slot": 1, "seats": ["fernando_alonso", "valtteri_bottas"] },
    { "slot": 4, "seats": ["lewis_hamilton", "stoffel_vandoorne"] },
    { "slot": 11, "team": "manor", "seats": ["pascal_wehrlein", "esteban_ocon"] }
  ],
  "changes": [
    { "round": 15, "slot": 8, "seat": 1, "$delete": true }
  ]
}
```

The effective value:

```json
{
  "data_group": "f1_career_progressive",
  "year": 2017,
  "teams": [
    { "slot": 1, "team": "mercedes", "seats": ["fernando_alonso", "valtteri_bottas"] },
    { "slot": 4, "team": "mclaren",  "seats": ["lewis_hamilton", "stoffel_vandoorne"] },
    { "slot": 11, "team": "manor", "seats": ["pascal_wehrlein", "esteban_ocon"] }
  ],
  "changes": []
}
```

What happened, element by element:

- `teams` is an `x-key: slot` array, so each patch element is matched to the base element with the same `slot`. Slots 1 and 4 merge: their `seats` are replaced and `team` is kept. Slot 11 merges too and gains a team.
- Arrays that aren't keyed (`seats`) are replaced whole, as in RFC 7396.
- In a keyed array, an element with its key fields and `"$delete": true` **removes** that element. Here the round 15 change goes away: Kvyat keeps his seat.
- A patch element with a key the base doesn't have is **appended**.

## Writing a layer

From the manifest, as a static file:

```json
{
  "overrides": [
    { "registry": "career.season", "file": "layers/seasons.json", "scope": "global" }
  ]
}
```

```json
// layers/seasons.json: patches by record id
{
  "f1_career_progressive/2017": { "teams": [ { "slot": 1, "seats": ["fernando_alonso", "valtteri_bottas"] } ] },
  "f1_career_progressive/2018": { "teams": [ { "slot": 1, "seats": ["fernando_alonso", "valtteri_bottas"] } ] }
}
```

From code:

```cpp
#include <swofty/registry.hpp>

using swofty::registry::Scope;

void give_alonso_the_mercedes(int year)
{
    auto patch = swofty::json::parse(R"({
        "teams": [
            { "slot": 1, "seats": ["fernando_alonso", "valtteri_bottas"] },
            { "slot": 4, "seats": ["lewis_hamilton", "stoffel_vandoorne"] }
        ]
    })");
    std::string id = "f1_career_progressive/" + std::to_string(year);
    swofty::registry::Result r = swofty::registry::layer("career.season", id, patch, Scope::global);
    if (r != swofty::registry::Result::ok)
        swofty::log("[whatif] %d: %s", year, swofty::registry::last_error().c_str());
}

void undo(int year)
{
    swofty::registry::clear_layer("career.season", "f1_career_progressive/" + std::to_string(year), Scope::global);
}
```

From a launcher page, as a profile layer, see [The launcher side](./launcher#editing-a-layer-from-a-launcher-page).

An addon has at most one layer per record per scope. Writing again replaces that addon's layer, it doesn't stack on it.

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

The core validates the **result** of applying a layer, not the patch on its own. If the result breaks the schema, that layer is dropped as a whole: every record it touches loses its change. The base and the other addons' layers still apply.

```text
[registry] career.season f1_career_progressive/2017: layer of tails.whatif-history (global) dropped:
           /teams/slot=1/seats/0 "fernando_alonso" is also in /teams/slot=4/seats/0 (a driver holds one seat a season)
```

Validation covers:

- the schema (types, `required`, `enum`, ranges, patterns);
- `x-ref` (a driver id that isn't in the game database fails);
- the registry's own rules, which the owner registers as a check:

```cpp
// the core's check for career.season: no driver in two seats
swofty::registry::check("career.season", [](const swofty::json::Value &season, swofty::registry::Errors &err) {
    std::set<std::string> seen;
    for (const auto &team : season["teams"].items())
        for (const auto &d : team["seats"].items())
            if (!d.as_string().empty() && !seen.insert(d.as_string()).second)
                err.add("/teams/slot=" + std::to_string(team["slot"].as_int()) + "/seats", d.as_string() + " already holds a seat");
});
```

## Conflicts

A conflict is any case where an addon's data doesn't end up in the effective value:

- **Field contest:** two addons' layers set the same field to different values. The later one wins.
- **Dropped layer:** a layer failed validation.
- **Provider clash:** two providers answered the same id.

The core logs each conflict once per session with both addons and the field. The launcher's conflicts view lists them per profile, and Send Logs includes them. Nothing pops up in game.

```text
[registry] conflict: career.season f1_career_progressive/2017 /teams/slot=1/seats:
           tails.whatif-history sets ["fernando_alonso","jenson_button"], someone.realism sets ["fernando_alonso","valtteri_bottas"];
           someone.realism wins (later in load order)
```
