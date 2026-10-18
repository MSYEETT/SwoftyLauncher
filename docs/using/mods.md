---
title: Mods and tracks
description: Import mods and community tracks on the Mods page, order them, turn them on per profile, and read what each one changes.
---

# Mods and tracks

Mods give you liveries, cars, in-game text and other content, with no code. Tracks such as Sepang or Okayama are mods too. You manage both on the **Mods** page, added to the sidebar by the built-in Team Simplified addon. Every profile shares this library, but keeps its own order and switches, so you can choose which mods to use in each profile.

## Import a mod or track

1. Open the **Mods** page.
2. Drop the file anywhere on the launcher window, or select **Import…**. You can import:
   - a `.serps` package;
   - a zip or folder with `mod.serps` or an `F1 25` folder inside it;
   - a track package zip, or a download zip that holds one.
3. The launcher copies it into the library, compares it with the clean game files, and turns it on in the active profile.

You can drop several files at once and follow their progress on a card. To replace a mod, import it again: it keeps its place and switch in every profile.

The launcher leaves out `.exe`, `.dll` and `.asi` files and names them in the result. It can't read 7z or RAR archives, so unpack those and import the folder.

## Order and turn mods on or off

The list shows your active profile's load order. Use it to choose which mod takes priority: **Where two mods change the same thing, the one lower in the list wins.**

- **Reorder:** expand a row and select **Move up** or **Move down**.
- **Turn on or off:** use the switch on the row.

You'll find the library size, the number of mods turned on and the load order rule in the subtitle. If a mod hasn't been compared with your installed game build yet, you'll see a **Compare** notice. This can happen after a game update; select it to compare the mod with that build.

## Read a mod's row

Look at a row's tags to see what the mod contains, which game version it was made for and where it overlaps with other mods:

| Tag | Meaning |
|---|---|
| **Track** | A community circuit |
| **Database** | Changes the game database |
| **Text** | Changes in-game text |
| **3 archives** | Changes resources inside game archives |
| **2 files** | Replaces or adds loose game files |
| **Made for 1.0.141.7865** | Built for a game version other than the one installed |
| **Overrides 26 in _mod_** | Wins over a lower mod on 26 changes |
| **26 overridden by _mod_** | A lower mod wins on 26 of its changes |

Expand a row to see the mod's version, author, size and import date. For a closer look at its changes, or to manage its files, use:

- **WHAT IT CHANGES** — counts of the database records, text entries, archive resources and files it changes.
- **CONFLICTS** — the exact records, resources, text and files where it overrides another enabled mod or is overridden by one.
- **Show files** — opens the mod's files in the library.
- **Remove from library** — deletes the mod after a confirmation and turns it off in every profile.

## Tracks

To import a track, drop the track package zip onto the launcher without unpacking it. You'll see the **Track** tag on its row. Turn it on to make it appear in Grand Prix track selection; you can turn on any number of tracks in one profile.

[How tracks are installed](/reference/maps) covers what activation changes.

## From a terminal

You can do the same work from a terminal with `swofty mods`. Use `--profile` to choose the profile:

| Command | Does |
|---|---|
| `swofty mods list` | List the library in load order |
| `swofty mods import <path>…` | Import packages, zips, folders and track zips |
| `swofty mods enable <id>` | Turn a mod on |
| `swofty mods disable <id>` | Turn a mod off |
| `swofty mods move <id> <position>` | Move a mod in the order (position 1 loads first) |
| `swofty mods remove <id>` | Delete a mod from the library |

```sh
swofty mods import Okayama_v0.17.zip
swofty mods enable okayama
swofty apply
```

You'll find each mod's files in `$DATA/addon-data/teamsimplified.mods/library/<id>/`.
