---
title: Cars
description: Cars in code - vehicles, packages, handling, damage models and detachable parts as typed registries, with the car events and operations, in standard library 2.0.
---

# Cars

A car is a `cars::Vehicle` record that points at its package, handling, engine sound, livery and power unit, and every one of those is a typed record other addons can layer.

## The header

<Example root="swofty-sdk" code-file="include/swofty/systems/cars.hpp">
<template #code>

```cpp
#include <swofty/registry.hpp>

namespace swofty::cars {

enum class Class { f1, f1_26, f2, classic, my_team, safety_car };
enum class Component;                              // a damage.damage_levels row: front_wing, gearbox, engine ... (11)
enum class Fault;                                  // a repair_times.faults entry: open_drs, closed_drs, front_wing ... (21)
enum class Attribute;                              // a live car's R&D attribute: engine_power ... (the map is an RE item)
enum class Department;                             // an R&D department: the one whose data a fit reads

struct Package {                                   // one entrants.vehicles.vehicle_packages row
    Ref<Package> donor;                            // the row it copies, all 72 keys (usually game/f1_2025)
    std::string resource_path;                     // "cs_2017_vehicle_package/teams/[FILENAME]/wep/[FILENAME]"
    std::string ai_package_folder;                 // the folder AI track data is found by
    std::string driver_animation_set;              // "cs_2017"
    bool halo = true;                              // has_cockpit_halo
    bool aero_2026 = false;                        // is_26_aero_regulations
    Ref<Package> setups_from;                      // setup presets copied from this package's row (default: donor)
    StatId stat_id = StatId::automatic;            // the core allocates it
};

struct AiSlot {
    Ref<ai::TrackPack> track_pack;                 // the baked AI track data the car drives on
    std::string asset_filename = "auto";           // the name AI tables are keyed by; auto takes a free stock one
};

struct Vehicle {                                   // one entrants.vehicles row
    Ref<Package> package;                          // its package
    std::string label;                             // the row label: "cs17_ferrari"
    Ref<Vehicle> donor;                            // the row add() copies, all 246 keys (a variant copies its base)
    Path model;                                    // the car model files (teams/<t>/idf/, wep/); empty: the donor's
    Ref<sounds::EngineSound> engine_sound;         // missing: falls back to the donor's
    Ref<liveries::Livery> livery;                  // the livery it races in
    Ref<Handling> handling;                        // its VTF
    Ref<power::Unit> power_unit;                   // its power unit
    AiSlot ai;                                     // where its AI data comes from
    Class performance_class = Class::f1;           // performance_class
    int performance_tier = 0;                      // performance_tier
    float tyre_wear_scale = 1.0f;                  // tyre_wear_scale
    StatId stat_id = StatId::automatic;            // vehicle ids run to 538
};

struct WingDamage {                                // F1WingDamage
    float break_threshold;                         // the impact that breaks the front wing
    float detach_threshold;                        // the impact that takes it off
};

struct SuspensionDamage {                          // F1SuspensionDamage
    float break_threshold;                         // the impact that bends a corner
    float wheel_detach_threshold;                  // the impact that takes a wheel off
};

struct Chassis { float mass; float com_z; };       // mass in kg (2025 cars: 798-813), centre of mass height
struct CoreAero { float downforce_scale; float drag_scale; };

struct Handling {                                  // one VTF, decoded; edits change fields, the core re-encodes it
    Ref<Handling> from;                            // start from another car's VTF, then edit
    Path vtf;                                      // or a whole VTF of your own
    Class cls = Class::f1;                         // the car's class (selectors match on it); "class" in data files
    float damage_scale = 1.0f;                     // scales every *_damage threshold at once
    Chassis chassis;                               // Chassis
    CoreAero core_aero;                            // CoreAero
    WingDamage wing_damage;                        // F1WingDamage
    SuspensionDamage suspension_damage;            // F1SuspensionDamage
    TyreDamage tyre_damage;                        // F1TyreDamage
    BodyDamage body_damage;                        // F1BodyDamage
    Tyres tyres;                                   // the tyre thermal and wear fields
    std::vector<RadUpgrade> rad_upgrades;          // the radUpgrade* blocks R&D fits
    // Engine, GearBox, FuelTank, FuelECU, BrakeECU, Steering, F1Aero, Differential, SuspensionSystem,
    // F1Tuning, F1Assists, Turbo, EnergyRecoverySystem, F1Plank: every section, typed the same way
};

struct DamageLevel {                               // one damage.damage_levels row
    float rate = 1.0f;                             // damage_rate_multiplier
    float minimal, repairable, retirement, terminal;   // the *_damage thresholds
    bool should_pit;                               // should_pit
};

struct DamageModel {
    std::map<Component, DamageLevel> levels;       // front_wing, gearbox, engine ... (11 rows)
    float puncture_limit;                          // damage.damage_limits
    std::map<Fault, float> repair_times;           // repair_times.faults: open_drs, closed_drs, front_wing ... (21)
};

struct Part {
    std::string node;                              // the model node: "front_wing_endplate_l"
    float detach_at;                               // the damage at which it comes off
    bool debris = true;                            // it stays on track as debris
};
struct Parts { Ref<Vehicle> vehicle; std::vector<Part> parts; };

inline Registry<Vehicle> vehicles{"vehicle", { .applies = Applies::boot }};
inline Registry<Package> packages{"vehicle.package", { .applies = Applies::boot }};
inline Registry<Handling> handling{"vehicle.handling", { .applies = Applies::session }};   // the VTF is served when the car loads
inline Registry<DamageModel> damage_models{"vehicle.damage_model", { .applies = Applies::boot }};
inline Registry<Parts> parts{"vehicle.parts", { .applies = Applies::read }};

Result set_attribute(int car, Attribute attribute, float value);     // a live car's R&D attribute
Result reset_upgrades(int car);                                      // undo the last fitted upgrade

}

namespace swofty::cars::events {

struct UpgradesFit {                               // Car_Fit_UpgradesToRating: before, after
    static constexpr const char *point = "car.upgrades.fit";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    int car;                                       // the live car
    Ref<Vehicle> vehicle;                          // its record
    Department department;                         // the department whose data is fitted
    float driver_stat;                             // the driver stat the fit reads
    float rating;                                  // the achieved rating; before handlers may change it
};

struct VtfRead {                                   // the Car_Read_<Section> readers: before
    static constexpr const char *point = "car.vtf.read";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    Ref<Vehicle> vehicle;
    std::string_view section;                      // "F1WingDamage"
    std::string_view field;                        // the field's name
    float value;                                   // in clear, before the game obfuscates it
};

struct PartsRegister {                             // Car_FrontWing_Init: after
    static constexpr const char *point = "car.parts.register";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    int car;
    Ref<Vehicle> vehicle;
    void *front_wing;                              // the game's front wing component
    std::function<Result(const Part &)> add;       // register one detachable part
};

}
```

