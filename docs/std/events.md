---
title: Events
description: The game's lifecycle as events, from boot to the chequered flag, each with the context of everything; scoped handlers; content owners who override events or decide who hooks them; and events any addon defines for others.
---

# Events

An **event** is a moment in the game's lifecycle: a career's season starts, a session loads, a car makes a pit stop. Addons subscribe to it, filtered to the content they care about. Every handler gets a **context** with everything about that moment: the mode, the content and its owner, the career, the season and round, the weekend, the session, the player and every participant, and each one's registry records.

## Events in full

<Split>

F1 1990 reacts to three moments, only inside its own career: it logs each season start, caps the rain chance of every race, and builds the grid from the better of two qualifying sessions.

Each event's payload lives in its system's header, in `swofty::<system>::events`. Subscriptions are code only: the core lists them in `registry/effective.json` for Send Logs.

<template #example>
<Example root="swofty.f1-1990" code-file="native/src/events.cpp" files="swofty.json, native/CMakeLists.txt, native/src/grid.cpp">
<template #code>

```cpp
#include <swofty/events.hpp>
#include <swofty/systems/career.hpp>
#include <swofty/systems/sessions.hpp>
#include <swofty/systems/weather.hpp>

using namespace swofty;
using namespace swofty::events;

static const Selector k1990 = content("swofty.f1-1990/f1_career_1990");

SWOFTY_ON_LOAD
{
    // after: observe the moment
    on<career::events::SeasonStart>().in(k1990).after([](const career::events::SeasonStart &e, const Context &ctx) {
        swofty::log("[f1-1990] season %d starts, round %d of %d, player %s", e.year,
                    ctx.career()->round, ctx.career()->rounds, ctx.player()->driver.id().data());
    });

    // before: change what the game is about to use
    on<weather::events::Setup>().in(k1990).when(session_type({ SessionType::race }))
        .before([](weather::events::Setup &w, const Context &) { w.rain_chance = std::min(w.rain_chance, 30u); });

    // replace: stand in for the game; return true when handled
    on<sessions::events::GroupResult>().in(k1990).replace([](sessions::events::GroupResult &g, const Context &ctx) {
        return f1990::rank_best_of(g, ctx.session()->group, 1, 2);   // the grid from the better of two qualifying sessions
    });
}

SWOFTY_ADDON()
```

</template>
</Example>
</template>
</Split>

```text
[events] career.season.start (driver_career, swofty.f1-1990/f1_career_1990, 1990): 1 after handler
[f1-1990] season 1990 starts, round 1 of 16, player ayrton_senna
[events] session.grid.group_result in swofty.f1-1990/f1_career_1990: replace by swofty.f1-1990 -> handled
```

## Kinds and phases

An event is a [hook point](/std/hooks), so it has the same phases. Which phases an event has depends on what the core can do at that moment:

```text
notify   after                      the core saw the moment
modify   before, after              the core hands the changed payload back to the game
cancel   before, replace, after     the core wraps the game code; a replace handler can stand in for it
```

```text
core reaches the moment
  fills the payload and takes a context
  drops handlers whose scope doesn't match            (no call, no cost)
  before    priority, then load order; the content owner's run LAST
  replace   the content owner's FIRST, then priority, then load order, until one returns true
  after     priority, then load order; 1.x subscribe() handlers run here
```

Asking for a phase an event doesn't have is refused when you attach:

```text
[events] career.loaded has no replace phase (notify event); handler from my-addon not attached
```

## The lifecycle

Every event the core fires, with its kind. `1.x` marks the events 1.x already has, under the same name or a legacy one.

