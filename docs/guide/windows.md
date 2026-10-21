---
title: Windows
description: Set up Swofty Launcher for F1 25 on Windows, point it at the game and launch your first modded session.
---

# Windows

## 1. Install the launcher

Follow [Installation](/guide/installation) to install the launcher with the installer or by hand.

## 2. Open it and find the game

1. Make sure you have F1 25 installed with Steam.
2. Open **Swofty Launcher** from the Start menu, or `bin\swofty-launcher.exe` in the install folder.

Watch the status line next to **Play**: you'll see **Detecting F1 25…** while the launcher looks for the game, then **Ready** when it finds it.

### If the game is not found

If the launcher can't find the game, you'll see **Install it with Steam or locate its folder** in the status line and a **Locate game** button. To point it to your installation:

1. Select **Locate game**.
2. Type the F1 25 folder's path, or select **Browse…** and choose it. It's the folder with `F1_25.exe`, usually `...\steamapps\common\F1 25`.
3. Select **Use this folder**.

## 3. Play

1. [Install your addons](/using/addons) and [mods](/using/mods), then pick a [profile](/using/profiles).
2. Press **Play**.
3. On your first launch, select **Accept and launch** in the **Modded play runs offline** dialog. For why modded play runs offline, see [Offline play](/reference/safety).

You'll see **Launching…** on the button, then **Stop** while the game runs. If Steam is closed, the launcher starts it and waits for it to sign in first; see [Steam](/reference/launch#steam). When you want to play vanilla, launch through Steam as usual.

To follow the logs as you play, select the arrow next to **Play** and choose **Play with console**, or run `swofty run` from a terminal.

## If the mods don't load

An antivirus such as Windows Defender can delete, quarantine or block the launcher's `version.dll` and its other DLLs. F1 25 then starts without mods. The launcher checks for this before the game starts and again once it is running, and shows a **Your antivirus removed mod files** or **Mods did not load** dialog that names the files. The game keeps running without mods.

To fix it:

1. Close F1 25.
2. Open **Windows Security** > **Virus & threat protection** > **Protection history** and restore the files the dialog names.
3. Add exclusions for the game folder and the launcher folder (`%LOCALAPPDATA%\Programs\SwoftyLauncher`).
4. Press **Play** again.

## If the game crashes

The session puts the game folder back to vanilla when the game exits, crash or not. If the launcher was closed first, it puts the folder back the next time it starts. See [Restoring the game](/using/profiles-and-restoring).

To read a summary of your last session, run `swofty logs`. You'll find the logs in `$DATA\sessions\<id>`.

## Next

- [Using the launcher](/using/): addons, mods, profiles and in-game settings.
- [Launching the game](/reference/launch): logs and what a session does.
