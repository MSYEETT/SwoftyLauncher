---
title: Helmets, suits and gloves
description: Helmets, suits, gloves and team kits per season as registries.
---

# Helmets, suits and gloves

```json
// helmet "giggles.helmets/hamilton_2017"
{ "archive": "helmets/hamilton_2017.erp", "driver": "game/lewis_hamilton", "season": 2017,
  "colours": ["#FFE600", "#00A19B", "#7A2A8B"] }
```

```text
registries   helmet, suit, gloves, wearable.season_set (team kit per season)          build
hook points  wearable.catalogue.build (Game_Build_CustomisationCatalogue)
operations   driver::set_look
ready        build: ready (shipped at scale). runtime: unknown
limits       item rows need every printf-built colour key; asset groups need deps on the team livery group,
             or cutscenes crash
```
