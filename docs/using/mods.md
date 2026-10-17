---
title: Mods and tracks
description: Import .serps mods, mod zips and community tracks into the Mods page, order them, turn them on per profile and read their conflicts.
---

# Mods and tracks

The **Mods** page holds your mod library: liveries, cars, text and other content packages with no code, and community tracks such as Sepang and Okayama. It comes from the built-in Team Simplified addon, which reads `.serps` packages made with Simplified Studio. The library is shared by every profile, and each profile keeps its own order and switches.

## Import a mod

Drop any of these anywhere on the launcher window, or select **Import…** on the Mods page:

- a `.serps` package;
- a zip with `mod.serps` or an `F1 25` folder inside it;
- a folder with `mod.serps` or an `F1 25` folder inside it;
- a track package zip, or a download zip that holds one.

The launcher copies the mod into the library, compares it with the clean game files and turns it on in the active profile. Several files can be dropped at once, and a card shows the progress of a drop.

- `.exe`, `.dll` and `.asi` files are left out and named in the result. Files whose paths leave the package are left out and counted.
- Importing a mod again replaces it and keeps its place and switch in every profile.
- 7z and RAR archives are not read. Unpack them and import the folder.

## Order and switches

The list is the load order of the active profile. Where two mods change the same thing, **the mod lower in the list wins**. Expand a row and use **Move up** and **Move down** to change the order. The switch on each row turns the mod on or off in the active profile.

The page's subtitle shows the library size, how many mods are on in the profile, and the rule. A notice appears when:

- Team Simplified is off in this profile. **Turn on** turns it on.
- The vanilla profile is active. It plays without addons or mods.
- Some mods are not compared with the installed game build yet, such as after a game update. **Compare** compares them.

## Read a mod's row

| Tag | Meaning |
|---|---|
| **Track** | A community track |
| **Database** | It changes the game database |
| **Text** | It changes in-game text |
| **3 archives** | It changes resources inside game archives |
| **2 files** | It replaces or adds loose game files |
| **Made for 1.0.141.7865** | It was made for another game version than the one installed |
| **Overrides 26 in &lt;mod&gt;** | It wins over a mod above it on 26 changes |
| **26 overridden by &lt;mod&gt;** | A mod below it wins on 26 of its changes |

The expanded row shows the mod's status, version, author, the game version it was made for, its Simplified Studio version, its size and when it was imported. Below that:

- **WHAT IT CHANGES** counts the database records it changes, adds and removes, the text entries it changes and adds per language, the resources it changes, adds and removes per archive, and the files it replaces and adds. A language the mod does not include takes its English changes, shown as `(from eng)`.
- **CONFLICTS** lists the exact records, resources, text entries and files where it overrides another enabled mod or is overridden by one.
- **Show files** opens the mod's files in the library.
- **Remove from library** deletes the mod after a confirmation, and turns it off in every profile.

## How mods reach the game

When you play, the launcher applies only what each enabled mod changes, in list order: the database records it changed, the resources inside each archive, the text entries and whole loose files. Mods that change different rows of the database or different resources of an archive combine.

- Text changes need the in-game loader. They apply in memory while the game boots, and the game's language files stay stock.
- A mod's removals of database records apply only when it was made for the installed game version. Otherwise they are left out, and the row says how many.
- Removals of resources from archives are not applied. The build warns with their count.
- Mods are checked against other addons as well. `swofty build` lists every conflict with the winner.

## Tracks

A track is a mod too. Import the track package zip as above, with no need to unpack it. The track's id is its track code, such as `okayama`, and its row has the **Track** tag. When it is on, it appears in Grand Prix track selection.

Any number of tracks can be on in one profile. With an in-game loader that cannot register asset groups, the limit is eight.

Track packages use the format made for the EGO GameDatabases Editor:

| File | Purpose |
|---|---|
| `manifest.json` | Track name, code and setup information |
| `<code>.erp` | Track data |
| `mipmaps/` | Optional textures |
| `ui/images.json` | Track artwork |
| `ui/minimap.json` | Minimap placement |

Track packages made with the Simplified Track Installer import as ordinary `.serps` mods.

Known issues:

- A track's menu card shows the language strings the package names, often its base track's name and stats.
- Track-select pictures share one invitational event picture, `invitational_image_2`. With an in-game loader that cannot register asset groups, each track also takes one invitational event's picture (`invitational_image_4`, `_10`, `_13` and on).

[How maps are installed](/reference/maps) describes what activation changes.

## From the command line

`swofty mods` does the same as the Mods page, for the profile given by `--profile`:

| Command | Does |
|---|---|
| `swofty mods list` | List the library in load order, with each mod's switch |
| `swofty mods import <path>…` | Import `.serps` packages, zips, folders and track zips |
| `swofty mods enable <id>` | Turn a mod on |
| `swofty mods disable <id>` | Turn a mod off |
| `swofty mods move <id> <position>` | Move a mod in the load order. Position 1 loads first. |
| `swofty mods remove <id>` | Delete a mod from the library |

```sh
swofty mods import Okayama_v0.17.zip
swofty mods enable okayama
swofty apply
```

`swofty maps` is another name for `swofty mods`.

## Where mods are stored

Mods are stored in `$DATA/addon-data/teamsimplified.mods/library/<id>/`, with the package's files under `files/` and a track package under `track/`. Earlier versions kept tracks in `$DATA/maps/` and listed them in each profile's `maps`. The launcher and `swofty` move them into the library, turned on in the same profiles, when they start. A track whose code is already the id of a mod in the library gets the id `<code>-track`.
