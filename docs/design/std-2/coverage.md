---
title: Coverage
description: Every addon on the Addon Browser and the common imported mods, what each does today, how it's expressed in standard library 2.0, and the conflicts that stop.
---

# Coverage

Every addon on the Addon Browser was checked, plus Custom Grid Size and about 110 imported mods from 37 player bundles. Each card says what the addon does today, what it becomes in 2.0, and which conflict that ends.

Nothing here changes for the player until an addon adopts 2.0: every card's "today" keeps working (see [Compatibility](./compatibility)).

## Seasons

```text
swofty.classic-seasons + 2016 to 2024 (10 addons)
  today    database rows and files: per season ~99 add-row, 170 set, 430 put-record; 80 car packages,
           164 helmets, engine bundles, logos, names, the era boxes
  2.0      provider of era.box, content.package, vehicle, vehicle.package, engine_sound, livery, helmet,
           suit, gloves, team.identity, driver.identity, driver.head, lang.string; all applied by the
           core's appliers. Its generators shrink to writing records.
  ends     whole logo-image replacement (logos become slices); copied stock rows (variants instead);
           hand-picked stat ids (the allocator)
```

## Career

```text
swofty.progressive
  today    career content, wizard page, Team HQ tab, 11 DLLs, years.json read by five of them,
           GetProcAddress between its own DLLs, wraps on contracts, standings, logos and the VM resolver
  2.0      provider of career.season, career.driver_ratings, career.mode (a variant of the 2026 career),
           career.standings, career.rnd.development, track.data (fuel); subscriber to the contract,
           calendar, season and fuel hook points; the progressive.history service
  ends     duplicate wraps with Career Editor on standings and contracts; the GetProcAddress coupling

career.classic-career (Bitey)
  today    its own tile and data group, two screens, team names and colours written profile-wide
  2.0      career.content, career.wizard, ui.screen; its season picker reads the effective content.package
           and era.box records instead of scanning other addons' operations; team.identity as a career layer
  ends     names and colours leaking into every other career

swofty.career-length
  today    extends the stock F1_career_22 event lists only
  2.0      a selector layer on every career.content (max_seasons), so Progressive and Classic Career get it
  ends     other addons' careers stuck at 10 or 20 seasons

swofty.career-editor
  today    a live editor: seats, ratings, calendar, wallet, R&D, facilities, weather; its own wraps
  2.0      career-scope layers on career.season, career.driver_ratings, career.calendar; operations
           career::set_wallet, career::rnd_start, career::set_facility, career::add_weekend,
           session::set_weather; the shared contract and season hook points
  ends     duplicate wraps with Progressive

swofty.driver-manager
  today    new drivers on ids 208-253 it picks itself, look rows, load_after: * to dodge label moves
  2.0      driver.identity variants (extends game/jarno_opmeer), driver.look, driver.head, driver.stats,
           career.driver_pool; ids from the allocator
  ends     id collisions with F1 1990 and Custom Grid Size; contested look rows with Helmet Exchange

swofty.rnd-tyres-thermal
  today    a fifth R&D department in native code; 91 handling files replaced whole; a My Team block
  2.0      career.rnd.department, career.rnd.upgrade, vehicle.handling field layers (tyre thermal fields),
           ui.texture_slice for the pie art; a declarative guard on My Team; saves.careers: "loaded"
  ends     handling clashes with Heavier Damage and every imported livery that carries handling files;
           careers silently losing Tyres progress

kizi.simulate-sessions
  today    a flag on four named stock game modes
  2.0      one layer on game/driver_career_26; modes other addons make are variants, so they inherit it
  ends     Progressive and Classic Career careers missing the flag depending on load order
```

## Teams, drivers, liveries, helmets

```text
custom.team-names
  today    writes the 2026 team rows and names
  2.0      profile layers on team.identity from its launcher page, its own presets registry
  ends     last-write-wins against Classic Career and Progressive (now field-level, by load order, listed)

mildtdesign.more-special-liveries
  today    special liveries, plus a whole replacement of settings_container.ui
  2.0      livery records with special state, livery.ai_assignment, a ui.screen record for its page
  ends     wiping the core's ADDON SETTINGS pages and Classic Career's screens

giggles.helmet-exchange
  today    imports helmet files, equips them, its own Team HQ tab
  2.0      helmet records with who may wear them, a career-scope driver.look layer for the equipped
           helmet, a hub.tab with its own page; helmets it imports come in through an import provider
  ends     contested helmet fields with Driver Manager, Classic Seasons and Progressive

crew8006.real-life-logos, crew8006.bp-ultimate-livery
  today    whole shader_package_2025 archives replaced by files
  2.0      shader.look records, one per shader resource, merged per resource
  ends     two shader packs, or any imported .serps on the same archive, wiping each other
```

