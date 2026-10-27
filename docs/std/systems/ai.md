---
title: AI
description: AI vehicle configs, difficulty maps and baked track packs as registries, with the AI config hook points.
---

# AI

```text
registries   ai.vehicle_config (the per-car race-craft block), ai.package_maps, ai.track_pack (which baked
             folder a car uses per track, by pace), ai.general, ai.track_difficulty               build
             ai.difficulty_profile                                                              runtime
hook points  ai.config.load (Sess_Load_VehicleAiConfig), ai.settings.resolve (Ai_Resolve_GeneralSettings)
operations   ai::set_vehicle_config, ai::assign_track_pack
ready        data: ready. runtime: partial (loaders L2, decision code L1, the difficulty slider unknown)
limits       baked tables cap pace; a missing AI folder hangs the load; the AI table holds 30 cars
```

AI Tweaks today names 599 stock cars one at a time, so cars other addons add never get its values. In 2.0 it writes one layer with a [selector](/std/layers#selectors) that matches every F1 car, including ones added later.
