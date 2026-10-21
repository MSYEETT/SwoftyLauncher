---
title: Launching the game
description: How Swofty Launcher for F1 25 starts a modded session on Windows and Linux, where the logs go, and what happens when the game exits.
---

# Launching the game

Modded sessions run offline. To get set up, follow [Windows](/guide/windows) or [Linux and Steam Deck](/guide/linux).

## Steam

F1 25 from Steam needs the Steam client running and signed in. When you select **Play** or run `swofty run` while Steam is closed, the launcher starts Steam in the background, waits for it to sign in, then launches the game. The status line next to **Play** reads **Starting Steam…**, then **Waiting for Steam to sign in…**.

When Steam doesn't come up, the launch stops with one of these messages:

| Message | What to do |
|---|---|
| `Steam didn't start within 90 s; open Steam and press Play again` | Open Steam yourself, then select **Play** again |
| `Steam is open but not signed in; sign in to Steam and press Play again` | Sign in to Steam, then select **Play** again |
| `could not start Steam (...); open Steam and press Play again` | Steam wasn't found or exited with an error. Open Steam yourself, then select **Play** again |

On Linux, when Steam runs but never reports signing in, the launch goes ahead after 90 seconds.

To turn this off, select the arrow next to **Play** and clear **Start Steam automatically**, or set `"start_steam": false` under `"launch"` in `$DATA/config.json`. For a single launch, run `swofty run --no-start-steam`. With it off, the launch goes ahead whether Steam runs or not. If Steam is open and the launch still waits for it, turn the setting off and tell the developers with [Send Logs to Developers](/using/send-logs).

The launcher doesn't start Steam for a game folder outside a Steam library, or in Steam Deck Game Mode.

| | Running and signed in | Started with |
|---|---|---|
| Windows | `HKCU\Software\Valve\Steam\ActiveProcess`: `pid` is a running `steam.exe` and `ActiveUser` isn't 0 | `SteamExe` or `SteamPath` from `HKCU\Software\Valve\Steam`, with `-silent`; otherwise `steam://open/main` |
| Linux | `~/.steam/steam.pid` or `registry.vdf` names a running `steam` process. It's signed in when `ActiveUser` in `registry.vdf` isn't 0, or when `config/loginusers.vdf` changed after Steam started | `steam -silent` |
| Flatpak Steam | The same files under `~/.var/app/com.valvesoftware.Steam/` | `flatpak run com.valvesoftware.Steam -silent` |

`swofty run` and the launcher's log (`$DATA/state/launcher-run.log`) show each step on `[steam]` lines:

```text
[steam] detected: not running
[steam] starting Steam: /usr/bin/steam -silent
[steam] waiting for sign-in (pid 4242, ActiveUser 0) at 3.5 s
[steam] ready after 9.0 s (pid 4242, ActiveUser 12345678)
```

`swofty steam` shows the same check on its own, and `swofty steam --start` runs the start and the wait without launching the game. `swofty doctor` shows it in its `steam client` row.

## Linux: the Steam wrapper

Set Steam's launch option to `swofty run -- %command%` so its Play button starts Swofty Launcher for F1 25. The launcher uses your configured Proton version and records session logs.

To go back to Steam's normal launch, clear the option.

Without the wrapper in the launch option, the launcher's **Play** and `swofty run --direct` start Proton themselves. Proton reads wheels and controllers through SDL2. When the system has no 64-bit SDL2, as under NixOS `steam-run`, the launcher links the copy from Steam's runtime into `$DATA/state/runtime-libs` and adds it to the game's library path. `swofty run` then prints `controllers: this system has no SDL2`.

## Windows

Select **Play** in the launcher or run `swofty run`. If Steam is closed, it [starts first](#steam). To follow the logs in the launcher, select the arrow next to **Play** and choose **Play with console**.

Modded sessions don't start while `F1_25.exe` is set to **Run this program as an administrator** and the launcher runs normally: Windows won't start the game unelevated (error 740), and the launch stops with a message naming the setting. To fix it, right-click `F1_25.exe` in the game folder, open **Properties > Compatibility** and untick the box, or untick it under **Change settings for all users** when it's set for all users. You can also run the launcher as administrator. `swofty doctor` shows the setting in its `run as admin` row.

## VR

**Play in VR** and `swofty run --vr` pass `-vr` to the game, the argument of Steam's VR launch option for F1 25. On Linux, SteamVR starts first when it isn't running, and the Steam wrapper passes on the arguments Steam gives the game, so Steam's **Launch in SteamVR mode** works too. See [Playing in VR](/using/vr).

## Logs

You'll find session logs in `$DATA/sessions/<id>/`. Run `swofty logs` for the latest summary, or use `swofty run` to follow the logs as you play.

If Proton can't create the game process, you'll see `Proton could not start the game process (CreateProcess error N)` instead of an exit code in `swofty run`, `swofty logs` and the launcher.

When the game exits, however it exits, the session puts the game folder back to vanilla. To check, run `swofty verify`. See [Restoring the game](/using/profiles-and-restoring) for the cases that need `swofty restore`.

## Next

- [Offline play](/reference/safety): what a modded session turns off.
- [Backups and recovery](/reference/restore): how the session rolls the game folder back.
