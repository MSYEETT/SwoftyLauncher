# Linux and Steam Deck

On Linux the game runs under Proton. The launcher keeps Steam's Proton, the Steam Linux Runtime, the prefix and ownership, and replaces only the target: it runs `F1_25.exe` directly in place of the EA AntiCheat launcher. The anticheat does not run under Proton.

## Set up

1. Install the launcher. <span class="todo">TODO: AppImage or archive name</span>
2. Start it. Setup finds the Steam roots (`~/.steam/steam`, the native install and Flatpak Steam), the libraries, the F1 25 install, its `compatdata` prefix and the configured Proton.
3. Set the launch option for F1 25 in Steam (Properties, General, Launch options):

   ```
   ~/.local/share/swofty/bin/swofty run -- %command%
   ```

   With Steam closed, the launcher can write the option into `localconfig.vdf` itself. For Flatpak Steam the path is `~/.var/app/com.valvesoftware.Steam/data/swofty/bin/swofty`.

## Play

Press Play in Steam, or Play in the launcher (which asks Steam to start the game). Steam runs the wrapper, which:

1. reads Steam's command line: the Proton verb, the runtime entry, the environment and the prefix;
2. builds the session (overlay, session file, ticket);
3. places `version.dll` and `swofty/` in the game folder, recorded in the restore journal;
4. adds the prefix-scoped DLL override for `version`;
5. runs the Proton chain on `F1_25.exe`, with the game folder as the working directory;
6. waits for the game, then restores the game folder and merges in-game changes.

If the wrapper does not recognise Steam's command line, it shows an error, logs it, and does not launch.

Run from a terminal, `swofty run` streams the loader log and Wine and Proton errors live, and prints a crash summary when the game exits. See [Command line](/reference/cli#swofty-run).

## The DLL override

Wine uses its builtin `version.dll` unless told otherwise. The wrapper scopes the override to `F1_25.exe` in the prefix's `user.reg`:

```
[Software\\Wine\\AppDefaults\\F1_25.exe\\DllOverrides]
"version"="native,builtin"
```

It also sets `WINEDLLOVERRIDES="version=n,b"` for the child processes. Leaving the key in place without a `version.dll` is harmless, because Wine falls back to the builtin.

## CPU affinity and environment

Affinity and environment variables that used to live in the launch option move into `$DATA/config.json`:

```json
{
  "launch": {
    "cpu_affinity": "0-11,14-31",
    "env": { "VKD3D_SHADER_CACHE_PATH": "0" }
  }
}
```

The wrapper applies the affinity before it starts Proton, and the game inherits it.

## Steam Deck

The same flow works on SteamOS. Use the launcher in desktop mode to set up mods and profiles. In Game Mode, Play goes through the wrapper, which builds headless, and MOD SETTINGS opens with R3.

## NixOS

Steam runs in an FHS environment on NixOS, and `swofty` is a normal glibc binary, so the launch option works unchanged. The repository's `flake.nix` has a development shell for building from source.

## Flatpak Steam

The data directory is `~/.var/app/com.valvesoftware.Steam/data/swofty`, which the Steam sandbox can see. Use that path in the launch option.

## Other Wine setups

Steam for Windows inside Lutris, Heroic or Bottles is best effort:

```
swofty run --wine <wine binary> --prefix <prefix dir> --game <path to F1_25.exe>
```

The Windows Steam client must be running in that prefix.
