---
title: Operations
description: The core's verbs that apply registry data to the game, built on verified bindings, in standard library 2.0.
---

# Operations

An **operation** is a core function that changes the game: put a driver in a seat, set a driver's ratings, set a team's logo. Operations take the same values the registries hold, so a consumer reads an effective record and hands it to an operation. The reverse engineering behind each one is done once, in the core, on bindings at L2 with independent verifies.

## Applying a season

<Split>

Progressive applies each season's line-up when the season starts. It reads the effective `career.season` record (with every addon's layers) and hands it to the core's operation, which reports what it changed and what it refused.

<template #example>
<Example root="swofty.progressive" code-file="native/src/seasons.cpp" files="swofty.json, native/CMakeLists.txt, native/src/register.cpp, data/years.json">
<template #code>

```cpp
#include <swofty/events.hpp>
#include <swofty/systems/career.hpp>

using namespace swofty;

SWOFTY_ON_LOAD
{
    events::on<career::events::SeasonStart>().in(events::content("swofty.progressive/f1_career_progressive"))
        .after([](const career::events::SeasonStart &e, const events::Context &) {
            auto season = career::seasons.get(career::seasons["swofty.progressive/" + std::to_string(e.year)]);
            if (!season)
                return;

            career::ApplyReport report;
            Result r = career::apply_lineup(*season, &report);    // seats, reserve seats and the entrant list
            swofty::log("[progressive] %d: %zu seat(s) set, %zu unchanged, %zu refused", e.year,
                        report.changed, report.unchanged, report.refused);
            for (const auto &why : report.refusals)
                swofty::log("[progressive]   %s", why.c_str());
            if (r != Result::ok)
                swofty::log("[progressive] %d: apply_lineup: %s", e.year, name(r));
        });
}

SWOFTY_ADDON()
```

</template>
</Example>
</template>
</Split>

## Setting ratings

```cpp
#include <swofty/systems/career.hpp>

void start_of_season_ratings(int year)
{
    using namespace swofty;
    for (Ref<career::DriverRatings> id : career::ratings.all("swofty.progressive/" + std::to_string(year) + "/")) {
        std::optional<career::DriverRatings> r = career::ratings.get(id);
        if (!r || r->driver == player_driver())
            continue;                                // the player's driver develops on his own
        career::set_driver_ratings(r->driver, *r);
    }
}
```

## The planned operations

Each one moves into the core when a second addon needs it. Its first version comes from the code that already does the job in Progressive Career or Career Editor:

```cpp
// swofty/systems/career.hpp
namespace swofty::career {
Result apply_lineup(const Season &season, ApplyReport *report = nullptr);
Result set_seat(int slot, int seat, Ref<drivers::Identity> driver);
Result set_driver_ratings(Ref<drivers::Identity> driver, const DriverRatings &ratings);
Result set_car_number(Ref<drivers::Identity> driver, int number);
}
// swofty/systems/teams.hpp
namespace swofty::teams {
Result set_logo(Ref<Identity> team, Ref<Logo> logo);
Result set_name(Ref<Identity> team, Names names);
}
// swofty/systems/tracks.hpp
namespace swofty::tracks {
Result set_track_fuel(Ref<Track> track, const Fuel &fuel);             // per-career, put back on unload
}
```

Every system page lists its operations in its header: [tyres](/std/systems/tyres#operations), [power units](/std/systems/power-units#events-and-operations), [sessions](/std/systems/sessions#events-and-operations), [weather](/std/systems/weather#events-and-operations), [AI](/std/systems/ai#events-and-operations), [career](/std/systems/career#operations), [UI](/std/systems/ui#events-and-operations), [graphics](/std/systems/graphics#settings).

## What an operation guarantees

- **Validated input:** the same `Ref<T>` and `check` rules as a registry edit. A refused part names the field and the reason.
- **Verified bindings only:** every function and field it uses is at L2 with an independent verify. On a build where one doesn't resolve, the operation returns `unavailable` and logs which binding is missing. The addon that called it keeps running.
- **Undo where the game keeps state:** an operation that changes shared game data, such as the track fuel tables, puts the original back when the career unloads, and logs both.
- **One log line per call,** with what changed. Send Logs then shows what an addon did to the game.
- **The flow thread:** operations that touch the career run there. From another thread they return `late`.
