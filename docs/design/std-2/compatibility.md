---
title: Compatibility
description: Why every 1.x addon keeps working on standard library 2.0, how each 1.x library maps onto registries and hooks, and the release gate that proves it.
---

# Compatibility

**No 1.x addon needs a change or a rebuild to work on 2.0.** Nothing a 1.x addon uses is removed or changes meaning in any 2.x release.

## What stays exactly as it is

- **`SwoftyApi`:** append-only. Version 9 adds `registry`, `hooks` and `services` after `era`, so a check like `api->version >= 8` still passes.
- **Exported tables:** `SwoftyCoreHub`, `SwoftyCoreHud`, `SwoftyCoreEra`, `SwoftyCoreCareerContent` and the rest keep their names, layouts and versions.
- **Result codes, feature gates and `available()`** don't change.
- **Mixins:** chains, priorities, load order and the conflict rules don't change.
- **Manifests:** every 1.x field keeps its meaning, and `schema_version` stays 1.
- **Builds:** a profile that uses no 2.0 feature builds to the same game files, with the same build hash. Players aren't forced to rebuild.

## 1.x libraries on top of registries

In 2.0, each 1.x library keeps its header and table, but stores what it's given in a registry or hook point. A 1.x call has the same visible result as before, and 2.0 addons can now see and override it.

**Era picker boxes:** the 1.x call doesn't change:

```cpp
// a 1.x addon, unchanged
swofty::era::add_box({ .entrant_list = "cs_f1_2016", .title = "lng_cs_era_2016" });
```

In 2.0 that call provides a record in the calling addon's name:

```json
// era.box "cs_f1_2016", provided by swofty.classic-seasons-2016
{ "entrant_list": "cs_f1_2016", "title": "lng_cs_era_2016", "image": "", "order": 0 }
```

So a 2.0 addon can, for example, hide or reorder boxes with a layer, without the 1.x addon knowing.

**Strings:** 1.x `swofty::lang::set` writes the calling addon's global layer of `lang.string`:

```cpp
// a 1.x addon, unchanged
swofty::lang::set("lng_team_mer_title", "Mercedes-AMG Petronas", "*");
```

```text
2.0 internally: layer  lang.string "lng_team_mer_title"  {"*": "Mercedes-AMG Petronas"}  global  by the calling addon
```

Two addons setting the same key used to be "last call wins" with no trace. In 2.0, load order decides, and the launcher lists the conflict. For a single addon the result is identical.

**The rest:**

- **Career contents:** registering a content provides a `career.content` record.
- **Team HQ tabs:** the `hub_tab` manifest field becomes a static provider of `hub.tab`.
- **Career wizard:** each call writes the addon's global layer of its data group's `career.wizard` record.
- **Characters:** each call writes a layer of the character's `career.character` record.
- **Events:** `subscribe` attaches an `after` handler to the event's hook point.
- **Settings:** options become `addon.option` records, which the launcher and ADDON SETTINGS already edit.
- **Storage:** unchanged. An addon's own files stay private.
- **Mixins:** unchanged. They chain with hook points (see [Living with mixins](./hooks#living-with-mixins)).

## Hook points can't drop a 1.x addon

- The core installs a game hook point's mixin only when a 2.0 addon subscribes to it. With no subscriber, the game function is patched exactly as in 1.x.
- When it is installed and collides with a 1.x addon's overwrite, the 1.x rule applies: the core's set is dropped and the 1.x addon keeps running. Only the 2.0 subscribers lose the point, and they're told (see [Hooks](./hooks#living-with-mixins)).

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

## Deprecation

- A 1.x table or call stays supported for all of 2.x.
- Removing one needs a launcher 3.0, with its replacement documented, and at least one 2.x release in which the core logs, once per session, which DLL still uses it.
