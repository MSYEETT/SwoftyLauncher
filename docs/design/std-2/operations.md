---
title: Operations
description: The core's verbs that apply registry data to the game, built on verified bindings, in standard library 2.0.
---

# Operations

An **operation** is a core function that changes the game: put a driver in a seat, set a driver's ratings, set a team's logo. Operations take the same values the registries hold, so a consumer reads an effective record and hands it to an operation. The reverse engineering behind each one is done once, in the core, on bindings at L2 with independent verifies.

## Applying a season

```cpp
#include <swofty/registry.hpp>
#include <swofty/career/season.hpp>
#include <swofty/career/ops.hpp>

using namespace swofty::career;

void on_season_start(int year)
{
    std::optional<Season> season = swofty::registry::get<Season>(std::string(kDataGroup) + "/" + std::to_string(year));
    if (!season)
        return;

    ApplyReport report;
    Result r = apply_lineup(*season, &report);    // seats, reserve seats and the entrant list
    swofty::log("[progressive] %d: %zu seat(s) set, %zu unchanged, %zu refused", year,
                report.changed, report.unchanged, report.refused);
    for (const auto &why : report.refusals)
        swofty::log("[progressive]   %s", why.c_str());
    if (r != Result::ok)
        swofty::log("[progressive] %d: apply_lineup: %s", year, name(r));
}
```

## Setting ratings

```cpp
#include <swofty/career/ratings.hpp>

void start_of_season_ratings(int year)
{
    for (std::string_view id : swofty::registry::ids(DriverRatings::registry, std::string(kDataGroup) + "/" + std::to_string(year) + "/")) {
        std::optional<DriverRatings> r = swofty::registry::get<DriverRatings>(id);
        if (!r || r->driver == player_driver())
            continue;                                // the player's driver develops on his own
        set_driver_ratings(r->driver, *r);
    }
}
```

## The planned operations

Each one moves into the core when a second addon needs it. Its first version comes from the code that already does the job in Progressive Career or Career Editor:

```cpp
namespace swofty::career {
Result apply_lineup(const Season &season, ApplyReport *report = nullptr);
Result set_seat(int slot, int seat, std::string_view driver);
Result set_driver_ratings(std::string_view driver, const DriverRatings &ratings);
Result set_car_number(std::string_view driver, int number);
}
namespace swofty::team {
Result set_logo(std::string_view team, std::string_view logo_key);
Result set_name(std::string_view team, std::string_view lang_key);
}
namespace swofty::session {
Result set_track_fuel(std::string_view track, const TrackFuel &fuel);   // per-career, restored on unload
}
```

## What an operation guarantees

- **Validated input:** the same `x-ref` and rule checks as a registry write. A refused part names the field and the reason.
- **Verified bindings only:** every function and field it uses is at L2 with an independent verify. On a build where one doesn't resolve, the operation returns `unavailable` and logs which binding is missing. The addon that called it keeps running.
- **Undo where the game keeps state:** an operation that changes shared game data, such as the track fuel tables, puts the original back when the career unloads, and logs both.
- **One log line per call,** with what changed. Send Logs then shows what an addon did to the game.
- **The flow thread:** operations that touch the career run there. From another thread they return `late`.