```text
game
├─ game.boot                       notify    every addon's SWOFTY_ON_LOAD has run
├─ game.ready                      notify    the first front-end frame
├─ flow.state                      notify    any front-end flow state changes                              1.x
├─ menu.route                      cancel    a front-end route runs; replace = it doesn't
├─ menu.screen                     notify    a screen opens                                                 1.x screen
├─ game.pause / game.resume        notify                                                                   1.x pause, resume
├─ game.saving                     modify    the profile is about to be written
├─ mode.select                     cancel    the player picks a mode
├─ content.select                  notify    a GP/TT box or a career tile is pressed
│
├─ career
│  ├─ career.starting              cancel    a career route; replace = career::block                       1.x
│  ├─ career.created               notify    a new career exists (career snapshots are taken here)
│  ├─ career.loaded                notify                                                                   1.x
│  ├─ career.hub                   notify    Team HQ is idle                                                1.x
│  ├─ career.season.start          notify    a season begins (the first, then each after the break)
│  ├─ career.time.advance          notify    time starts or stops advancing
│  ├─ career.day                   notify    the career day changes
│  ├─ career.calendar.add_activity     cancel   an activity is added to a calendar
│  ├─ career.calendar.remove_activity  cancel
│  ├─ career.rnd.start / .cancel   cancel    research or development starts or is cancelled
│  ├─ career.facility.complete     notify
│  ├─ career.weekend.start / .end  notify
│  ├─ career.contracts.open        notify    the contract scene opens
│  ├─ career.contract.decide       cancel    the game decides whether to negotiate
│  ├─ career.contract.sign         cancel
│  ├─ career.contract.changed      notify
│  ├─ career.driver.moved          notify
│  ├─ career.season.process        cancel    the season change processes contracts and moves
│  ├─ career.moves.finalised       notify    the line-up changes for the new season are final
│  ├─ career.season.end            notify
│  ├─ career.season.break          notify
│  ├─ career.saving                modify    the last moment to write a career layer into this save
│  └─ career.unloaded              notify                                                                   1.x
│
├─ weekend
│  ├─ weekend.start                notify
│  └─ weekend.end                  notify
│
└─ session
   ├─ session.load                 modify
   ├─ session.weather.setup        modify
   ├─ session.fuel.load            cancel
   ├─ session.start                notify                                                                   1.x
   ├─ session.state                notify    any session state changes                                      1.x state
   ├─ session.grid                 notify    the grid rundown
   ├─ session.formation_lap        notify
   ├─ race.start                   notify                                                                   1.x
   ├─ session.sector               notify
   ├─ session.lap                  notify                                                                   1.x lap
   ├─ session.pit / .pit_stop      notify
   ├─ session.flag                 notify                                                                   1.x flag
   ├─ session.safety_car           notify
   ├─ session.red_flag             notify
   ├─ session.penalty.issue        cancel
   ├─ session.car.status           notify    retired, disqualified, terminal damage
   ├─ session.player.control       cancel    who drives the player's car
   ├─ session.finish               notify
   ├─ session.grid.group_result    cancel    a session group's result, which sets a grid
   ├─ session.results              notify                                                                   1.x results
   ├─ session.podium               notify
   ├─ session.end                  notify    finished, abandoned or restarted                               1.x
   └─ session.unload               notify
```

**Every start has one end.** The core keeps a lifecycle stack. If the player quits from the pause menu, the core fires the missing ends itself, with `reason = abandoned`:

```text
[events] stack: game > career(driver_career, swofty.f1-1990/f1_career_1990) > season 1990 > weekend r3 > session qualifying_1
player quits to the main menu
[events] session.end (reason abandoned, synthesised)
[events] weekend.end (reason abandoned)
[career] career.unloaded (mode driver_career)
```

## The context

Every handler of every event, and of every game hook point, gets the same view:

```cpp
namespace swofty::events {

class Context {
public:
    Mode mode() const;                               // grand_prix, time_trial, driver_career, my_team, ...
    const ContentView &content() const;              // what the player is playing, its owner, its variant chain
    const std::optional<CareerView> &career() const; // data group, year, season, round of rounds, day
    const std::optional<WeekendView> &weekend() const;
    const std::optional<SessionView> &session() const;   // type, group, track, weather, laps, setup mode
    const std::optional<Entrant> &player() const;
    std::span<const Entrant> participants() const;   // car, driver, team, vehicle, human
    bool in(const Selector &s) const;

    // the effective record of this moment's own thing: the season of the career, the package of a GP,
    // the track of a session. Inside a career it reads the career's snapshot.
    template <class T> std::optional<T> record() const;
};

}
```

```cpp
on<sessions::events::Load>().after([](const sessions::events::Load &, const Context &ctx) {
    auto season = ctx.record<career::Season>();                  // the career's season, from its snapshot
    auto track  = tracks::tracks.get(ctx.session()->track);
    for (const auto &e : ctx.participants())
        if (e.human)
            swofty::log("[my-addon] you drive %s for %s at %s", e.driver.id().data(), e.team.id().data(), track->name.c_str());
});
```

The context is a small handle over facts the core caches once per lifecycle change. It costs nothing until you read it:

```text
[events] epoch 17: session.load (driver_career, swofty.f1-1990/f1_career_1990 owner swofty.f1-1990, 1990 r3, qualifying_1, game/imola)
[events] ctx.participants for epoch 17: 26 entrants in 0.21 ms
```

- **A field that doesn't apply is empty:** `session()` in the menus, `career()` in Grand Prix.
- **Live per-car values** (position, lap, tyres) are in the event's payload, not the context.
- **The name.** It's `swofty::events::Context`, because `swofty::Context` is already the register file mid-hooks use.

## Scope

A handler names where it runs. The core checks the scope before calling, so a scoped handler costs nothing anywhere else:

