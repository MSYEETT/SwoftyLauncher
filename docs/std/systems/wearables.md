---
title: Helmets, suits and gloves
description: Helmets, suits, gloves and team kits per season in code - typed registries, the catalogue event and the look operation, in standard library 2.0.
---

# Helmets, suits and gloves

A helmet, suit or pair of gloves is a record with its archive and colours, and a team's kit for a season is a `wearables::SeasonSet`.

## The header

<Example root="swofty-sdk" code-file="include/swofty/systems/wearables.hpp">
<template #code>

```cpp
#include <swofty/registry.hpp>

namespace swofty::wearables {

struct Helmet {                                    // a customisation.custom_avatar.helmets row and its archive
    Path archive;                                  // the helmet archive (or the resources of one)
    Ref<Helmet> from;                              // the shell and row it starts from
    Text name;                                     // its name in the picker
    Ref<drivers::Identity> driver;                 // who can pick it; empty: any driver
    std::array<Colour, 4> colours;                 // colour_1..4_default; every item row needs them
    bool official_team_item = false;               // is_official_team_item
    StatId stat_id = StatId::automatic;            // the item id
};

struct Suit {                                      // a suits row and its archive
    Path archive;
    Ref<Suit> from;
    Ref<teams::Identity> team;                     // the team it belongs to
    std::array<Colour, 4> colours;                 // colour_N_default
    StatId stat_id = StatId::automatic;
};

struct Gloves {                                    // a gloves row and its archive
    Path archive;
    Ref<Gloves> from;
    Ref<teams::Identity> team;
    std::array<Colour, 4> colours;
    StatId stat_id = StatId::automatic;
};

struct SeasonSet {                                 // one team_customisation_for_year row
    Ref<teams::Identity> team;                     // the team
    int season;                                    // the year
    Ref<Suit> suit;                                // its suit that season
    Ref<Gloves> gloves;                            // its gloves that season
};

inline Registry<Helmet> helmets{"helmet", { .applies = Applies::boot }};
inline Registry<Suit> suits{"suit", { .applies = Applies::boot }};
inline Registry<Gloves> gloves{"gloves", { .applies = Applies::boot }};
inline Registry<SeasonSet> season_sets{"wearable.season_set", { .applies = Applies::boot }};

}

namespace swofty::wearables::events {

struct CatalogueBuild {                            // Game_Build_CustomisationCatalogue: after
    static constexpr const char *point = "wearable.catalogue.build";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    std::vector<Ref<Helmet>> &helmets;             // append items that have no database row
    std::vector<Ref<Suit>> &suits;
    std::vector<Ref<Gloves>> &gloves;
};

}
```

</template>
</Example>

A driver wears items through a [look](/std/systems/drivers): `drivers::set_look` and `drivers::looks`.

## Helmet Exchange, in code

<Split>

Helmet Exchange turns helmet mods into helmets of their own, instead of replacing a driver's helmet. Its launcher page converts each mod on the player's PC and writes the list to the addon's storage. The addon registers each helmet in the game.

- **Who can pick it:** the driver the mod was made for (in any year or team), another driver, or any driver.
- **The career choice** is a career-scope edit on the player's look, so it's worn in every session of that career and nowhere else.
- In the game, helmet names end in (HX).

<template #example>
<Example root="giggles.helmet-exchange" code-file="native/src/helmets.cpp" data-file="registry/helmets.json" files="swofty.json, helmets/hamilton_2017_helmet.erp, launcher/main.go, THIRD_PARTY_NOTICES.md">
<template #code>

```cpp
#include <swofty/events.hpp>
#include <swofty/registry.hpp>
#include <swofty/storage.hpp>
#include <swofty/systems/drivers.hpp>
#include <swofty/systems/wearables.hpp>

#include "imports.hpp"   // parses the page's helmets.json

using namespace swofty;

SWOFTY_REGISTER
{
    std::string json;
    if (storage::read("helmets.json", json) != storage::Result::ok)
        return;

    for (const hx::Import &h : hx::parse(json)) {
        if (!h.enabled)
            continue;
        wearables::helmets.add(h.key, {
            .archive = file("helmets/" + h.key + "_helmet.erp"),
            .from = wearables::helmets[h.shell],                         // game/lewis_hamilton for career helmets
            .name = Text::literal(h.name + " (HX)"),
            .driver = h.any_driver ? Ref<drivers::Identity>{} : drivers::identities[h.driver],
            .colours = h.colours,
        });
    }
}

SWOFTY_ON_LOAD
{
    // the Helmets tab saved a choice for this career: wear it in every session of it
    events::on<career::events::Loaded>().after([](const career::events::Loaded &, const events::Context &ctx) {
        auto pick = hx::career_choice(ctx.career()->id);
        if (pick.empty())
            return;
        drivers::looks.layer(drivers::looks[ctx.player()->driver.id()], [pick](drivers::Look &l) {
            l.helmet = wearables::helmets["giggles.helmet-exchange/" + pick];
        }, Scope::career);
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/helmets.json: one imported helmet, written by hand
{
  "helmet": {
    "hamilton_2017": {
      "archive": "helmets/hamilton_2017_helmet.erp", "from": "game/lewis_hamilton",
      "name": { "text": "Hamilton 2017 (HX)" }, "driver": "game/lewis_hamilton",
      "colours": ["#FFE600", "#00A19B", "#7A2A8B", "#FFFFFF"]
    }
  }
}
```

