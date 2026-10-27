---
title: How changes reach seasons and careers
description: How each game system's data passes from the stock game to what every year shares, to each Grand Prix and Time Trial box, to a career and a session; which addon wins when two change the same thing; and what happens when an addon is removed.
---

# How changes reach seasons and careers

Your addon can change what every year shares, change one year, or change things inside a single career or session. Where you make the change decides who sees it and whose value wins.

Install a Red Bull livery. Every year that uses the stock Red Bull now shows it. Classic Seasons' 2017 Red Bull has a livery of its own, so the 2017 box keeps that one. A running career reads its cars and liveries from its boxes, so it picks up the new livery while keeping the ratings and rules it saved when it began. A handling change made for one session lasts until that session ends.

Every system reads its data through the same five levels. Each level starts with the values of the one above it and can set values of its own:

1. **The stock game** supplies the original records under `game/`: car models for each model year, teams, drivers, tracks, tyres and rules. A record is one item of data, such as a car or a driver.
2. **Every year** shares one set of data, called the **base** in code. It holds the stock records, everything addons and imports add, and the changes they make to stock records.
3. **A box** is one of the year tiles a player picks in Grand Prix or Time Trial, or one season of a career. In code it's a **content package**. It inherits the base and can set its own values.
4. **A career** uses the box chosen for each of its seasons, plus changes scoped to that career. Its season choices, ratings and rules are frozen when the career is created.
5. **A session** can make temporary changes. They're undone when the session ends.

A **layer** changes values on a record that already exists. A **variant** is a separate record that inherits another record's values and overrides some of them. A year pack, for example, makes its 2017 car as a variant of a stock car, then gives it a different livery and engine sound.

## Three rules

1. **Adding to the base adds to every year.** An added track joins every year's track list. A livery, helmet, engine sound or team logo layered on a base record reaches every box and career that uses that record, unless the box sets its own.
2. **A box's own value wins over the base.** If a year pack sets its car's livery, addons that layer the base car's livery can't replace it. Only the fields the box sets are protected. Every other field still comes from the base.
3. **Changes to the same record compete by load order.** An addon that layers a box's own record competes directly with that box, and the later one in load order wins. Retargeting an import to that record works the same way.

```cpp
// Rule 1: Hockenheim joins the base, so every box offers it.
tracks::tracks.add("hockenheim", { .base = tracks::tracks["game/austria"],
                                   .provides = { tracks::venues["venue/hockenheimring"] } });

// Rule 2: Classic Seasons 2017's Red Bull sets its own livery and sound.
auto rb17 = cars::vehicles.variant("redbull", cars::vehicles["game/redbull_2025"], [](cars::Vehicle &v) {
    v.livery = liveries::liveries["swofty.classic-seasons-2017/redbull"];
    v.engine_sound = sounds::engine_sounds["swofty.classic-seasons-2017/renault"];
});

// Rule 3: layering the 2017 car itself competes with the box, by load order.
cars::vehicles.layer(cars::vehicles["swofty.classic-seasons-2017/redbull"],
                     [](cars::Vehicle &v) { v.livery = liveries::liveries["someone/rb13_monaco"]; });
```

## How far a change reaches

Layer a base record when you want the change everywhere that record is inherited. Changing the stock Red Bull's livery reaches every year that uses it, and a year with its own livery keeps it.

Use a **selector** when you want to change every record that matches a condition. A selector layers each matching record itself, so it reaches a year pack's own cars as well as stock cars, and its change competes on each record by load order.

```cpp
// Change the base car. Years that set their own livery keep it.
cars::vehicles.layer(cars::vehicles["game/redbull_2025"], [](cars::Vehicle &v) { v.livery = mine; });

// Change every F1 car's AI, including the cars year packs own.
// This replaces a box's own value, unless a later layer on that record changes it again.
auto f1 = where<ai::VehicleConfig>([](const ai::VehicleConfig &c) {
    return c.cls == ai::Class::f1 || c.cls == ai::Class::f1_26;
});
ai::vehicle_configs.layer(f1, [](ai::VehicleConfig &c) { c.racecraft.overtake_aggression = 0.85f; });
```

- Cosmetic and content changes, such as liveries, helmets, sounds, logos and tracks, layer the base. Years that set their own keep them.
- Behaviour and tuning changes, such as AI, damage, physics fixes and difficulty, use a selector. They reach every year.
- The importer layers the base by default. Its **Applies to** setting moves an import onto a box's own record instead.

## Cars

The base holds the stock `game/<team>_<year>` cars. Car ports and imports layer them: the model, the livery, the handling.

A year pack makes its own cars as variants of a **donor**, the stock car each one inherits from. They can set their own model, livery, engine sound and power unit. Classic Seasons' 2017 Red Bull inherits a stock car and supplies its own period livery and sound.

A career uses the cars of its current season's box. Progressive switches the box each season. Handling and damage changes can also be scoped to one session: Heavier Damage's session option applies its changes again at each load.

## Liveries

A livery layered on a base car appears wherever that car's livery is inherited. A year pack's own livery on its own car wins. A stock Red Bull repaint reaches every box that uses the stock livery, and the 2017 Red Bull keeps its 2017 design.

Special liveries, the extra choices in the livery picker, add to the list in every box that uses the car.

## Teams

Stock teams are base records. Name and logo mods layer them in every year. A year pack can give its own team records their own names, logos and colours.

A team logo update reaches every year that inherits the logo, and a year with its own logo keeps it. Team names that Classic Career sets belong to that career and never apply across the whole profile.