```cpp
on<E>().in(content("swofty.classic-seasons-2017/f1_2017"))   // that package and every variant of it
on<E>().in(content_exact("swofty.classic-seasons-2017/f1_2017"))
on<E>().in(owner("swofty.f1-1990"))                          // any content that addon provides
on<E>().in(data_group("f1_career_progressive"))
on<E>().in(mode(Mode::time_trial))
on<E>().when(session_type({ SessionType::race, SessionType::sprint }))
on<E>().when(track("game/monaco"))
on<E>().when(year(2016, 2019))
on<E>().in(any({ content("a/x"), content("b/y") })).when(not_(track("game/monaco")))
on<E>().in(setup_mode("swofty.f1-1990/f1_1990"))                  // every session run under that setup mode
on<E>().in(vehicle_package("swofty.f1-1990/f1_1990"))              // per car: cars of that package
on<E>().in(f1990::scope)                                           // the scope your system declares (a Selector)
```

A rule written for the 2017 package also runs in "F1 2017 (V10)", because that's a variant of it. Use `content_exact` for the one record only.

## Content owners

The **owner** of a moment is the addon that provides the content being played (and every owner along its variant chain). Inside its own content, an owner can do two things other addons can't.

### Override an event

The owner's `replace` runs first in its own content, so it decides how the game handles the moment there:

```cpp
// Classic Seasons 2017: races in the 2017 package start on 2017 fuel loads
on<tracks::events::FuelLoad>().in(content("swofty.classic-seasons-2017/f1_2017"))
    .replace([](tracks::events::FuelLoad &f, const Context &) {
        f.kg = cs::fuel_2017(f.track);
        return true;
    });
```

```text
[events] session.fuel.load in swofty.classic-seasons-2017/f1_2017: replace by swofty.classic-seasons-2017 (owner, first)
         -> handled; 1 other replace not reached (someone.fuelsaver)
```

### Decide who hooks an event

By default every addon's handlers run in every content. The owner still has the last word through ordering: its `before` runs last and its `replace` first. An owner can also choose, per event and phase, to keep other addons' `before` and `replace` handlers out of its own content, and name the ones it allows:

```cpp
// swofty.f1-1990: a record of events::policies ("event.policy"), keyed by the content it owns
SWOFTY_REGISTER
{
    using events::Access;
    events::policies.add("f1_career_1990", { .events = {
        { "session.grid.group_result", { .replace = Access::owner, .allow = { "community.goodyear-1990-plus" } } },
        { "career.contract.decide",    { .replace = Access::owner, .before = Access::owner } },
        { "*",                         { .replace = Access::open } },
    } });
}
```

```json
// the same, without code: registry/policy.json
{ "event.policy": { "f1_career_1990": { "events": {
    "session.grid.group_result": { "replace": "owner", "allow": ["community.goodyear-1990-plus"] },
    "career.contract.decide":    { "replace": "owner", "before": "owner" },
    "*":                         { "replace": "open" } } } } }
```

The policy only ever stays narrow:

- **`after` is never kept out.** Every addon can always observe every moment.
- **1.x handlers are never kept out.** A 1.x `subscribe` is an `after` handler, and raw mixins sit outside the chain.
- **It applies inside the owner's own content only.** Grand Prix, other careers and other packages are unaffected.
- **Other addons, and the player, can only open it.** A layer can add to `allow` or turn `owner` back into `open`, never close it; the schema refuses a closing layer. The player opens it per profile from the conflicts view.
- **Every skip is logged and listed in the launcher:**

```text
[events] policy swofty.f1-1990/f1_career_1990: session.grid.group_result replace is owner-only; skipped community.gridrules
         (allowed: community.goodyear-1990-plus; the player can open it in Profile > Conflicts)
```

## Your own events

Any addon can define events under its own id, and fire them from its code. Other addons subscribe to them like core events, with full context and scope. An event can be global, or declared to fire only inside some content.

```cpp
// api/progressive_api/events.hpp, shipped by Progressive for other addons to include
namespace progressive_api {

struct SeatChange {
    static constexpr const char *point = "swofty.progressive.seat_change";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    int year, round;
    uint32_t slot, seat;
    const char *out_driver, *in_driver;   // record ids
    int replaced;                         // set by the core: a replace handler made the change instead
};

}
```

```cpp
// progressive.dll: define it, then fire it where Progressive applies a mid-season swap
SWOFTY_ON_LOAD
{
    swofty::events::define<progressive_api::SeatChange>(swofty::events::owner("swofty.progressive"));
}

void apply_change(const Change &c)
{
    progressive_api::SeatChange e{ sizeof e, 1, c.year, c.round, c.slot, c.seat, c.out.c_str(), c.in.c_str(), 0 };
    if (swofty::events::fire(e))          // false: a replace handler made the swap itself
        swofty::career::set_seat(c.slot, c.seat, swofty::drivers::identities[c.in]);
}
```

