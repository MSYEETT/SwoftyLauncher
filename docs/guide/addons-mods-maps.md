---
title: Addons, mods and maps
description: The three kinds of content Swofty Launcher for F1 25 loads, and how profiles and ADDON SETTINGS fit around them.
---

# Addons, mods and maps

Swofty Launcher for F1 25 loads three kinds of content into F1 25.

| | Addons | Mods | Maps |
|---|---|---|---|
| What it is | Code, settings and in-game UI | Content: ERPs and SERPs | Community tracks |
| Contains code | Yes, native plugins | No | No |
| Package | A folder with `swofty.json` | `.serps` packages, mod zips and folders | A track package zip or a track `.serps` |
| Reaches the game through | The launcher's build and SwoftyMod | An addon | The Team Simplified addon, as a mod |
| Turned on | Per profile, on the **Addons** page | Per profile, on the **Mods** page | Per profile, on the **Mods** page |
| In game | A page in **ADDON SETTINGS** | Under its addon | A page in **ADDON SETTINGS** |

## Addons

An addon is a folder with a [`swofty.json`](/reference/addons/manifest) manifest at its root. It can carry:

- **Native plugins.** Windows DLLs that use the **Advanced Mod Mixin System** to edit the F1 25 engine directly: wrap a game function, run code at an instruction, replace a virtual method, redirect a call or patch bytes. Plugins bind to game functions by name from the [Game API](/reference/game/), and SwoftyMod finds each one by signature when the game starts. See [Hooking a game function](/reference/addons/hooking).
- **Settings.** Toggles, sliders and choices, shown in the launcher and in game. See [the `options` field](/reference/addons/manifest#options).
- **In-game screens.** Screen files that SwoftyMod registers with the game's own UI system.
- **Content.** [Asset ops](/reference/addons/asset-api): database edits, archive resources, text, asset groups and loose files, applied when the launcher builds the profile or while the game boots.

Two addons ship with the launcher's source: **SwoftyMod** (`swofty.core`), which is built in and always on in modded profiles, and **Custom Grid Size** (`swofty.custom-grid-size`), which changes how many cars race in F1 2026.

## Mods

A mod is content with no code: `.erp` archives and resources, or `.serps` packages made with Simplified Studio. Mods reach the game in three ways.

| Delivery | How |
|---|---|
| Bundled | An addon ships the mod and applies it with [asset ops](/reference/addons/asset-api) in its manifest |
| Asset API | An addon produces asset ops from its own code, such as a library of mods it imported ([Asset API](#asset-api)) |
| Mods page | The player imports, orders and turns mods on and off in the launcher's **Mods** page, from the built-in Team Simplified addon ([Team Simplified](#team-simplified)) |

### Asset API {#asset-api}

An addon declares asset ops in `swofty.json` or returns them from a build hook: replace or add resources in game archives, edit database records, set text, register asset groups and place loose files. swofty_core applies them in memory while the game boots, and the game folder keeps its stock files. See the [Asset API](/reference/addons/asset-api) and [Loading mods at runtime](/reference/addons/runtime-assets).

### Team Simplified {#team-simplified}

The launcher's **Mods** page holds the mod library. Drop `.serps` packages, mod zips, mod folders or track zips anywhere on the launcher window, or use **Import…**. Each profile keeps its own order and switches, and a mod lower in the list wins. `swofty mods` does the same from the command line. See [Mods and tracks](/using/mods).

The launcher compares each mod with the clean game files and applies only what each enabled mod changes: database records, resources inside archives, text entries and whole files. A mod's expanded row lists what it changes and which other mods it overrides or is overridden by.

## Maps

A map is a community track, such as Sepang or Okayama. Tracks are mods: drop a track package zip or a track `.serps` on the launcher window, or import it on the **Mods** page, and turn it on per profile. It then appears in Grand Prix track selection. There is no Maps page, and `swofty maps` is a hidden alias of `swofty mods`. Tracks that earlier versions kept in `$DATA/maps/` move into the library. See [Tracks](/using/mods#tracks).

## Profiles

A [profile](/using/profiles) keeps one setup: its addons in load order, its mods and tracks in load order, and every addon setting. Switch profiles to switch setups. `vanilla` is the built-in profile with nothing installed. [Restoring the game](/using/profiles-and-restoring) returns every game file to its original state.

## ADDON SETTINGS

SwoftyMod adds **ADDON SETTINGS** to the game's home menu, directly under **Career**. It lists every enabled addon and track as a tile, and each tile opens a page with that addon's settings. Changes save to the profile. See [In-game addon settings](/using/swofty-mod).

::: tip Making your own
See [Creating mods](/reference/mods/) and [Creating addons](/reference/addons/). The [Standard library](/reference/std/) lists every API an addon can use.
:::
