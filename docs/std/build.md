---
title: When changes apply
description: Addons run live in the game. The core applies each registered record and layer at the moment the game reads that data - at boot, when content becomes active, at a session load or on every read - in standard library 2.0.
---

# When changes apply

Addons run live in the game. The core applies each record and layer at the moment the game reads that data, and keeps it live. The launcher never needs to know what addons register.

<Split>

One addon, four moments. The car's sound goes into the game database at boot. The 2017 fuel rule applies when the 2017 package becomes active, and is put back when it ends. The season is read through the registry whenever the career asks. The bundle file is served in memory when the game opens it.

Nothing here is written into the game folder.

<template #example>
<Example root="mod.esp" code-file="native/src/esp.cpp" data-file="registry/content.json" files="swofty.json, native/esp.dll, sounds/v10_screamer.bdl, registry/layers.json">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/career.hpp>
#include <swofty/systems/content.hpp>
#include <swofty/systems/sessions.hpp>
#include <swofty/systems/sounds.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    // file: served in memory through the runtime asset route when the game opens it
    auto v10 = sounds::engine_sounds.add("v10_screamer", {
        .bundle = file("sounds/v10_screamer.bdl"),
        .settings_from = sounds::engine_sounds["game/ferrari_2025"],
    });

    // boot: vehicle is a database record, applied before the database loads
    cars::vehicles.layer(cars::vehicles["swofty.classic-seasons-2017/ferrari"], [=](cars::Vehicle &v) {
        v.engine_sound = v10;
    });

    // content: applied when the 2017 package becomes active, put back when it ends
    content::packages.layer(content::packages["swofty.classic-seasons-2017/f1_2017"], [](content::Package &p) {
        p.rules.refuelling = false;
        p.rules.race_fuel_kg = 105;
    });

    // read: seasons are read through the registry, every time
    career::seasons.layer(career::seasons["swofty.progressive/2017"], [](career::Season &s) {
        s.teams.at(1).seats[0] = drivers::identities["game/fernando_alonso"];
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
{
  "engine_sound": {
    "v10_screamer": { "bundle": "sounds/v10_screamer.bdl", "settings_from": "game/ferrari_2025" }
  }
}
```

```json
// registry/layers.json
{
  "vehicle":         { "swofty.classic-seasons-2017/ferrari": { "engine_sound": "mod.esp/v10_screamer" } },
  "content.package": { "swofty.classic-seasons-2017/f1_2017": { "rules": { "refuelling": false, "race_fuel_kg": 105 } } },
  "career.season":   { "swofty.progressive/2017": { "teams": [ { "slot": 1, "seats": ["game/fernando_alonso", "game/valtteri_bottas"] } ] } }
}
```

</template>
</Example>
</template>
</Split>

`swofty/loader.log`, from the game start to the end of a Time Trial in the 2017 Ferrari:

```text
[14:13:47.300] trigger: KERNEL32!CreateMutexW from 0x140178667 (.xtls) name 'Codemasters F1 25' on thread 29952, 120 ms after attach; loading addons now (early stage)
[registry] register: mod.esp (esp.dll) 1 engine_sound, 3 layers on vehicle, content.package, career.season (early stage, 0.4 ms)
[registry] vehicle swofty.classic-seasons-2017/ferrari: edit by mod.esp changed /engine_sound -> mod.esp/v10_screamer
[registry] applied at boot: engine_sound 1 (1 bundle served in memory), vehicle 1 (3 database records)
[registry] content swofty.classic-seasons-2017/f1_2017 active: 4 scoped changes applied
[registry] content swofty.classic-seasons-2017/f1_2017 ended: 4 scoped changes put back
```

## The moments

Each registry says when the game reads its data, with `RegistryOptions::applies` (`x-applies` in its schema):

```cpp
inline Registry<Vehicle> vehicles{"vehicle", { .applies = Applies::boot }};
inline Registry<Package> packages{"content.package", { .applies = Applies::content }};
inline Registry<RulesProfile> rules_profiles{"session.rules_profile", { .applies = Applies::session }};
inline Registry<Season> seasons{"career.season", { .applies = Applies::read, .snapshot = true, .snapshot_budget = 65536 }};
```

```text
boot      before the database loads       database-backed records (vehicles, teams, drivers, tracks, modes)
                                          go into the game database in memory
content   when content becomes active     a career loads, a GP/TT box is picked, a season starts: scoped changes
                                          apply, and are put back when that scope ends
session   at each session load            the session's records apply for that session
read      on every read                   runtime registries (seasons, ratings, rules), read through the registry
files     when the game opens the path    archives and engine bundles, served in memory by the runtime asset route
```

The core logs every apply and every put-back.

## At boot

`SWOFTY_REGISTER` runs once per game start, at the early stage, on the loader thread, addon by addon in load order. This is the 1.x early-stage plugin load: `"early": true`, which the build turns on for any DLL that has a `SWOFTY_REGISTER`. Registering only declares. Nothing is written yet.

Then the core resolves every registry (bases, variants, layers, selectors, options), and puts the boot records into the game database in memory before the game loads it.

**Database room.** The game's value map can't grow after it's loaded. The core counts the records addons register in `SWOFTY_REGISTER` and sizes the map before the game loads the database. Until that lands (an RE item for the core), the room is about 95k records. A registration past the room is refused with a log line naming the addon:

```text
[registry] register: someone.megapack (megapack.dll) 4,200 vehicle: 1,180 refused, past the database room (~95k records this game start)
```

## When content becomes active

A change to a content-scoped registry applies when its content becomes active, and is put back when that scope ends. That's how Progressive 1.2.2's fuel patch works today: the 2017 rules hold while the 2017 package is active, and Grand Prix in 2026 is stock again afterwards.

## On every read

Runtime registries apply nothing up front. Every read goes through the registry and returns the effective value (see [Reading](/std/reading)). Their providers can be dynamic, and they can have [career snapshots](/std/scopes#career-snapshots). An [operation](/std/operations) or an addon's own code applies them to the game.

## Files

A record field of type `Path` (`file("sounds/v10_screamer.bdl")`) names a file in the addon's folder. When the game opens the file the record maps it to, the core serves it in memory through the runtime asset route. The game folder stays as the launcher built it.

## The core's appliers

An **applier** turns an effective record into the change the game needs: database rows, a served file, an archive edit. The core has one per core registry, in `swofty_core`, and runs it at that registry's moment:

```cpp
// swofty_core: src/std/engine_sound.cpp
void apply(Ref<EngineSound> id, const EngineSound &sound, ApplyCtx &ctx)
{
    std::string bundle = ctx.bundle_name(id);                              // "esp_v10_screamer"
    ctx.serve(sound.bundle, "audio/2025/bundles/" + bundle + ".bdl");      // in memory, when the game opens it
    db::Row settings = ctx.db_row("audio.audio_bundle_settings", sound.settings_from);
    ctx.db_add_row("audio.audio_bundle_settings", settings.with_label(bundle));
    ctx.note("engine sound %s: bundle %s, settings from %s", id, bundle, sound.settings_from);
}
```

```text
apply     engine_sound "mod.esp/v10_screamer"
            serve    mod.esp/sounds/v10_screamer.bdl -> audio/2025/bundles/esp_v10_screamer.bdl
            db add   audio.audio_bundle_settings (copied from ferrari_2025, bundle esp_v10_screamer)
          vehicle "swofty.classic-seasons-2017/ferrari"
            db set   audio.audio_car_data.<cs17_ferrari switch>.*.bundle_name = esp_v10_screamer
```

**An addon's own registries** are applied by the addon's own code: it reads its effective records and adds core records or calls operations (see [Your own systems](/std/systems/custom)).

**Order:** records apply in load order among themselves, a record at the position of the addon that provides it, a layer at the position of the addon that wrote it. They act on the game as the launcher built it, so they come after every addon's 1.x operations.

## While the game runs

Addons can register and layer at any time, not only in `SWOFTY_REGISTER`. The change takes effect at the next moment the game reads that data. When that's too late for this session, the core says so:

```text
[registry] layer from tails.whatif-history on career.season swofty.progressive/2017 registered at run time; applies from the next session load
[registry] layer from someone.realism on vehicle game/mercedes_2025 registered at run time; applies from the next game start
```

`SWOFTY_ON_LOAD` keeps its 1.x meaning: events, hooks, services, and registration from code while the game runs.

## The launcher

The launcher installs addons, builds their 1.x operations, checks the dependencies the manifest declares, and starts the game. It never needs to know what addons register.

- **Its editing pages** (a "what if" editor) write the addon's own storage, and the addon's code applies that as a layer in the game (see [The launcher side](/std/launcher)).
- **Diagnostics come from the game.** The core writes `registry/effective.json` for Send Logs and the conflicts report at each session load. The launcher can show the last one, but nothing depends on it.
- **Every 1.x build path stays.** Hand-written `operations`, imports and launcher plugin build replies work as before. A profile of 1.x addons builds byte-identical game files (see [Compatibility](/std/compatibility)). For imported mods, see [The Simplified importer](/std/importer).

## What this means for addons

- **Pure data addons stay pure data.** An engine sound pack, a livery pack, a season variant: data files plus their assets, no DLL, no launcher plugin.
- **Nothing gets copied.** A pack that changes one car layers that car's record, instead of shipping its own copy of Classic Seasons' rows.
- **Changes are live.** An addon that changes things in game, like a "2017 as a base" editor, registers and layers while the game runs.

## Without code

Data files in `registry/` apply at the same moments, by their registry's `x-applies`. The core reads them in game at the early stage, after the addon's DLL (if any) has registered.
