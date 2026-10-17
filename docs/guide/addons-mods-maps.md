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
| Package | A folder with `swofty.json` | `.erp` archives, `.serps` packages | A track package zip |
| Reaches the game through | The launcher's build and SwoftyMod | An addon | The launcher's map installer |
| Turned on | Per profile, on the **Addons** page | With the addon that carries it | Per profile, on the **Maps** page |
| In game | A page in **ADDON SETTINGS** | Under its addon | A page in **ADDON SETTINGS** |

## Addons

An addon is a folder with a [`swofty.json`](/modding/manifest) manifest at its root. It can carry:

- **Native plugins.** Windows DLLs that use the **Advanced Mod Mixin System** to edit the F1 25 engine directly: wrap a game function, run code at an instruction, replace a virtual method, redirect a call or patch bytes. Plugins bind to game functions by name from the [Game API](/reference/game/), and SwoftyMod finds each one by signature when the game starts. See [Hooking a game function](/tutorials/hooking).
- **Settings.** Toggles, sliders and choices, shown in the launcher and in game. See [the `options` field](/modding/manifest#options).
- **In-game screens.** Screen files that SwoftyMod registers with the game's own UI system.
- **Content.** [Database edits](/modding/database-ops) and [ERP edits](/modding/erp-ops), applied when the launcher builds the profile.

Two addons ship with the launcher's source: **SwoftyMod** (`swofty.core`), which is built in and always on in modded profiles, and **Custom Grid Size** (`swofty.custom-grid-size`), which changes how many cars race in F1 2026.

## Mods

A mod is content with no code: `.erp` archives and resources, or `.serps` packages made with Simplified Studio. Mods reach the game in three ways.

| Delivery | How | Status |
|---|---|---|
| Bundled | An addon ships the mod and applies it with [ERP](/modding/erp-ops) and [database](/modding/database-ops) operations in its manifest | Available |
| Asset API | An addon loads the mod while the game runs: it inserts, replaces or removes resources and ERPs | <Badge type="info" text="coming soon" /> [Asset API](#asset-api) |
| Team Simplified panel | The player imports, orders and turns mods on and off in the launcher's **Team Simplified** addon panel | <Badge type="info" text="coming soon" /> [Team Simplified](#team-simplified) |

### Asset API {#asset-api}

<Badge type="info" text="coming soon" /> An addon loads mods while the game runs.

### Team Simplified {#team-simplified}

<Badge type="info" text="coming soon" /> The player manages `.serps` mods in the launcher.

## Maps

A map is a community track package, such as Sepang or Okayama. The launcher imports the package, stores it in `$DATA/maps/` and installs it into a track slot when a profile uses it. See [Importing maps](/using/importing-maps).

## Profiles

A [profile](/using/profiles) keeps one setup: its addons in load order, its maps and every addon setting. Switch profiles to switch setups. `vanilla` is the built-in profile with nothing installed. [Restoring the game](/using/profiles-and-restoring) returns every game file to its original state.

## ADDON SETTINGS

SwoftyMod adds **ADDON SETTINGS** to the game's home menu, directly under **Career**. It lists every enabled addon and map as a tile, and each tile opens a page with that addon's settings. Changes save to the profile. See [In-game addon settings](/using/swofty-mod).

::: tip Making your own
Start with [Your first addon](/tutorials/first-addon). The [Standard library](/reference/std/) lists every API an addon can use.
:::
