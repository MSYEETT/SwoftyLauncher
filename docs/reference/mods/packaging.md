---
title: Packaging a mod
description: Ship a mod as an addon folder or as a Team Simplified .serps package, and what the launcher's importer reads from each.
---

# Packaging a mod

A mod ships one of two ways:

| Package | Made with | Players install it |
|---|---|---|
| An addon folder with `swofty.json` | A text editor and the [operations](/reference/mods/#operations) | On the **Addons** page, by drop, or `swofty addons install` |
| A `.serps` package | Simplified Studio, or by hand | On the **Mods** page, by drop, or `swofty mods import` |

An addon folder applies exactly the operations it lists and can carry settings. A `.serps` package carries whole game files, and the launcher works out what they change.

## As an addon folder

A mod ships as a folder with `swofty.json` at its root and every file its operations name. Players drop the folder, a zip of it or a `.swoftymod` file (a zip under another name) on the launcher, or install it from the command line:

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

The built-in Team Simplified addon imports `.serps` packages into the library on the [Mods page](/using/mods). The format is the Simplified Suite's.

```
Scuderia AlphaTauri ORLEN.serps
├── mod.serps
└── F1 25/
    ├── game_package/gamedatabases/gamedatabases.2025.bin
    ├── localisatioa/2025_release/language_eng.lng
    └── shader_package_2025/object_opaque.nefx2.sm51.erp
```

### mod.serps

`mod.serps` at the package root is JSON. Every field is optional.

| Field | Meaning |
|---|---|
| `mod_name` or `name` | The mod's name. `mod_name` wins. |
| `author` | The author |
| `version` | The mod's version |
| `game` | The game, such as `F1® 25` |
| `game_version` | The game version the mod was made for, such as `1.0.141.7865` |
| `studio_version` | The Simplified Studio version that made it |

The reader accepts a UTF-8 byte order mark, comments, trailing commas, and keys in camel or Pascal case.

### What the importer accepts

| Input | Claimed when |
|---|---|
| A `.serps` file | Always |
| A zip | It holds `mod.serps` or an `F1 25/` folder at its root or under one top folder, or its top folders are game folders |
| A folder | It holds `mod.serps` or an `F1 25` folder |

A `.serps` file is a zip. 7z and RAR archives are refused.

### The content root

The content root holds files in the game folder's layout:

1. An `F1 25` folder within three levels of the package root.
2. Otherwise, the first folder within three levels whose subfolders are game folders: `asset_groups`, `actionmaps`, `audio`, `videos`, `game_package`, `character_package`, `localisation`, `localisatioa`, or a name that ends in `_package`, `_package_shared` or `_asset_groups`, or starts with `shader_package_`.

`localisatioa/` is the folder the Simplified launcher points a patched game at. The importer reads it as `localisation/`.

### Files

| Path under the content root | Imported as |
|---|---|
| `game_package/gamedatabases/gamedatabases.2025.bin` | Database changes. It must parse as F1 25's database, or the import fails. |
| `localisation/*/language_<code>.lng`, `localisatioa/*/language_<code>.lng` | Text changes |
| A `.erp` the game ships | Archive changes |
| Any other file the game ships | A replacement |
| A file the game does not ship | An addition |
| `.exe`, `.dll`, `.asi` | Skipped and named in the result |

Entries with absolute paths, `..`, drive letters or symbolic links are skipped and counted.

### The mod's id

The id is the author and the name, lowercased, with every run of other characters turned into `-`, joined by a dot: `RM Designs` and `Scuderia AlphaTauri ORLEN` give `rm-designs.scuderia-alphatauri-orlen`. Without both, the file name is used. Importing a package with an id already in the library replaces it.

### What the launcher applies

The importer compares each file with the clean game files of the installed build and keeps the differences:

| Class | Compared by | Applied as |
|---|---|---|
| Database | Record | The records the mod changes or adds (`db.put-record`), and the ones it removes (`db.remove-record`) |
| Archive | Resource | The resources the mod changes or adds (`erp.put-resource`) |
| Text | Language and key | The strings the mod changes or adds (`lang.set`) |
| Replacement, addition | Whole file | `file.put` |

- Two mods that change different records of the database or different resources of an archive combine. Where they change the same one, the mod lower in the player's list wins.
- Removals of database records apply only when `game_version` matches the installed game. Otherwise the player sees how many were left out.
- Removals of resources from archives are not applied.
- A whole language file changes every string that differs from the installed game's, including strings the game changed after the mod was made. A language the package lacks takes the changes of its `language_eng.lng`, or of its first language file by name.
- Text changes need the in-game loader.

Ship only the files the mod changes. A whole `gamedatabases.2025.bin` from an older game build changes every record the game has updated since.

### Tracks

A community track ships as the zip made for the EGO GameDatabases Editor ([Tracks](./tracks)). The importer claims a zip with `manifest.json` at its root, or a zip that holds exactly one such zip, and stores it as a mod whose id is the track code.

The `.serps` reader, the content root rules and the entry checks are ported from the Simplified Launcher. Based on Simplified Suite by Team Simplified, [github.com/TeamSimplified/simplified-suite](https://github.com/TeamSimplified/simplified-suite).
