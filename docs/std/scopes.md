---
title: Scopes and snapshots
description: Global, profile and career scopes, and the career snapshot that freezes a career's data, in standard library 2.0.
---

# Scopes and snapshots

Every [layer](/std/layers) has a **scope**: how long it lasts and what it applies to.

<Split>

Tails' "what if" addon uses all three scopes on Progressive Career's 2017 season:

- **global:** a change that applies wherever the addon is on, in every profile;
- **profile:** the grid the player edited on the addon's launcher page, for this launcher profile only;
- **career:** a swap made by the in-game editor, saved with the career that's loaded.

A scope is the last argument of `layer`. It defaults to `Scope::global`.

<template #example>
<Example root="tails.whatif-history" code-file="native/src/whatif.cpp" data-file="registry/layers.json" files="swofty.json, registry.json, native/whatif.dll, bin/whatif.exe">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/storage.hpp>
#include <swofty/systems/career.hpp>
#include <swofty/systems/drivers.hpp>

using namespace swofty;

static Ref<drivers::Identity> driver(std::string_view id) { return drivers::identities[id]; }

SWOFTY_REGISTER
{
    auto y2017 = career::seasons["swofty.progressive/2017"];

    // global: every profile the addon is on in
    career::seasons.layer(y2017, [](career::Season &s) {
        s.teams.at(1).seats = { driver("game/fernando_alonso"), driver("game/valtteri_bottas") };
    });

    // profile: what the launcher page saved for the active profile
    std::string text;
    if (storage::read("profile/whatif.json", text) == storage::Result::ok)
        for (const whatif::Seat &seat : whatif::parse(text))
            career::seasons.layer(career::seasons[seat.season], [seat](career::Season &s) {
                s.teams.at(seat.slot).seats[seat.seat] = driver(seat.driver);
            }, Scope::profile);
}

