---
title: Sessions and weekends
description: Setup modes, weekend formats, points, rules profiles, safety car and grid as registries, with the session hook points.
---

# Sessions and weekends

```json
// session.weekend_format "f1990/two_qualifying"
{ "sessions": ["practice_1", "practice_2", "qualifying_1", "qualifying_2", "warm_up", "race"],
  "grid": { "from": ["qualifying_1", "qualifying_2"], "rule": "best_of" } }
```

```json
// session.points "f1990/points_1990"
{ "positions": [9, 6, 4, 3, 2, 1], "fastest_lap": 0, "best_results": 11 }
```

```text
registries   session.mode (a setup mode; stat id <= 31), session.weekend_format, session.length,
             session.points, session.rules_profile, session.safety_car, session.grid               build
             session.points.runtime (best-N rules; career snapshot), session.grid_rule             runtime
hook points  session.entrants.select (Sess_Select_Entrants), session.weekend.start, session.grid.build
             (Game_GridSetup_SetupStartingGrid), session.results.finalise, session.state.enter,
             session.penalty.issue (the Sess_Issue_* trio), session.safety_car.deploy
operations   session::set_grid, session::issue_penalty
ready        partial: the entrant and grid pipeline is L2 (Custom Grid Size and F1 1990 use it); points and
             penalties are L1
limits       grid limits; points arrays padded to the grid size
```
