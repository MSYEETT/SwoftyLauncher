# Windows

## Set up

1. Follow [Installation](/guide/installation) to download and extract the app.
2. Make sure you have F1 25 installed, then start Steam.
3. Open `bin\swofty-launcher.exe`.

Watch the status line next to **Play**: you'll see **Detecting F1 25…** while the launcher looks for the game, then **Ready** when it finds it.

## If the game is not found

If the launcher can't find the game, you'll see **Install it with Steam or locate its folder** in the status line and a **Locate game** button. To point it to your installation:

1. Select **Locate game**.
2. Choose the F1 25 folder — look for `F1_25.exe` inside it. You'll usually find it at `...\steamapps\common\F1 25`.

## Play

1. [Install your addons](/using/addons) and [mods](/using/mods), then pick a [profile](/using/profiles).
2. Press **Play**.
3. On your first launch, select **Accept and launch** in the **Modded play runs offline** dialog. For why modded play runs offline, see [Offline play](/reference/safety).

You'll see **Launching…** on the button, then **Stop** while the game runs. When you want to play vanilla, launch through Steam as usual.

To follow the logs as you play, select the arrow next to **Play** and choose **Play with console**, or run `swofty run` from a terminal.

## If the game crashes

1. Close the game.
2. Before you play vanilla again, run `swofty restore`. See [Restoring the game](/using/profiles-and-restoring) for the details.
3. To read a summary of your last session, run `swofty logs`. You'll find the logs in `$DATA\sessions\<id>`.
