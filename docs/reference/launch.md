# Launching the game

Modded play is offline only. For setup, see [Windows](/guide/windows) or [Linux and Steam Deck](/guide/linux).

## Linux: the Steam wrapper

The launch option `swofty run -- %command%` starts F1 Mod Manager through Steam. It uses the configured Proton version and records session logs.

With this option set, Steam's Play button uses the manager. Clear the option to return to Steam's normal launch.

## Windows

Keep Steam running, then use `swofty run`. The desktop Play control is still in development.

## Logs

Session logs are stored in `$DATA/sessions/<id>/`. Use `swofty logs` to read the latest summary, or `swofty run` to follow logs during play.

## In-game loader

A modded launch places the loader in the game folder and removes it when the game exits:

| File | Role |
|---|---|
| `version.dll` | Loads every DLL in `swofty/mods/core/`, then `swofty/mods/*.dll` |
| `swofty/mods/core/swofty_core.dll` | SwoftyMod: the MOD SETTINGS entry, the settings bridge, plugin mixins and the crash log |
| `swofty/mods/core/swofty_gamelog.dll` | Copies the game's own log into `loader.log` |
| `swofty/mods/core/bindings.json`, `symbols.tsv`, `log_channels.tsv` | Game bindings for plugins, function names for crash reports, and the game's log channels |
| `swofty/settings.json` | The profile's mod settings for the session |

Everything logs to `<game>/swofty/loader.log`, which is copied to the session folder when the game exits. `swofty run` shows its lines as `[loader]` and `[core]`.

- **Crash log:** an exception writes its code, address, registers and a named stack. `ExitProcess` and `TerminateProcess` calls are logged with the caller's stack.
- **Game log:** warnings and errors from every engine channel, and info from the UI and flow channels. Set `SWOFTY_GAME_LOG` to `off`, `info`, `warn` or `error`, and `SWOFTY_GAME_LOG_CHANNELS` to a channel list, in the launch environment to change it.
- **Settings bridge:** changes made in MOD SETTINGS reach plugins at once and are saved to the profile when the game exits.

The full launch and restore flow is still being connected. See [Current status](/design/#current-status). Close the game and run `swofty restore` before returning to vanilla play.
