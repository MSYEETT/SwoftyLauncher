---
title: Teams
description: Team identity, logos per context and pit times as registries, with the logo and name hook points.
---

# Teams

```json
// team.identity "swofty.classic-seasons-2017/toro_rosso"
{ "names": { "title": "lng_cs_team_17_toro_rosso", "short": "lng_cs_team_17_toro_rosso_short" },
  "colours": { "primary": "#0A1E3C", "secondary": "#C8102E" },
  "logo": "swofty.classic-seasons-2017/toro_rosso_logo", "stat_id": "auto" }
```

```text
registries   team.identity (both), team.logo (one image per context: driver select, team select, HUD,
             broadcast, hub; runtime), team.pit_times (build)
hook points  team.logo.resolve (the two logo format sites and the HUD badge), team.name.resolve (needs RE)
operations   team::set_logo, team::set_name, team::set_colours
ready        partial: the logo sites are L2 and shipped; the team record layout is L1
limits       stat ids; the story_info keys a copied team must carry; team_position and garages (grid)
```

`team.logo` replaces whole logo images with one slice per team per context. Classic Seasons, car ports and season packs each change their own teams' slices, and every one of them applies.
