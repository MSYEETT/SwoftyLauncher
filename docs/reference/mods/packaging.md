---
title: Packaging a mod
description: Ship a mod as an addon folder today, and the Team Simplified .serps format the launcher's Mods section will import.
---

# Packaging a mod

## As an addon folder

A mod ships as a folder with `swofty.json` at its root and every file its operations name. Players install the folder:

```sh
swofty addons install my-mod
swofty addons enable author.my-mod
```

Before shipping:

- Set `id`, `name`, `version`, `author` and `description` in [`swofty.json`](/reference/addons/manifest).
- Set `compat.game_builds` and `compat.seasons` to the game builds and seasons the mod was made for.
- Remove files no operation names.
- Ship a README that credits every author whose files the mod carries, and get their permission to redistribute them.

## As a .serps package

<Badge type="info" text="coming soon" /> The built-in Team Simplified addon will import `.serps` packages made with Simplified Studio in the launcher's **Mods** section. The format below is the one the importer is built against ([design](/design/addons-and-mods#_4-2-the-serps-format)).

- A package is a zip archive under any name, `.serps` included, or a folder.
- `mod.serps` at the package root is JSON: `mod_name` or `name` (`mod_name` wins), `author`, `version`, `game` (for example `F1® 25`), `game_version` (for example `1.0.141.7865`) and `studio_version`. Every field is optional.
- `F1 25/` holds whole files in the game folder's layout, such as `game_package/gamedatabases/gamedatabases.2025.bin`, language files under `localisation/` or `localisatioa/`, and archives.
- Without `F1 25/`, the content root is the first folder, within three levels, whose subfolders match top-level folders of the installed game, or `localisation` or `localisatioa`.

| Path | Imported as |
|---|---|
| `game_package/gamedatabases/gamedatabases.2025.bin` | Database changes |
| `localisation/*/language_<code>.lng`, `localisatioa/*/language_<code>.lng` | Text changes |
| A `.erp` the game ships | Archive changes |
| Any other file the game ships | A replacement |
| A file the game does not ship | An addition |
| `.exe`, `.dll`, `.asi` | Skipped |

The importer compares each file with the stock game and keeps only the changes, so mods that change different rows of one file combine.
