---
title: Power units, ERS and DRS
description: Power units, engine suppliers, DRS and component rules as registries, with the engine wear and penalty hook points.
---

# Power units, ERS and DRS

```text
registries   power_unit (Engine, Turbo, FuelTank, FuelECU, ERS VTF blocks and upgrade blocks), engine.supplier,
             rules.drs (a DRS lap group), rules.pu_components (allocation, grid drops, wear)      build
             rules.pu (which component set and DRS group a career uses)                         runtime
hook points  session.drs.allowed (needs RE), career.engine.wear (Game_EngineMgmt_ApplySessionWear),
             career.engine.grid_penalty (Game_EngineMgmt_AddGridPenalty)
operations   engine::add_grid_penalty, engine::set_component_wear
ready        build: ready (PU swaps shipped and verified). runtime: partial (engine management L2; DRS and ERS
             decisions L1 or unknown)
limits       AI tables carry baked ERS data per car; the HUD dial differs per package
```
