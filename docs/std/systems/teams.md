---
title: Teams
description: Teams in code - team identity, logos per context and pit times as typed registries, with the logo and name events and the team operations, in standard library 2.0.
---

# Teams

A team is a `teams::Identity` record: its names, colours, logo and the side data the game finds by its label.

## The header

<Example root="swofty-sdk" code-file="include/swofty/systems/teams.hpp">
<template #code>

```cpp
#include <swofty/registry.hpp>

namespace swofty::teams {

struct Colours {
    Colour menu;                                   // ui_team_colour: menus and the timing tower
    Colour primary;                                // primary_colour
    Colour secondary;                              // secondary_colour
    Colour tertiary;                               // tertiary_colour
};

struct Names {
    Text title;                                    // display_name
    Text short_name;                               // display_name_short
    Text super_short;                              // display_name_super_short
    std::string code;                              // display_name_code: "MER"
};

struct Identity {                                  // one entrants.teams row
    Ref<Identity> donor;                           // the row it copies, all 30 keys with story_info (new teams)
    Names names;                                   // the team's names
    Colours colours;                               // the team's colours
    std::optional<Colours> wear;                   // team_wear_*: empty follows colours (the kit follows primary)
    std::string livery_name;                       // the label HUD badges are found by
    Ref<Logo> logo;                                // its logos
    std::string audio_id;                          // audio_id
    int prestige = 0;                              // prestige
    std::string nationality;                       // nationality
    StatId stat_id = StatId::automatic;            // team ids run to 514
};

enum class LogoContext { driver_select, team_select, hud, broadcast, hub };

struct Logo {
    std::map<LogoContext, Path> images;            // one image per context; a slice each, not a whole array
};

struct PitTimes {                                  // one pit_lane.pit_times row, found by team label
    Ref<Identity> team;
    Ref<PitTimes> from;                            // copy another team's row
};

inline Registry<Identity> identities{"team.identity", { .applies = Applies::boot }};   // names and colours also on read
inline Registry<Logo> logos{"team.logo", { .applies = Applies::read }};
inline Registry<PitTimes> pit_times{"team.pit_times", { .applies = Applies::boot }};

Result set_logo(Ref<Identity> team, Ref<Logo> logo);     // from now on, every context
Result set_name(Ref<Identity> team, Names names);        // the loaded team's names
Result set_colours(Ref<Identity> team, Colours colours); // needs the Sess_Load_Teams record layout

}

namespace swofty::teams::events {

struct LogoResolve {                               // the two logo format sites and the HUD badge: before
    static constexpr const char *point = "team.logo.resolve";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    LogoContext context;                           // where the logo shows
    Ref<Identity> team;                            // whose logo
    std::string_view livery_name;
    std::string key;                               // the texture key the game will use; before handlers may change it
};

struct NameResolve {                               // display-name lookups: before (needs RE)
    static constexpr const char *point = "team.name.resolve";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    Ref<Identity> team;
    Text name;
};

}
```

</template>
</Example>

## Team Names, in code

<Split>

Team Names lets the player rename teams and change their colours from a launcher page. The page writes the player's choices to the addon's own storage. The addon's code reads them in the game and applies them as profile-scope edits on the stock team records.

- A name is written as text in every language (`Text::literal`); the core makes the lang key.
- The kit follows the primary colour unless `wear` is set.
- Classic Career and Progressive change the same 2026 teams. Load order decides per field, and the conflicts report names both.

<template #example>
<Example root="custom.team-names" code-file="native/src/names.cpp" data-file="registry/layers.json" files="swofty.json, bin/team-names-helper.exe, launcher/main.go">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/storage.hpp>
#include <swofty/systems/teams.hpp>

#include "choices.hpp"   // parses the page's choices.json

using namespace swofty;

