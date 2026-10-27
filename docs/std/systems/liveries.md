---
title: Liveries
description: Liveries in code - liveries, special liveries, decal sets, sponsor packs and AI livery choices as typed registries, with the livery events and operations, in standard library 2.0.
---

# Liveries

A livery is a `liveries::Livery` record for one car: the textures or archive it wears, and, for a special livery, its entry in the livery picker.

## The header

<Example root="swofty-sdk" code-file="include/swofty/systems/liveries.hpp">
<template #code>

```cpp
#include <swofty/registry.hpp>

namespace swofty::liveries {

enum class Unlock { owned, game };                 // owned: the core reports it owned; game: the game's unlock row decides

struct Livery {
    Ref<cars::Vehicle> vehicle;                    // the car it fits (compatible_vehicle)
    Path textures;                                 // a folder of textures over the car's own
    Path archive;                                  // or a whole wep archive for the car
    bool special = false;                          // a picker entry: an se_livery row and an unlock row
    std::string suffix = "auto";                   // wep_override: "_barcelona_26"; auto takes the next free "_lxNN"
    std::string picker_name;                       // the lang key the picker shows
    Ref<tracks::Venue> event;                      // the grand prix it's made for, if any
    Unlock unlock = Unlock::owned;                 // who decides whether the player has it
    StatId stat_id = StatId::automatic;            // the se_livery and unlock rows' ids
};

struct ProjectionArea { Side side; Rect area; };   // decal_customisation_system.projection_areas, per car side

struct DecalSet {                                  // a My Team decal set
    Path textures;                                 // its decal images
    std::vector<ProjectionArea> areas;             // where they project
    StatId stat_id = StatId::automatic;
};

struct SponsorPack {
    std::map<std::string, Path> slices;            // texture-array slice -> image: "sponsors_2026[petronas]"
};

struct AiAssignment {
    Ref<cars::Vehicle> vehicle;                    // the AI car
    int season;                                    // the season it applies to
    Ref<Livery> livery;                            // the special livery it races; locked ones race the default
};

inline Registry<Livery> liveries{"livery", { .applies = Applies::boot }};
inline Registry<DecalSet> decal_sets{"livery.decal_set", { .applies = Applies::boot }};
inline Registry<SponsorPack> sponsor_packs{"livery.sponsor_pack", { .applies = Applies::boot }};   // slices served when the array opens
inline Registry<AiAssignment> ai_assignments{"livery.ai_assignment", { .applies = Applies::session }};

Result grant(Ref<Livery> livery);                  // report it owned to the game
Result assign(int car, Ref<Livery> livery);        // a live car's livery (later)

}

namespace swofty::liveries::events {

struct SpecialList {                               // Game_Build_SpecialLiveryList: after
    static constexpr const char *point = "livery.special.list";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    Ref<cars::Vehicle> vehicle;                    // the car whose picker is built
    std::vector<Ref<Livery>> &list;                // append to it
};

struct FrontendSet {                               // Game_FrontendSceneComponentCar_SetCarLivery: before
    static constexpr const char *point = "livery.frontend.set";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    Ref<cars::Vehicle> vehicle;
    Ref<Livery> livery;                            // the livery the front end shows; before handlers may change it
};

struct AiSelect {                                  // the AI car's livery suffix at session load: before
    static constexpr const char *point = "livery.ai_select";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    int car;
    Ref<cars::Vehicle> vehicle;
    Ref<Livery> livery;                            // empty: the team's own
};

}
```

</template>
</Example>

## A livery pack and a special livery, in code

<Split>

One addon, two kinds of livery:

- **The pack** gives three of Classic Seasons' 2017 cars a different look. Each livery is a record for its car, and an edit on the vehicle makes the car race in it, in every place the 2017 cars appear.
- **The special livery** adds a Scuderia AlphaTauri car to the 2026 Racing Bulls' livery picker, next to the team's own. The core writes the `se_livery` row, the unlock row and the archive slot, and reports it owned.
- Classic Seasons doesn't know the pack exists, and the pack copies none of its files.

