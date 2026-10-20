---
title: Playing in VR
description: Start F1 25 in VR from Swofty Launcher for F1 25, the same way Steam's Launch in SteamVR mode does.
---

# Playing in VR

F1 25 supports VR headsets through SteamVR. The launcher starts it in VR the same way Steam's **Launch in SteamVR mode** option does: it passes `-vr` to the game.

## Start in VR

1. Install SteamVR from Steam and connect your headset.
2. Select the arrow next to **Play** and choose **Play in VR**.

The game starts in VR, and **Play** becomes **Play in VR**. The launcher remembers the choice for the active profile, so the next **Play in VR** starts in VR again. To go back to a normal launch, select the arrow and choose **Play**.

On Linux, the launcher starts SteamVR first when it isn't running, then starts the game. On Windows, the game starts SteamVR itself.

## From the command line

```sh
swofty run --vr
```

On Linux, with the [Steam launch option](/guide/linux) set, Steam's **Launch in SteamVR mode** also starts a modded session in VR. To make every Steam launch start in VR, set the launch option to `swofty run --vr -- %command%`.
