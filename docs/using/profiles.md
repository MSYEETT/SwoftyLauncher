# Profiles

A profile is a named set of enabled mods, enabled maps and option values. Each modded session runs one profile.

## File format

Profiles live in `$DATA/profiles/<name>.json`:

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

- `mods` lists mods in load order. A disabled mod keeps its place and its options.
- `maps` lists enabled track codes.
- `options` holds per-mod option values; an option that is missing takes the mod's default.
- `swofty.core` is always present and always enabled. The launcher adds it if it is missing and ignores `"enabled": false` for it.

`vanilla` is a built-in profile that cannot be edited. It launches the game exactly as Steam's own Play does.

## Builds, saves and restoring

Each profile has its own build cache, keyed by a hash of the game build, the ordered mods with their versions and content hashes, and the option values. Switching back to a profile reuses its build. Each profile also has its own saves, kept away from vanilla saves and Steam Cloud.

How profiles, the baseline and the journal keep the game folder restorable is in [Profiles and restoring the game](/using/profiles-and-restoring).

## Changes made in game

Changing an option or toggling a mod in the in-game settings page writes a request to `state/requests/<profile>.json`. The launcher merges it into the profile when the game exits. Options bound to the database need **Apply and restart**, which rebuilds and relaunches without returning to Steam. Options bound to native code take effect immediately.