<template #example>
<Example root="rmdesigns.liveries" code-file="native/src/liveries.cpp" data-file="registry/liveries.json" files="swofty.json, liveries/ferrari_2017_monza/ferrari_d.tif.mipmaps, liveries/mclaren_2017_papaya/mclaren_d.tif.mipmaps, liveries/redbull_2017_matte/redbull_d.tif.mipmaps, archives/toro_rosso_alphatauri.erp, lang/liveries.json, registry/layers.json">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/liveries.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    // the pack: one livery per car, and the car races in it
    for (const char *team : { "ferrari", "mclaren", "redbull" }) {
        auto car = cars::vehicles[std::string("swofty.classic-seasons-2017/") + team].optional();
        auto look = liveries::liveries.add(std::string(team) + "_2017", {
            .vehicle = car,
            .textures = file(std::string("liveries/") + team + "_2017/"),
        });
        cars::vehicles.layer(car, [=](cars::Vehicle &v) { v.livery = look; });
    }

    // the special livery: a picker entry for the 2026 Racing Bulls
    liveries::liveries.add("alphatauri_2026", {
        .vehicle = cars::vehicles["game/toro_rosso_2026"],
        .archive = file("archives/toro_rosso_alphatauri.erp"),
        .special = true,
        .picker_name = "lng_rm_livery_alphatauri",
        .unlock = liveries::Unlock::owned,
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/liveries.json
{
  "livery": {
    "ferrari_2017": { "vehicle": "swofty.classic-seasons-2017/ferrari?", "textures": "liveries/ferrari_2017/" },
    "mclaren_2017": { "vehicle": "swofty.classic-seasons-2017/mclaren?", "textures": "liveries/mclaren_2017/" },
    "redbull_2017": { "vehicle": "swofty.classic-seasons-2017/redbull?", "textures": "liveries/redbull_2017/" },
    "alphatauri_2026": { "vehicle": "game/toro_rosso_2026", "archive": "archives/toro_rosso_alphatauri.erp",
                         "special": true, "picker_name": "lng_rm_livery_alphatauri", "unlock": "owned" }
  }
}
```

```json
// registry/layers.json
{
  "vehicle": {
    "swofty.classic-seasons-2017/ferrari": { "livery": "rmdesigns.liveries/ferrari_2017" },
    "swofty.classic-seasons-2017/mclaren": { "livery": "rmdesigns.liveries/mclaren_2017" },
    "swofty.classic-seasons-2017/redbull": { "livery": "rmdesigns.liveries/redbull_2017" }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: rmdesigns.liveries (liveries.dll) 4 livery, 3 layers on vehicle (early stage, 0.3 ms)
[registry] vehicle swofty.classic-seasons-2017/ferrari: edit by rmdesigns.liveries changed /livery -> rmdesigns.liveries/ferrari_2017
[registry] livery rmdesigns.liveries/alphatauri_2026: special, wep_override _lx01, stat id 22 (se_livery), owned
[registry] applied at boot: livery 4 (2 rows, 1 unlock row, 1 archive and 3 texture folders served in memory), vehicle 3
```

Without Classic Seasons 2017, the three pack liveries are dropped (`optional()`), and the special livery still loads.

## A livery is in every year

<Split>

A livery dropped in through the importer becomes a livery record under the mod's own key, plus a layer on the base team car. Every box whose car for that team is the base car, or a variant of it that doesn't set its own livery, shows it.

- **A year addon's own livery wins in its own box.** Classic Seasons' 2017 Red Bull is a variant of `game/redbull_2025` that sets its own 2017 livery. A variant's own value beats layers on its base, so the drop-in doesn't leak into 2017.
- **To put it on the 2017 car too,** the player retargets it on the importer page ("Applies to": F1 2017 Red Bull), or another addon layers `swofty.classic-seasons-2017/redbull` directly. Then it wins there.
- Tracks follow the same rule: the base is every year, and a year addon overrides per box by setting its own value.

<template #example>
<Example root="mod.macarena-red-bull" code-file="native/src/livery.cpp" data-file="registry/livery.json" files="swofty.json, ops/erp.json, livery/redbull_d.tif.mipmaps, livery/redbull_m.tif.mipmaps">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/liveries.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto base = cars::vehicles["game/redbull_2025"];
    auto look = liveries::liveries.add("macarena", { .vehicle = base, .textures = file("livery/") });

    // every year whose Red Bull is this car, or a variant that keeps its livery
    cars::vehicles.layer(base, [=](cars::Vehicle &v) { v.livery = look; });

    // retargeted on the importer page: the 2017 car too
    if (option<bool>("apply_2017"))
        cars::vehicles.layer(cars::vehicles["swofty.classic-seasons-2017/redbull"].optional(),
                             [=](cars::Vehicle &v) { v.livery = look; });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/livery.json: what the importer writes
{
  "livery": { "macarena": { "vehicle": "game/redbull_2025", "textures": "livery/" } }
}
```

```json
// registry/layers.json
{
  "vehicle": {
    "game/redbull_2025": { "livery": "mod.macarena-red-bull/macarena" }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] vehicle game/redbull_2025: edit by mod.macarena-red-bull changed /livery -> mod.macarena-red-bull/macarena
[registry] vehicle swofty.classic-seasons-2017/redbull: /livery set by the variant; layer on game/redbull_2025 from
           mod.macarena-red-bull not inherited for this field
[registry] vehicle swofty.custom-grid-size/redbull_t11: extends game/redbull_2025, no own /livery: mod.macarena-red-bull/macarena
```

The livery's handling files stay out of it. The importer routes a `.vtf` inside a livery's `wep.erp` to `vehicle.handling`, so installing a livery no longer undoes R&D Tyres' handling.

## AI liveries

<Split>

More Special Liveries lets the player pick which special livery each AI team races. In 1.x it ships `ai_livery.dll`, which patches where the game picks a car's livery suffix. In 2.0 each pick is an `ai_assignments` record bound to the addon's option, and the core applies it at session load through `livery.ai_select`.

<template #example>
<Example root="mildtdesign.more-special-liveries" code-file="native/src/ai.cpp" files="swofty.json">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/liveries.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    liveries::ai_assignments.add("mclaren_2026", {
        .vehicle = cars::vehicles["game/mclaren_2026"],
        .season = 2026,
    });

    liveries::ai_assignments.layer(liveries::ai_assignments["mildtdesign.more-special-liveries/mclaren_2026"],
        [](liveries::AiAssignment &a) {
            auto pick = option<std::string>("ai_2026_mclaren");       // default, monaco, silverstone
            if (pick != "default")
                a.livery = liveries::liveries["game/mclaren_" + pick + "_25"];
        });
}

SWOFTY_ADDON()
```

</template>
</Example>
</template>
</Split>

```text
[registry] livery.ai_assignment mildtdesign.more-special-liveries/mclaren_2026: livery game/mclaren_silverstone_25
[events] livery.ai_select (car 7, game/mclaren_2026): game/mclaren_silverstone_25 (owned); car 8: the same
[registry] livery.ai_assignment mildtdesign.more-special-liveries/mclaren_2026: game/mclaren_monaco_25 is locked; races the default
```

## The picker and ownership

```cpp
// livery.special.list: add a livery to a car's picker at run time, without a database row
events::on<liveries::events::SpecialList>().after([](liveries::events::SpecialList &l, const events::Context &) {
    if (l.vehicle.id() == "game/toro_rosso_2026")
        l.list.push_back(liveries::liveries["rmdesigns.liveries/alphatauri_2026"]);
});

liveries::grant(liveries::liveries["game/mclaren_silverstone_25"]);   // report a stock special livery owned
```

My Team decal sets and sponsor slices:

```cpp
liveries::sponsor_packs.add("real_sponsors", {
    .slices = { { "sponsors_2026[petronas]", file("sponsors/petronas.png") } },
});
```

## Readiness and limits

- **Records: ready.** The data path for special liveries is documented and shipped.
- **Events and operations: unknown (L1).** `Game_Build_SpecialLiveryList`, `Game_FrontendSceneComponentCar_SetCarLivery`, `Game_FrontendSceneManager_SetCustomCarLivery` and the custom-livery colour setters are L1. `assign` comes later.
- **Unknown:** where AI drivers' My Team livery colours come from, and how the game picks a random livery design. Runtime My Team livery compositing for AI is a known hang risk.
- **Ownership keys** are djb2(`eSELivery` + label). A special livery's label must not collide with a stock one.
- **Shader archives come in pairs** per GPU (`object_my_team.nefx2.sm51.erp` and `_amd`). Whole-archive shader replacements (Real Life Logos, BP Ultimate Livery) are outside this registry and replace each other by file.
- **A wep slot** takes a same-length token for the files named after the team or car model (`lx01rbs` for `rbswapc`), so nothing clashes with the stock car or another slot.
- **A locked AI pick** races the default livery.
