---
title: Keys, bases and variants
description: Every addon registers its own keys; a record can be a base other addons extend in place or build variants of, in standard library 2.0.
---

# Keys, bases and variants

## Every addon registers its own keys

An addon puts its things in a registry under **its own keys**: ids that start with its addon id. Many addons share one registry, and each owns the records it registers.

```text
content.package                                   <- one shared registry
  swofty.classic-seasons-2016/f1_2016             <- Classic Seasons 2016's key
  swofty.classic-seasons-2017/f1_2017             <- Classic Seasons 2017's key
  community.f1-2012/f1_2012                       <- another addon's season, its own key
  mod.esp/f1_2017_v10                             <- a variant built on Classic Seasons' 2017 (see below)
era.box
  swofty.classic-seasons-2017/f1_2017             <- the GP/TT box for that package
  mod.esp/f1_2017_v10                             <- a second box: the variant shows as its own entry
engine_sound
  swofty.classic-seasons-2017/ferrari             <- cs17_ferrari.bdl and its settings
  mod.esp/v10_screamer                            <- a sound another addon brings
```

- **The owner's name comes first.** An id is `<owned name>/<key>`. An addon owns its addon id, plus every name it registers as its own: the data group of its career content (`f1_career_progressive`), its entrant lists (`cs_f1_2017`). The launcher refuses a record whose id starts with a name another addon owns, so two addons never fight over a name.
- **The core's records** describe the stock game and use `game/` instead: `game/ferrari_2025`, `game/f1_2026`.
- **Everyone else can still change your records**, through [layers](./layers). Owning a key means you provide its base, not that you lock it.

```cpp
// Classic Seasons 2017 registers its own keys
swofty::registry::provide("content.package", "swofty.classic-seasons-2017/f1_2017", package_2017());
swofty::registry::provide("era.box",         "swofty.classic-seasons-2017/f1_2017", box_2017());

// refused: the id starts with another addon's id
swofty::registry::provide("content.package", "swofty.classic-seasons-2016/f1_2016", ...);
// [registry] content.package swofty.classic-seasons-2016/f1_2016: refused for swofty.classic-seasons-2017
//            (an addon provides only ids under its own id; use a layer to change another addon's record)
```

Most addons provide from a static file, which needs no code:

```json
// swofty.classic-seasons-2017/registry/content.json
{
  "content.package": {
    "swofty.classic-seasons-2017/f1_2017": { "title": "lng_cs_era_2017", "entrant_list": "cs_f1_2017", "...": "..." }
  },
  "era.box": {
    "swofty.classic-seasons-2017/f1_2017": { "package": "swofty.classic-seasons-2017/f1_2017", "order": 2017 }
  }
}
```

## Bases

A **base** is any record other addons build on. Nothing marks it: every record can be one. Classic Seasons' F1 2017 package is the base for everything else about 2017 GP/TT.

Another addon can build on a base in two ways.

### Extend it in place, with a layer

A [layer](./layers) changes the base itself. Every user of the base sees the change: the 2017 GP/TT box, Progressive Career's 2017 season, anything else that reads it.

```json
// mod.esp: give the 2017 Ferrari the V10 sound, everywhere the 2017 package is used
// layer of content.package "swofty.classic-seasons-2017/f1_2017"
{
  "teams": [
    { "key": "ferrari", "engine_sound": "mod.esp/v10_screamer" }
  ]
}
```

### Build a variant, with `extends`

A **variant** is a new record, under your own key, that starts from a base and changes it. The base stays as it was. The variant shows up as its own entry, here a second GP/TT box.

```json
// mod.esp/registry/content.json
{
  "content.package": {
    "mod.esp/f1_2017_v10": {
      "extends": "swofty.classic-seasons-2017/f1_2017",
      "title": "lng_esp_era_2017_v10",
      "teams": [
        { "key": "mercedes", "engine_sound": "mod.esp/v10_screamer" },
        { "key": "ferrari",  "engine_sound": "mod.esp/v10_screamer" },
        { "key": "redbull",  "engine_sound": "mod.esp/v10_screamer" }
      ]
    }
  },
  "era.box": {
    "mod.esp/f1_2017_v10": { "package": "mod.esp/f1_2017_v10", "order": 2017 }
  }
}
```

The variant's value is a patch on the base, applied with the same rules as a layer. So the variant above is the 2017 package with three sounds changed and its own title. Every team it doesn't mention, and every other field, comes from the base.

## Copies are variants

In 1.x an addon that needs its own version of a stock row copies it: Progressive copies the 2026 Driver Career's game mode and data group, and Classic Seasons copies entrant lists. A copy misses every later change to the original. When Simulate Sessions changes the stock game mode, Progressive's copy doesn't get it.

In 2.0 the copy is a variant of the stock record, so changes to the original flow into it:

```json
// swofty.progressive: its career mode, a variant of the stock 2026 Driver Career
{
  "career.mode": {
    "swofty.progressive/progressive": {
      "extends": "game/f1_career_2026",
      "data_group": "f1_career_progressive",
      "save_container": "driver_career_progressive",
      "title": "lng_progressive_title"
    }
  }
}
```

```text
[registry] career.mode swofty.progressive/progressive: extends game/f1_career_2026
           + layer from kizi.simulate-sessions on game/f1_career_2026 (/sessions/simulate) flows in
```

The applier writes the variant as its own rows with its own stat ids, the same rows the copy makes today, plus every change on the base.

## How a variant resolves

```text
effective(mod.esp/f1_2017_v10) =
    effective(swofty.classic-seasons-2017/f1_2017)    the base with all of ITS layers
  + the variant's own value                           as a patch
  + layers on mod.esp/f1_2017_v10                     in load order
```

- **The base's layers flow into the variant.** If a livery addon layers a new Williams livery onto the 2017 base, the V10 variant gets it too, unless the variant sets Williams' livery itself.
- **Variants can extend variants.** `someone/f1_2017_v10_wet` can extend `mod.esp/f1_2017_v10`. The chain resolves from the root.
- **A cycle is refused**, and so is a chain deeper than 8. The record that closes the loop fails validation.
- **A missing base** (the addon that provides it is off) leaves the variant unresolved: `get` returns `no_record`, and the launcher names the missing addon. Declaring the base's addon in `requires` makes the launcher say so before Play:

```json
{ "requires": { "content.package:swofty.classic-seasons-2017/f1_2017": "*" } }
```

## Extend or vary?

- **Layer the base** when your change should apply wherever the base is used. A sound pack that fixes a car's engine note, a livery update, a corrected driver number.
- **Make a variant** when players should be able to choose between the original and yours. A "2017 with V10s" GP/TT entry next to the real 2017, or a "what if 2017" grid in its own career content.

Both only add. Neither stops the base, or any other addon's change, from working.
