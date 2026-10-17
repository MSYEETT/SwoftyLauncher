# Launch and injection

## Overview

```
 host (Windows or Linux)                 game process (Windows, or inside the Proton prefix)
 swofty-launcher, swofty                 F1_25.exe
   build the overlay                       version.dll (C): forwards the real exports,
   write sessions/<id>/session.json  --->    checks the host and the gate,
   place version.dll and swofty/             waits for the first window,
   (journaled)                               loads swofty/mods/core/*.dll, then swofty/mods/*.dll
   start the game                          swofty_core.dll (C++, safetyhook): VFS, offline guard,
   restore on exit                <---       home tile hook, settings bridge, title suffix
```

## The `version.dll` proxy

`version.dll` is a static import of `F1_25.exe`, so the OS loader maps the copy next to the exe and runs its `DllMain` before the game's entry point. It is not on the input path and not a KnownDLL.

At attach the proxy:

1. loads the real `version.dll` from the system directory and forwards all 17 exports through jump stubs resolved with `GetProcAddress` (Wine rejects absolute-path `.def` forwarders);
2. stops there unless the host process is `F1_25.exe`;
3. checks the exe's size and PE timestamp against the analysed build;
4. starts a worker thread that waits for the game's first visible top-level window, settles for 2 seconds, then loads `swofty/mods/core/*.dll` and `swofty/mods/*.dll` in sorted order and calls each one's `SwoftyModInit`.

It logs to `swofty/loader.log` and keeps the previous run as `loader.prev.log`.

The image loads at `0x140000000` every time: the exe has no relocations and no dynamic base. Hooks still resolve by signature, and the result is cached per build.

Under Proton the prefix-scoped override `[Software\\Wine\\AppDefaults\\F1_25.exe\\DllOverrides] "version"="native,builtin"` makes Wine use the proxy. Windows needs no configuration.

Fallbacks, if the proxy cannot be used: `dinput8.dll`, or a create-suspended injector, `SwoftyHost.exe`, which is not written.

## The session file

The launcher writes `$DATA/sessions/<id>/session.json` before every modded launch. The loader only reads it.

```json
{
  "schema": 1,
  "ticket": "b64-random-32-bytes",
  "expires": "2026-10-17T12:34:56Z",
  "mode": "offline-modded",
  "game": { "app_id": 3059520, "build": "25251167", "exe_sha256": "d873a1c8…" },
  "profile": "grid24",
  "overlay": {
    "root": "Z:\\home\\swofty\\.local\\share\\swofty\\cache\\overlay\\grid24\\9f3c…",
    "files": { "game_package/gamedatabases/gamedatabases.2025.bin": "gdb.bin" }
  },
  "plugins": [ { "id": "swofty.core", "dll": "swofty/mods/core/swofty_core.dll", "sha256": "…" } ],
  "options": { "swofty.f1-2026-grid24": {} },
  "ui_bridge": { "notify_map": { "3404951093": ["swofty.core", "open_mod_settings", true] } },
  "policy": { "offline": "strict", "save_namespace": "swofty-grid24" }
}
```

Paths are in the game process's view; under Proton `/home/...` becomes `Z:\home\...`.

## The file channel

Files are the only channel that crosses the Wine boundary and works while the launcher is closed.

| File | Writer, reader | Purpose |
|---|---|---|
| `sessions/<id>/session.json` | launcher, loader | The session, read-only for the loader |
| `state/requests/<profile>.json` | loader, launcher | Changes made in game, written atomically |
| `sessions/<id>/status.json` | loader, launcher | A heartbeat every 2 seconds: phase, hook table, errors |
| `sessions/<id>/loader.log`, `crash/` | loader, copied after exit | Logs and minidumps |

## Data delivery

- **Journalled in place (today).** The profile's built files are copied over the originals before launch and restored after exit. Each original goes into the baseline store and each step into `state/journal/<id>.json` before it happens. The launcher rolls back any open journal on start and before every launch, and `swofty restore` does it on demand. See [Baseline and journal](/reference/restore).
- **VFS (later).** `swofty_core.dll` redirects the game's file opens (`NtCreateFile`, `NtOpenFile`) to the overlay for every path listed in the session, and no data files are written to the game folder at all. Under Proton the hook runs inside the Windows process, so it never touches host files directly.

The loader files themselves, `version.dll` and `swofty/`, are always placed through the journal, because the OS loader has to find `version.dll` on disk before any of our code runs.

## Linux: the Steam wrapper

The launch option `swofty run -- %command%` makes Steam run the wrapper with its own command line. The wrapper:

1. parses the Proton verb, the Steam Linux Runtime entry, the environment and the prefix, and fails closed on a shape it does not recognise;
2. builds the session and places the loader files;
3. writes the prefix-scoped override and sets `WINEDLLOVERRIDES="version=n,b"`;
4. applies the CPU affinity and environment from `config.json`;
5. replaces the EA AntiCheat launcher target with `F1_25.exe` and runs the Proton chain with the game folder as the working directory;
6. waits, forwarding signals, then restores the game folder and merges in-game changes.

Proton, the prefix, the Steam overlay, ownership and `SteamAppId` are unchanged.

## Windows: Play modded (offline)

The launcher builds the session, places the loader files, sets `SteamAppId` and `SteamGameId` to 3059520, and starts `F1_25.exe` itself. It does not use `steam -applaunch`, because Steam's launch command runs the anticheat launcher. Steam's own Play button stays vanilla.

## After exit

The wrapper or the launcher records the outcome, prints or shows a crash summary if the game crashed ([`swofty run`](/reference/cli#swofty-run)), copies the loader log and minidumps into the session, runs the journal restore (removing `version.dll` and `swofty/` and clearing the prefix override), and merges `state/requests/<profile>.json` into the profile. If the game asked for **Apply and restart**, it rebuilds and relaunches.

## Status

The proxy (forwarding, host and build checks, the window wait, mod loading, API version 1) and the home tile hook in `swofty_core.dll` are written and tested under Wine against a synthetic exe. The gate, the VFS, the offline guard, the settings bridge and the wrapper are not written yet.