The career choice has no data form: it needs the Helmets tab's code.

</template>
</Example>
</template>
</Split>

```text
[registry] register: giggles.helmet-exchange (helmet_exchange.dll) 12 helmet (early stage, 0.5 ms)
[registry] stat ids: helmet giggles.helmet-exchange/hamilton_2017 -> 1041
[registry] applied at boot: helmet 12 (12 rows, 12 asset groups with deps on the team livery group; archives served in memory)
[registry] layer from giggles.helmet-exchange on driver.look game/lewis_hamilton registered at run time (career); applies from the next session load
[registry] content career 3f2a active: 1 scoped change applied (driver.look game/lewis_hamilton /helmet)
```

## Team kit per season

<Split>

Classic Seasons gives each season's teams their suits and gloves. A season set is one `team_customisation_for_year` row; the suit and gloves are records of their own.

<template #example>
<Example root="swofty.classic-seasons-2016" code-file="native/src/kit.cpp" data-file="registry/kit.json" files="swofty.json, kit/mercedes_2016_suit.erp, kit/mercedes_2016_gloves.erp">
<template #code>

```cpp
SWOFTY_REGISTER
{
    auto mercedes = teams::identities["swofty.classic-seasons-2016/mercedes"];
    auto suit = wearables::suits.add("mercedes_2016", {
        .archive = file("kit/mercedes_2016_suit.erp"), .from = wearables::suits["game/mercedes"], .team = mercedes,
    });
    auto gloves = wearables::gloves.add("mercedes_2016", {
        .archive = file("kit/mercedes_2016_gloves.erp"), .from = wearables::gloves["game/mercedes"], .team = mercedes,
    });
    wearables::season_sets.add("mercedes_2016", { .team = mercedes, .season = 2016, .suit = suit, .gloves = gloves });
}
```

</template>
<template #data>

```json
{
  "suit":   { "mercedes_2016": { "archive": "kit/mercedes_2016_suit.erp",   "from": "game/mercedes", "team": "swofty.classic-seasons-2016/mercedes" } },
  "gloves": { "mercedes_2016": { "archive": "kit/mercedes_2016_gloves.erp", "from": "game/mercedes", "team": "swofty.classic-seasons-2016/mercedes" } },
  "wearable.season_set": {
    "mercedes_2016": { "team": "swofty.classic-seasons-2016/mercedes", "season": 2016,
                       "suit": "swofty.classic-seasons-2016/mercedes_2016", "gloves": "swofty.classic-seasons-2016/mercedes_2016" }
  }
}
```

</template>
</Example>
</template>
</Split>

## Items without rows

```cpp
// wearable.catalogue.build: add a helmet to the customisation catalogue at run time
events::on<wearables::events::CatalogueBuild>().after([](wearables::events::CatalogueBuild &c, const events::Context &) {
    c.helmets.push_back(wearables::helmets["giggles.helmet-exchange/hamilton_2017"]);
});
```

## Readiness and limits

- **Records: ready,** shipped at scale: the Classic Seasons shared addon has 124 season helmets (ids 868 to 1040, 253 MB) and suits with ids 974 to 1034.
- **Events: unknown (L1).** `Game_Build_CustomisationCatalogue`, the helmet decal, spawn and thumbnail functions are L1.
- **Every item row needs every printf-built colour key** (`colour_N_default`). The catalogue build crashes on a row without them; the core fills them from `colours` or the `from` row.
- **Asset groups need deps on the team livery group,** or cutscenes crash. The core registers them with the deps.
- **Customisable designs** (`tier01_liveryNN`) have no diffuse; the game composites them from `_m` masks with the row's default colours.
- **Converted F1 23 materials render invisible,** and `.mipmaps` sidecar names must match exactly, year suffix included.
- **The Helmets tab** lists up to 63 helmets per driver.
- **Per-event helmets** (a helmet for one race weekend) have no record yet.
