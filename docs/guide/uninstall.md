# Uninstalling

1. Close the game.
2. Run `swofty restore`. It rolls back every open journal, removes `version.dll`, `swofty/mods` and the prefix override, restores every baselined file, and verifies the folder against the baseline. It exits non-zero and lists anything it could not restore.
3. Delete the data directory (`~/.local/share/swofty`, `~/.var/app/com.valvesoftware.Steam/data/swofty` or `%LOCALAPPDATA%\Swofty`).
4. On Linux, clear the F1 25 launch option in Steam.
5. On Linux, remove the override from the prefix, with the game and its wineserver stopped: delete the two-line `[Software\\Wine\\AppDefaults\\F1_25.exe\\DllOverrides]` stanza from `compatdata/3059520/pfx/user.reg`. Leaving it is harmless.

After `swofty restore` the game folder is byte-identical to the baseline. Steam's "Verify integrity of game files" also restores the data files, but it does not remove `version.dll` or `swofty/`. See [Profiles and restoring the game](/using/profiles-and-restoring).
