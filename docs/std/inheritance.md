---
title: The base, years and careers
description: How every system's data passes from the stock game to the base every year shares, to each GP/TT box and career season, to a running career and a session; who wins when addons, imports and year packs meet; and what happens when something is removed.
---

# The base, years and careers

Every record in every system is read through the same chain. Each level starts from the one above it and can set its own values:

```text
game        the stock game: game/ records (cars per model year, teams, drivers, tracks, tyres, rules)
  base      what every year shares: game/ plus everything addons and imports add or layer on game/ records
    box     one GP/TT box or career season (a content package): a variant of the base, with its own values
      career    one running career: the box for each season, plus career layers, frozen into the career
        session one session: changes scoped to this session only, put back when it ends
```

Three rules decide everything:

1. **Adding to the base adds to every year.** A track, a livery on a base car, a helmet on a base driver, an engine sound on a base car, a team logo: every box and career that uses that base record gets it.
2. **A box's own value beats the base.** A year pack that sets a value on its variant keeps it, however many addons layer the base. Only the fields it sets are protected; everything else still comes from the base.
3. **A layer on a specific record wins by load order.** An addon that layers the box's own record (or retargets an import to it) competes with the box on that record, and the later one in load order wins.

```cpp
// rule 1: Hockenheim joins the base, so every box offers it
tracks::tracks.add("hockenheim", { .base = tracks::tracks["game/austria"],
                                   .provides = { tracks::venues["venue/hockenheimring"] } });

// rule 2: Classic Seasons 2017's Red Bull is a variant that sets its own livery and sound
auto rb17 = cars::vehicles.variant("redbull", cars::vehicles["game/redbull_2025"], [](cars::Vehicle &v) {
    v.livery = liveries::liveries["swofty.classic-seasons-2017/redbull"];
    v.engine_sound = sounds::engine_sounds["swofty.classic-seasons-2017/renault"];
});

// rule 3: an addon that targets the 2017 car itself competes with the box, by load order
cars::vehicles.layer(cars::vehicles["swofty.classic-seasons-2017/redbull"],
                     [](cars::Vehicle &v) { v.livery = liveries::liveries["someone/rb13_monaco"]; });
```

## Layer the base, or every record

An addon picks how far its change reaches:

```cpp
// "everywhere the base car is used, unless a year sets its own": layer the base record
cars::vehicles.layer(cars::vehicles["game/redbull_2025"], [](cars::Vehicle &v) { v.livery = mine; });

// "every matching car, years included": a selector layers each matching record itself, so it beats
// the boxes' own values (rule 3: load order decides against any box that layers the same record later)
ai::vehicle_configs.layer(where<ai::VehicleConfig>([](auto &c) { return c.class_ == "f1"; }),
                          [](ai::VehicleConfig &c) { c.racecraft.overtake_aggression = 0.85f; });
```

- **Cosmetic and content changes** (liveries, helmets, sounds, logos, tracks) layer the base. Years keep their history.
- **Behaviour and tuning changes** (AI, damage, physics fixes, difficulty) use a selector. They apply everywhere.
- **The importer** layers the base by default. **Applies to** retargets an import to a box's record instead.

## Every system through the chain

