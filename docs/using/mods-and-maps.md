# Mods and maps

The library has two sections, **Mods** and **Maps**. They are separate kinds of content, installed, listed and enabled separately.

## Mods

A mod is a directory or a `.swoftymod` zip with a [`swofty.json` manifest](/modding/manifest). It can change:

- **the game database**, by named path, with row additions and removals ([database operations](/modding/database-ops));
- **assets** inside ERP archives: replace, add, copy or remove resources ([ERP operations](/modding/erp-ops));
- **the UI**, as node-graph patches over `!pkg` resources ([UI patches](/modding/ui-patches));
- **loose files** in the game folder's view, delivered through the overlay;
- **native code**, as plugin DLLs loaded by the in-game loader, with consent ([native plugins](/modding/native-plugins)).

Mods have ids, versions, dependencies, a load order and options. Conflicts between mods are reported before the game starts.

Install a mod from the launcher (Library, Mods, Install) or with `swofty mods install <path>`. It is stored under `$DATA/mods/<id>/`.

## Maps

A map is a track package in the community format that the EGO GameDatabases Editor uses: a zip with `manifest.json`, one `<code>.erp` and optional extras. The launcher imports it as it ships, without repacking the ERP. See [Importing maps](/using/importing-maps).

Maps are listed under Library, Maps, and are enabled per profile by their track code.

## SwoftyMod

[SwoftyMod](/using/swofty-mod) (`swofty.core`) is a built-in mod. It is always enabled and cannot be disabled or removed.

## What a session contains

A profile names the mods and maps to enable. At Play, the launcher resolves the profile, builds its files from all of it in load order, and starts a modded session. Every change to the game folder is journalled and undone when the session ends; see [Profiles and restoring the game](/using/profiles-and-restoring).
