---
title: Packaging a mod
description: Package your mod as a .swoftyaddon file or a Team Simplified .serps package, and learn what the importer reads from each.
---

# Packaging a mod

You can share your mod in either of these formats:

| Package | Made with | Players install it |
|---|---|---|
| A `.swoftyaddon` file | `zip`, from the mod folder | On the **Addons** page, by drop, or `swofty addons install` |
| A `.serps` package | Simplified Studio, or by hand | On the **Mods** page, by drop, or `swofty mods import` |

The difference is how you describe your changes. With a `.swoftyaddon`, you ship the mod folder and the exact operations it lists. With a `.serps` package, you supply whole game files and let the importer work out what they change.

## As a `.swoftyaddon`

A `.swoftyaddon` is a zip of your mod folder's contents. The manifest, `swofty.json`, has to sit at the zip root, not inside a subfolder, so change into the folder before zipping it:

```sh
cd my-mod
zip -r ../yourname.my-mod.swoftyaddon .
```

Then install and enable it:

```sh
swofty addons install yourname.my-mod.swoftyaddon
swofty addons enable yourname.my-mod
```

You can pass `swofty addons install` the `.swoftyaddon` file, a plain `.zip` with the same layout, or the folder itself. If you have a `.swoftymod` file from an earlier version, that installs too.

Before you zip:

- Set `id`, `name`, `version`, `author` and `description` in [`swofty.json`](/reference/addons/manifest).
- Set `compat.game_builds` and `compat.seasons` to the builds and seasons your mod was made for.
- Remove any file that none of your operations name.
- Include a README crediting each author whose files you ship, and get their permission to redistribute those files. Never include the game's own archives, database or executable.

## As a `.serps` package

To share whole game files, use a `.serps` package. This is the Simplified Suite's format, which the built-in Team Simplified addon imports: a zip with your game files under a content root, as in this layout:

```
Scuderia AlphaTauri ORLEN.serps
├── mod.serps
└── F1 25/
    ├── game_package/gamedatabases/gamedatabases.2025.bin
    ├── localisatioa/2025_release/language_eng.lng
    └── shader_package_2025/object_opaque.nefx2.sm51.erp
```

Import it:

```sh
swofty mods import "Scuderia AlphaTauri ORLEN.serps"
```

### mod.serps

Put your mod's details in `mod.serps` at the package root. It's JSON, and every field is optional. The reader also accepts a UTF-8 byte order mark, comments, trailing commas, and camel- or Pascal-case keys.

| Field | Meaning |
|---|---|
| `mod_name` or `name` | The mod's name; `mod_name` wins |
| `author` | The author |
| `version` | The mod's version |
| `game` | The game, such as `F1® 25` |
| `game_version` | The game version the mod was made for, such as `1.0.141.7865` |
| `studio_version` | The Simplified Studio version that made it |

### What the importer accepts

| Input | Claimed when |
|---|---|
| A `.serps` file | Always |
| A zip | It holds `mod.serps` or an `F1 25/` folder at its root or under one top folder, or its top folders are game folders |
| A folder | It holds `mod.serps` or an `F1 25` folder |

Inside the content root, arrange your files as they appear in the game folder. The importer uses an `F1 25` folder within three levels of the package root, or the first folder within three levels whose subfolders are game folders (`asset_groups`, `game_package`, `localisation`, a name ending in `_package`, and so on). It reads `localisatioa/` as `localisation/`.

### What the importer applies

To work out what your mod changes, the importer compares each file with the clean files of the installed build. It keeps the differences:

| Path under the content root | Compared by | Applied as |
|---|---|---|
| `game_package/gamedatabases/gamedatabases.2025.bin` | Record | `db.put-record` for changed and added records, `db.remove-record` for removed ones |
| A `.erp` the game ships | Resource | `erp.put-resource` for changed and added resources, `erp.remove-resource` for removed ones |
| `localisation/*/language_<code>.lng`, `localisatioa/*/…` | Language and key | `lang.set` for each changed or added string |
| Any other game file | Whole file | `file.put` |
| A file the game does not ship | Whole file | `file.put` (addition) |
| `.exe`, `.dll`, `.asi` | — | Skipped and named in the result |

Your database file must parse as F1 25's database, or the import fails. Record and resource removals apply only when `game_version` matches the installed game. Otherwise, the player sees how many removals were left out.

The importer builds your mod's id from its author and name: it lowercases both, turns each run of other characters into `-`, then joins them with a dot. For example, `RM Designs` and `Scuderia AlphaTauri ORLEN` give `rm-designs.scuderia-alphatauri-orlen`. If that id is already in the library, importing replaces the existing mod.

Ship only the files your mod changes. Because the importer keeps every difference, a whole `gamedatabases.2025.bin` from an older build changes every record the game has updated since.

### Track packages

If you're sharing a community track, use the EGO GameDatabases Editor zip, not a `.serps` ([Tracks](./tracks)). The importer recognises a zip with `manifest.json` at its root and stores it as a mod with the track code as its id.

The `.serps` reader is ported from the Simplified Launcher and is based on Simplified Suite by Team Simplified, [github.com/TeamSimplified/simplified-suite](https://github.com/TeamSimplified/simplified-suite).