```text
cars          base: game/<team>_<year> models; car ports and imports layer them (model, livery, handling)
              box:  a year pack's own vehicles (variants of a donor) with their own model, livery, sound, PU
              career: the season's box vehicles; Progressive switches the active box each season
              session: handling and damage edits scoped to a session (Heavier Damage's option re-applies at load)

liveries      base: a livery layered on a base car shows wherever that car isn't overridden
              box:  the year's livery on its own vehicle beats base liveries
              special liveries (a picker choice) add to the list in every box that uses the car

teams         base: game/ team identities; logo and name mods layer them (every year)
              box:  per-year names, logos and colours on the year's own team records
              career: a career's team names (Classic Career) are career-scope, never profile-wide

drivers       base: game/ drivers; heads, looks and helmets layer them (every year)
              box:  per-year helmets, numbers and suits (Classic Seasons sets 2017 Vettel's helmet)
              career: ratings per season from career.driver_ratings, frozen at career creation
              new drivers (Driver Manager) join the base: every box's free agents, every career's market

helmets/suits base: a helmet on a base driver shows in every year that doesn't set its own
              box:  team kits per season beat base kits

sounds        base: an engine sound on a base car (the stock 2025 Ferrari) shows wherever that car is used
              box:  a year's own bundle on its own car beats it (2016-2018 period sounds)

tracks        base: every track, stock and added, with base data cloned from its base track
              box:  which tracks the box offers and in what order (default: all of the base), and per-track data
                    for that box (layout, laps, fuel, tyres, names, date), applied when the box is picked
              career: the season's calendar picks tracks by venue; a raced track becomes a career dependency
              session: track data for this session (fuel, laps) applied at session load

tyres         base: the stock compounds and classes, plus added compounds (a selectable visual, a physics set)
              box:  the year's tyre class and allocations (2016 boxes on 2016 compounds), scoped to the box
              career: tyre rules per season (mandatory compounds, sets), frozen with the career
              session: the class each car runs, answered per session (session.tyres.class_of)

power units   base: the stock 2025 and 2026 power units
              box:  the PU and DRS/ERS rules a year uses (Classic Seasons: the 2025 PU, 2016 DRS)
              career: Progressive's per-season PU, from the box of that season

sessions      base: stock weekend formats, points, rules profiles, grid size
              box:  a year's format and points (two qualifying sessions, 9-6-4-3-2-1), scoped to its setup mode
              career: best-N and other championship rules per season, frozen with the career

weather       base: per-track climate
              box:  a year's climate per track (rare); a scripted forecast is session or career scope

AI            base: per-car AI config; selector tweaks apply to every car, years included
              box:  a year's AI track packs per car (which baked folder by pace)

career data   base: stock career modes and data groups; new modes are variants (they inherit stock changes)
              box:  career seasons (line-ups, swaps, ratings) per year, from the year's data
              career: everything career-scope, frozen into the career at creation

UI/strings    base: stock strings and screens; string mods layer them
              box:  per-year strings (team names, era box title) scoped to the box or career
              tiles and era boxes add to the menus; they never replace another addon's

graphics      base only: looks and passes stack in load order; settings are global
```

## When the same thing is set twice

```text
two imports on the same base record            the later in load order wins that field (per resource for raw parts)
an import and a year pack                      the year pack's own value wins in its box; the import wins on the base
an import retargeted to a year's record        competes with the year pack on that record: load order
two year packs for the same year               two boxes under two keys: both exist, the player picks
a year pack layering another year pack's box   load order, on that record
a selector and a box's own value               the selector wins (it layers the box's record itself)
a career layer and anything else               the career layer wins inside that career
```

Every one of these is logged with both addons and the field, and listed in the launcher's conflicts view.

## When something is removed

```text
cosmetic (livery, helmet, sound, logo, strings)   falls back to the next value down the chain; careers are unaffected
content a career used (a track raced, a car driven, a team in the line-up)
                                                  the career needs it: its dependency record names it, the launcher
                                                  says before Play, and a guard stops the load with a popup
content a career skipped (an optional track not raced)   not needed: the career records it as skipped
a year pack                                       its boxes go; careers made in them need it (as above)
a selector tweak (AI, damage)                     the tuning reverts at the next load; careers keep going
```

## What a career freezes

A career copies, when it's created:

- its seasons, ratings and rules (`career.season`, `career.driver_ratings`, tyre and championship rules);
- which box each season uses.

It doesn't copy content. Cars, liveries, tracks and sounds are read live from the boxes, so a better livery or a fixed track reaches running careers. Content a career depends on is protected by its dependency record instead (see [Dependencies and guards](/std/dependencies)).

## Saves

Nothing here writes to the game's own save data except the career's `swofty_registry` container (its frozen data and its dependency record). Turning addons on and off never changes a save. It only changes what loads, and the guards stop a load that would lose progress.
