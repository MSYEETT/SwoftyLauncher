---
title: Compatibility
description: Why every 1.x addon keeps working on standard library 2.0, how each 1.x library maps onto registries and hooks, and the release gate that proves it.
---

# Compatibility

**No 1.x addon needs a change or a rebuild to work on 2.0.** Nothing a 1.x addon uses is removed or changes meaning, in 2.x or in any later version.

## A 1.x addon on 2.0

<Split>

The 1.x calls don't change. In 2.0, each 1.x library keeps its header and table, but stores what it's given in a registry or a hook point, in the calling addon's name.

- `swofty::era::add_box` provides a `content::era_boxes` record.
- `swofty::lang::set` writes the calling addon's global layer of a `ui::strings` record.

A 1.x call has the same visible result as before, and 2.0 addons can now see and change it.

<template #example>
<Example root="swofty.classic-seasons-2016" code-file="native/src/classic_seasons.cpp" files="swofty.json, native/build.sh, lang/en.json">
<template #code>

```cpp
// a 1.x addon, unchanged: built against the 1.x headers, API v8
#include <swofty/mixin.hpp>
#include <swofty/era.hpp>
#include <swofty/lang.hpp>

SWOFTY_ON_LOAD
{
    swofty::era::add_box({ .entrant_list = "cs_f1_2016", .title = "lng_cs_era_2016" });
    swofty::lang::set("lng_team_mer_title", "Mercedes-AMG Petronas", "*");
}

SWOFTY_ADDON()
```

</template>
</Example>
</template>
</Split>

What the 2.0 core does with those two calls, written as the 2.0 API:

```cpp
// provided in swofty.classic-seasons-2016's name: id "swofty.classic-seasons-2016/cs_f1_2016"
content::era_boxes.add("cs_f1_2016", { .entrant_list = "cs_f1_2016", .title = "lng_cs_era_2016", .image = {}, .order = 0 });

// swofty.classic-seasons-2016's global layer, in its load-order position
ui::strings.layer(ui::strings["game/lng_team_mer_title"], [](ui::String &s) {
    s.text["*"] = "Mercedes-AMG Petronas";      // "*": every language
});
```

```text
[registry] era.box swofty.classic-seasons-2016/cs_f1_2016: provided by swofty.classic-seasons-2016 (1.x era::add_box)
[registry] lang.string game/lng_team_mer_title: global layer by swofty.classic-seasons-2016 (1.x lang::set)
```

So a 2.0 addon can, for example, reorder or hide that box with a layer, without the 1.x addon knowing:

```cpp
content::era_boxes.layer(content::era_boxes["swofty.classic-seasons-2016/cs_f1_2016"], [](content::EraBox &b) {
    b.order = 2016;
});
```

Two addons setting the same string key used to be "last call wins" with no trace. In 2.0, load order decides, and the core's conflicts report lists it. For a single addon the result is identical.

**The rest:**

- **Career contents:** registering a content provides a `career::contents` record.
- **Team HQ tabs:** the `hub_tab` manifest field provides a `hub.tab` record.
- **Career wizard:** each call writes the addon's global layer of its data group's `career::wizards` record.
- **Characters:** each call writes a layer of the character's `career::characters` record.
- **Events:** `subscribe` attaches an `after` handler to the event's hook point.
- **Settings:** options become `addon.option` records, which the launcher and ADDON SETTINGS already edit.
- **Storage:** unchanged. An addon's own files stay private.
- **Mixins:** unchanged. They chain with hook points (see [Living with mixins](/std/hooks#living-with-mixins)).

## What stays exactly as it is

- **`SwoftyApi`:** append-only. Version 9 adds `registry`, `hooks` and `services` after `era`, so a check like `api->version >= 8` still passes.
- **Exported tables:** `SwoftyCoreHub`, `SwoftyCoreHud`, `SwoftyCoreEra`, `SwoftyCoreCareerContent` and the rest keep their names, layouts and versions.
- **Result codes, feature gates and `available()`** don't change.
- **Loading:** a 1.x DLL has no `SwoftyAddonRegister`, so it keeps loading at the stage its manifest says.
- **Mixins:** chains, priorities, load order and the conflict rules don't change.
- **Manifests:** every 1.x field keeps its meaning, and `schema_version` stays 1.
- **Builds:** 2.0 records never change what the launcher builds: the core applies them in game. A profile of 1.x addons builds to the same game files, with the same build hash. Players aren't forced to rebuild.

## Hook points can't drop a 1.x addon

- The core installs a game hook point's mixin only when a 2.0 addon subscribes to it. With no subscriber, the game function is patched exactly as in 1.x.
- When it is installed and collides with a 1.x addon's overwrite, the 1.x rule applies: the core's set is dropped and the 1.x addon keeps running. Only the 2.0 subscribers lose the point, and they're told (see [Hooks](/std/hooks#living-with-mixins)).

## Progressive Career's exports

Progressive keeps `ProgressiveRealTeam`, `ProgressiveFollowTeam`, `ProgressiveFollowMode` and `ProgressiveNamesYear` as `GetProcAddress` exports for all of 2.x, next to the `progressive.history` service.

## The release gate

2.0.0 can't be tagged until three checks pass. Each later 2.x release runs them again.

**1. Every addon on the Addon Browser, on both cores.** For every published version whose launcher range includes 1.5:

```text
tools/compat/run.sh
  for each addon version on the Addon Browser:
    download it
    build a profile with only it (plus its requirements) on the 1.5 core  -> files-1.5/, build.json
    build the same profile on the 2.0 core                                 -> files-2.0/, build.json
    compare: the game files byte for byte, and the build hash
    start each build in the Wine harness to the main menu                  -> loader-1.5.log, loader-2.0.log
    compare: the DLLs loaded, the dropped DLLs (none new), the mixin chains (identical apart from
             2.0 hook points, which only appear with a 2.0 subscriber), each addon's start lines
  report: one line per addon version, pass or the first difference
```

**2. Shim tests.** For each 1.x table, a fixture addon makes every call, and the test compares the resulting registry records and game-side effects with what 1.5 produced.

**3. In game.** The existing checks run on the 2.0 core with the 1.x builds of those addons:

- a Progressive Career 1.2 career start (Follow your driver, 2016) to Team HQ;
- a Classic Seasons era box to Time Trial;
- a Team HQ tab;
- a wizard route.

## Nothing is removed

- Every 1.x table, call, manifest field and mixin rule stays, in 2.x and in every later version.
- When 2.0 has a better way, the 1.x page says so and links to it, and the old way keeps working. Players never see a warning about it.
