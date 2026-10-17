# Importing maps

Import a community track package as a zip. You do not need to unpack or rebuild it.

## Import a track

```sh
swofty maps import Okayama_v0.17.zip
swofty maps list
```

The package is stored in `$DATA/maps/<track_code>/`. Add its track code to your [profile](/using/profiles).

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

## Known issue

A track's menu card may show the base track's name and stats. Game language files are not editable yet.
