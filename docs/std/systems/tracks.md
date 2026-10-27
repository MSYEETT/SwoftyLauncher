---
title: Tracks
description: Tracks, per-season track data, world layers and slots as registries, with the track data and fuel hook points.
---

# Tracks

```json
// track "mod.hockenheim/hockenheim"
{
  "slot": "auto",
  "names": { "title": "lng_hockenheim_title", "gp": "lng_hockenheim_gp", "acronym": "HOC" },
  "world": { "asset_group": "environment_package/tracks/hockenheim", "eri": "hockenheim_f1_26" },
  "layers": { "f1": { "grid": 22, "garages": 11 }, "myteam": { "grid": 24, "garages": 12 } },
  "data": "mod.hockenheim/hockenheim_data"
}
```

```json
// track.data "game/f1_2026/hockenheim", a layer from a 1990 season on its own data set
{ "laps": 45, "fuel": { "burn_per_lap": 2.2 }, "pit": { "speed_limit": 100 }, "tyres": { "class": "f1990.goodyear/dry" } }
```

```text
registries   track (venue, layout files, names), track.data (one TrackData block per data set and season:
             fuel, laps, tyres, pit, safety car, names, date), track.layer (world variants per mode: grid
             and garage counts), track.slots (grid slots, garages, pit boxes per world)    build
             track.data.runtime (per-career changes to the loaded track objects, undone on unload)  runtime
hook points  track.data.get (Game_TrackData_GetTrack), session.fuel.load
operations   session::set_track_fuel, track::set_data
ready        build: ready (a dozen community tracks ship). runtime: partial (Game_TrackData_GetTrack L2,
             CareerEntity +0xf58 L2, the object layout proven by Progressive 1.2.2)
limits       stock slots without assets (paul_ricard, portimao, sochi, hanoi...), stack-built keys a new track
             must carry, track data found by mode mask, AI packs per car folder, Time Trial on added tracks
```

Tracks are dependencies: a season's calendar names `track` records other addons own (see [Dependencies and guards](/std/dependencies)).
