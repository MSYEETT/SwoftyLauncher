# Addons

**Addons** change the game with code, settings and in-game screens, such as a bigger grid or new menus. Content with no code, such as liveries and tracks, is a [mod](/using/mods).

## Install an addon

An addon is a folder with a `swofty.json` file, or a `.swoftymod` file or zip that holds one. Install it in any of these ways:

- Drop the folder, `.swoftymod` file or zip anywhere on the launcher window.
- On the **Addons** page, select **Install addon…** and choose the folder.
- From the command line:

```sh
swofty addons install <path>
swofty addons list
swofty addons enable <id>
swofty addons disable <id>
```

Installed addons are stored in `$DATA/addons/<id>/`. Installing an addon with an id that is already installed replaces it.

## Turn addons on and off

The **Addons** page lists every installed addon with a switch for the active [profile](/using/profiles). Expand a row for its version, author, game builds, requirements and settings.

Addons can require other addons or conflict with them. Check any warnings before playing.

## Addon pages

Some addons add their own page to the sidebar, under the divider after **Addons**. The built-in Team Simplified addon adds [Mods](/using/mods).

## Built-in addons

| Addon | Id | What it does |
|---|---|---|
| [SwoftyMod](/using/swofty-mod) | `swofty.core` | Loads addons in game and adds **ADDON SETTINGS**. Always on in modded profiles. |
| Team Simplified | `teamsimplified.mods` | The [Mods](/using/mods) page and `swofty mods`. On by default, and can be turned off per profile. |

Earlier versions stored addons in `$DATA/mods/`. The launcher and `swofty` move them to `$DATA/addons/` when they start.
