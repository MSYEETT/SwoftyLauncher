---
title: Liveries
description: Liveries, decal sets and sponsor packs as registries, and the hook points for special liveries.
---

# Liveries

```json
// livery "someone.liveries/ferrari_2017_monza"
{ "vehicle": "swofty.classic-seasons-2017/ferrari", "textures": "liveries/ferrari_2017_monza/",
  "picker_name": "lng_someone_livery_monza", "unlock": "owned" }
```

```text
registries   livery, livery.decal_set, livery.sponsor_pack                             build
             livery.ai_assignment (which special livery an AI car wears per event)    runtime
hook points  livery.special.list (Game_Build_SpecialLiveryList), livery.frontend.set (SetCarLivery)
operations   livery::grant, livery::assign
ready        build: ready. runtime: unknown (L1)
limits       ownership keys must not collide with stock; shader archives come in pairs per GPU (_amd)
```