// career: the in-game editor, while a career is loaded
void swap_from_round(int round)
{
    career::seasons.layer(career::seasons["swofty.progressive/2017"], [round](career::Season &s) {
        career::SeasonChange &c = s.changes.at(round, 1, 1);   // round, slot 1, seat 1; appended when missing
        c.out = driver("game/valtteri_bottas");
        c.in = driver("game/esteban_ocon");
        c.kind = career::ChangeKind::swap;
    }, Scope::career);
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// a data file is always a global layer
{
  "career.season": {
    "swofty.progressive/2017": {
      "teams": [ { "slot": 1, "seats": ["game/fernando_alonso", "game/valtteri_bottas"] } ]
    }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: tails.whatif-history (whatif.dll) 3 layers on career.season (early stage, 0.3 ms)
[registry] career.season swofty.progressive/2017: edit by tails.whatif-history (global) changed /teams/slot=1/seats
[registry] career.season swofty.progressive/2017: edit by tails.whatif-history (profile) changed /teams/slot=4/seats/0
[registry] layer from tails.whatif-history on career.season swofty.progressive/2017 registered at run time; applies from the next session load
```

```cpp
enum class Scope {
    global,   // every profile the addon is on in; the addon's code and its data files
    profile,  // one launcher profile; the addon's code applies what its launcher page saved
    career,   // one career; written in game while that career is loaded, saved with it
};
```

## Global

A global layer applies wherever the addon is on. Layers in `SWOFTY_REGISTER` and every data file are global unless they say otherwise. `SWOFTY_REGISTER` runs at every game start, so a layer there is on for as long as the addon is.

A global layer added later, at run time, lasts until the addon clears it or the game closes:

```cpp
career::seasons.layer(y2017, give_alonso_the_mercedes);   // from SWOFTY_ON_LOAD or later: this session
career::seasons.clear_layer(y2017);                         // Scope::global by default
```

## Profile

A profile layer belongs to one launcher profile. Players can keep a "what if" profile and a real-history profile side by side, and switch between them in the launcher.

The launcher doesn't know registrations. The addon's launcher page saves the player's edits in the addon's own per-profile state, and the addon's build step puts them in its folder in the game. The addon's code reads that file and applies it with `Scope::profile`:

```text
the active profile's state for tails.whatif-history              written by the launcher page (set_state)
<game>/swofty/addons/tails.whatif-history/profile/whatif.json    put there by the addon's build step,
                                                                 read in SWOFTY_REGISTER with swofty::storage
```

The page side is in [The launcher side](/std/launcher#editing-a-layer-from-a-launcher-page).

- The profile layer changes when the player edits the page and plays again. In game, an addon doesn't rewrite it: a change made in game uses `global` (this session) or `career` (this career).
- Within one addon, its `global` layer applies before its `profile` layer.
- A data file can't be profile-scoped. Per-profile data needs code that reads it.

## Career

A career layer belongs to the career that's loaded, and is saved with it. Use it for changes made inside a career, for example an in-game editor that moves a driver mid-season: `swap_from_round(8)` above puts Ocon in the Mercedes from round 8, in this career only.

With no career loaded, the layer is refused with `late`:

```text
[registry] career.season swofty.progressive/2017: career layer from tails.whatif-history refused (late): no career is loaded
```

## Career snapshots

A registry declared with `.snapshot = true` is frozen into every new career. `career.season` and `career.driver_ratings` are.

<Split>

While a career is loaded:

- `get` returns **snapshot + career layers** for snapshot registries. Global and profile layers added after the career started don't reach it.
- Registries without a snapshot keep reading live: base plus every layer.
- [Watchers](/std/reading#watching) fire once for every snapshot record when the career loads, and again when it unloads.

An addon's own registry takes the same options.

<template #example>
<Example root="tails.whatif-history" code-file="native/src/presets.cpp" files="swofty.json, native/src/whatif.cpp, native/whatif.dll">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/career.hpp>

using namespace swofty;

// the core's declarations, in swofty/systems/career.hpp:
//   inline Registry<Season> seasons{"career.season", { .applies = Applies::read, .snapshot = true, .snapshot_budget = 65536 }};
//   inline Registry<DriverRatings> ratings{"career.driver_ratings", { .applies = Applies::read, .snapshot = true, .snapshot_budget = 65536 }};

// an addon's own registry, frozen into each career too
struct Preset {
    std::string name;
    int start_year = 2016;
    static constexpr Version version{1, 0};
};
SWOFTY_RECORD(Preset, name, start_year);
inline Registry<Preset> presets{"tails.whatif.presets", { .snapshot = true, .snapshot_budget = 16384 }};

SWOFTY_ON_LOAD
{
    // fires for every snapshot record when a career loads, and again when it unloads
    static Token watch = career::seasons.watch([](Ref<career::Season> id) {
        if (id.id().empty())
            return;                                   // the end of the batch
        if (auto s = career::seasons.get(id))         // the snapshot, plus this career's layers
            swofty::log("[whatif] %.*s: %zu teams", int(id.id().size()), id.id().data(), s->teams.size());
    });
}

SWOFTY_ADDON()
```

</template>
</Example>
</template>
</Split>

```text
new career, data group f1_career_progressive
  career.created
    core: snapshot career.season         34 records    41 KB -> 9 KB compressed
    core: snapshot career.driver_ratings 412 records   88 KB -> 17 KB compressed
    core: saved in the career's swofty_registry container
  ...
  player edits the what-if page in the launcher (the profile layer changes)
  -> this career still reads the snapshot; careers started after the edit get the new grid

load the career again later
  career.loaded
    core: career.season and career.driver_ratings read from the snapshot (schema 1.0, current 1.1: read as is)
```

The snapshot lives in a core-owned save container, `swofty_registry`, inside the profile save, next to the containers career contents keep. An addon that isn't installed when the save is written doesn't lose its records: the save splice keeps containers it doesn't recognise.

### Budget

The game's save heap is small: a full `SAVE_DATA` heap crashed a 1.5 test build. So each snapshot registry has a budget, `.snapshot_budget`, in bytes after compression, per career.

- When a registry's snapshot is over its budget, the core doesn't save it, logs the sizes, and that career reads the registry live.
- A whole career's `swofty_registry` container is capped at 256 KB.

```text
[registry] career.driver_ratings: snapshot 71 KB compressed, budget 64 KB; not saved, this career reads it live
```

### Migration

A snapshot keeps the schema version it was written with. On load:

- **Same major, older or newer minor:** read as is. Fields the reader doesn't know are kept (see [the tolerant reader rule](/std/versioning#the-tolerant-reader-rule)).
- **Older major:** the registry owner's `migrate` converts it. Without one, the core upgrades it through the version [adapter](/std/versioning)'s `up`, and logs that it did.

```cpp
// the core's migration for career.season 1.x -> 2.0 (an example: 2.0 turns each seat into a driver and a number)
swofty::migrate(career::seasons, career::v2::seasons, [](career::Season v1) {
    career::v2::Season v2{ .data_group = v1.data_group, .year = v1.year };
    for (const career::SeasonTeam &t : v1.teams) {
        career::v2::SeasonTeam &team = v2.teams.at(t.slot);
        team.team = t.team;
        for (std::size_t i = 0; i < t.seats.size(); ++i)
            team.seats[i] = { .driver = t.seats[i], .number = std::nullopt };
    }
    v2.changes = v1.changes;
    return v2;
});
```
