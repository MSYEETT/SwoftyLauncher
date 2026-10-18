# Linux and Steam Deck

You run F1 25 through Steam and Proton. With mods, you'll be playing single-player and offline.

## Set up

1. Follow [Installation](/guide/installation) to download and extract the app.
2. Start Steam. You need F1 25 installed and a Proton version selected for it.
3. Open F1 25 in Steam → the gear icon → **Properties** → **General**.
4. To have Steam launch the modded game, enter this line in **Launch options**. Replace `<extracted folder>` with the full path of the folder you extracted:

   ```text
   <extracted folder>/bin/swofty run -- %command%
   ```

You can now start the modded game with either Steam's **Play** button or the launcher's **Play** button. To launch vanilla through Steam again, clear the launch option.

If you use **Flatpak Steam**, extract the archive inside `~/.var/app/com.valvesoftware.Steam/`. Use that folder's `bin/swofty` in the launch option.

## Play

1. Open `bin/swofty-launcher`.
2. [Install your addons](/using/addons) and [mods](/using/mods), then pick a [profile](/using/profiles).
3. Press **Play**.
4. On your first launch, you'll see the **Modded play runs offline** dialog. Select **Accept and launch**. For details, see [Offline play](/reference/safety).

To see live logs as you play, keep Steam running and run `swofty run` from a terminal.

## Steam Deck

Extract the app and set the launch option in Desktop Mode. Once that's set, you can use **Play** in the Deck's library to start the modded game in Game Mode too.

## If the game crashes

1. Close the game.
2. Run `swofty restore` before playing vanilla. See [Restoring the game](/using/profiles-and-restoring).

## Advanced launch settings

If your setup needs CPU affinity or environment variables, set them in `$DATA/config.json`:

```json
{
  "launch": {
    "cpu_affinity": "0-11,14-31",
    "env": { "VKD3D_SHADER_CACHE_PATH": "0" }
  }
}
```

You can use NixOS too. To run the latest code instead of a release, [build from source](/reference/development).
