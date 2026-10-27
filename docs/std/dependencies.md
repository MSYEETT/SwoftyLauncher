---
title: Dependencies and guards
description: Dependencies on addons, registries, services and single records such as tracks; what happens when one is missing; guards that turn the player back with a clear message; and careers that remember what they were made with, in standard library 2.0.
---

# Dependencies and guards

Content depends on other content. A 2016 season races at Sochi, which a track addon provides. A career made with R&D Tyres has Tyres upgrades in its save. 2.0 checks every dependency four times:

```text
at install      the Addon Browser offers what's needed and recommended
before Play     the launcher names what's missing, what's skipped, and which careers won't load
in game         a guard cancels the start, the load or the box press, with a popup that names the fix
after load      a career that loaded without its addons is stopped before it can save over its progress
```

**Optional at start, required once used.** A calendar round on a missing track can be skipped when a career starts. Once the career has raced at that track, the career needs it.

## Declaring what you need

`requires` in `registry.json` is an array, like the 1.x one in `swofty.json`. Each entry names an addon, a registry, a service, a hook point or one record:

```json
// swofty.classic-seasons-2016/registry.json
{
  "requires": [
    { "registry": "content.package", "version": "^1" },
    { "registry": "track", "version": "^1" }
  ],
  "recommends": [
    { "record": "track:venue/sochi",          "for": "the 2016 Russian Grand Prix" },
    { "record": "track:venue/hockenheimring", "for": "the 2016 German Grand Prix" },
    { "record": "track:venue/sepang",         "for": "the 2016 Malaysian Grand Prix" }
  ]
}
```

- **`requires`:** Play stops while one is missing, the same way it stops for a missing addon today.
- **`recommends`:** optional. It never stops Play. The Addon Browser offers it at install, and the launcher names it before Play when something is skipped because of it.
- **`for`:** shown to players. Keep it short and concrete.
- **A `record` entry** means "some enabled addon provides this record". Its `version` is a range on the providing addon's version.

## Tracks have stable identities

A track is a record its addon provides. Every real circuit also has a **venue** id the core owns, so content can ask for "Sochi" without caring which addon brings it:

```text
track
  game/catalunya               stock, playable
  game/sochi                   stock row, no assets (playable: false)
  mod.sochi/sochi              Sochi Autodrom: fills the game/sochi slot, provides venue/sochi
  mod.bahrain24/sepang         Sepang: fills the game/bahrain24 slot, provides venue/sepang
  mod.hockenheim/hockenheim    a new track code, provides venue/hockenheimring
venue/sochi, venue/sepang, venue/hockenheimring, venue/portimao, ...     one per real circuit
```

```json
// mod.sochi/registry/track.json. For a 1.x track addon, the launcher derives the same record from its track.add op.
{ "track": { "mod.sochi/sochi": { "code": "sochi", "slot": "game/sochi", "name": "Sochi Autodrom", "provides": ["venue/sochi"] } } }
```

- **A `venue/…` reference** resolves to the first enabled track that provides it, in load order. An exact id (`mod.sochi/sochi`) pins one addon.
- **Stat ids are pinned.** Today a new track's stat id depends on which other tracks are on and in what order, so a saved career's round can load a different track. In 2.0 the first id a track gets is kept for the install, in `$DATA/registry/track-ids.json`:

```text
[tracks] hockenheim: stat id 43 (pinned since 2026-10-27)
[tracks] okayama: pinned stat id 44 is taken by autopolis; using 45 and re-pinning (careers made with okayama at 44: 0)
```

## When a reference is missing

The registry's owner sets what may happen when a referenced record isn't there. The author of the referencing record picks within that:

```json
// the content.package schema: a calendar round's track
"track": {
  "type": "string",
  "x-ref": "track",
  "x-missing": { "default": "required", "allow": ["required", "optional", "fallback"], "drop": "element" }
}
```

```json
// the vehicle schema: a car's engine sound falls back to its donor's
"engine_sound": { "type": "string", "x-ref": "engine_sound", "x-missing": { "default": "fallback", "to": "/donor" } }
```

In a record, a reference is a string with two modifiers:

```json
// swofty.classic-seasons-2016's calendar
"calendar": [
  { "round": 4,  "race": "lng_gp_russia",   "track": "venue/sochi?" },
  { "round": 12, "race": "lng_gp_germany",  "track": "venue/hockenheimring?" },
  { "round": 16, "race": "lng_gp_malaysia", "track": "venue/sepang|game/bahrain" }
]
```