## Drivers

Stock drivers are base records. Changes to their heads, looks and helmets reach every year that inherits those values. A year pack can set its own helmets, numbers and suits: Classic Seasons sets Vettel's 2017 helmet.

Career ratings come from `career.driver_ratings` for each season and are frozen when the career is created. New drivers from Driver Manager join the base, so they're free agents in every box and on every career's driver market.

## Helmets and suits

A helmet on a base driver appears in every year that doesn't set its own. A helmet replacement reaches the stock driver across every box and leaves Classic Seasons' 2017 Vettel helmet alone.

A season's own team kits win over base kits.

## Engine sounds

An engine sound on a base car follows that car wherever its sound is inherited. Replacing the stock 2025 Ferrari's sound changes it in every box that uses that car without setting its own.

A year pack's own sound on its own car wins. Cars with their own 2016–2018 period sounds keep them.

## Tracks

The base track list holds every stock and added track. An added track starts from a copy of the track it names as its base: the Hockenheim example above starts from Austria.

A box decides which tracks it offers and in what order. By default it offers every track in the base. It can also set its own layout, lap count, fuel, tyres, names and dates for each track, applied when the player picks the box.

A career's calendar picks tracks by venue. Once a track has been raced, the career depends on it. Track data for one session, such as fuel and lap count, applies when the session loads.

## Tyres

The base holds the stock compounds and classes, plus added compounds: a visual a player can select, or a set of physics.

A box picks its tyre class and allocations: a 2016 box runs 2016 compounds, scoped to that box. A career freezes each season's tyre rules, such as mandatory compounds and the sets each driver gets.

The class each car runs is answered once per session by `session.tyres.class_of`.

## Power units

The base holds the stock 2025 and 2026 power units. Each box picks its power unit and its DRS and ERS rules.

Classic Seasons uses the 2025 power unit with 2016 DRS rules. Progressive takes each season's power unit from that season's box.

## Session rules

The base holds the stock weekend formats, points systems, rules profiles and grid size. A box can set its own format and points, scoped to its setup mode.

A year can run two qualifying sessions and score 9-6-4-3-2-1. A career freezes each season's championship rules, such as counting only a driver's best results.

## Weather

Each track has a base climate. A box can set its own climate for a track in its year, though few do. A scripted forecast belongs to a session or a career.

## AI

The base holds each car's AI config. A selector reaches every matching car, including the cars year packs own: the aggression example above changes every F1 car.

A year can also pick an AI track pack for each car: which folder of baked AI data it uses, by pace.

## Career data

The base holds the stock career modes and data groups. New career modes are variants of a stock mode, so they inherit changes to it.

Each box supplies its career season: the line-up, the swaps and the ratings. A historical season brings its own drivers and ratings. Everything scoped to a career is frozen into it when it's created.

## Menu text and screens

The stock strings and screens are base records. Text mods layer them. Text for one year, such as a team's name that season or a box's title, is scoped to its box or career, so a historical team name shows in its season without becoming the name everywhere.

Menu tiles and boxes add to the menus. They never replace another addon's.

## Graphics

Graphics exist only in the base. Looks and rendering passes stack in load order, and their settings are global: turning on a look changes every box.

## When two addons change the same thing

What decides it is the record each change targets:

- **Two imports change the same base record.** The later one in load order wins each field they both set. Raw parts are decided per resource.
- **An import changes a base record and a year pack sets its own value.** The import changes the base. The year pack keeps its own value in its box.
- **An import is retargeted to a year's own record with Applies to.** It competes with the year pack on that record, by load order.
- **Two year packs provide the same year.** Each makes its own box under its own key. Both appear, and the player picks one.
- **A year pack layers another year pack's box.** They compete on that record, by load order.
- **A selector changes a field a box also sets.** The selector wins, because it layers the box's record directly. A later layer on that record can still change it again.
- **A career layer and any other change.** The career layer wins inside that career.

A Red Bull livery imported onto the base can't replace the 2017 box's own livery. Retargeting the import to the 2017 car makes the two compete by load order.

The core logs every one of these with both addons and the field. It writes them to `registry/conflicts.json` when a session loads, and the launcher's conflicts view shows the last one.

## When an addon is removed

Removing an addon changes what loads. What happens depends on what it supplied and whether a career used it:

- **Cosmetics fall back to the next value up the chain.** Liveries, helmets, sounds, logos and text: removing a livery reveals the one it covered. Careers carry on.
- **Content a career used stays required.** A track it raced, a car it drove or a team in its line-up is in the career's dependency record. The launcher warns before Play, and a guard stops the load with a popup if it's missing.
- **Content a career skipped isn't required.** An optional track the career never raced is recorded as skipped.
- **Removing a year pack removes its boxes.** Careers created in those boxes need the pack, with the same warning and guard.
- **Removing a selector change undoes it at the next load.** Take out an AI or damage tweak and careers carry on with what's left.

## What a career keeps

When a career is created, it copies:

- its seasons and ratings: `career.season` and `career.driver_ratings`;
- each season's tyre and championship rules;
- which box each season uses.

It doesn't copy content. Cars, liveries, tracks and sounds are read live from the boxes, so a better livery or a fixed track reaches a running career, and the career keeps its frozen ratings and rules. The content a career depends on is protected by its dependency record: see [Dependencies and guards](/std/dependencies).

The only save data the registry writes is each career's `swofty_registry` container: its frozen data and its dependency record. Turning addons on or off never changes a save. It changes what loads, and guards stop any load that would lose progress.
