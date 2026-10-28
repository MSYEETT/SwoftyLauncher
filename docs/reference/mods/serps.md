---
title: .serps packages
description: Lay out a .serps package or a zip of game files, write mod.serps, import it, and see what the importer reads, compares and merges.
---

# `.serps` packages

A `.serps` package is the Team Simplified suite's mod format: a zip with whole game files under a content root. The Simplified for Swofty addon imports it on the **Mods** page. It compares each file with the clean game files and merges only the differences: database records, archive resources, text and loose files. A zip, 7z or RAR archive or a folder laid out the same way imports too, with or without `mod.serps`.

## 1. Lay out the package

Put your files under an `F1 25/` folder, at the paths they have in the game folder:

```
Scuderia AlphaTauri ORLEN.serps
├── mod.serps
└── F1 25/
    ├── game_package/gamedatabases/gamedatabases.2025.bin
    ├── localisatioa/2025_release/language_eng.lng
    └── shader_package_2025/object_opaque.nefx2.sm51.erp
```

- The content root is the first folder, at most three levels down, whose name starts with `F1 25`.
- Without one, it's the first folder, from the top down to three levels, that holds a game folder: `asset_groups`, `actionmaps`, `audio`, `videos`, `game_package`, `character_package`, `localisation`, `localisatioa`, or a folder whose name ends in `_package`, `_package_shared` or `_asset_groups`, or starts with `shader_package_`.
- `localisatioa/` and `localisation/` both name the stock language file with the same file name.

Ship only the files your mod changes. The importer keeps every difference, so a whole `gamedatabases.2025.bin` from an older game build changes every record the game has updated since.

## 2. Write `mod.serps`

Put your mod's details in `mod.serps` at the package root. It's JSON, and every field is optional. The reader accepts a UTF-8 byte order mark, comments, trailing commas, and camel- or Pascal-case keys.

| Field | Meaning |
|---|---|
| `mod_name` or `name` | The mod's name; `mod_name` wins |
| `author` | The author |
| `version` | The mod's version |
| `game` | The game, such as `F1® 25` |
| `game_version` | The game version the mod was made for, such as `1.0.141.7865` |
| `studio_version` | The Simplified Studio version that made it |

```json [mod.serps]
{
  "mod_name": "Scuderia AlphaTauri ORLEN",
  "author": "RM Designs",
  "version": "1.0",
  "game": "F1® 25",
  "game_version": "1.0.141.7865"
}
```

## 3. Import and build

```sh
swofty mods import "Scuderia AlphaTauri ORLEN.serps"
swofty mods enable rm-designs.scuderia-alphatauri-orlen
swofty build
```

The mod's id is `<author>.<name>`: both lowercased, each run of other characters turned into `-`. `RM Designs` and `Scuderia AlphaTauri ORLEN` give `rm-designs.scuderia-alphatauri-orlen`. Without either, the id comes from the file name. Importing an id that's already in the library replaces that mod, and every profile keeps its place and switch.

`swofty build` prints the mod's ops among the rest. Read it for conflicts with other mods and addons, and for ops waiting on the in-game loader. The **Mods** page shows the same counts on the mod's row ([Mods and tracks](/using/addons#check-an-addon)).

## What the importer accepts

| Input | Claimed when |
|---|---|
| A `.serps` file, 7z or RAR archive | Always, by its extension |
| A zip | It holds `mod.serps` at its root, a folder whose name starts with `F1 25`, or a game folder at most three folders down |
| A folder | As for a zip |
| A track zip | It holds `manifest.json` with a `track_code` at its root, or holds one such zip ([Track packages](./tracks)) |

Encrypted archives aren't supported. Entries with absolute paths or `..` are skipped and counted.

## What the importer reads

Each file under the content root gets a class:

| Path | Class |
|---|---|
| `game_package/gamedatabases/gamedatabases.2025.bin` | Database. It must parse as F1 25's database, or the import fails |
| `localisation/*/language_<code>.lng`, `localisatioa/*/language_<code>.lng` | Text |
| A `.erp` the game ships | Archive |
| Any other file the game ships | Replacement |
| A file the game doesn't ship | Addition |
| `.exe`, `.dll`, `.asi`, and EA AntiCheat files other than `EAAntiCheat.splash.png` | Skipped and named in the result |

## What the importer applies

The importer compares each file with the clean files of the installed game build, never with the game folder as modded, and keeps the differences. At each build, it turns them into [operations](/reference/data/operations):

| Class | Compared by | Changed | Added | Removed | Applied as |
|---|---|---|---|---|---|
| Database | Record key | Same key, other tag or bytes | Key not in stock | Stock key missing from the package | `db.put-record`, `db.remove-record` |
| Archive | Resource name | Type, fragment layout or sizes, or index digest differ | Name not in stock | Stock name missing from the package | `erp.put-resource`, `erp.remove-resource` |
| Text | Language and key | Value differs | Key not in stock | Never | `lang.set` |
| Replacement, Addition | Whole file | Whole file | Whole file | Never | `file.put` |

- Text counts like any other change: every changed string and every key the stock files don't have becomes a `lang.set`, and new keys are added to the game's text. A language the package lacks takes the changes of its `language_eng.lng`, or of its first language file by name.
- Removals apply only when `game_version` matches the installed game. Otherwise they're dropped, and the mod's row says how many and which version the package was made for.
- The launcher skips every database removal of a stock record, whatever the game version, and of a record a track adds, and warns with the count per mod. See [Removing stock records](/reference/ops/database#removing-stock-records).
- A database change touches only the records the package changed, and archives merge one resource at a time.
- Every resource a mod adds or changes goes into an overlay of the `2025_asset_groups/` archive it belongs to. A changed resource may change size there.
- Resources a mod carries in a `shader_package_2025/` archive stay in that archive. The launcher merges them into the game's copy and writes it into the game files, and each GPU build of the archive gets its own copy.
- Team logo texture arrays in `ui_texture*` archives merge slice by slice across every enabled mod.
- When a team rename package fills in the game's own text for the teams it doesn't rename, those defaults don't undo another mod's rename.
- A special livery row the package adds is reported as owned, so the livery shows in the picker.
- A write that repeats a value an earlier mod already put in place is left out.

Without the in-game loader, `lang.set` ops wait and the text doesn't change.

## Parts

A mod can come in parts: folders that each hold game files of their own, such as `2026 Broadcast graphics/shader_package_2025/` beside `Realistic Tyre colours/shader_package_2025/`. A mod with two or more parts imports as one library entry per part, `<mod id>.<part>`, each with its own switch, place in the load order and conflicts.

## Before you share it

- Set `mod_name`, `author`, `version` and `game_version` in `mod.serps`.
- Include a README that credits each author whose files you ship, and get their permission to redistribute those files.
- Don't ship executables or DLLs: the importer skips them.

The `.serps` reader is ported from the Simplified Launcher and is based on Simplified Suite by Team Simplified, [github.com/TeamSimplified/simplified-suite](https://github.com/TeamSimplified/simplified-suite).
