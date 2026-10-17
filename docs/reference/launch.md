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

The loader consists of `version.dll` and `swofty_core.dll`. It loads plugins and adds the MOD SETTINGS menu entry. Its log is `<game>/swofty/loader.log`.

The full launch and restore flow is still being connected. See [Current status](/design/#current-status). Close the game and run `swofty restore` before returning to vanilla play.
