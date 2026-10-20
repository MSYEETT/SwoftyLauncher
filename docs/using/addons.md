---
title: Addons
description: Install addons on the Addons page, turn them on per profile, check their details and settings, and remove them.
---

# Addons

An addon is anything with a `swofty.json`. A data addon changes the game's database, text, archives and files through operations; a code addon adds an engine plugin, a launcher page or both. Mods and tracks have no `swofty.json`; you import those on the [Mods](/using/mods) page.

## Install an addon

You can install a folder that holds `swofty.json`, a `.swoftyaddon` file, or a zip of either. Drag it anywhere on the launcher window, or:

1. Open the **Addons** page.
2. Select **Install addon…**.
3. Type the addon's path, or select **Browse…** and choose the addon folder.
4. Select **Install**.

The launcher copies the addon to `$DATA/addons/<id>/` and lists it on the **Addons** page. If you've already installed an addon with the same id, this replaces it. A new addon starts off in every profile.

## Turn an addon on or off

1. Open the **Addons** page and find your addon's row. Its switch shows whether the addon is on in the active [profile](/using/profiles).
2. Toggle the switch.

The next time you press **Play**, the launcher builds the profile with the change.

Some addons, such as Driver Manager, change what your careers store. Before you turn one of those off, the launcher asks first: **This addon has affected your saves**. Careers you played with it may not load without it. Select **Turn off anyway** to go ahead, or **Cancel** to leave it on. Removing such an addon asks the same, with **Remove anyway**. Turning an addon on never asks.

If you're using the `vanilla` profile, switch to a modded profile first: on `vanilla`, rows read **Switch away from vanilla to use addons** instead of showing a switch. Addons the launcher needs show **Always on** with a lock.

## Check an addon

Expand an addon's row to see its status, version, author, licence, the game builds it was made for, the addons it requires and its id. **GAME BUILDS** reads **matches** with your build, or **not tested on** it when the addon wasn't made for it. The tags on the row show what the addon changes: **Database**, **ERP**, **UI**, **Files** or **Native**.

If an addon requires another addon that's off, or conflicts with one that's on, the launch stops with an error that names both. Turn the required addon on, or one of the conflicting addons off.

## Change its settings

1. Expand the addon's row.
2. Under **SETTINGS**, change a value. **Reset to defaults** puts every setting back.

Settings save to the active profile. When you change a database setting, the status line next to **Play** says the profile rebuilds on launch. You can change the same settings in game; see [In-game addon settings](/using/swofty-mod).

## Remove an addon

1. On the **Addons** page, expand the addon's row.
2. Select **Remove addon**.
3. In the **Remove _name_?** dialog, select **Remove addon**.

Removing an addon deletes its files and takes it out of every profile. You can't remove the built-in addons.

## Built-in addons

| Addon | What it does |
|---|---|
| **SwoftyMod** | Loads addons in game and adds **ADDON SETTINGS**. It's always on in modded profiles. |
| **Simplified for Swofty** | Team Simplified's addon. Adds the [Mods](/using/mods) page. The launcher installs it, [keeps it up to date](/using/updating#simplified-for-swofty) and keeps it on. |

Each release also installs a few addons switched off, such as xanpavle's F1 25 RE-Optimization. You can turn them on per profile or remove them; see [Addons that ship with the launcher](/using/bundled-addons).

If an addon adds its own page, you'll find it in the sidebar under a heading with the addon's name.

## From a terminal

| Command | Does |
|---|---|
| `swofty addons list` | List installed addons and whether each is on in the profile |
| `swofty addons install <path>` | Install a folder, `.swoftyaddon` or zip |
| `swofty addons enable <id>` | Turn an addon on |
| `swofty addons disable <id>` | Turn an addon off |
| `swofty addons remove <id>` | Remove an addon and take it out of every profile |
| `swofty settings list [id]` | List addon settings with the profile's values |
| `swofty settings set <id> <setting> <value>` | Change a setting |
| `swofty settings reset <id>` | Put an addon's settings back to their defaults |

Add `--profile <name>` to work on a profile other than the active one. For an addon that has affected your saves, `disable` and `remove` print the warning and ask before going ahead; add `--yes` to skip the question, which is required when the command isn't run from a terminal.

## Next

- [Mods and tracks](/using/mods): import content with no `swofty.json`.
- [Profiles](/using/profiles): keep different sets of addons apart.
- To make an addon, see [Addons](/reference/addons/) in the developer docs.
