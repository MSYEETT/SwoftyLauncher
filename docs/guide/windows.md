# Windows

## Set up

1. Follow [Installation](/guide/installation).
2. Keep Steam running with F1 25 installed.
3. Run `bin\swofty.exe doctor` from the extracted folder to check that the game is found.
4. [Install your addons](/using/addons) and choose a [profile](/using/profiles).

## Play

Open `bin\swofty-launcher.exe` and select **Play**. The first launch shows the [offline play terms](/reference/launch#offline-play-ea-anticheat-and-consent). Run `swofty run` from a terminal to launch and follow the logs instead.

Modded play is offline only. Normal Steam launches stay vanilla.

## If the game crashes

Close the game, then run `swofty restore` before playing vanilla. See [Restoring the game](/using/profiles-and-restoring).

Run `swofty logs` to read the last session's summary. Logs are stored in `$DATA/sessions/<id>`.
