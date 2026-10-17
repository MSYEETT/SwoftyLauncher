# Profiles

A profile keeps a set of addons, maps and addon options together. Use different profiles for different setups.

`vanilla` is the built-in profile with no addons. It cannot be edited. Modded profiles always include SwoftyMod.

## Profile file

Profiles are stored in `$DATA/profiles/<name>.json`:

```json
{
  "schema_version": 2,
  "name": "grid24",
  "addons": [
    { "id": "swofty.core", "enabled": true },
    { "id": "swofty.f1-2026-grid24", "enabled": true }
  ],
  "maps": ["okayama"],
  "options": {
    "swofty.f1-2026-grid24": { "team_livery": "titlesponsor" }
  }
}
```

- `addons` lists addons in load order. Set `enabled` to `false` to turn one off. Profiles from earlier versions name this list `mods`; it is read the same way and saved as `addons`.
- `maps` lists track codes.
- `options` stores each addon's settings that differ from the default.
- `applied` records the database settings the last build used. When they differ from `options`, the next launch rebuilds the profile.

The command line uses `--profile <name>` to choose a profile. The default is `default`.

## Saves and restoring

Separate saves for each profile are part of the [design](/design/#profiles-and-saves) and are still in development. See [Restoring the game](/using/profiles-and-restoring) to return game files to their original state.
