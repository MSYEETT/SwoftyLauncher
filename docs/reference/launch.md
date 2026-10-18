# Launching the game

You'll need to play offline when using mods. To get set up, follow [Windows](/guide/windows) or [Linux and Steam Deck](/guide/linux).

## Linux: the Steam wrapper

Set Steam's launch option to `swofty run -- %command%` so its Play button starts Swofty Launcher for F1 25. The launcher uses your configured Proton version and records session logs.

To go back to Steam's normal launch, clear the option.

## Windows

Keep Steam running, then select **Play** in the launcher or run `swofty run`.

## Logs

You'll find session logs in `$DATA/sessions/<id>/`. Run `swofty logs` for the latest summary, or use `swofty run` to follow the logs as you play.

If Proton can't create the game process, you'll see `Proton could not start the game process (CreateProcess error N)` instead of an exit code in `swofty run`, `swofty logs` and the launcher.

Before you return to vanilla play, close the game, then run `swofty restore`.
