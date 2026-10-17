# Importing maps

## The package format

A track package is a zip in the format the EGO GameDatabases Editor uses:

| Entry | Contents |
|---|---|
| `manifest.json` | Track code, name, author, version, activation, ERS values, directories to copy, UI files |
| `<code>.erp` | The track: geometry, world files, AI data, cameras and its own UI art |
| `mipmaps/` | Optional texture mipmaps |
| `ui/images.json` | Track-installer art resources |
| `ui/minimap.json` | Minimap placement |

Its `manifest.json` has these top-level keys:

| Key | Meaning |
|---|---|
| `track_code` | The track id. Either an empty slot that ships with the game (`bahrain24`) or a new id (`okayama`) |
| `track_name`, `author_name`, `version` | Shown in the library |
| `erp` | The ERP file name |
| `activation.copy_from_track` | The stock track whose database rows the new track starts from |
| `activation.copy_slot` | Clones a slot and creates a new track id, instead of filling an existing empty slot |
| `activation.overrides` | Database values per dataset, `{path, value, datasets?}` |
| `activation.track_overrides` | Track-level database values, `{path, value, datasets?}` |
| `ers` | ERS values for the track |
| `copy_directories` | `{source, destination}` pairs, such as `mipmaps/` into the track's environment folder |
| `ui` | `images_file`, `minimap_file` and `schema_version` |

An override without `datasets` applies to every dataset. Datasets include `f1`, `f1_2025`, `f1_2026`, `sprint_f1_2025` and `sprint_f1_2026`. Values are strings, with arrays comma-joined.

## Import

In the launcher: Library, Maps, **Import**, and pick the zip.

From the command line:

```sh
swofty maps import Okayama_v0.17.zip
swofty maps list
swofty maps remove okayama
```

The package is stored unchanged under `$DATA/maps/<track_code>/`. Importing a package with a track code that is already installed replaces it after confirmation.

## What the launcher does with it

The ERP is never rebuilt. At build time an adapter turns the public JSON into the launcher's own operations:

| Package | Becomes |
|---|---|
| `activation.copy_slot`, `activation.copy_from_track` | `add-row` clones of the track tables and the per-dataset track rows, keyed by `_metadata_` and `track` |
| `activation.overrides`, `activation.track_overrides` | `set` operations on those dotted database paths, scoped by `datasets` |
| `ers` | `set` operations |
| `copy_directories` | Loose files in the overlay |
| `ui/*.json` | Recorded for the UI and texture stage |
| `<code>.erp` | Mounted into the overlay as it is |

The adapter is a separate concept from mods: a map cannot carry native code or UI patches.

## Known issues

- The track menu card shows the base track's stats and names. Packages point their name fields at existing language keys (Okayama uses Japan's), and the game's language files are not editable yet.
