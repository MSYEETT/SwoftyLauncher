---
title: Drivers
description: Drivers in code - driver identity, heads, looks and stock ratings as typed registries, with the ratings, number and head events and the driver operations, in standard library 2.0.
---

# Drivers

A driver is a `drivers::Identity` record with a head, a look and a ratings holder, each a record of its own.

## The header

<Example root="swofty-sdk" code-file="include/swofty/systems/drivers.hpp">
<template #code>

```cpp
#include <swofty/registry.hpp>

namespace swofty::drivers {

struct Names {
    Text first;                                    // first_name
    Text last;                                     // last_name
    Text full;                                     // full_name
    std::string code;                              // initials: "VER"
};

struct Identity {                                  // one entrants.drivers row
    Ref<Identity> donor;                           // the row it copies, all 129 keys (game/jarno_opmeer)
    Names names;                                   // the driver's names
    std::string nationality;                       // one of entrants.nationality (90)
    int number = 0;                                // car_number
    Ref<Head> head;                                // the head
    Ref<Look> look;                                // helmet, suit, gloves, pose; a driver needs one
    std::string audio_name;                        // the commentary name (entrants.audio_names)
    int seasons_completed = 0;                     // f1_seasons_completed
    int garage_slot = 0;                           // garage_slot_preference
    std::vector<std::string> pools;                // driver pools with a free-agent row: "f1_2026_driver_pool"
    StatId stat_id = StatId::automatic;            // driver ids 0-254, 58 free
};

enum class BodyType { male, female };

struct Head {                                      // an entrants.animation.driver_heads row and its files
    Path archive;                                  // character_package/heads/<...>/<head>/
    Path morph;                                    // morph_poses/<head>.erp
    Ref<Head> rig_from;                            // the head row it copies (game/male_ls222, game/female_ls112)
    BodyType body = BodyType::male;                // body_type
    bool customisable = false;                     // available_for_customisation
    StatId stat_id = StatId::automatic;            // heads from 265
};

struct Look {                                      // a driver_customisations row
    Ref<wearables::Helmet> helmet;                 // the helmet
    Ref<wearables::Suit> suit;                     // the suit
    Ref<wearables::Gloves> gloves;                 // the gloves
    std::string pose;                              // one of the 59 poses
    std::string podium_emote;                      // one of the 52 emotes
    StatId stat_id = StatId::automatic;            // look ids
};

struct Curve { float start; float peak; };         // a rating from the first season to the driver's peak

struct Stats {                                     // a DriverStatisticsHolder: the stock ratings
    Ref<Identity> driver;
    Curve experience, racecraft, awareness, pace, focus;
};

inline Registry<Identity> identities{"driver.identity", { .applies = Applies::boot }};
inline Registry<Head> heads{"driver.head", { .applies = Applies::boot }};
inline Registry<Look> looks{"driver.look", { .applies = Applies::boot }};   // career-scope layers: content
inline Registry<Stats> stats{"driver.stats", { .applies = Applies::boot }};

Result set_look(Ref<Identity> driver, Ref<Look> look);     // refuses an empty look

}

namespace swofty::drivers::events {

struct RatingsRead {                               // Career_Get_DriverStatValue: before
    static constexpr const char *point = "driver.ratings.read";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    Ref<Identity> driver;
    Stat stat;                                     // experience, racecraft, awareness, pace, focus
    float value;                                   // before handlers may change it
};

struct NumberAssign {                              // Career_DriverNumbers_AssignFree: before, replace
    static constexpr const char *point = "driver.number.assign";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    Ref<Identity> driver;
    int number;                                    // the number the game picked
};

struct HeadResolve {                               // Ent_EntrantsSystem_FindDriverHead: before
    static constexpr const char *point = "driver.head.resolve";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    Ref<Identity> driver;
    Ref<Head> head;                                // the head the game found by label
};

}
```

</template>
</Example>

The career operations on drivers, `career::set_car_number` and `career::set_driver_ratings`, are on [Career](/std/systems/career).

## Driver Manager, in code

<Split>

