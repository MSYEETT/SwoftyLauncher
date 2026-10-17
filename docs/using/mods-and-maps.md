# Mods and maps

**Mods** change the game, such as car data, artwork or menus. **Maps** add community tracks, such as Sepang and Okayama.

## Install a mod

A mod is a folder or a `.swoftymod` file. Use these commands while the desktop controls are in development:

```sh
swofty mods install <path>
swofty mods list
swofty mods enable <id>
swofty mods disable <id>
```

Use the mod's ID from `mods list`. Installed mods are stored in `$DATA/mods/<id>/`.

Mods can require other mods or conflict with them. Check any warnings before playing.

## Add a map

See [Importing maps](/using/importing-maps). Maps are stored separately from mods and selected by track code in a [profile](/using/profiles).

## Built-in mod

[SwoftyMod](/using/swofty-mod) adds the in-game ADDON SETTINGS entry. It is always enabled in modded profiles.
