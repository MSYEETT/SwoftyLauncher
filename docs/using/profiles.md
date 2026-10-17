# Profiles

A profile keeps a set of addons, mods, tracks and addon options together. Use different profiles for different setups.

`vanilla` is the built-in profile with no addons. It cannot be edited. Modded profiles always include SwoftyMod.

## Profile file

Profiles are stored in `$DATA/profiles/<name>.json`:

```json
{
  "schema_version": 2,
  "name": "grid24",
  "addons": [
    { "id": "swofty.core", "enabled": true },
    { "id": "swofty.f1-2026-grid24", "enabled": true },
    { "id": "teamsimplified.mods", "enabled": true }
  ],
  "options": {
    "swofty.f1-2026-grid24": { "team_livery": "titlesponsor" }
  },
  "state": {
    "teamsimplified.mods": {
      "mods": [{ "id": "okayama", "enabled": true }]
    }
  }
}
```

- `addons` lists addons in load order. Set `enabled` to `false` to turn one off. Profiles from earlier versions name this list `mods`; it is read the same way and saved as `addons`.
- `state` holds each addon's own data for the profile. Team Simplified keeps the profile's mods and tracks there, in load order. Profiles from earlier versions list tracks in `maps`; they move into `state` when the launcher starts.
- `options` stores each addon's settings that differ from the default.
- `applied` records the database settings the last build used. When they differ from `options`, the next launch rebuilds the profile.

The command line uses `--profile <name>` to choose a profile. The default is `default`.

## Saves and restoring

Separate saves for each profile are part of the [design](/design/#profiles-and-saves) and are still in development. See [Restoring the game](/using/profiles-and-restoring) to return game files to their original state.