```cpp
// another addon: a news ticker for Progressive careers
#include <progressive_api/events.hpp>

SWOFTY_ON_LOAD
{
    using namespace swofty::events;
    on<progressive_api::SeatChange>().after([](const progressive_api::SeatChange &e, const Context &ctx) {
        swofty::hud::toast("Driver change", std::string(e.in_driver) + " replaces " + e.out_driver);
    });
}
```

- **The name starts with your addon's id.** The launcher refuses a `define` under another addon's id.
- **The scope is enforced.** An event fired outside the scope it was declared with isn't delivered:

```text
[events] swofty.progressive.seat_change fired outside its scope (content game/f1_career_2025); not delivered
```

- **Versions** follow the same rules as hook payloads: append fields and raise `version`, never remove (see [Versioning](/std/versioning)).
- **When a second addon needs the same moment,** the core adopts it into its own catalogue under a core name, and your name keeps working as an alias.
- **Subscribers by name** don't need your header: `on("swofty.progressive.seat_change").after(...)` gets the payload as bytes plus the full context.

## Launcher events

Launcher plugins get a `lifecycle` notification for the launcher's own moments:

```text
launcher  {"method":"lifecycle","params":{"event":"profile.switched","profile":"progressive"}}
launcher  {"method":"lifecycle","params":{"event":"addons.changed","profile":"progressive","changed":["mod.sochi"]}}
launcher  {"method":"lifecycle","params":{"event":"build.done","profile":"progressive","ok":true}}
launcher  {"method":"lifecycle","params":{"event":"play.starting","profile":"progressive"}}
launcher  {"method":"lifecycle","params":{"event":"game.ended","profile":"progressive","exit_code":0}}
launcher  {"method":"lifecycle","params":{"event":"logs.sending"}}
```

A plugin subscribes by listing them in its `launcher` block: `"lifecycle": ["build.done", "game.ended"]`.

## Adding an event

Adding a core event is one catalogue entry, a hand-written payload, one fire call and a test. The catalogue is `packages/sdk/std/events/catalogue.json`:

```json
{
  "name": "session.pit_stop",
  "since": "2.1.0",
  "status": "experimental",
  "kind": "notify",
  "thread": "session",
  "parent": "session",
  "payload": { "c": "SwoftyEvPitStop", "cpp": "swofty::sessions::events::PitStop", "header": "swofty/systems/sessions.hpp", "version": 1 },
  "context": { "session": "always", "player": "always", "participants": "always", "career": "in_career" },
  "source": { "how": "bus", "message": "MessagePitStateChanged", "binding": "Ne_Subscribe_BusMessage" },
  "doc": "A car stops in its pit box and leaves it."
}
```

```cpp
// swofty/systems/sessions.hpp
namespace swofty::sessions::events {
struct PitStop {
    static constexpr const char *point = "session.pit_stop";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    uint32_t car;            // vehicle index in the session
    float stationary_s;      // time in the box
    uint8_t tyres_changed;   // 0 or 1
};
}
```

```cpp
// native/src/core/session_events.cpp: the one fire call, from the pit state message
void on_pit_state_changed(const MessagePitStateChanged &m)
{
    if (m.state == PitState::LeavingPitBox) {
        swofty::sessions::events::PitStop e{ sizeof e, 1, m.car, pit_timer(m.car).stop(), tyres_changed(m.car) };
        swofty::events::fire(e);
    }
}
```

`tools/std/eventcheck.py` checks the rest in CI:

```text
ok   62 events, every name unique and dotted; released entries unchanged since v2.0.0
ok   swofty/systems/sessions.hpp: struct sessions::events::PitStop, point == "session.pit_stop", version == 1
ok   native/src/core: a fire of session.pit_stop (session_events.cpp:88)
ok   native/test: a test names session.pit_stop
ok   docs: a section for session.pit_stop
```

- **The KB first.** An event that changes the game's behaviour (`modify`, `cancel`) needs every binding it uses at L2 with an independent verify. A `notify` event from an L1 site is allowed, because it only reads.
- **Threads:** the core logs the thread of each event's first fire, and checks it against the catalogue:

```text
[events] first session.pit on thread 0x3a08 (flow thread 0x2f1c): catalogue says session, ok
```

## 1.x events keep working

Every 1.x event name still works, and `subscribe` attaches an `after` handler. The eight names 1.x accepted but never fired now fire, as aliases of their 2.0 events: `session.end`, `lap`, `flag`, `state`, `screen`, `pause`, `resume`, `results`.
