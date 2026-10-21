---
title: Career
description: Track F1 25 front-end flow states and the running career, reach its components through the game's registry and post on the game bus with swofty/career.hpp.
---

# Career

The core watches the game's front-end flow states and the championship entity, so you don't need hooks of your own to know whether a career is loaded, in which mode, and whether Team HQ is idle. It publishes those facts as [events](./events#flow-and-career-events) and answers the same questions through getters for an addon that subscribes late. In a loaded career it also gets the championship entity's components through the game's own registry, and posts messages on the game's message bus. An addon that supports only one career mode can block the other: the core then keeps the player from starting or loading a career in that mode.

| | |
|---|---|
| Header | `swofty/career.hpp` |
| Namespace | `swofty::career` |
| Table | `SwoftyApi::career` (`SwoftyCareer`) |
| Feature | `career` — stable, always on |
| API version | 6 (core 0.2.11); `block`, `unblock` and `career.starting` 7 (core 0.2.12) |
| Thread | Getters, `block`, `unblock`: any. `entity`, `component`, `post`: flow thread |

Every call that returns a `swofty::career::Result` uses the shared [result codes](./#result-codes).

## Calls

| Call | Returns |
|---|---|
| `available()` | `true` when the core tracks the flow states on this build; settled before any `SWOFTY_ON_LOAD` runs (in `SwoftyAddonInit` it is `true` when this core can track them) |
| `state(State *out)` | `ok` with the current `SwoftyCareerState` (also when no career is loaded) |
| `flow(Flow *out)` | `ok` with the last flow transition; `late` before the first |
| `flow_status(hash or name, Status *status, uint64_t *age_ms = nullptr)` | `ok` with a state's status and how long it has held it; `invalid` for a state the core has not seen |
| `entity()` | the championship entity (the career entity in a career), or `nullptr` |
| `component(type)` | a component of the entity, or `nullptr`; `type` is the class name (`"CareerBankComponent"`) or the descriptor's binding id (`"k_Career_CareerBankComponentDesc"`) |
| `component_by_desc(const void *desc)` | the same, by the address of the component's static type descriptor |
| `post(void *msg)` | `ok` once every subscriber has handled the message; `late` when the bus is missing or inactive; `invalid` for a message without a vtable in the game image |
| `block(mode, title = {}, body = {})` | `ok`: from now on the core cancels starting or loading a career in `mode` and shows the [popup](./popup) with `title` and `body`; `invalid` for `Mode::none`; `unavailable` on a core older than API 7 |
| `unblock(mode)` | `ok` once this addon's block on `mode` is lifted; `invalid` when it held none |
| `state_hash(name)` | the djb2 hash the game keys flow states by (constexpr) |
| `name(Mode)`, `name(Status)`, `name(Start)`, `name(Result)` | `"my_team"`, `"Updating"`, `"pick"`, `"ok"` and so on |

`component` resolves `k_Career_<Class>Desc` from the bindings, so it finds every component class the KB has a descriptor for. It calls the registry the game's own components use: `get(registry, &desc, 1)` at vtable slot `+0x10` of the registry embedded in `CareerEntity`. `post` hands your message to `Ne_Post_BusMessage`, which runs every subscriber before it returns. Build the message exactly as the game's own posters do.

## The career state

```c
typedef struct SwoftyCareerState {
    uint32_t loaded;          // a career is loaded
    uint32_t mode;            // SWOFTY_CAREER_NONE, _MY_TEAM (1), _DRIVER (2)
    uint32_t hub;             // Team HQ is idle for the mode
    uint32_t humans;          // human entrants in the championship
    uint32_t player_driver;   // the human entrant's packed driver key, when there is one human
    uint32_t player_team;     // and its packed team key
    uint32_t has_player;
    uint32_t components;      // SWOFTY_CAREER_COMPONENTS_UNSEEN, _LIVE, _FALLBACK
    uint32_t component_mode;  // what the components say
    uint32_t team_hq_mode;    // what Team HQ answered when it opened
    uint32_t flow_career;     // the flow states alone say a career is up
    uint32_t reserved;
    char key[32];             // "mt-<team>-<driver>" or "dc-<driver>", "" until the player is known
} SwoftyCareerState;
```

The mode comes from the components first: a `CareerComponent` means a loaded career, `CareerBankComponent` or `MyTeamFacilitiesComponent` means My Team, and `DriverCareerPlayerDriverStatisticsComponent` means Driver Career. When the components have no answer, the core uses what Team HQ answered as it opened (`Career_Is_MyTeamGameMode`, read at `site_Career_TeamHqIsMyTeamResult`). Both career modes use the same `StateTeamHQ` and `StateTeamHQUpdate` states. If Team HQ has been up for 4 seconds and the components still show no career, `components` turns to `_FALLBACK` and the flow states decide whether a career is loaded.

`career.loaded` fires once the mode is known. A change of mode or career key while a career is loaded fires `career.unloaded` and then `career.loaded`.

## Blocking a career mode

The Career menu starts and loads careers through fixed front-end routes. The core runs every route through a wrap on `Vm_Run_Route`, the game's route runner, and knows these:

| Route | Screen | Mode | `Start` |
|---|---|---|---|
| `career_dlc_select_my_team`, `career_dlc_select_driver` | `fe_career` | My Team, Driver Career | `pick`: the season or save choice comes next |
| `career_wizard_my_team_career`, `career_wizard_my_team_season_length` | `fe_career`, `fe_career_dlc_select`, `career_continue_my_team` | My Team | `new_career` |
| `career_wizard_driver_career`, `career_wizard_driver_career_to_f1` | `fe_career`, `fe_career_dlc_select`, `career_continue_driver` | Driver Career | `new_career` |
| `solo_career_continue_my_team`, `solo_career_continue_driver` | `fe_career`, `fe_career_dlc_select` | My Team, Driver Career | `continue_career` |
| `to_manage_career_my_team`, `to_manage_career_driver` | `career_wizard_career_type_select` | My Team, Driver Career | `continue_career` |

For each of them the core fires `career.starting` (see [events](./events#flow-and-career-events)) with a `SwoftyCareerStart`:

```c
typedef struct SwoftyCareerStart {
    uint32_t mode;      // SWOFTY_CAREER_MY_TEAM (1) or SWOFTY_CAREER_DRIVER (2)
    uint32_t kind;      // SWOFTY_CAREER_START_PICK (0), _NEW (1), _CONTINUE (2)
    uint32_t blocked;   // a block cancelled the route
    uint32_t reserved;
    char route[64];
} SwoftyCareerStart;
```

When an addon holds a block on the route's mode, the core returns from the wrap without running the route. No step of the route has run, so the screen the player pressed on stays open and nothing of the chosen career loads. The core then queues the blocking addon's popup, at most once every 1.5 seconds per mode. A block with an empty title and body cancels without a popup. If the popup library is unavailable, the route is still cancelled and the core log says so.

A block lasts for the session. Each addon holds at most one block per mode; calling `block` again replaces its text. When several addons block the same mode, the first one's popup shows. A block goes away with `unblock`, or when the addon's `SwoftyAddonInit` fails.

```cpp [native/src/my_plugin.cpp]
#include <swofty/career.hpp>

SWOFTY_ON_LOAD
{
    auto r = swofty::career::block(swofty::career::Mode::my_team, "My Team not available",
                                   "This addon works in Driver Career only.");
    if (r != swofty::career::Result::ok)
        swofty::log("[my-addon] My Team is not blocked: %s", swofty::career::name(r));
}
```

The core log shows each block and each cancelled route:

```
[career] my-addon blocks my_team careers; the start gate is live
[career] career.starting: my_team (pick, route 'career_dlc_select_my_team'); blocked by my-addon
[career] route 'career_dlc_select_my_team' cancelled; the player stays on the menu and my-addon's popup is queued
```

## Flow states

```c
typedef struct SwoftyFlowState {
    uint32_t hash;          // djb2 of the state name
    uint32_t from, to;      // SWOFTY_FLOW_NONE, _ENTERING, _UPDATING, _EXITING
    uint32_t named;         // name is the real state name
    uint64_t transitions;   // transitions seen so far
    char name[64];          // "StateTeamHQUpdate", or "#0e245469" when unreadable
} SwoftyFlowState;
```

The core installs one read-only site, `site_FeFlow_SetStateStatus`, in the setter every flow-state change goes through. It tracks up to 256 distinct states. The game keeps state names XOR-encoded until it decodes them, so the core decodes a copy itself.

## Example

```cpp [native/src/my_plugin.cpp]
#include <swofty/career.hpp>
#include <swofty/events.hpp>

static void on_hub(const SwoftyEvent *ev, void *)
{
    auto *s = static_cast<const SwoftyCareerState *>(ev->data);
    if (!ev->a)
        return;
    swofty::log("[my-addon] Team HQ idle in %s, career %s", swofty::career::name(swofty::career::mode(*s)), s->key);
    if (void *bank = swofty::career::component("CareerBankComponent"))
        swofty::log("[my-addon] bank at %p", bank);
}

SWOFTY_ON_LOAD
{
    if (!swofty::career::available())
        return;
    swofty::events::subscribe(SWOFTY_EVENT_CAREER_HUB, on_hub);
    swofty::career::State s{};
    if (swofty::career::state(&s) == swofty::career::Result::ok && s.hub)
        swofty::log("[my-addon] already in Team HQ");
}

SWOFTY_ADDON()
```

## Availability

With `SWOFTY_UI_DIAG=1` the core also logs every other route it runs (`[career] route 'open' (not a career start)`).

The start gate needs `Vm_Run_Route`, found by its signature (`vm.run_route`). On a build where it does not match, `career.starting` never fires and `block` still returns `ok`, but no route is cancelled; the core log's `[career] start gate:` line says so.

The library needs the flow-state site. On a build where it does not resolve, the core drops only that site, `available()` returns `false` and every call returns `unavailable` or `nullptr`; the rest of the core keeps working. The Team HQ answer site, each component descriptor, `g_Game_Manager`, `g_Game_MessageBus` and `Ne_Post_BusMessage` are looked up one at a time when first needed, and a missing one fails only the calls that need it. A core built without the KB structs it reads (`FlowState`, `CareerEntity`, `EntrantRecord`, `GameMessageBusHolder`) reports the matching parts unavailable. The core log's `[career] setup:` line lists what is live.

Requires `swofty.core` 0.2.11 or later (0.2.12 for `block`, `unblock` and `career.starting`). Declare it in your manifest:

```json [swofty.json]
"requires": [{ "id": "swofty.core", "version": ">=0.2.11" }]
```
