# Linux and Steam Deck

F1 25 runs through Steam and Proton. Modded play is offline only.

## Set up

1. Follow [Installation](/guide/installation).
2. Run `bin/swofty doctor` from the extracted folder to check Steam, F1 25 and Proton.
3. In Steam, open F1 25 → **Properties** → **General** → **Launch options**.
4. Add this option, with `<extracted folder>` replaced by the full path of the folder you extracted:

   ```text
   <extracted folder>/bin/swofty run -- %command%
   ```

For Flatpak Steam, extract the archive inside `~/.var/app/com.valvesoftware.Steam/` and use that folder's `bin/swofty`.

## Play

Open `bin/swofty-launcher` and select **Play**. With the launch option set, the launcher starts the game through Steam, and Steam's **Play** button uses Swofty Launcher for F1 25 too. Clear the option to return to Steam's normal launch.

Run `swofty run` from a terminal to see live logs and a crash summary. Steam must be running. See [Command line](/reference/cli#swofty-run).

## Steam Deck

Use Desktop Mode for setup. The Steam launch option also works in Game Mode.

## NixOS

The repository includes a development shell. Run `nix develop` before [building from source](/reference/development).

## Advanced launch settings

CPU affinity and environment variables go in `$DATA/config.json`:

```json
{
  "launch": {
    "cpu_affinity": "0-11,14-31",
    "env": { "VKD3D_SHADER_CACHE_PATH": "0" }
  }
}
```

Only set these if your setup needs them.
