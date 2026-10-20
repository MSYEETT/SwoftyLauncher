---
title: Launching the game
description: How Swofty Launcher for F1 25 starts a modded session on Windows and Linux, where the logs go, and what happens when the game exits.
---

# Launching the game

Modded sessions run offline. To get set up, follow [Windows](/guide/windows) or [Linux and Steam Deck](/guide/linux).

## Linux: the Steam wrapper

Set Steam's launch option to `swofty run -- %command%` so its Play button starts Swofty Launcher for F1 25. The launcher uses your configured Proton version and records session logs.

To go back to Steam's normal launch, clear the option.

## Windows

Keep Steam running, then select **Play** in the launcher or run `swofty run`. To follow the logs in the launcher, select the arrow next to **Play** and choose **Play with console**.

Modded sessions don't start while `F1_25.exe` is set to **Run this program as an administrator** and the launcher runs normally: Windows won't start the game unelevated (error 740), and the launch stops with a message naming the setting. To fix it, right-click `F1_25.exe` in the game folder, open **Properties > Compatibility** and untick the box, or untick it under **Change settings for all users** when it's set for all users. You can also run the launcher as administrator. `swofty doctor` shows the setting in its `run as admin` row.

## Logs

You'll find session logs in `$DATA/sessions/<id>/`. Run `swofty logs` for the latest summary, or use `swofty run` to follow the logs as you play.

If Proton can't create the game process, you'll see `Proton could not start the game process (CreateProcess error N)` instead of an exit code in `swofty run`, `swofty logs` and the launcher.

When the game exits, however it exits, the session puts the game folder back to vanilla. To check, run `swofty verify`. See [Restoring the game](/using/profiles-and-restoring) for the cases that need `swofty restore`.

## Next

- [Offline play](/reference/safety): what a modded session turns off.
- [Backups and recovery](/reference/restore): how the session rolls the game folder back.
