# Importing maps

Tracks are mods. Import a community track package as a zip on the Team Simplified `Mods` page, by dropping it on the launcher window, or with `swofty mods import`. You do not need to unpack or rebuild it.

## Import a track

```sh
swofty mods import Okayama_v0.17.zip
swofty mods list
```

The package is stored in `$DATA/addon-data/teamsimplified.mods/library/<track_code>/`, with the track package under `track/`. Its mod id is the track code. Import turns the track on in the active profile.

## Enable a track

```sh
swofty mods enable okayama
swofty apply
```

`swofty apply` installs every track enabled in the [profile](/using/profiles). The track then appears in Grand Prix track selection. `swofty mods disable okayama` followed by `swofty apply`, or `swofty restore`, removes it.

Any number of tracks can be enabled in one profile. With an in-game core that cannot register asset groups, the limit is eight.

To remove an imported track:

```sh
swofty mods remove okayama
```

`swofty maps` is a hidden alias of `swofty mods`. Packages in `$DATA/maps/` and the `maps` list of each profile move into the library and the profile's mod list on first start.

## Supported packages

Use the track-package format made for the EGO GameDatabases Editor:

| File | Purpose |
|---|---|
| `manifest.json` | Track name, code and setup information |
| `<code>.erp` | Track data |
| `mipmaps/` | Optional textures |
| `ui/images.json` | Track artwork |
| `ui/minimap.json` | Minimap placement |

## Known issues

- A track's menu card shows the language strings the package names, often its base track's name and stats. Game language files are not editable yet.
- Each map's archive replaces one of the game's invitational event pictures (`invitational_image_4`, `_10`, `_13` and on). Those invitational events keep their pictures.

See [How maps are installed](/reference/maps) for what activation changes.
