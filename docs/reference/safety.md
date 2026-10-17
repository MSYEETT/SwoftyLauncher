# Offline play

Modded sessions are for single-player play only. Multiplayer is off.

## Return to vanilla

Close the game and run `swofty restore` before a normal Steam launch. On Linux, also clear the launcher's Steam launch option.

See [Restoring the game](/using/profiles-and-restoring) if any files need repair.

## Saves

Separate saves for each profile are planned. Save isolation is still in development; see [Design notes](/design/#profiles-and-saves).

## Offline play staging

A modded session parks `EAAntiCheat.GameServiceLauncher.exe` as `.bak`, drops
a stub that starts the game, and places the loader hook. Every action is
recorded in a staging journal with SHA-256 digests. Restore replays the
journal backwards and refuses to touch a file that changed since staging.

Restoration runs when the game exits, from `swofty restore`, and
automatically at the next start (crash recovery), so a crash cannot leave the
game folder modified. `swofty offline status` shows whether staging is clean.

## Sharing mods

Share mod changes, not copies of game files. This repository contains no game data. Tests that need it read your local copy through `F1_25_DIR`.
