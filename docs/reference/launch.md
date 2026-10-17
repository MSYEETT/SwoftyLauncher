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

Keep Steam running, then select **Play** in the launcher or run `swofty run`.

## Logs

Session logs are stored in `$DATA/sessions/<id>/`. Use `swofty logs` to read the latest summary, or `swofty run` to follow logs during play.

When Proton cannot create the game process, `swofty run`, `swofty logs` and the launcher report `Proton could not start the game process (CreateProcess error N)` instead of an exit code.

Close the game and run `swofty restore` before returning to vanilla play.
