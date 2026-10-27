---
title: Hooks
description: Game hook points, phases, addon hook points and how hooks share game functions with raw mixins, in standard library 2.0.
---

# Hooks

A **hook point** is a named place where the core runs every subscribed addon in one ordered chain. Game hook points cover game functions, and the core owns their mixins, so two addons that need the same game function no longer each patch it.

## A hook in full

Progressive Career keeps the contract event off a Follow-your-driver player's calendar. In 1.x it wraps `Game_CareerActivities_AddActivity` itself. In 2.0 it subscribes to the core's point:

```cpp
#include <swofty/hooks.hpp>
#include <swofty/career/hooks.hpp>

using swofty::career::AddActivity;

SWOFTY_ON_LOAD
{
    // replace: return true to stand in for the game's code; false lets the next handler (or the game) run
    swofty::hooks::replace<AddActivity>([](AddActivity &a) -> bool {
        if (a.type != AddActivity::contract_event || !a.player || progressive::follow_mode() != 1)
            return false;
        if (!progressive::has_contract(a.driver)) {
            swofty::log("[progressive] follow: contract event on day %u: no contract yet; the first Team HQ signs", a.day);
            return false;                    // a new career: let the game add it, Team HQ removes it later
        }
        return progressive::sign_instead(a);  // signed: the event is never added
    });
}
```

The payload is a plain struct with a size and a version. The core fills it from the game function's arguments:

```cpp
// swofty/career/hooks.hpp
namespace swofty::career {

struct AddActivity {
    static constexpr const char *point = "career.calendar.add_activity";
    static constexpr uint32_t version = 1;
    static constexpr uint8_t contract_event = 0x0f;

    uint32_t size;           // bytes the core filled; never read past it
    uint32_t payload_version;
    void *calendar;          // the driver calendar the activity goes on
    const void *activity;    // the activity definition
    uint32_t driver;         // the calendar's driver, stat id << 16
    uint32_t day;            // before handlers may move it
    uint8_t type;            // the activity type
    bool player;             // the calendar belongs to the human player
};

} // namespace swofty::career
```

## Phases

Every call to a hook point runs three phases:

```text
game calls Game_CareerActivities_AddActivity(calendar, activity, day)
  core fills AddActivity
  before   handlers, highest priority first, then load order; each may change the payload
  replace  handlers, same order, until one returns true; if none does, the game's own code runs
  after    handlers, all of them; they see the final payload and the result
  core returns to the game
```

```cpp
// before: change the input
swofty::hooks::before<AddActivity>([](AddActivity &a) {
    if (a.type == AddActivity::contract_event && a.player)
        a.day = std::max(a.day, progressive::first_race_day());   // never before round 1
});

// after: observe what happened
swofty::hooks::after<AddActivity>([](const AddActivity &a, const swofty::hooks::Outcome &o) {
    if (a.type == AddActivity::contract_event)
        swofty::log("[career-editor] contract event %s on day %u", o.replaced ? "skipped" : "added", a.day);
});

// a priority, for when order matters (default 0; higher runs first)
swofty::hooks::before<AddActivity>(swofty::hooks::Priority{10}, [](AddActivity &a) { ... });
```

A hook point's reference says which thread it runs on: the flow thread for career points, the session thread for session points. Handlers must return quickly and must not call the same hook point.

## Game hook points

The core adds a game hook point when a second addon needs the same game function, with the payload and a verified binding. The first planned points:

```text
career.calendar.add_activity     Game_CareerActivities_AddActivity      Progressive (contract events), Career Editor
career.contract.decide           Career_Decide_ContractNegotiation      Progressive (follow), Career Editor (contracts)
career.contract.sign             Career_Sign_DriverContract             Progressive, Career Editor
career.season.process            the season processor 0x140d766b0       Progressive (seasons), Career Editor
session.fuel.load                the race fuel lookup                   Progressive (seasons before 2026)
team.logo.resolve                the logo key lookups                   Progressive, Classic Seasons
```

The core installs a game hook point's mixin only once an addon subscribes to it. With no subscriber, the game's code is untouched.

## Addon hook points

An addon can define its own point and fire it, so other addons can extend it:

```cpp
// progressive.dll: let other addons adjust a season just before Progressive applies it
struct SeasonApply {
    static constexpr const char *point = "progressive.season.apply";
    static constexpr uint32_t version = 1;
    uint32_t size;
    uint32_t payload_version;
    int year;
    swofty::career::Season *season;    // before handlers may change it
};

SWOFTY_ON_LOAD
{
    swofty::hooks::define<SeasonApply>();
}

void apply_season(swofty::career::Season season)
{
    SeasonApply p{ sizeof(SeasonApply), SeasonApply::version, season.year, &season };
    if (!swofty::hooks::fire(p))                 // false: a replace handler applied it instead
        return;
    swofty::career::apply_lineup(season);
}
```

```cpp
// another addon: a driver who skipped a season comes back on the reserve seat
swofty::hooks::before<progressive_api::SeasonApply>([](progressive_api::SeasonApply &p) {
    if (p.year == 2017)
        swofty::log("[whatif] season %d has %zu teams", p.year, p.season->teams.size());
});
```

The owning addon ships the payload header (`progressive_api/hooks.hpp`) for others to include, and declares the point in its manifest (see [Manifest](/std/manifest)).

## Living with mixins

A hook point is one entry in the game function's mixin chain, at the core's priority. Raw mixins keep working around it:

```text
[mixin] Game_CareerActivities_AddActivity: swofty_core.dll(hooks, 100) > old_addon.dll(0)
```

- **A 1.x addon's `SWOFTY_WRAP` on the same function** chains below the hook point, as any two wraps chain today.
- **A 1.x addon's overwrite on the function** conflicts with the hook point's mixin. The 1.x rule applies: the core's set is the one dropped. The hook point reports `unavailable`, its subscribers don't run, and the old addon keeps working:

```text
[hooks] career.calendar.add_activity unavailable: old_addon.dll overwrites Game_CareerActivities_AddActivity;
        subscribers swofty.progressive, swofty.career-editor not run (the launcher lists this as a conflict)
```

Subscribers can check before relying on a point:

```cpp
if (!swofty::hooks::live<AddActivity>())
    swofty::log("[progressive] the add-activity hook point isn't live; falling back to the contract scene skip");
```