SWOFTY_REGISTER
{
    std::string json;
    if (storage::read("choices.json", json) != storage::Result::ok)
        return;

    for (const team_names::Choice &c : team_names::parse(json)) {      // { team, title, short, tag, code, colours }
        teams::identities.layer(teams::identities["game/" + c.team], [c](teams::Identity &t) {
            if (c.title)  t.names.title = Text::literal(*c.title);
            if (c.short_) t.names.short_name = Text::literal(*c.short_);
            if (c.tag)    t.names.super_short = Text::literal(*c.tag);
            if (c.code)   t.names.code = *c.code;
            if (c.colours) t.colours = *c.colours;
        }, Scope::profile);
    }
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/layers.json: the same choices, written by hand
{
  "team.identity": {
    "game/mercedes_2026": {
      "names": { "title": { "text": "Mercedes-AMG Petronas" }, "super_short": { "text": "Mercedes" }, "code": "MER" },
      "colours": { "menu": "#00D2BE", "primary": "#00D2BE", "secondary": "#0C0C0C", "tertiary": "#C8CCCE" }
    }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: custom.team-names (team_names.dll) 3 layers on team.identity (early stage, 0.4 ms)
[registry] team.identity game/mercedes_2026: edit by custom.team-names (profile) changed /names/title, /names/super_short,
           /colours/primary -> #00D2BE
[registry] team.identity game/racing_bulls_2026: custom.team-names and career.classic-career both set /names/title;
           career.classic-career wins (later in load order)
[registry] applied at boot: team.identity 3 (3 rows, 33 records; 6 strings in 11 languages)
```

## A new team

A new team copies a stock row. The core copies every key, `story_info` included, writes its pit times and allocates the stat id:

```cpp
auto toro_rosso = teams::identities.add("toro_rosso", {
    .donor = teams::identities["game/tororosso"],
    .names = { .title = Text::key("lng_cs_team_17_toro_rosso"), .code = "STR" },
    .colours = { .menu = 0xFF0A1E3C, .primary = 0xFF0A1E3C, .secondary = 0xFFC8102E },
    .livery_name = "cs17_toro_rosso",
    .logo = teams::logos["swofty.classic-seasons/toro_rosso_2017"],
});
teams::pit_times.add("toro_rosso", { .team = toro_rosso, .from = teams::pit_times["game/tororosso"] });
```

## Logos per context

<Split>

Each logo record holds one image per context. The core builds the texture arrays from every logo record, slice by slice, so Classic Seasons, car ports and season packs each change their own teams' slices and all of them apply. Today they replace whole `overrides.uidb` images.

Progressive changes logos per season. In 1.x `progressive_logos.dll` redirects the format sites itself. In 2.0 it subscribes to `team.logo.resolve`.

<template #example>
<Example root="swofty.progressive" code-file="native/src/logos.cpp" files="swofty.json, logos/williams_2017_driver_select.png">
<template #code>

```cpp
#include <swofty/events.hpp>
#include <swofty/systems/teams.hpp>

using namespace swofty;

SWOFTY_ON_LOAD
{
    events::on<teams::events::LogoResolve>().in(events::owner("swofty.progressive"))
        .before([](teams::events::LogoResolve &l, const events::Context &ctx) {
            auto key = progressive::season_logo(l.team, ctx.career()->year, l.context);
            if (!key.empty())
                l.key = key;
        });
}

SWOFTY_ADDON()
```

</template>
</Example>
</template>
</Split>

```text
[events] team.logo.resolve (driver_select, game/williams, 2017): swofty.progressive -> progressive_williams_2017
```

```cpp
teams::set_logo(teams::identities["game/williams"], teams::logos["swofty.progressive/williams_2017"]);
```

## Readiness and limits

- **Partial.** The logo format sites (`site_Game_DriverSelect_TeamLogoFormat`, `site_Game_TeamSelect_TeamLogoFormat`) are L2 and shipped; the HUD badge builders are L1; the team record layout behind `Sess_Load_Teams` is L1, so `set_colours` waits on it.
- **Unknown:** a single logo-key resolver (logos are formatted at several sites) and the display-name lookups behind `team.name.resolve`.
- **Stat ids:** teams use ids up to 514.
- **A new team row needs all 30 keys,** `story_info` included, or the game reads garbage.
- **Side data** is found by team label: pit times, showroom history, animation team data, team customisation per season, initial cash, renown, team icons, standings.
- **Grid:** `team_position` and garages limit how many teams a session holds (see [Tracks](/std/systems/tracks)).
- **Colours** are signed ARGB in the database; the core converts them.
