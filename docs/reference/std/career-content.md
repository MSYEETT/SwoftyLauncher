---
title: Career contents
description: Add a separate career to F1 25, with its own tile on the career content select and its own save slots, with swofty/career_content.hpp.
---

# Career contents

A career content is a tile on the career content select (Career > Driver Career) after the stock 2025 and 2026 tiles. It has its own save slots, filed under its own data group, so its careers never appear under the stock tiles and stock careers never appear under it. You register the content from your addon; the core builds the tile and owns every native step the stock code would refuse for a new career. The data side (the mode row, the championship, the entrant lists) comes from your addon's [database operations](/reference/ops/database).

| | |
|---|---|
| Header | `swofty/career_content.hpp` |
| Namespace | `swofty::career::content`, plus `swofty::career::data_group` / `is` / `current_data_group` |
| Table | `SwoftyApi::career_content` (`SwoftyCareerContent`) |
| Feature | `career` |
| API version | 8 (the `career_content` member), feature-detected (see [Availability](#availability)) |
| Thread | `add`: addon init. `data_group`, `is`, `current_data_group`: flow thread. Others: any |

Every call that returns a `Result` uses the shared [result codes](./#result-codes).

## Registering a content

Call `add()` from the check function you pass to `SWOFTY_ADDON`, which runs before the core's mixin commit:

```cpp [native/src/my_career.cpp]
#include <swofty/career_content.hpp>

namespace content = swofty::career::content;

bool init()
{
    content::Result r = content::add({
        .id = "f1-1990",
        .title = "lng_f1990_career_content",
        .description = "lng_f1990_career_content_description",
        .data_group = "f1_career_1990",
        .suffix = "90",
        .mode = "driver_career_1990",
        .car_group = 2,
    });
    swofty::log("[my-career] content %s", swofty::career::name(r));
    return true;
}

SWOFTY_ADDON(init)
```

| Field | Meaning |
|---|---|
| `id` | Names the content in logs and `info()`. 1 to 48 characters of `a-z`, `0-9`, `_`, `-` |
| `title`, `description` | `lng_*` keys for the tile's labels. Add the strings with your addon's language table |
| `data_group` | The save-slot key: the name of the entrant list that your championship's `ComCareerSetup.entrant_list` names. Not `f1_career_2025` or `f1_career_2026` |
| `suffix` | The tile's suffix. The driver select formats `F1CAREER_%s`, `F2CAREER_%s` and `ICONS_%s` with it. Not `25` or `26` |
| `mode` | The `save_data.name` of your `game_modes.ModeChampionship` row. The core reads that row's `statId` at runtime |
| `model` | The stock tile the content copies: `Model::f1_2026` (default) or `Model::f1_2025`. The tile takes the model's route, menu systems and vtable |
| `car_group` | The `stat_id` of the customisation car group the press selects; `-1` (default) takes what the model tile's press takes |
| `badge` | The tile's notification value; `0` (default) shows no badge |
| `entitlement` | Replaces the 2026 model's entitlement string (`f1_2026`); empty keeps it |

`add()` returns `ok` when the content is queued (or live at once, if another addon's content already installed the hooks), `invalid` for a bad or duplicate descriptor (the log names the field), and `unavailable` on an older core, with the `career` feature off, or when the hooks could not be installed. Up to 8 contents can be registered; their tiles follow the stock ones in registration order (`button_3`, `button_4`, ...).

## What the core does for a content

The core installs four hooks as one core mixin set, at the first mixin commit after a content is registered. A game where no addon registers a content runs none of them.

| Hook | Binding | What it does for a content |
|---|---|---|
| Tile | `site_FeTile_CareerContentEnd` (site, end of `FeTile_Build_List` case 6) | appends `button_N` with `FeHome_Add_CareerTile`, then replaces its home item with a career content item built by `FeTile_Construct_CareerItem`: the model's route (`+0x178`, `driver_career`), your data group (`+0x180`) and suffix (`+0x188`), and a copy of the model's vtable whose populate sets your title and description and whose press selects your car group and calls the router |
| Router | `FeTile_Route_CareerItem` (wrap) | records the pressed tile's data group and fires `career.content.pressed` |
| Gate | `Game_Career_IsDriverCareerMode` (wrap) | passes the mode championship whose `statId` (`+0x2a8`) is your mode's; the stock gate accepts only the 2025 and 2026 driver careers, so without this the new-career path reads through a null slot |
| Slot predicate | `vt_Game_DriverCareerSlotPredicate` slot 2 | asked for your data group, a stock "no" for your mode's career becomes the predicate's own data-group check. Questions for other data groups keep the game's answer |

At the press, a content tile whose menu systems (`+0x150..+0x178`) are unset takes the model tile's, as the 2026 tile's are what the career wizard needs.

## Data your addon must provide

The core checks the first four rows once the database has loaded and logs the result for each content: `[career] content '<id>' ready: mode ... statId ..., data group ... (entrant list stat_id ...)`, or `[career] content '<id>' (<addon>) is off: <reason>`. A content that is off gets no tile and its mode keeps the stock gate.

| Requirement | Checked | Why |
|---|---|---|
| A `game_modes.ModeChampionship` row whose `save_data.name` (or `save_data_0.name`) is `mode` | yes, required | its `statId` is how the gate and the predicate recognise your career |
| An `entrants.entrant_lists` row named `data_group`, with a `stat_id` below 256 | yes, required | the `CareerSetupComponent` stores the `stat_id` of the list `ComCareerSetup.entrant_list` names, as a byte; the predicate reads the name back from that index |
| A content set official entrant list `F1CAREER_<suffix>` | yes, required | the driver select looks it up with the tile's suffix |
| `F2CAREER_<suffix>` and `ICONS_<suffix>` | yes, warning only | the F2 and Icons tabs of the driver select are empty without them |
| Your mode name and setup mode in the content set's `available_game_modes_0.game_mode_championships` and `available_game_setup_modes_0.setup_modes` | no | the wizard only offers modes the content set lists |
| A championship row cloned with all its component rows (`ComCareer`, `ComCareerSetup`, `ComInfluence`, ...) and `ComCareerSetup_0.entrant_list` = `data_group` | no | a championship missing component rows reads as My Team |
| A data-group row named `data_group` in each data-group table (`career.career_setup.CareerSetupDataGroup`, `accolades.AccoladeDataGroup`, `career.leverage.LeverageDataGroup`, `recognition.RecognitionDataGroup`, `resource_points.data_group`, `specialist_manager.dataGroup`, and the My Team ones) | no | the career systems look their data up by the career's data group |

The F1 1990 addon's `tools/career.py` and `tools/grid.py` write a complete set of these rows; use them as the reference.

## Which career is loaded

| Call | Returns |
|---|---|
| `data_group(std::string *out, const void *career = nullptr)` | `ok` with the data group of a career entity (`nullptr`: the loaded one); `late` when no career is loaded; `invalid` when the career has no named data group |
| `current_data_group()` | the loaded career's data group, or `""` |
| `is(data_group, career = nullptr)` | the career is filed under that data group |
| `content::pressed(std::string *out)` | `ok` with the data group of the last career content tile pressed, stock ones included; `late` before the first press |
| `content::pressed_is(data_group)` | the last pressed tile has that data group |
| `content::info(id, Info *out)` | a content's state (`queued`, `live`, `off` with `why`), its `mode_stat_id`, `data_group_index`, and its tile and press counts |
| `content::available()` | the core implements contents and the `career` feature is on |

`data_group` reads the career the way the save-slot predicate does: each championship's `CareerSetupComponent` data-group index into its entrant-list names. Use `is()` to gate behaviour on one career, for example on the [`career.loaded`](./events#flow-and-career-events) event:

```cpp
void on_career(const SwoftyEvent *ev, void *)
{
    bool mine = !strcmp(ev->name, SWOFTY_EVENT_CAREER_LOADED) && swofty::career::is("f1_career_1990");
    swofty::log("[my-career] %s", mine ? "1990 career loaded" : "another career");
}
```

## Events

| Event | `a` | `b` | `data` |
|---|---|---|---|
| `career.content.pressed` | the registered content's index, or `-1` for a stock tile | 0 | `const char *` data group of the pressed tile |

It fires on the flow thread before the router picks the save-slot list or the new-career wizard, and only while at least one content is registered.

## Availability

The table has no API version of its own; its `SwoftyApi::career_content` member arrived with version 8. The header reads `SwoftyApi::career_content` only when the running `swofty_core.dll` exports `SwoftyCoreCareerContent`, so an addon built against it also loads on an older core, where `available()` is `false` and `add()` returns `unavailable`. To keep code that predates the header building, guard on the header:

```cpp
#if __has_include(<swofty/career_content.hpp>)
#include <swofty/career_content.hpp>
#define MY_CONTENT_API 1
#endif
```

The hooks rely on bindings that are L2 in the KB with an independent verify row each; the press path, the gate pass and the slot predicate were confirmed in game for the 2026 model only. The 2025 model also needs `FeTile_CareerDefaultItem_Populate`, whose verify row is still `partial`, so it is not exported yet and a 2025-model content turns off with `needs FeTile_CareerDefaultItem_Populate`.
