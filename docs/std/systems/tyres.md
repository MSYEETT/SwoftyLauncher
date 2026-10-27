---
title: Tyres
description: Tyre compounds, classes, allocations, wear and rules as registries, with the class, allocation and set-choice hook points.
---

# Tyres

```json
// tyre.compound "f1990.goodyear/b"
{ "physical": { "from": "game/c4", "vcmpd": "tyres/goodyear_b.xml" },
  "visual": { "colour": "#E4002B", "texture": "tyres/goodyear_b", "icon": "f1990_b" },
  "name": "lng_f1990_tyre_b", "stat_id": "auto" }
```

```text
registries   tyre.compound (physical + visual pair), tyre.class (ordered options, masks, wear scales),
             tyre.allocation (sets per session), tyre.wear_scale (per track per option)        build
             tyre.rules (mandatory compounds, minimum stops, blankets; career snapshot)        runtime
hook points  session.tyres.class (Sess_TyreClass_FromName), session.tyres.allocate (setPresetChoice,
             setupRaceWeekendTyreSet), session.tyres.closest (Sess_Tyres_ClosestAvailableCompound),
             session.tyres.aws_budget (the Sess_TyreRules_* drains)
operations   tyres::set_class (before the tyre component is built), tyres::grant_sets
ready        partial to ready: class and allocation entry points L2 and used by F1 1990; physics L1;
             wear and thermal writers unknown
limits       a class needs at least 5 options; models are built when the tyre component starts; compound
             icons are glyphs in a font page; physics live per package (a class needs matching VCMPD)
```
