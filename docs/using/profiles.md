# Profiles

A profile keeps a set of mods, maps and mod options together. Use different profiles for different setups.

`vanilla` is the built-in profile with no mods. It cannot be edited. Modded profiles always include SwoftyMod.

## Profile file

Profiles are stored in `$DATA/profiles/<name>.json`:

```json
{
  "schema_version": 1,
  "name": "grid24",
  "mods": [
    { "id": "swofty.core", "enabled": true },
    { "id": "swofty.f1-2026-grid24", "enabled": true }
  ],
  "maps": ["okayama"],
  "options": {
    "swofty.f1-2026-grid24": { "team_livery": "titlesponsor" }
  }
}
```

- `mods` lists mods in load order. Set `enabled` to `false` to turn one off.
- `maps` lists track codes.
- `options` stores each mod's settings that differ from the default.
- `applied` records the database settings the last build used. When they differ from `options`, the next launch rebuilds the profile.

The command line uses `--profile <name>` to choose a profile. The default is `default`.

## Saves and restoring

Separate saves for each profile are part of the [design](/design/#profiles-and-saves) and are still in development. See [Restoring the game](/using/profiles-and-restoring) to return game files to their original state.