</template>
</Example>

## Heavier Damage, in code

<Split>

Heavier Damage makes wings and suspension break harder on every car, and gives the 2026 cars the Realism Overhaul's performance.

- **A selector** picks the cars: every F1, 2026 and My Team handling record, from any addon, now and later. Damage is tuning, so it applies in every year's cars too, not only the base.
- **Two option-bound edits** read `wing_damage` and `suspension_damage`. When the player changes one, the change applies from the next session load.
- **No VTF files ship.** Today the addon carries 16 pre-built copies of every car, one per option pair, in 1,348 package entries.
- **R&D Tyres** changes the tyre fields and the `radUpgrade*` blocks of the same cars. Both edit them and both apply, where today they replace the same 91 files whole.

<template #example>
<Example root="william09leo.heavier-damage-realism" code-file="native/src/damage.cpp" data-file="registry/layers.json" files="swofty.json, native/CMakeLists.txt, README.txt">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>

using namespace swofty;

namespace {

cars::WingDamage wing(const std::string &level)
{
    if (level == "light") return { .break_threshold = 0.85f, .detach_threshold = 0.90f };
    if (level == "heavy") return { .break_threshold = 0.45f, .detach_threshold = 0.55f };
    return { .break_threshold = 0.65f, .detach_threshold = 0.75f };       // normal (recommended)
}

cars::SuspensionDamage suspension(const std::string &level)
{
    if (level == "light") return { .break_threshold = 0.85f, .wheel_detach_threshold = 0.95f };
    if (level == "heavy") return { .break_threshold = 0.50f, .wheel_detach_threshold = 0.60f };
    return { .break_threshold = 0.70f, .wheel_detach_threshold = 0.80f };
}

}

