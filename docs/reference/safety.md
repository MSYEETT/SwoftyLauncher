# Safety

The launcher modifies single-player, offline play only. These rules hold on every platform.

## Mods load only in a session the launcher prepared

- `version.dll` and `swofty/` exist in the game folder only for the length of a modded session. They are placed through the journal and removed on exit, including after a crash.
- The proxy acts only inside `F1_25.exe`. In any other process it only forwards.
- The gate requires a session named by `SWOFTY_SESSION`, with a fresh, random, single-use, short-lived ticket, a matching exe hash, and no anticheat module loaded. If any check fails, nothing loads and the game runs vanilla.
- On Windows, Steam's Play button and a double-click on the exe both run vanilla: neither the loader files nor a ticket are present.

## A modded session is never online

- On Linux, the EA Javelin preloader refuses to run under Wine, Proton and Steam Deck, so there is no anticheat path and the game reaches its offline front end.
- On Windows, the modded and vanilla entry points are separate. The vanilla one keeps the anticheat and online exactly as shipped, and the launcher never touches it.
- In the process, an offline guard blocks outbound connections from the game to EA and online endpoints at the WinHTTP and WS2_32 layer. Steam's local ownership pipe stays intact.
- The "Retry connection" action is removed from the home screen's data in a modded session.
- If the loader detects that the session is not offline, or that an anticheat module is present, it unloads every mod, stops the VFS, records an error in the session status, and lets the game continue vanilla or exit.

## No evasion

Nothing in the launcher or the loader is designed to make the anticheat report a false state, to run mods while the anticheat or online services are active, or otherwise to hide mods. The loader only checks that the session is already offline and free of the anticheat, and refuses to load mods otherwise.

## Saves are isolated

Modded saves go to `$DATA/saves/<profile>/`, which Steam Cloud does not sync. Vanilla saves stay pristine and syncable, and modded progress cannot load into a vanilla or online session.

## Native code needs consent

Each native plugin DLL needs explicit consent, per content hash, with its declared capabilities shown. The session pins every consented hash, and the loader refuses anything unpinned. Hooks go only into the game's `.xtls` code; `.data1` bodies are never patched.

## The game's files stay intact

Every change to the game folder (data files, loader files, directories and the prefix override) is journalled before it happens and rolled back after the session, on the next start after a crash, or by `swofty restore`. Originals are kept in a content-addressed baseline store, and originals from an old build are never restored over a new one. The `vanilla` profile places nothing at all. Today data files are delivered in place under the journal; the later VFS mode writes none. See [Baseline and journal](/reference/restore).

## Game data and redistribution

Mods ship operations, not copies of the game's files. The repository contains no game archives, databases, executables, extracted assets or decompiled code. Format tests read the user's own game files from a path given in `F1_25_DIR`.
