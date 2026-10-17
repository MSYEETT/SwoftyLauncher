# Windows

## Set up

1. Install the launcher. <span class="todo">TODO: installer or archive name</span>
2. Start it. The setup step finds Steam, its libraries and the F1 25 install.
3. Pick or create a [profile](/using/profiles), and enable the mods and maps you want.

Steam's launch options are left alone on Windows.

## Play

Press **Play modded (offline)** in the launcher. It:

1. builds the overlay for the profile, or reuses the cached one;
2. writes a session file with a fresh, short-lived ticket;
3. places `version.dll` and the `swofty/` folder next to `F1_25.exe`, recorded in the restore journal;
4. sets `SteamAppId` and `SteamGameId` and starts `F1_25.exe` directly, without the EA AntiCheat launcher.

Steam must be running. The game window title ends in `[Swofty Mod · Offline]`.

When the game exits, the launcher rolls back the session's journal: it restores the data files it replaced and removes `version.dll` and `swofty/`. It copies the loader log into the session and merges any changes made in game into the profile.

## Vanilla and online play

Steam's Play button starts the game exactly as shipped: with the anticheat, online, and without mods. The loader files exist only during a modded session, so that path never sees them. A double-click on `F1_25.exe` also runs vanilla, because there is no session ticket.

## If something goes wrong

- Run `swofty restore` to roll back any open journal and remove loader files left by a crash. See [Profiles and restoring the game](/using/profiles-and-restoring).
- Run `swofty doctor` to check the install, the game build and the data directory.
- Session logs are in `$DATA\sessions\<id>\`.
