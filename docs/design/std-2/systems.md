---
title: Systems catalogue
description: Every game system in standard library 2.0 - its registries, hook points and operations, how ready the reverse engineering is, and the limits the core enforces for every registry.
---

# Systems catalogue

Every system an addon can touch has a domain here. Each domain lists:

- **registries:** what addons provide, layer and extend, at build time or run time;
- **hook points:** the game functions the core owns, one ordered chain each;
- **operations:** the core's verbs;
- **readiness:** how much of it the reverse engineering has verified;
- **limits:** the game's own limits that the core enforces for it.

The [first-party addons](./coverage) and the [custom systems](./custom-systems) are built from these.

## Limits every registry inherits

The game has hard limits. In 1.x every addon finds them alone, usually as a crash. In 2.0 the core owns them, once, for every registry:

```text
row counts     A table only reads rows below its _instancecount_. Appliers bump it; nobody writes it by hand.
missing keys   A copied row that lacks a key a stock row has reads garbage, not null. Appliers copy every key
               of the donor row, including the printf-built and stack-built ones (tools/track_keys.py, deps.rs).
side data      Rows are found by label too: setup presets by package folder, track data by mode mask, AI data
               by ai_package_folder/asset_filename. Appliers write the side data with the row.
record budget  The game's value map is sized at load and grows at 838,860 records; runtime edits get about 95k.
               The launcher counts every applier's records and bakes the database when it has to.
stat ids       Small, per-table pools: entrant lists 0-255, drivers 0-254 (58 free), teams to 514, vehicles to
               538, setup modes and seasons <= 31. The core allocates them; a record says "stat_id": "auto".
grid           32 vehicle slots, a 27-car clamp at session enter, an AI table of 30, a wheel map that rehashes
               at the 26th car. session.grid refuses a size the build can't hold, naming the limit.
save heap      Profile saves live in a 3 MB SAVE_DATA heap. Snapshots are budgeted (see Scopes and snapshots).
boot-once      Driver statistics, AI handling and setup presets are read once (at boot or session enter).
               Runtime layers on them fire at those moments, through hook points.
EA build       Every hook point and operation resolves on both the Steam and the EA build, or reports unavailable.
```

```json
// a record asking the core for ids instead of picking them
{ "driver.identity": { "f1990/alain_prost": { "stat_id": "auto", "name": "lng_f1990_prost", "number": 1 } } }
```

```text
[registry] stat ids: driver f1990/alain_prost -> 241 (pool 208-254, 57 free after this)
[registry] stat ids: setup mode f1990/f1_1990 -> 16 (pool 5,7,16-31)
```

An id the core allocated stays the same in every later build of the profile, so saves keep pointing at the same rows.

## Cars

```json
// vehicle "swofty.classic-seasons-2017/ferrari"
{
  "package": "swofty.classic-seasons-2017/cs_2017_ferrari",
  "label": "cs17_ferrari",
  "donor": "game/ferrari_2025",
  "engine_sound": "swofty.classic-seasons-2017/ferrari",
  "livery": "swofty.classic-seasons-2017/ferrari",
  "handling": "swofty.classic-seasons-2017/ferrari_2017",
  "power_unit": "game/pu_2025",
  "ai": { "track_pack": "swofty.classic-seasons-2017/ferrari", "slot": "auto" }
}
```

```text
registries   vehicle, vehicle.package (+ setup presets), vehicle.handling (VTF sections as JSON patches),
             vehicle.damage_model                                                      build
             vehicle.parts (detachable parts per model node)                           runtime
hook points  car.upgrades.fit (Car_Fit_UpgradesToRating), car.vtf.read (the Car_Read_<Section> readers),
             car.parts.register (Car_FrontWing_Init)
operations   vehicle::set_attribute, vehicle::reset_upgrades
ready        build: ready (VTF round trip proven on 74 of 74 files). runtime: partial (fit and reset L2,
             VTF readers and parts L1)
limits       setup presets are side data by package folder; live physics values are XOR-obfuscated, so
             handling layers apply to the VTF before load or through car.vtf.read
```

`vehicle.handling` is a patch on the handling file, not a copy of it. R&D Tyres changes the tyre thermal fields and Heavier Damage the damage fields of the same car. Both layer onto it and both apply, where today they replace the same 91 files whole.

## Liveries

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

## Teams

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

## Drivers

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

## Helmets, suits and gloves

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

## Sounds

```json
// engine_sound "mod.esp/v10_screamer"
{ "bundle": "sounds/v10_screamer.bdl", "settings_from": "game/ferrari_2025", "tuning_from": "game/ferrari_2025" }
```

```text
registries   engine_sound, audio.car (a car's switch name and its audio_car_data)         build
             audio.commentary                                                          later
hook points  audio.bundle.resolve (Game_Build_AudioBundleRegistry, Game_AudioF1Car_Setup)
ready        build: ready. runtime: unknown (L1)
limits       the reader expects 16-layer engine parts (2016-2018 bundles are widened by the applier);
             a car reaches its bundle through its switch name
```

## Tracks

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

Tracks are dependencies: a season's calendar names `track` records other addons own (see [Dependencies and guards](./dependencies)).

## Tyres