```text
"venue/sochi"                   required: the record doesn't resolve without it
"venue/sochi?"                  optional: drop the element (here the calendar round) when nothing provides it
"venue/sepang|game/bahrain"     fallback chain: the first one that resolves
```

Readers get the resolved value, never the modifiers. `swofty::registry::missing(registry, id)` lists what was dropped or replaced:

```text
[registry] content.package swofty.classic-seasons-2016/f1_2016: calendar round 12 dropped (venue/hockenheimring: no
           enabled track provides it; optional); round 16 races at mod.bahrain24/sepang
```

The player can change a policy from the launcher's Missing content panel ("Skip round 12", "Race it at Red Bull Ring"). That writes a profile layer, within what the schema allows.

## Guards

A guard stops a front-end action and tells the player why. It generalises `career::block`, which R&D Tyres uses to keep players out of My Team. Guards run at these points:

```text
career.start          a new-career route
career.load           a continue route
career.content.open   a career tile press
era.box.open          a GP/TT box press
route                 any front-end route, by name
```

**Declarative**, in `registry.json`:

```json
{
  "guards": [
    { "on": "career.start", "mode": "my_team",
      "block": { "title": "My Team not available",
                 "body": "R&D: Tyres & Thermal Management only works in Driver Career. To play My Team, turn the addon off in the launcher (or use a profile without it), then relaunch." } },
    { "on": "career.load", "mode": "my_team", "same_as": 0 },
    { "on": "career.content.open", "data_group": "f1_career_rnd_challenge",
      "require": [ { "ref": "addon:swofty.rnd-tyres-thermal", "version": ">=0.11.0", "for": "the Tyres R&D department" } ] }
  ]
}
```

**In code:**

```cpp
#include <swofty/guards.hpp>

SWOFTY_ON_LOAD
{
    using namespace swofty::guards;
    on(Point::career_content_open, [](const Target &t, Verdict &v) {
        if (t.data_group != "f1_career_rnd_challenge")
            return;
        if (!swofty::addons::on("swofty.rnd-tyres-thermal"))
            v.missing({ .name = "R&D: Tyres & Thermal Management", .id = "swofty.rnd-tyres-thermal",
                        .need = ">=0.11.0", .state = Missing::State::off, .for_ = "the Tyres R&D department" });
    });
}
```

- **`block`** cancels with your own title and body.
- **`missing`** cancels, and the core writes the popup from every missing item, in the same words everywhere.
- **The 1.x call keeps working.** `career::block(mode, title, body)` becomes a guard underneath, with the same log lines and popup.

The core also runs a **dependency guard** at every point: a career's dependencies on `career.load`, and record requirements on `career.content.open` and `era.box.open`. The popups it writes:

```text
Missing addons for this career
  This career was made with Sochi Autodrom (mod.sochi). Turn it on in the launcher, then load it again.
  The career hasn't been loaded, so none of its progress is lost.

Missing addons for this career
  This career was made with 2 addons that are off: Sochi Autodrom (mod.sochi) and R&D: Tyres & Thermal
  Management (swofty.rnd-tyres-thermal). Turn them on in the launcher, then load it again.

Missing addons for this career
  This career was made with Sochi Autodrom 1.1.0 (mod.sochi), and 1.0.0 is installed. Update it in the
  launcher's Addon Browser, then load the career again.

F1 2017 (V10) can't start
  F1 2017 (V10) builds on F1 2017 GP/TT Addon (swofty.classic-seasons-2017), which is off. Turn it on in the
  launcher, then pick the box again.
```

```text
[guards] career.load: driver_career (continue, route 'solo_career_continue_driver', career
         f1_career_progressive/pg-1849223-max_verstappen); blocked by deps: mod.sochi off
[guards] route 'solo_career_continue_driver' cancelled; the player stays on the menu and the popup is queued
[guards] era.box.open: mod.esp/f1_2017_v10 blocked by deps: content.package swofty.classic-seasons-2017/f1_2017
         unresolved (swofty.classic-seasons-2017 off)
```

## Careers remember what they were made with

Every career gets a dependency record, written when it starts and again at each Team HQ visit after a change. It's saved in the career's own `swofty_registry` container, and mirrored to `<game>/swofty/careers/index.json` for the launcher:

```json
// career.deps "f1_career_progressive/pg-1849223-max_verstappen"
{
  "label": "Progressive Career: Max Verstappen, 2016, season 1",
  "match": { "driver": 4103, "team": 2, "season_index": 0 },
  "made_with": { "launcher": "2.0.0", "core": "0.3.0", "game_build": 25251167 },
  "addons": [
    { "id": "swofty.progressive", "version": "2.0.0", "need": "^2.0.0", "why": "career content f1_career_progressive" },
    { "id": "mod.sochi",          "version": "1.1.0", "need": "^1.1.0", "why": "calendar round 4: track mod.sochi/sochi" }
  ],
  "records": [
    { "ref": "track:venue/sochi", "resolved": "mod.sochi/sochi", "stat_id": 18, "used": "calendar round 4" }
  ],
  "dropped": [
    { "ref": "track:venue/hockenheimring", "policy": "optional", "at": "start", "for": "calendar round 12" }
  ]
}
```

- **Which addons go in:** every addon whose records the career uses, plus any addon that says so. A new `saves.careers` field in `swofty.json`, or a code call:

```json
// swofty.rnd-tyres-thermal/swofty.json: new field, next to the 1.x ones
"saves": {
  "affects": true,
  "message": "A Driver Career played with Tyres R&D loses its Tyres upgrades if it's saved without the addon.",
  "careers": "loaded"
}
```

```cpp
#include <swofty/deps.hpp>

swofty::deps::used("swofty.rnd-tyres-thermal", "the Tyres R&D department");    // this career now needs the addon
swofty::deps::used_record("track", "mod.hockenheim/hockenheim", "calendar round 12");
```

- **`dropped`** remembers what was skipped at the start. Turning Hockenheim on later doesn't make the career need it, and the launcher can say "this career skips Hockenheim".

## Before Play

```text
Missing content
  F1 2016 GP/TT Addon: the German Grand Prix (round 12) is left out, because no track provides Hockenheim.
  The Malaysian Grand Prix (round 16) races at Sepang (mod.bahrain24).
  [Install Hockenheim]

  2 careers won't load in this profile:
    Progressive Career: Max Verstappen, 2016: needs Sochi Autodrom (mod.sochi), which is off.
    Driver Career: Lando Norris, season 3: needs R&D: Tyres & Thermal Management, which is off.
  [Turn on Sochi Autodrom]  [Turn on R&D: Tyres & Thermal Management]
  The launcher backs up your saves before this session.
```

- **What stops Play:** a missing `requires` entry, as for a missing addon today.
- **What doesn't:** a reference inside a record. That leaves the record unresolved, which hides its era box or tile, or guards it.
- **Install buttons** appear when the Addon Browser knows a published addon that provides the record.

## Turning an addon off

```text
Sochi Autodrom is in 1 career
  Progressive Career: Max Verstappen, 2016 races at Sochi in round 4. That career won't load until Sochi
  Autodrom is back on. Your other careers aren't affected.
  [Turn off]  [Cancel]
```

`swofty addons disable mod.sochi` prints the same text and needs `--yes`. `swofty careers` lists every career and what it needs, checked against a profile.

## After load: the backstop

Resume Career is checked before the load, from the career index. A career picked from the save-slot list is identified only once it has loaded, until the slot press is reverse-engineered. If it loaded with a required addon missing:

1. The core shows the popup: quit to the menu without saving, turn the addon on, load again.
2. A **save fence** writes back the bytes this career had at load in place of the new ones, so even a save can't drop progress. This needs the career's position inside its save container to be reverse-engineered, and is planned for 2.1.
3. Until then, the launcher's save backup before the session is the safety net. It covers every addon a career in the index depends on, not only `saves.affects` addons.

## The Addon Browser

The server derives what each addon version **provides** (registry records, tracks with their venues, services) and **needs**. It serves both:

```text
GET /v1/provides?record=track:venue/sochi       -> [ { "addon": "mod.sochi", "version": "1.1.0", "record": "mod.sochi/sochi" } ]
GET /v1/addons/<id>/versions/<v>/tree           -> the full dependency tree with providers
```

- **At install:** a needed record resolves to an addon. If one addon provides it, it installs like an addon requirement. If several do, the player picks.
- **When nothing provides it:** "No addon on the Addon Browser provides Hockenheim yet. F1 2016 races without the German Grand Prix."
- **At publish:** the server suggests dependencies the author forgot.
- **1.x launchers:** the `requires` the API serves them is unchanged.