Driver Manager creates new drivers from a launcher page. The page writes them to the addon's storage, and the addon registers each one in the game.

- **A variant of Jarno Opmeer.** The new driver starts from every key of `game/jarno_opmeer` and changes its own fields. Layers on Jarno's row flow in.
- **The core allocates the ids** the addon tracks by hand today: driver ids in 208 to 253, look ids from 366.
- **The look is required.** A seated driver with no look hangs the next session load, so the core refuses an identity without one.
- About 150 records per driver: the row, the look, the statistics holder, the entrant and the pool rows.

<template #example>
<Example root="swofty.driver-manager" code-file="native/src/drivers.cpp" data-file="registry/drivers.json" files="swofty.json, launcher/src/main.rs, bin/driver-manager-helper.exe">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/storage.hpp>
#include <swofty/systems/drivers.hpp>
#include <swofty/systems/wearables.hpp>

#include "drivers_json.hpp"   // parses the page's drivers.json

using namespace swofty;

SWOFTY_REGISTER
{
    std::string json;
    if (storage::read("drivers.json", json) != storage::Result::ok)
        return;

    for (const dm::Driver &d : dm::parse(json)) {
        auto look = drivers::looks.add(d.key + "_look", {
            .helmet = wearables::helmets[d.helmet],
            .suit = wearables::suits[d.suit],
            .gloves = wearables::gloves[d.gloves],
            .pose = d.pose,
            .podium_emote = d.emote,
        });

        auto driver = drivers::identities.variant(d.key, drivers::identities["game/jarno_opmeer"],
            [=](drivers::Identity &i) {
                i.names = { Text::literal(d.first), Text::literal(d.last), Text::literal(d.full), d.code };
                i.nationality = d.nationality;
                i.number = d.number;
                i.head = drivers::heads[d.head];
                i.look = look;
                i.audio_name = d.commentary_name;
                i.seasons_completed = d.seasons;
                i.garage_slot = d.garage_slot;
                if (d.active)
                    i.pools = { "f1_2026_driver_pool", "f1_2025_driver_pool" };
            });

        drivers::stats.add(d.key, {
            .driver = driver,
            .experience = { d.experience.start, d.experience.peak },
            .racecraft  = { d.racecraft.start,  d.racecraft.peak },
            .awareness  = { d.awareness.start,  d.awareness.peak },
            .pace       = { d.pace.start,       d.pace.peak },
            .focus      = { d.focus.start,      d.focus.peak },
        });
    }
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/drivers.json: one driver, written by hand
{
  "driver.look": {
    "mika_salo_look": { "helmet": "game/helmet_tier01_livery03", "suit": "game/suit_generic_01",
                        "gloves": "game/gloves_generic_01", "pose": "arms_crossed", "podium_emote": "fist_pump" }
  },
  "driver.identity": {
    "mika_salo": {
      "extends": "game/jarno_opmeer",
      "names": { "first": { "text": "Mika" }, "last": { "text": "Salo" }, "full": { "text": "Mika Salo" }, "code": "SAL" },
      "nationality": "finland", "number": 17, "head": "game/male_ls222", "look": "swofty.driver-manager/mika_salo_look",
      "audio_name": "salo", "pools": ["f1_2026_driver_pool", "f1_2025_driver_pool"]
    }
  },
  "driver.stats": {
    "mika_salo": { "driver": "swofty.driver-manager/mika_salo",
                   "pace": { "start": 78, "peak": 84 }, "racecraft": { "start": 75, "peak": 82 } }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: swofty.driver-manager (driver_manager.dll) 3 driver.identity, 3 driver.look, 3 driver.stats (early stage, 0.6 ms)
[registry] stat ids: driver swofty.driver-manager/mika_salo -> 208 (pool 208-254, 45 free after this)
[registry] stat ids: driver.look swofty.driver-manager/mika_salo_look -> 366
[registry] driver.identity swofty.driver-manager/mika_salo: extends game/jarno_opmeer (129 keys)
[registry] applied at boot: driver.identity 3, driver.look 3, driver.stats 3 (452 database records; 11 strings)
```

Driver Manager no longer needs `load_after: ["*"]`. It references records, not row positions another mod may have moved.

## Heads

A head is its archive, its morph poses and a rig row copied from a stock head. An imported face mod provides head records, and Driver Manager lists them with `drivers::heads.all()`:

```cpp
auto senna = drivers::heads.add("senna", {
    .archive = file("heads/senna/"),
    .morph = file("heads/morph_poses/senna.erp"),
    .rig_from = drivers::heads["game/male_ls222"],
});
drivers::identities.layer(drivers::identities["swofty.f1-1990/ayrton_senna"], [=](drivers::Identity &i) { i.head = senna; });
```

```text
[registry] driver.head mod.senna-face/senna: rig from game/male_ls222, 2 asset groups registered, stat id 286
```

## Stock drivers

<Split>

A change to a stock driver is an edit on `game/<driver>`. Driver Manager sets the head and helmet of drivers the game already has; Helmet Exchange and Classic Seasons change the same looks. Each edit names the fields it changes, load order decides each field, and the conflicts report shows the contest.

<template #example>
<Example root="swofty.driver-manager" code-file="native/src/stock.cpp" data-file="registry/layers.json" files="swofty.json">
<template #code>

```cpp
SWOFTY_REGISTER
{
    drivers::identities.layer(drivers::identities["game/daniil_kvyat"], [](drivers::Identity &i) {
        i.head = drivers::heads["mod.kvyat-face/kvyat"].or_else(i.head);
    });
    drivers::looks.layer(drivers::looks["game/daniil_kvyat"], [](drivers::Look &l) {
        l.helmet = wearables::helmets["giggles.helmet-exchange/kvyat_2017"].optional();
    });
}
```

</template>
<template #data>

```json
{
  "driver.identity": { "game/daniil_kvyat": { "head": "mod.kvyat-face/kvyat|game/kvyat" } },
  "driver.look":     { "game/daniil_kvyat": { "helmet": "giggles.helmet-exchange/kvyat_2017?" } }
}
```

</template>
</Example>
</template>
</Split>

## Ratings, numbers and heads at run time

```cpp
// driver.ratings.read: the game reads statistics once at boot; this changes them at every read
events::on<drivers::events::RatingsRead>().in(events::owner("swofty.progressive"))
    .before([](drivers::events::RatingsRead &r, const events::Context &ctx) {
        r.value = progressive::season_rating(r.driver, r.stat, ctx.career()->year, r.value);
    });

// driver.number.assign: keep a driver's own number
events::on<drivers::events::NumberAssign>().replace([](drivers::events::NumberAssign &n, const events::Context &) {
    if (n.driver.id() != "swofty.f1-1990/ayrton_senna") return false;
    n.number = 27;
    return true;
});

drivers::set_look(drivers::identities["game/lewis_hamilton"], drivers::looks["swofty.classic-seasons-2017/hamilton"]);
```

## Readiness and limits

- **Partial to ready.** The ratings getters (`Career_Get_DriverOverallRating`, `Career_Get_DriverStatValue`), the number check (`Career_DriverNumbers_CheckExisting`) and the entrant parse (`Sess_Parse_Entrant`) are L2. Heads and looks (`Ent_EntrantsSystem_FindDriverHead`, `Career_DriverNumbers_AssignFree`) are L1, but proven by data.
- **58 free driver ids** in 0 to 254. The core allocates them; two addons picking ids by hand collide.
- **About 150 records per new driver:** the row, the statistics holder, the accolade row, the entrant row, wear rates, seasonal infos, audio names and the head.
- **A driver with no look hangs the session.** The core refuses one.
- **Statistics are read once at boot.** `driver.stats` applies at boot; per-season ratings change them through `driver.ratings.read`.
- **Heads are found by label only.** A generic head looks wrong with a face map; use a scanned donor head. Only 9 real retro heads ship in `character_package/heads/male_driver/`.
- **The entrant parse reads the team label unchecked,** and a missing team crashes; the core checks the reference first.
