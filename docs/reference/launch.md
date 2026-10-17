# Launching the game

Modded play is offline only. For setup, see [Windows](/guide/windows) or [Linux and Steam Deck](/guide/linux).

## Offline play: EA AntiCheat and consent

Addons run in-process code, which EA AntiCheat (Javelin) blocks, so a modded
session runs without it. The first launch shows the offline play terms and
asks for a typed `accept`; if you decline, the game is not launched and
nothing is changed. The launcher's **Play** shows the same terms as a dialog
the first time: accepting records consent and launches, declining does
nothing. The terms record that this is outside EA's terms of service and
that EA may take action against your account, including suspension or
termination.

| Command | Effect |
|---|---|
| `swofty offline accept` | Show the terms and accept them (also asked on first launch) |
| `swofty offline status` | Consent state, extension files, digests and staging |
| `swofty offline fetch` | Download the pinned extension release and verify its SHA-256 digests |
| `swofty offline install <dir>` | Install from a folder you downloaded yourself |
| `swofty offline withdraw` | Withdraw consent; modded launches are refused |

The extension (a separate project, [Swofty-Developments/F1-OfflinePlay](https://github.com/Swofty-Developments/F1-OfflinePlay))
is verified against digests pinned in the manager at install time and again
before every use, so a tampered or lookalike download is refused however it
arrived. Sessions stay offline: multiplayer is unavailable and cannot be
enabled. Every staged change is journaled and the game folder returns to
vanilla when the game exits, on `swofty restore`, and at the next start.

## Linux: the Steam wrapper

The launch option `swofty run -- %command%` starts Swofty Launcher for F1 25 through Steam. It uses the configured Proton version and records session logs.

With this option set, Steam's Play button uses the launcher. Clear the option to return to Steam's normal launch.

## Windows

Keep Steam running, then use `swofty run`. The desktop Play control is still in development.

## Logs

Session logs are stored in `$DATA/sessions/<id>/`. Use `swofty logs` to read the latest summary, or `swofty run` to follow logs during play.

When Proton cannot create the game process, `swofty run`, `swofty logs` and the launcher report `Proton could not start the game process (CreateProcess error N)` instead of an exit code.

## In-game loader

A modded launch places the loader in the game folder and removes it when the game exits:

| File | Role |
|---|---|
| `version.dll` | Loads every DLL in `swofty/addons/core/`, then `swofty/addons/*.dll`. It also loads `swofty/mods/core/` and `swofty/mods/*.dll` with them, in name order |
| `swofty/addons/core/swofty_core.dll` | SwoftyMod: the ADDON SETTINGS entry, the settings bridge, plugin mixins, addon-added special liveries reported as owned, and the crash log |
| `swofty/addons/core/swofty_gamelog.dll` | Copies the game's own log into `loader.log` |
| `swofty/addons/core/bindings.json`, `symbols.tsv`, `log_channels.tsv` | Game bindings for plugins, function names for crash reports, and the game's log channels |
| `swofty/settings.json` | The profile's addon settings for the session under `addons`, the `ui` block listing the screens to register, and the `owned_items` block listing the ownership keys of unlock rows that addons add |
| `swofty/ui/*.ui` | ADDON SETTINGS and addon screens, registered by `swofty_core.dll` |

With the loader, the build also replaces four archives in `2025_asset_groups/ui_package/`: `flow_frontend.erp`, `persistent.erp`, `databases.erp` and `flow_persistent.erp`. They are built from the game's own copies for the profile's addons and maps. `swofty restore` puts the originals back.

Everything logs to `<game>/swofty/loader.log`. The loader moves the previous log to `loader.prev.log` when it attaches. A session shows and copies `loader.log` only when the loader wrote a new one during that session. `swofty run` shows its lines as `[loader]` and `[core]`, and the launcher console shows them on the Game tab.

- **Crash log:** an exception writes its code, address, registers and a named stack. `ExitProcess` and `TerminateProcess` calls are logged with the caller's stack.
- **Game log:** warnings and errors from every engine channel, and info from the UI and flow channels. Set `SWOFTY_GAME_LOG` to `off`, `info`, `warn` or `error`, and `SWOFTY_GAME_LOG_CHANNELS` to a channel list, in the launch environment to change it.
- **Settings bridge:** changes made in ADDON SETTINGS reach plugins at once and are saved to the profile when the game exits.

The full launch and restore flow is still being connected. See [Current status](/design/#current-status). Close the game and run `swofty restore` before returning to vanilla play.