SWOFTY_REGISTER
{
    // every car: stock, My Team, every year pack's and any other addon's
    auto modern = where<cars::Handling>([](const cars::Handling &h) {
        return h.cls == cars::Class::f1 || h.cls == cars::Class::f1_26 || h.cls == cars::Class::my_team;
    });

    cars::handling.layer(modern, [](cars::Handling &h) {
        auto level = option<std::string>("wing_damage");          // re-runs when the player changes it
        if (level != "stock")
            h.wing_damage = wing(level);
    });

    cars::handling.layer(modern, [](cars::Handling &h) {
        auto level = option<std::string>("suspension_damage");
        if (level != "stock")
            h.suspension_damage = suspension(level);
    });

    // the 2026 Career Realism Overhaul's performance, 2026 cars only
    cars::handling.layer(where<cars::Handling>([](const cars::Handling &h) { return h.cls == cars::Class::f1_26; }),
                         [](cars::Handling &h) {
                             h.core_aero.downforce_scale *= 0.97f;
                             h.core_aero.drag_scale *= 1.02f;
                         });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/layers.json: selector layers, one per option value
{
  "vehicle.handling": {
    "$select": [
      { "where": { "class": ["f1", "f1_26", "my_team"] }, "when": { "option": "wing_damage", "is": "light" },
        "patch": { "wing_damage": { "break_threshold": 0.85, "detach_threshold": 0.90 } } },
      { "where": { "class": ["f1", "f1_26", "my_team"] }, "when": { "option": "wing_damage", "is": "normal" },
        "patch": { "wing_damage": { "break_threshold": 0.65, "detach_threshold": 0.75 } } },
      { "where": { "class": ["f1", "f1_26", "my_team"] }, "when": { "option": "wing_damage", "is": "heavy" },
        "patch": { "wing_damage": { "break_threshold": 0.45, "detach_threshold": 0.55 } } },
      { "where": { "class": ["f1", "f1_26", "my_team"] }, "when": { "option": "suspension_damage", "is": "normal" },
        "patch": { "suspension_damage": { "break_threshold": 0.70, "wheel_detach_threshold": 0.80 } } },
      { "where": { "class": "f1_26" },
        "patch": { "core_aero": { "downforce_scale": 0.97, "drag_scale": 1.02 } } }
    ]
  }
}
```

A data patch sets values; it can't scale them. The 2026 patch sets the result outright.

</template>
</Example>
</template>
</Split>

```text
[registry] register: william09leo.heavier-damage-realism (heavier_damage.dll) 3 layers on vehicle.handling (early stage, 0.2 ms)
[registry] vehicle.handling: selector class in (f1, f1_26, my_team) from william09leo.heavier-damage-realism matches 171 records
           (91 game/, 80 swofty.classic-seasons-*)
[registry] vehicle.handling game/ferrari_2026: edit by william09leo.heavier-damage-realism changed /wing_damage/break_threshold,
           /wing_damage/detach_threshold (option wing_damage=normal)
[registry] vehicle.handling game/ferrari_2026: 2 addons, no field contested (swofty.rnd-tyres-thermal: /tyres, /rad_upgrades;
           william09leo.heavier-damage-realism: /wing_damage, /suspension_damage, /core_aero)
[registry] applied at session load: vehicle.handling 22 (22 VTFs served in memory)
[registry] option william09leo.heavier-damage-realism/wing_damage: normal -> heavy; vehicle.handling applies from the next session load
```

The options stay in `swofty.json`, as in 1.x:

```json
{
  "id": "william09leo.heavier-damage-realism",
  "options": [
    { "id": "wing_damage",       "type": "choice", "values": ["stock", "light", "normal", "heavy"], "default": "normal" },
    { "id": "suspension_damage", "type": "choice", "values": ["stock", "light", "normal", "heavy"], "default": "normal" }
  ]
}
```

## A car of your own

<Split>

Classic Seasons 2017 adds the 2017 Ferrari: a package, then a vehicle on it. The vehicle is a variant of the stock 2025 Ferrari.

- The core copies every key of the base rows, and writes the setup preset row for the new package folder from `setups_from`. Without one, the AI's setups ground the car.
- A field the variant sets (here `livery`) beats layers on its base, so a drop-in livery for the 2025 Ferrari doesn't reach the 2017 car (see [Liveries](/std/systems/liveries)).
- The core allocates both stat ids.
- `engine_sound` and `livery` are [sounds](/std/systems/sounds) and [liveries](/std/systems/liveries) records.
- Both apply at boot, before the database loads.

<template #example>
<Example root="swofty.classic-seasons-2017" code-file="native/src/cars.cpp" data-file="registry/cars.json" files="swofty.json, native/src/content.cpp, sounds/cs17_ferrari.bdl">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/sounds.hpp>
#include <swofty/systems/liveries.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto package = cars::packages.add("cs_2017_ferrari", {
        .donor = cars::packages["game/f1_2025"],
        .resource_path = "cs_2017_vehicle_package/teams/[FILENAME]/wep/[FILENAME]",
        .driver_animation_set = "cs_2017",
    });

    auto handling = cars::handling.variant("ferrari", cars::handling["game/ferrari_2025"], [](cars::Handling &h) {
        h.chassis.mass = 728.0f;
        h.core_aero.downforce_scale = 0.92f;
    });

    // a variant of the stock car: every field it doesn't set, and every layer on game/ferrari_2025, flows in
    cars::vehicles.variant("ferrari", cars::vehicles["game/ferrari_2025"], [=](cars::Vehicle &v) {
        v.package = package;
        v.label = "cs17_ferrari";
        v.engine_sound = sounds::engine_sounds["swofty.classic-seasons-2017/ferrari"];
        v.livery = liveries::liveries["swofty.classic-seasons-2017/ferrari"];
        v.handling = handling;
        v.ai = { .track_pack = ai::track_packs["swofty.classic-seasons-2017/ferrari"] };
        v.tyre_wear_scale = 1.04f;
    });
}
```

</template>
<template #data>

```json
// registry/cars.json
{
  "vehicle.package": {
    "cs_2017_ferrari": { "donor": "game/f1_2025", "driver_animation_set": "cs_2017",
                         "resource_path": "cs_2017_vehicle_package/teams/[FILENAME]/wep/[FILENAME]" }
  },
  "vehicle.handling": {
    "ferrari": { "extends": "game/ferrari_2025", "chassis": { "mass": 728 },
                 "core_aero": { "downforce_scale": 0.92 } }
  },
  "vehicle": {
    "ferrari": {
      "extends": "game/ferrari_2025",
      "package": "swofty.classic-seasons-2017/cs_2017_ferrari", "label": "cs17_ferrari",
      "engine_sound": "swofty.classic-seasons-2017/ferrari", "livery": "swofty.classic-seasons-2017/ferrari",
      "handling": "swofty.classic-seasons-2017/ferrari",
      "ai": { "track_pack": "swofty.classic-seasons-2017/ferrari" }, "tyre_wear_scale": 1.04
    }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] stat ids: vehicle swofty.classic-seasons-2017/ferrari -> 515 (pool to 538)
[registry] vehicle.package swofty.classic-seasons-2017/cs_2017_ferrari: setup presets from game/f1_2025 (1 row, 421 keys)
[registry] applied at boot: vehicle.package 1, vehicle 1 (2 rows, 3 database records)
```

## Damage models

`vehicle.damage_model` holds the damage levels, the puncture limit and the repair times, per car or for every car (`game/default`):

```cpp
cars::damage_models.layer(cars::damage_models["game/default"], [](cars::DamageModel &d) {
    d.levels[cars::Component::front_wing].repairable *= 0.8f;
    d.repair_times[cars::Fault::front_wing] = 9.5f;
});
```

## Parts at run time

<Split>

Parts Detach registers detachable parts when the game builds a car's front wing. In 1.x it wraps `Car_FrontWing_Init` itself, and Race Director, which embeds the same module, wraps it a second time and registers every part twice.

In 2.0 both subscribe to `car.parts.register`, and `cars::parts` holds the parts per car, so the second addon finds them already there.

<template #example>
<Example root="vmknolle.parts-detach" code-file="native/src/parts.cpp" files="swofty.json, native/CMakeLists.txt">
<template #code>

```cpp
#include <swofty/events.hpp>
#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    // the same parts for every car
    cars::parts.provide([](std::string_view id) -> std::optional<cars::Parts> {
        float endplate = option<float>("endplate_threshold");
        return cars::Parts{ .vehicle = cars::vehicles[id], .parts = {
            { .node = "front_wing_endplate_l", .detach_at = endplate },
            { .node = "front_wing_endplate_r", .detach_at = endplate },
            { .node = "mirror_l", .detach_at = 0.6f, .debris = false },
        }};
    });
}

SWOFTY_ON_LOAD
{
    events::on<cars::events::PartsRegister>().after([](cars::events::PartsRegister &p, const events::Context &) {
        if (auto parts = cars::parts.get(cars::parts[p.vehicle.id()]))
            for (const auto &part : parts->parts)
                p.add(part);
    });
}

SWOFTY_ADDON()
```

</template>
</Example>
</template>
</Split>

```text
[events] car.parts.register (car 4, game/mclaren_2026): 1 after handler; 3 parts added (vmknolle.parts-detach)
```

## Upgrades and handling at load

```cpp
// car.upgrades.fit: cap the rating an upgrade reaches
events::on<cars::events::UpgradesFit>().before([](cars::events::UpgradesFit &f, const events::Context &) {
    f.rating = std::min(f.rating, 0.95f);
});

// car.vtf.read: change a field as the game reads it, in clear
events::on<cars::events::VtfRead>().before([](cars::events::VtfRead &r, const events::Context &) {
    if (r.section == "F1WingDamage")
        r.value *= 0.9f;
});
```

```cpp
cars::reset_upgrades(car);                                        // Car_Reset_LastFittedUpgrade
cars::set_attribute(car, cars::Attribute::engine_power, 1.02f);   // unavailable until the attribute map is traced
```

```text
[ops] cars::set_attribute: unavailable (site_Car_AttrArrayIter is L1; needs the L3 attribute map)
```

## Readiness and limits

- **Records: ready.** The VTF round trip is proven on 74 of 74 files.
- **Events and operations: partial.** `car.upgrades.fit` and `reset_upgrades` are L2 (`Car_Fit_UpgradesToRating`, `Car_Reset_LastFittedUpgrade`); the `Car_Read_<Section>` readers and `Car_FrontWing_Init` are L1.
- **Unknown:** the attribute-name to element-index map (`set_attribute` needs an L3 trace at `site_Car_AttrArrayIter`), the damage writers, and handling write-back while a car is live.
- **Setup presets are side data,** found by package folder. The core writes the row with the package; a missing row grounds the AI's setups, and a `_metadata_`-only row crashes session load.
- **Live physics values are XOR-obfuscated,** so handling edits apply to the VTF as the car loads, or through `car.vtf.read`.
- **AI data** is found by `ai_package_folder/asset_filename`. `AiSlot` maps an added car onto a stock slot.
- **A car is registered** only when it's available and an entrant list names it.
- **Dead stock rows:** `f1_classic` and the 23 `f1cg_*` packages have no archives and no AI data. Use `f1_2025` or `f1_2026` as the donor.
- **Scale:** Classic Seasons 2016 to 2024 add about 80 packages.
