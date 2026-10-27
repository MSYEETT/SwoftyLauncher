---
title: Cars
description: Car packages, handling, damage and parts as registries; the hook points and operations for cars at run time.
---

# Cars

```json
// vehicle "swofty.classic-seasons-2017/ferrari"
{
  "package": "swofty.classic-seasons-2017/cs_2017_ferrari",
  "label": "cs17_ferrari",
  "donor": "game/ferrari_2025",
  "engine_sound": "swofty.classic-seasons-2017/ferrari",
  "livery": "swofty.classic-seasons-2017/ferrari",
  "handling": "swofty.classic-seasons-2017/ferrari_2017",
  "power_unit": "game/pu_2025",
  "ai": { "track_pack": "swofty.classic-seasons-2017/ferrari", "slot": "auto" }
}
```

```text
registries   vehicle, vehicle.package (+ setup presets), vehicle.handling (VTF sections as JSON patches),
             vehicle.damage_model                                                      build
             vehicle.parts (detachable parts per model node)                           runtime
hook points  car.upgrades.fit (Car_Fit_UpgradesToRating), car.vtf.read (the Car_Read_<Section> readers),
             car.parts.register (Car_FrontWing_Init)
operations   vehicle::set_attribute, vehicle::reset_upgrades
ready        build: ready (VTF round trip proven on 74 of 74 files). runtime: partial (fit and reset L2,
             VTF readers and parts L1)
limits       setup presets are side data by package folder; live physics values are XOR-obfuscated, so
             handling layers apply to the VTF before load or through car.vtf.read
```

`vehicle.handling` is a patch on the handling file, not a copy of it. R&D Tyres changes the tyre thermal fields and Heavier Damage the damage fields of the same car. Both layer onto it and both apply, where today they replace the same 91 files whole.