## Tracks

```text
mod.sochi, mod.autopolis
  today    track.add copies a slot (Autopolis takes Suzuka's) and sets ~400 track-data records
  2.0      track records that claim a slot and provide a venue; track.data per data set and season;
           extends: game/<donor track> instead of copy_slot; pinned stat ids
  ends     two mods silently sharing a slot (two imported Igora Drives); careers loading a different
           track after the track list changes

kizi.f1-25-26-more-aggressive-ai
  today    replaces every track's common world archive for its AI line
  2.0      track.ai_line layers on game/<track>, with a layout version the line requires
  ends     an AI line applied to a track mod with a different layout

Custom Grid Size (ours)
  today    per-track slot and garage edits for 22-30 cars, gated by an option
  2.0      a content.package variant of game/f1_2026 with more teams; track.slots per layout as option-bound
           layers; session.grid refusing a size the build can't hold
```

## AI, sessions and driving

```text
kizi.ai-tweaks
  today    599 stock cars named one by one; native contact and steering hooks
  2.0      a selector layer on ai.vehicle_config (every F1 car, including added ones); hook points
           session.car.contact and ai.steering.apply
  ends     Classic Seasons, Driver Manager and imported cars never getting its values

kizikafuka.ai-drive, kizi.pitlane
  today    both take and give control of the player car through their own hooks
  2.0      hook point session.player.control (ordered), session.pit_lane.control
  ends     two addons patching the same control state

vmknolle.parts-detach
  today    its own byte patterns for the front wing init and debris
  2.0      hook point car.parts.register, vehicle.parts records, operation car::register_debris_part;
           its patterns become bindings the core verifies (see below)
  ends     double registration when another addon embeds the same module; breaking on game updates

william09leo.heavier-damage-realism
  today    16 pre-built handling sets per car, chosen by an option; 91 handling files replaced whole
  2.0      option-bound selector layers on vehicle.handling: damage fields only
  ends     the R&D Tyres clash; 16 copies of every car

xanpavle.re-optimization
  today    engine setting overrides from inside an early mutex hook
  2.0      graphics.setting records; hook points engine.setting.get and engine.config.parse, which the
           core installs early
  ends     a second graphics addon fighting it over the same reads

nitrocosm.replay-unlocker
  today    launcher only
  2.0      nothing to change
```

## Imported mods

```text
tracks (21 of 32 players; Hockenheim 19, Sepang 17, Nurburgring 14)
           track records with venues and pinned ids; a season's calendar finds them by venue
liveries (60 ids, 53 of them .serps)
           livery records plus vehicle layers; handling inside them split out as its own layer
car ports (fanfreluche F1 2016, 2019, 2020) and season packs (F1 2012 Temporada)
           vehicle variants per team and a dormant content.package; "own GP/TT entry" lets ports coexist
engine sound packs
           engine_sound records and layers; a pack's options become variants, one chosen per car
helmets, driver models, team logos
           helmet, driver.head, driver.look and team.logo layers
everything else
           stays raw and builds exactly as today
```

See [The Simplified importer](./importer) for how each is recognised.

## Bindings an addon brings

Several third-party DLLs (AI Tweaks, Pit Lane, AI Drive, Parts Detach) find game functions with their own byte patterns, so they break on game updates and on the EA build. In 2.0 an addon can ship its patterns as bindings:

```json
// kizi.pitlane/bindings.json
{ "bindings": [ { "name": "PitLane_Set_Limiter", "pattern": "48 89 5C 24 ?? 57 48 83 EC 20 8B FA", "resolve": { "kind": "addr" } } ] }
```

The core checks each pattern against the running exe at load, the same way it checks its own. A pattern that doesn't match one unique site drops only that addon's hooks on it, with a log line, not the whole DLL. Patterns that prove stable are offered to the knowledge base, so they become core bindings with verify rows and resolve on both builds.

## What the coverage added to the design

- `vehicle.handling` with field-level layers ([Systems catalogue](./systems#cars)).
- [Selector layers](./layers#selectors) and [option-bound layers](./layers#option-bound-layers).
- [Copies as variants](./bases#copies-are-variants).
- A specified `track` record with slots, venues and pinned ids ([Dependencies](./dependencies#tracks-have-stable-identities)).
- `ui.screen`, `ui.texture_slice` and `shader.look`, so whole-resource UI and shader replacements end.
- Session and driving hook points: `session.player.control`, `session.pit_lane.control`, `session.car.contact`, `car.parts.register`.
- Live career operations for Career Editor.
- Bindings an addon brings, above.
- Raw build-operation clashes in the conflicts view, next to registry conflicts.
