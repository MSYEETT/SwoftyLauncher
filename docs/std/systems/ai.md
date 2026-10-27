---
title: AI
description: AI vehicle configs, package difficulty maps, baked track packs, global AI settings and difficulty in code, with the AI config events.
---

# AI

The AI system is how each AI car races (its race-craft block), which baked track data it drives on, and the global AI settings.

## The header

<Split>

`swofty/systems/ai.hpp` has five registries:

- `ai::vehicle_configs`: the race-craft block of one car (`ai.vehicle_config`);
- `ai::package_maps`: difficulty maps per car package (`ai.package_maps`);
- `ai::track_packs`: which baked AI folder a car uses on each track (`ai.track_pack`);
- `ai::general`: the global AI settings, behaviours and driver variation (`ai.general`);
- `ai::difficulty`: difficulty scaling per career or session (`ai.difficulty_profile`).

<template #example>
<Example root="swofty-sdk" code-file="include/swofty/systems/ai.hpp">
<template #code>

```cpp
#pragma once
#include <swofty/registry.hpp>
#include <swofty/events.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/tracks.hpp>

namespace swofty::ai {

enum class Class { f1, f1_26, f2, classic, safety_car, other };

struct VehicleConfig {                         // entrants.vehicles.vehicle_N.ai_vehicle_config: 221 values in 30 groups
    Ref<cars::Vehicle> vehicle;                // the car it belongs to
    Class cls;                                 // "class" in data files; only ers_use, traction control, trailbraking and race-start rpm differ by class
    struct { float overtake_aggression, defend_aggression; } racecraft;
    struct { Curve vehicle_buffer; } buffer_space;                       // space kept from other cars, by speed
    struct { float proportional_gain, integral_gain, derivative_gain, output_smoothing_half_life; } steering_pid;
    struct { float time_to_half_rotate_steering_wheel; } steering_controller;   // reaction after a hit
    struct { float lateral_traction_slip_angle_cut_in_proportion; } pedal_controller;   // lifts off when sliding
    struct { float angle; } lost_control;                               // where it gives up the car
    struct { Curve grip_atc_strength; } traction_control;
    struct { float aero_speed_reduction_factor, burst_speed_reduction_factor; } damage;   // driving a damaged car
    // ... allow_to_pass, blocking, drafting, defending, overtake, ers_use, swerve, trailbraking, yielding, race starts
};
inline Registry<VehicleConfig> vehicle_configs{"ai.vehicle_config", { .applies = Applies::session }};   // (read at session enter)

struct PackageMaps {
    Ref<cars::Package> package;                // vehicle_package.ai_vehicle_package_config
    std::map<std::string, Curve> driver_parameter_maps;   // difficulty x characteristic
};
inline Registry<PackageMaps> package_maps{"ai.package_maps", { .applies = Applies::boot }};

enum class Slot { by_pace, fixed };

struct TrackPack {                             // a car points at one through cars::Vehicle::ai (AiSlot::track_pack)
    std::string folder;                        // ai_package_folder: environment_package/tracks/<t>/ai/<folder>/
    Slot slot = Slot::by_pace;                 // by_pace: the core picks the baked car closest in lap time
    Ref<cars::Vehicle> pace_of;                // by_pace: the car whose lap time it matches; empty: the car itself
};
inline Registry<TrackPack> track_packs{"ai.track_pack", { .applies = Applies::session }};

struct General {
    std::map<std::string, float> settings;     // ai.general_ai_settings (5)
    std::vector<std::string> behaviours;       // ai.behaviour_system.behaviours (5)
    std::map<std::string, Curve> variation;    // ai.driver_variation.in_race: 84 form curves
};
inline Registry<General> general{"ai.general", { .applies = Applies::boot }};

struct Difficulty {
    events::Selector within;                   // a career, a content, a session type
    float pace = 1.0f;                         // scale on the difficulty the player picked
};
inline Registry<Difficulty> difficulty{"ai.difficulty_profile", { .applies = Applies::read }};   // needs the slider found (unknown)

namespace events {

struct ConfigLoad {                            // Sess_Load_VehicleAiConfig
    static constexpr const char *point = "ai.config.load";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    StatId vehicle;                            // the car's stat id
    void *config;                              // the live AiHandlingConfig; after handlers may patch its nodes
};

struct ResolveSettings {                       // Ai_Resolve_GeneralSettings
    static constexpr const char *point = "ai.settings.resolve";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    General *settings;                         // before handlers may change it
};

} // namespace events

Result set_vehicle_config(Ref<cars::Vehicle> vehicle, Edit<VehicleConfig> edit);   // before session load
Result assign_track_pack(Ref<cars::Vehicle> vehicle, Ref<TrackPack> pack);   // before session load

} // namespace swofty::ai
```

</template>
</Example>
</template>
</Split>

## AI Racing Tweaks

<Split>

AI Racing Tweaks makes the AI race closer and lose control more like a person. In 1.x it names 599 stock cars one at a time (13,371 database operations), so cars other addons add never get its values.

In 2.0 it writes one layer with a selector over every F1 car: stock, Classic Seasons, F1 1990, and cars added after it was written. A selector layers each matching record itself, so the tuning reaches every year, past the years' own values (see [The base, years and careers](/std/inheritance)). Each value comes from one of its options, so changing an option re-runs the edit.

