# Importing maps

Import a community track package as a zip. You do not need to unpack or rebuild it.

## Import a track

```sh
swofty maps import Okayama_v0.17.zip
swofty maps list
```

The package is stored in `$DATA/maps/<track_code>/`.

## Enable a track

```sh
swofty maps enable okayama
swofty apply
```

`swofty apply` installs every map enabled in the [profile](/using/profiles). The track then appears in Grand Prix track selection. `swofty maps disable okayama` followed by `swofty apply`, or `swofty restore`, removes it.

Up to eight maps can be enabled in one profile.

To remove an imported map:

```sh
swofty maps remove okayama
```

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
