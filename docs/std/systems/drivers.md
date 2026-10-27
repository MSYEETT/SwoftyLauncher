---
title: Drivers
description: Driver identity, heads, looks and season ratings as registries, with the ratings, number and head hook points.
---

# Drivers

```json
// driver.identity "f1990/ayrton_senna"
{ "stat_id": "auto", "names": { "first": "lng_f1990_senna_first", "last": "lng_f1990_senna_last" },
  "nationality": "brazil", "number": 27, "head": "f1990/senna_head", "look": "f1990/senna_1990",
  "audio_name": "senna" }
```

```text
registries   driver.identity (both), driver.head (head archive, morph, sidecars, rig; build),
             driver.look (suit, gloves, helmet, pose; build), career.driver_ratings (runtime, snapshot)
hook points  driver.ratings.read (Career_Get_DriverStatValue), driver.number.assign
             (Career_DriverNumbers_AssignFree), driver.head.resolve (Ent_EntrantsSystem_FindDriverHead)
operations   career::set_car_number, career::set_driver_ratings, driver::set_look
ready        partial to ready: ratings getters, the number check and the entrant parse are L2; heads and
             looks are L1 but proven by data
limits       58 free driver ids; about 150 records per new driver (statistics holder, accolade, entrant row);
             a driver with no look hangs the session, so the applier refuses one
```
