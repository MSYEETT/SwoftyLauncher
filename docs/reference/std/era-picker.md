---
title: Era picker boxes
description: Add a box to the Addons row of the Grand Prix and Time Trial era picker with swofty/era.hpp.
---

# Era picker boxes

The era picker is the row of boxes shown after Grand Prix or Time Trial (and on the split-screen game mode select): F1 2026, F1 2025, F2 2025 and so on. SwoftyMod gives it a second row. The game's own boxes stay in the top row, **Built In**, and boxes that addons register go in the bottom row, **Addons**, which scrolls on its own. Up and down move between the rows.

You register a box from your addon; the core builds it every time the picker is built, presses it like a stock box and, when it is pressed, narrows the host setup mode to the box's entrant list so Time Trial loads only that list's cars. The data side (the entrant list, its teams and drivers, and its place in the host setup mode) comes from your addon's [database operations](/reference/ops/database).

| | |
|---|---|
| Header | `swofty/era.hpp` |
| Namespace | `swofty::era` |
| Table | `SwoftyApi::era` (`SwoftyEra`) |
| API version | 8, feature-detected (see [Availability](#availability)) |
| Thread | `add_box`: addon init. Others: any |

## Registering a box

Call `add_box()` from the check function you pass to `SWOFTY_ADDON`, which runs before the core's mixin commit:

```cpp [native/src/boxes.cpp]
#include <swofty/era.hpp>

bool init()
{
    for (int year = 2016; year <= 2024; year++) {
        std::string list = "cs_f1_" + std::to_string(year);
        swofty::era::add_box({
            .entrant_list = list,
            .title = "lng_" + list,
            .image = "#overrides.home_screen.tiles_solo_6x6[" + list + "]",
            .order = year,
        });
    }
    return true;
}

SWOFTY_ADDON(init)
```

| Field | Meaning |
|---|---|
| `entrant_list` | The entrant list the box selects. 1 to 63 letters, digits or underscores. One box per entrant list |
| `title` | The `lng_*` key the box shows. Add the string with your addon's language table. Empty means `lng_<entrant_list>` |
| `image` | The texture path the box shows. Empty means `#overrides.home_screen.tiles_solo_6x6[<entrant_list>]`: add an entry with that name to the `home_screen.tiles_solo_6x6` overrides and a slice to its texture array |
| `order` | Boxes show in ascending `order`, then in registration order. Default `0` |

The strings are copied. `add_box()` returns `ok` when the box is queued (or live from the next build of the picker, if the hooks are already installed), `invalid` for a bad entrant list name, an over-long string or a second box for the same entrant list (the log says which), and `unavailable` on an older core or when the era picker hooks could not be installed on this build. Up to 64 boxes can be registered.

## The data side

A box only selects an entrant list; the list itself must be in the game database, and in the entrant lists of the core's host setup mode, `swofty_f1`. The core adds `swofty_f1` (a game setup mode and its entrant setup mode) to every F1 mode. Your database operations then:

1. add the entrant list (`entrants.entrant_lists`), its teams and its drivers;
2. `list-append` the entrant list onto the `swofty_f1` entrant setup mode's `entrant_lists` and `selectable_entrant_lists`;
3. add the `game_setup.scenarios` row the game checks the list against.

Every addon shares `swofty_f1`, so the game's per-mode setup list stays the size it was. Each time the core builds the picker it checks every box with the stock check (`FeTile_Is_EraScenarioAvailable_Thunk(mode, "swofty_f1", entrant list)`) and skips, with one log line, a box whose entrant list the screen's mode does not offer under `swofty_f1`. A box for a list that a missing database operation leaves out never reaches the game.

When a box is pressed, every `swofty_f1` entrant setup mode offers only that box's entrant list until another box is pressed or the picker is built again, and the setup's selected list is cleared. Time Trial loads every car of every list the entrant setup mode offers and the Grand Prix takes its first list, so this is what makes each box load its own season.

## Availability

```cpp
if (!swofty::era::available())
    swofty::log("[my-addon] this core has no era picker boxes");
```

`available()` is true when the running `swofty_core.dll` exports `SwoftyCoreEra` and its era picker hooks are not off. An addon built against this header also loads on an older core and sees `available()` false and `add_box()` returning `unavailable` there. `count()` returns how many boxes every addon has registered.

An addon can replace the era picker's menu with its own copy, as some imported season mods do. When the launcher can't edit that copy, it leaves it as it is and warns at build time, naming the addon. While that addon is on, the menu has no Addons row and the boxes don't show there; `available()` and `add_box()` return what they would otherwise.

## Logs

The core logs to `swofty/loader.log` with the `[era]` prefix: each registration, the hooks, the first builds of the picker (`era select: N stock box(es), M addon box(es) of K registered`), each skipped box and each narrowing.
