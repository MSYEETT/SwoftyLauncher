# Addons

Use an addon to change the game through pages, settings and engine code: a bigger grid, for example. If you're adding content with no code, such as liveries and tracks, that's a [mod](/using/mods).

## Install an addon

You can install a folder that holds `swofty.json`, a `.swoftyaddon` file, or a zip of either. In the launcher, you have two ways to do it:

- **Drop it:** drag the folder, `.swoftyaddon` file or zip anywhere on the launcher window.
- **Browse to it:** open the **Addons** page, select **Install addon…**, select **Install** (the browse button), choose the addon folder, then select **Install**.

You'll find the addon on the **Addons** page once the launcher copies it to `$DATA/addons/<id>/`. If you've already installed an addon with the same id, this replaces it.

To install from a terminal and list your addons:

```sh
swofty addons install <path>
swofty addons list
```

## Turn an addon on or off

1. Open the **Addons** page and find your addon's row. Its switch controls whether the addon is on in the active [profile](/using/profiles).
2. Toggle the switch to turn the addon on or off in that profile.

If you're using the `vanilla` profile, switch to a modded profile first. The switch is locked on `vanilla`, and the row reads **Switch away from vanilla to use…**.

To see an addon's version, author, game builds, requirements and settings, expand its row. Check for requirements or conflicts here and resolve any you see before you press Play.

## Remove an addon

1. On the **Addons** page, expand the addon's row.
2. Select **Remove addon**.
3. In the **Remove _name_?** dialog, select **Remove addon**.

Removing an addon deletes its files and takes it out of every profile.

## Built-in addons

| Addon | What it does |
|---|---|
| **SwoftyMod** | Loads addons in game and adds **ADDON SETTINGS**. It's always on in modded profiles. |
| **Team Simplified** | Adds the [Mods](/using/mods) page and is on by default. If you turn it off, you'll hide the Mods page and leave those mods out of the profile. |

If an addon adds its own page, you'll find it in the sidebar under a heading with the addon's name.
