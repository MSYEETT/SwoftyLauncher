---
title: Career
description: Career modes, seasons, calendars, contracts, R&D, economy and characters as registries, with the career hook points.
---

# Career

```text
registries   career.content, career.mode (mode row + save container + data group), career.season,
             career.calendar, career.driver_ratings, career.market, career.contract_rules, career.rnd.tree,
             career.rnd.department, career.rnd.upgrade_type, career.facility, career.economy,
             career.sponsor, career.character, career.wizard, hub.tab
hook points  career.calendar.add_activity, career.contract.decide, career.contract.sign,
             career.season.process, career.lineup.changes, career.weekend.create,
             career.standings.rebuild, career.rnd.start
operations   career::apply_lineup, career::set_seat, career::set_driver_ratings, career::set_car_number
ready        the most ready domain: 59 career and 46 game functions at L2, saves at L3
limits       the save heap; R&D shows four departments (a fifth needs the native layer R&D Tyres built)
```