```json
// tyre.compound "f1990.goodyear/b"
{ "physical": { "from": "game/c4", "vcmpd": "tyres/goodyear_b.xml" },
  "visual": { "colour": "#E4002B", "texture": "tyres/goodyear_b", "icon": "f1990_b" },
  "name": "lng_f1990_tyre_b", "stat_id": "auto" }
```

```text
registries   tyre.compound (physical + visual pair), tyre.class (ordered options, masks, wear scales),
             tyre.allocation (sets per session), tyre.wear_scale (per track per option)        build
             tyre.rules (mandatory compounds, minimum stops, blankets; career snapshot)        runtime
hook points  session.tyres.class (Sess_TyreClass_FromName), session.tyres.allocate (setPresetChoice,
             setupRaceWeekendTyreSet), session.tyres.closest (Sess_Tyres_ClosestAvailableCompound),
             session.tyres.aws_budget (the Sess_TyreRules_* drains)
operations   tyres::set_class (before the tyre component is built), tyres::grant_sets
ready        partial to ready: class and allocation entry points L2 and used by F1 1990; physics L1;
             wear and thermal writers unknown
limits       a class needs at least 5 options; models are built when the tyre component starts; compound
             icons are glyphs in a font page; physics live per package (a class needs matching VCMPD)
```

## Power units, ERS and DRS

```text
registries   power_unit (Engine, Turbo, FuelTank, FuelECU, ERS VTF blocks and upgrade blocks), engine.supplier,
             rules.drs (a DRS lap group), rules.pu_components (allocation, grid drops, wear)      build
             rules.pu (which component set and DRS group a career uses)                         runtime
hook points  session.drs.allowed (needs RE), career.engine.wear (Game_EngineMgmt_ApplySessionWear),
             career.engine.grid_penalty (Game_EngineMgmt_AddGridPenalty)
operations   engine::add_grid_penalty, engine::set_component_wear
ready        build: ready (PU swaps shipped and verified). runtime: partial (engine management L2; DRS and ERS
             decisions L1 or unknown)
limits       AI tables carry baked ERS data per car; the HUD dial differs per package
```

## Sessions and weekends

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

## Weather

```text
registries   weather.track (climate weights and temperatures per track; build), weather.forecast (a scripted
             forecast; runtime, career snapshot)
hook points  session.weather.setup (Sess_GameWeatherSystem_SetupSession)
operations   weather::set_seed, weather::script
ready        ready for setup and seeds (L2, shipped in Career Editor); dataset layout L1
```

## AI

```text
registries   ai.vehicle_config (the per-car race-craft block), ai.package_maps, ai.track_pack (which baked
             folder a car uses per track, by pace), ai.general, ai.track_difficulty               build
             ai.difficulty_profile                                                              runtime
hook points  ai.config.load (Sess_Load_VehicleAiConfig), ai.settings.resolve (Ai_Resolve_GeneralSettings)
operations   ai::set_vehicle_config, ai::assign_track_pack
ready        data: ready. runtime: partial (loaders L2, decision code L1, the difficulty slider unknown)
limits       baked tables cap pace; a missing AI folder hangs the load; the AI table holds 30 cars
```

AI Tweaks today names 599 stock cars one at a time, so cars other addons add never get its values. In 2.0 it writes one layer with a [selector](./layers#selectors) that matches every F1 car, including ones added later.

## Career

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

## UI and strings

```text
registries   era.box, ui.screen, ui.tile, ui.texture_slice (one slice of a texture array), ui.route,
             ui.option, ui.theme, hud.widget, lang.string
hook points  ui.era.tiles, ui.home.tiles, ui.career_content.tiles, ui.car_category.tiles, ui.screen.built,
             ui.vm.resolve (Vm_Resolve_RegistryNode), ui.route.open, hud.mfd.pages, lang.lookup
operations   ui::set_text, ui::set_bool, ui::set_ref, popup::show, hud::toast
ready        ready for tiles, era boxes, view models, routes, popups and strings; HUD partial
limits       native screens with fixed counts (era tiles, four R&D departments, eight MFD pages)
```

## Graphics, replays and telemetry

```text
registries   graphics.look (a LUT with strength, stacked in order), graphics.pass, graphics.setting
             replay.hero_shots, telemetry.channel (a value an addon publishes per car or session)
hook points  render.present, graphics.setting.read, replay.recordable.register, session.tick
ready        looks and passes ready (shipped); settings L2; replays and telemetry partial
limits       HDR and frame generation change the pipeline; replay holders size the grid
```

## The order

Domains move into the core in this order. It follows readiness, how many addons need each domain, and today's conflicts:

1. **Career seasons, line-ups and contracts:** the most L2 code, and two addons already wrap the same functions.
2. **Content at build time:** vehicle, package, engine sound, livery, wearables, heads, team and driver identity, texture slices. The most addons, and most of today's file conflicts.
3. **Tracks and track data.**
4. **Tyres:** the custom tyre systems.
5. **Session rules:** grid size, weekend formats, points.
6. **UI and strings.**
7. **Later:** AI, power unit and DRS at run time, weather, graphics, telemetry, replays. The reverse engineering they need is scheduled now: the AI difficulty application, the DRS decision, the engine component selector, and the attribute map.
