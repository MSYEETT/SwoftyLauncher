# SwoftyMod

SwoftyMod (`swofty.core`) is the launcher's built-in mod. It is part of every profile, it is always enabled, and it cannot be disabled or removed. Its source is in [`mods/swoftymod`](https://github.com/Swofty-Developments/F1ModManager/tree/main/mods/swoftymod) and in the native loader.

## What it adds in game

| Element | Where | How |
|---|---|---|
| **MOD SETTINGS** tile | Home menu, directly under Career | Native: one `MidHook` in the home tile-list builder adds a tile using the Career template, labelled MOD SETTINGS, that fires the `mod_menu` route |
| **F9 prompt** | Home screen button bar | Data: a key listener on `Menu Right Stick Click` (F9 on keyboard, R3 on controller) and a "Mod Settings" tooltip entry |
| **Launcher panel** | Bottom left of the home screen | Data: the offline contextual panel reads "Launched with Swofty Mod Launcher, press [F9] to open settings" |
| **Settings page** | Opened by the tile or F9 | Lists the profile, its mods and their options. In progress |

The `mod_menu` route opens the game's Settings until the dedicated settings page lands.

The bottom-left panel no longer offers "Retry connection" in a modded session: its reconnect action is removed from the data.

## Window title

The game window title ends in `[Swofty Mod · Offline]` during a modded session.

## Without the native hook

If a signature does not match the running build, the tile hook is not installed and the game runs without it. The F9 prompt and the panel are data and still work.