<template #example>
<Example root="kizi.ai-tweaks" code-file="native/src/racecraft.cpp" data-file="registry/layers.json" files="swofty.json, native/CMakeLists.txt, native/src/contact.cpp">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/events.hpp>
#include <swofty/systems/ai.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto f1 = where<ai::VehicleConfig>([](const ai::VehicleConfig &c) {
        return c.cls == ai::Class::f1 || c.cls == ai::Class::f1_26;
    });

    ai::vehicle_configs.layer(f1, [](ai::VehicleConfig &c) {
        c.buffer_space.vehicle_buffer = Curve::flat(50, 500, option<float>("spacing"));    // 0.35 .. 0, default 0.15
        c.pedal_controller.lateral_traction_slip_angle_cut_in_proportion = option<float>("throttle_cut");
        c.lost_control.angle = option<float>("give_up");

        if (auto steer = option<std::string>("steer_correction"); steer != "stock") {
            float k = std::stof(steer) / 100.0f;                                        // 75, 50, 25 % of the game's gains
            c.steering_pid = { -1.5f * k, -0.75f * k, -0.375f * k, 0.4f };
        }
        if (option<std::string>("traction_control") == "off")
            c.traction_control.grip_atc_strength = Curve::flat(0, 1, 0.0f);
    });
}

SWOFTY_ON_LOAD
{
    events::on<ai::events::ConfigLoad>().after([](const ai::events::ConfigLoad &l, const events::Context &) {
        kizi::contact::arm(l.vehicle);                     // the contact handling, in contact.cpp
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
{
  "ai.vehicle_config": {
    "$select": [
      {
        "where": { "class": ["f1", "f1_26"] },
        "patch": {
          "buffer_space": { "vehicle_buffer": "@v2@minx:50,maxx:500,miny:0,maxy:1,0:0.15,1:0.15" },
          "pedal_controller": { "lateral_traction_slip_angle_cut_in_proportion": 0.75 },
          "lost_control": { "angle": 0.3 }
        }
      }
    ]
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: kizi.ai-tweaks (kizi_ai_tweaks.dll) 1 selector layer on ai.vehicle_config (early stage, 0.2 ms)
[registry] ai.vehicle_config: selector of kizi.ai-tweaks matched 612 records (599 game/, 80 swofty.classic-seasons-*,
           13 swofty.f1-1990; reads options spacing, throttle_cut, give_up, steer_correction, traction_control)
[registry] option kizi.ai-tweaks/spacing changed: 612 ai.vehicle_config records re-resolved; applies from the next session load
```

The data version holds the default values only. Following the options takes code.

## Track packs

The AI drives from baked tables per track, car package and car (acpd, brpd, ers_boost_data, vp_times, ...; about 7 MB per team per track). EA's bake tools are compiled out of the game, so a new car borrows the baked set of a stock car. `Slot::by_pace` picks the one closest in lap time, and the car points at the pack:

```cpp
auto pack = ai::track_packs.add("ferrari", { .folder = "f1_2026", .slot = ai::Slot::by_pace });

cars::vehicles.layer(cars::vehicles["swofty.classic-seasons-2017/ferrari"], [=](cars::Vehicle &v) {
    v.ai = { .track_pack = pack, .asset_filename = "auto" };
});
```

```text
[registry] ai.track_pack swofty.classic-seasons-2017/ferrari: slot by pace -> f1_2026/ferrari (0.12 % off)
```

## General settings and difficulty

```cpp
ai::general.layer(ai::general["game/default"], [](ai::General &g) { g.settings["blue_flag_compliance"] = 0.8f; });

ai::difficulty.add("progressive", {
    .within = events::content("swofty.progressive/f1_career_progressive"),
    .pace = 1.02f,
});
```

## Events and operations

```cpp
// patch a live config at session enter, for one car
events::on<ai::events::ConfigLoad>().in(events::content("swofty.f1-1990/f1_career_1990"))
    .after([](const ai::events::ConfigLoad &l, const events::Context &) { f1990::soften_starts(l.config); });

ai::set_vehicle_config(cars::vehicles["game/williams_2025"], [](ai::VehicleConfig &c) { c.lost_control.angle = 0.4f; });
ai::assign_track_pack(cars::vehicles["swofty.f1-1990/mclaren_1990"], ai::track_packs["swofty.f1-1990/f1_1990"]);
```

## Readiness and limits

- **Data: ready.** The configs, maps and track packs are database rows and files.
- **Events: partial.** The loaders (`Sess_Load_VehicleAiConfig`, `Ai_Resolve_GeneralSettings`, `Sess_Build_AiSystem`) are L2. The decision code is L1 or unexplored.
- **Unknown:** where the 0 to 110 difficulty slider scales the controllers (so `ai::difficulty` has nothing to apply yet), whether a config changed after load takes effect, the grip index source, the AI path builder.
- **Read once:** the AI config is read at session enter. A layer that changes applies from the next session load.
- **Baked tables cap pace.** A car faster than its baked set gets nothing from more grip; physics changes must keep the car inside the baked envelope.
- **A missing AI folder hangs the load.** The core refuses a track pack whose folder isn't there.
- **The AI table holds 30 cars.**
- **Simulated sessions** use the baked lap times, driver statistics and randomness, not physics or R&D.
- **Third-party natives** that patch raw addresses break on game updates and on the EA build.
