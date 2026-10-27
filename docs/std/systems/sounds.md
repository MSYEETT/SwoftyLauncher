---
title: Sounds
description: Engine sounds and a car's audio data in code - typed registries and the bundle event, in standard library 2.0.
---

# Sounds

An engine sound is a `sounds::EngineSound` record: a bundle, plus the stock sounds its mixing settings and tuning come from.

## The header

<Example root="swofty-sdk" code-file="include/swofty/systems/sounds.hpp">
<template #code>

```cpp
#include <swofty/registry.hpp>

namespace swofty::sounds {

struct EngineSound {                               // an audio.audio_bundle_settings row and its .bdl
    Path bundle;                                   // the bundle file, served as audio/2025/bundles/<id>.bdl
    Ref<EngineSound> settings_from;                // copy the mixing settings of this sound (usually a game/ one)
    Ref<EngineSound> tuning_from;                  // copy its tuning (engine, era, cabin, exhaust)
};

struct CarAudio {                                  // one car's audio.audio_car_data.<switch> records
    Ref<cars::Vehicle> vehicle;                    // the car
    std::string switch_name;                       // the car template's AudioF1Car switch: "f1_M_mercedes"
    Ref<EngineSound> engine_sound;                 // the bundle the switch reaches
    Ref<CarAudio> from;                            // copy every audio_car_data field of another car
};

inline Registry<EngineSound> engine_sounds{"engine_sound", { .applies = Applies::boot }};   // the bundle is served when the game opens it
inline Registry<CarAudio> car_audio{"audio.car", { .applies = Applies::boot }};

}

namespace swofty::sounds::events {

struct BundleResolve {                             // Game_Build_AudioBundleRegistry, Game_AudioF1Car_Setup: before
    static constexpr const char *point = "audio.bundle.resolve";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    int car;                                       // the live car
    Ref<cars::Vehicle> vehicle;
    std::string_view switch_name;                  // the car's audio switch
    Ref<EngineSound> engine_sound;                 // the sound it gets; before handlers may change it
};

}
```

</template>
</Example>

A vehicle points at its sound with `cars::Vehicle::engine_sound` (see [Cars](/std/systems/cars)). The core writes the car's `audio_car_data` records from it, so a sound pack never writes raw audio keys.

## The V10 pack, in code

<Split>

An engine sound pack brings a V10 under its own key and points cars at it. Today the imported packs overwrite stock bundles by name, such as `audi_2026.bdl`, and two options of the same pack conflict on 36 audio files.

- **The V10 is a record** under the pack's key. Nothing stock is overwritten.
- **It layers the base cars** (`game/`), so every year whose car is one of them, or a variant that doesn't set its own sound, hears it. Classic Seasons' 2016 to 2018 cars set their own period sounds and keep them.
- **An option picks the cars:** every 2026 car, or the Ferrari only. The edit re-runs when the option changes, from the next game start.
- **Other packs compose.** A V12 pack on the McLaren and this pack on the Ferrari both apply; on the same car, load order decides.

