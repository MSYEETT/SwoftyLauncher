---
title: Addons, mods and tracks
description: The three kinds of content Swofty Launcher for F1 25 loads — addons, mods and tracks — what each one is and which page installs it.
---

# Addons, mods and tracks

You can load three kinds of content into F1 25 through the launcher. Drop any of them on the launcher window and it goes to the right page, or use the button on that page:

| | What it is | What you get | Install on |
|---|---|---|---|
| **Addon** | Anything with a `swofty.json`: a folder, a `.swoftyaddon` file or a zip | Data changes, engine code, launcher pages and settings | [Addons](/using/addons) page |
| **Mod** | A `.serps` package, or a zip, 7z, RAR or folder of game files such as `.erp` archives | Liveries, cars, text and other content, with no code | [Mods](/using/mods) page |
| **Track** | A community track zip (a kind of mod) | A new circuit, such as Sepang or Okayama | [Mods](/using/mods#tracks) page |

## Addon

An addon is anything with a `swofty.json`. It can carry any mix of these:

| Part | What it does |
|---|---|
| **Operations** | Change the game's data: database rows, in-game text, textures and models in `.erp` archives, menu layouts and loose files. |
| **Engine plugin** | Adds code inside F1 25, such as a bigger grid or a HUD. |
| **Launcher plugin** | Adds a page to the launcher's sidebar, file imports and `swofty` commands. |
| **Settings** | Toggles, sliders and choices you set per profile, in the launcher and in game. |

An addon with only operations is a **data addon**. An addon with an engine or launcher plugin is a **code addon**. One addon can be both. [Operations](/reference/data/operations) lists every change an addon can make without code.

Install your addon on the **Addons** page, then turn it on or off for each profile. Two addons come with the launcher:

- **SwoftyMod** loads addons in game and adds **ADDON SETTINGS**. It's always on in modded profiles.
- **Simplified for Swofty**, Team Simplified's addon, adds the **Mods** page. The launcher installs it, keeps it up to date and keeps it on.

## Mod

A mod is content made for the Simplified Suite, with no `swofty.json` and no code. It comes as:

- a `.serps` package made with Simplified Studio;
- a zip, 7z or RAR archive, or a folder, with files laid out like the game folder: `.erp` archives, the game database, language files and other game files.

Import it on the **Mods** page, then choose its place in the list and turn it on for your profile. Order matters: if two mods change the same thing, the one lower in the list wins. The launcher leaves out `.exe`, `.dll` and `.asi` files.

## Track

A community track, such as Sepang or Okayama, comes as a track package, a zip made for the EGO GameDatabases Editor, or as a data addon that adds a track. Import either on the **Tracks** page without unpacking it, turn it on, and you'll find it in Grand Prix track selection.

An addon that only adds tracks and their names is listed on the **Tracks** page instead of the **Addons** page. An addon that adds a track and changes other things too is listed on both. Without a Tracks page in the profile, every addon is on the **Addons** page.

## Profiles and in-game settings

A [profile](/using/profiles) keeps your addons, mods, tracks and every addon setting together as one setup. To switch setups, switch profiles. You can choose the built-in `vanilla` profile for a setup with nothing installed.

To change settings while you play, open **ADDON SETTINGS** in the home menu, directly under **Career**. You'll find every enabled addon, mod and track there; open an addon to change its settings without leaving the game. See [In-game addon settings](/using/swofty-mod).

## Next

- [Install the launcher](/guide/installation), or go straight to [Addons](/using/addons) and [Mods and tracks](/using/mods).
- To make your own, see the [Developer overview](/reference/): [mods](/reference/mods/) for the Mods page, [addons](/reference/addons/) for everything with a `swofty.json`.
