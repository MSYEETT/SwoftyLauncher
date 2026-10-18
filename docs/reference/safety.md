# Offline play

When you play a modded session, F1 25 runs in single-player, offline. Multiplayer is off, and you can't turn it on. Launch through Steam normally and you'll play vanilla.

The loader and addons need the game to run without EA AntiCheat, so the launcher disables it for each modded session. It undoes that change every time the game exits.

## The one-time step

The first time you press **Play**, you'll see the **Modded play runs offline** dialog. It explains what you're accepting: modded sessions run offline, without EA AntiCheat, and EA's terms of service do not cover playing without EA AntiCheat even offline.

Select **Accept and launch** to start the game. If you decline, nothing changes and the game won't launch. You only need to accept once; after that, **Play** launches directly.

## Return to vanilla

1. Close the game.
2. Run `swofty restore`.
3. On Linux, clear the launcher's Steam launch option.

If any files need repair, follow [Restoring the game](/using/profiles-and-restoring).