<template #example>
<Example root="mod.esp" code-file="native/src/esp.cpp" data-file="registry/sounds.json" files="swofty.json, sounds/v10_screamer.bdl, registry/layers.json">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/sounds.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    // provide under your own key: the id is "mod.esp/v10_screamer"
    auto v10 = sounds::engine_sounds.add("v10_screamer", {
        .bundle = file("sounds/v10_screamer.bdl"),
        .settings_from = sounds::engine_sounds["game/ferrari_2025"],
        .tuning_from = sounds::engine_sounds["game/ferrari_2025"],
    });

    // the base 2026 cars, or only the Ferrari; years with their own sound keep it
    cars::vehicles.layer(under<cars::Vehicle>("game/"), [=](cars::Vehicle &v) {
        if (v.performance_class != cars::Class::f1_26)
            return;
        if (option<std::string>("cars") == "all" || v.label == "ferrari_2026")
            v.engine_sound = v10;
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/sounds.json
{
  "engine_sound": {
    "v10_screamer": { "bundle": "sounds/v10_screamer.bdl", "settings_from": "game/ferrari_2025", "tuning_from": "game/ferrari_2025" }
  }
}
```

```json
// registry/layers.json
{
  "vehicle": {
    "$select": [
      { "ids": "game/*", "where": { "performance_class": "f1_26" }, "when": { "option": "cars", "is": "all" },
        "patch": { "engine_sound": "mod.esp/v10_screamer" } },
      { "ids": "game/ferrari_2026", "when": { "option": "cars", "is": "ferrari" },
        "patch": { "engine_sound": "mod.esp/v10_screamer" } }
    ]
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: mod.esp (esp.dll) 1 engine_sound, 1 layer on vehicle (early stage, 0.4 ms)
[registry] vehicle: selector ids game/* from mod.esp: 22 records changed (performance_class f1_26)
[registry] vehicle game/ferrari_2026: edit by mod.esp changed /engine_sound -> mod.esp/v10_screamer (option cars=all)
[registry] vehicle swofty.custom-grid-size/ferrari_t11: extends game/ferrari_2026, no own /engine_sound: mod.esp/v10_screamer
[registry] applied at boot: engine_sound 1 (1 bundle served in memory), vehicle 23 (92 database records)
```

[Content](/std/content) puts the V10 on Classic Seasons' 2017 cars, in place and as a separate GP/TT box.

## A car's audio data

<Split>

Classic Seasons writes 430 `put-record`s of raw hex per season to give its cars their sounds. In 2.0 that's one `car_audio` record per car, copied from a stock car. The car reaches its bundle through its switch name, not through the car model's `bundleName`.

<template #example>
<Example root="swofty.classic-seasons-2017" code-file="native/src/sounds.cpp" data-file="registry/sounds.json" files="swofty.json, sounds/cs17_mercedes.bdl, sounds/cs17_ferrari.bdl, sounds/cs17_honda.bdl, sounds/cs17_renault.bdl">
<template #code>

```cpp
SWOFTY_REGISTER
{
    auto mercedes = sounds::engine_sounds.add("mercedes", {
        .bundle = file("sounds/cs17_mercedes.bdl"),
        .settings_from = sounds::engine_sounds["game/mercedes_2025"],
        .tuning_from = sounds::engine_sounds["game/mercedes_2025"],
    });

    sounds::car_audio.add("mercedes", {
        .vehicle = cars::vehicles["swofty.classic-seasons-2017/mercedes"],
        .switch_name = "f1_M_mercedes",
        .engine_sound = mercedes,
        .from = sounds::car_audio["game/mercedes_2025"],
    });
}
```

</template>
<template #data>

```json
{
  "engine_sound": {
    "mercedes": { "bundle": "sounds/cs17_mercedes.bdl", "settings_from": "game/mercedes_2025", "tuning_from": "game/mercedes_2025" }
  },
  "audio.car": {
    "mercedes": { "vehicle": "swofty.classic-seasons-2017/mercedes", "switch_name": "f1_M_mercedes",
                  "engine_sound": "swofty.classic-seasons-2017/mercedes", "from": "game/mercedes_2025" }
  }
}
```

</template>
</Example>
</template>
</Split>

## Swapping a sound at run time

```cpp
// audio.bundle.resolve: a different bundle for one car, without changing its record
events::on<sounds::events::BundleResolve>().before([](sounds::events::BundleResolve &b, const events::Context &) {
    if (b.vehicle.id() == "game/williams_2026")
        b.engine_sound = sounds::engine_sounds["mod.esp/v10_screamer"];
});
```

## Readiness and limits

- **Records: ready.** Classic Seasons ships period sounds this way.
- **Events: unknown (L1).** `Game_Build_AudioBundleRegistry` (keys rows by `bundle_name`, maker and year) and `Game_AudioF1Car_Setup` are L1.
- **The reader expects 16-layer engine parts.** 2016 to 2018 bundles have fewer; the core widens them when it serves them.
- **A car reaches its bundle through its switch name.** The car model's `bundleName` doesn't pick the sound.
- **Commentary** (`audio.commentary`: track commentator overrides, speech lines) comes later.
