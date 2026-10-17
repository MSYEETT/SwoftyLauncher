# Addons and maps

**Addons** change the game, such as car data, artwork or menus. **Maps** add community tracks, such as Sepang and Okayama.

## Install an addon

An addon is a folder or a `.swoftymod` file. Use these commands while the desktop controls are in development:

```sh
swofty addons install <path>
swofty addons list
swofty addons enable <id>
swofty addons disable <id>
```

Use the addon's ID from `addons list`. Installed addons are stored in `$DATA/addons/<id>/`. `swofty mods` is another name for `swofty addons`.

Earlier versions stored addons in `$DATA/mods/`. The launcher and `swofty` move them to `$DATA/addons/` when they start.

Addons can require other addons or conflict with them. Check any warnings before playing.

## Add a map

See [Importing maps](/using/importing-maps). Maps are stored separately from addons and selected by track code in a [profile](/using/profiles).

## Built-in addon

[SwoftyMod](/using/swofty-mod) adds the in-game ADDON SETTINGS entry. It is always enabled in modded profiles.
