---
title: Addons
description: An addon is a folder or .swoftyaddon with a swofty.json. It can change game data through operations, run an engine plugin in the game, run a launcher plugin, or all three. The five steps to make one.
---

# Addons

An addon is a folder with a `swofty.json` at its root, shared as a `.swoftyaddon` file and installed on the launcher's **Addons** page. The manifest lists what the addon ships:

- **Data.** [Operations](/reference/data/operations) that change the game's database, text, archives and files. No code runs. An addon with only operations is a **data addon**.
- **Code.** An [engine plugin](./native-plugins), a DLL that runs inside F1 25, and a [launcher plugin](./launcher-api), a program that adds pages and commands to the launcher. An addon with either is a **code addon**.

One addon can ship data and code together. Content players drop on the **Mods** page (`.serps` packages, track zips) is a mod, not an addon; see [Mods](/reference/mods/).

## Make an addon

1. **Create the folder and `swofty.json`.** Give the addon an `id`, `name`, `version` and `author`; [Your first addon](./first-addon) walks through it, and [the manifest](./manifest) lists every field.
2. **Add operations, code or both.** List data changes under `operations` ([Data addons](/reference/data/)), name a DLL in `operations.native` ([Engine plugins](./native-plugins)), or name a program in `launcher` ([Launcher plugins](./launcher-api)). A [template](./template) gives you a working start.
3. **Add settings.** Declare `options` so players can change your addon from the launcher and in game ([Settings](./settings)).
4. **Build and test.** Install the folder, run `swofty build`, read the warnings, then play with `swofty run --apply` ([Build and test](./publishing#build)).
5. **Package and publish.** Zip the folder's contents as a `.swoftyaddon` and share it ([Packaging and publishing](./publishing#package-it)).

The smallest addon installs and does nothing:

```json [swofty.json]
{
  "schema_version": 1,
  "id": "me.my-addon",
  "name": "My Addon",
  "version": "0.1.0",
  "author": "Me",
  "compat": { "game": "f1_25" }
}
```

```sh
swofty addons install ./my-addon
```

## Pick your path

| You want to | Path | Manifest field | Start at |
|---|---|---|---|
| Change database values, in-game text, textures, models or loose files | Data addon | `operations` | [Data addons](/reference/data/) |
| Run code in the game: hooks, telemetry, an overlay | Engine plugin | `operations.native` | [Engine plugins](./native-plugins) |
| Add launcher pages, an importer, commands or a build step | Launcher plugin | `launcher` | [Launcher plugins](./launcher-api) |
| Move or add elements on a menu screen, or add a screen | Menu edit | `operations.erp`, `operations.screens` | [Menus and UI](/reference/ui/) |
| Change the race's colours with a LUT | Shader pack | `shader` | [Shader packs](./shader-packs) |
| Run your own shader on the race picture | Post-process pass | `postfx` | [Post-process passes](./postfx) |

One manifest can combine them. This one has a database operation, an engine plugin, a launcher page and a setting:

```json [swofty.json]
{
  "schema_version": 1,
  "id": "me.my-addon",
  "name": "My Addon",
  "version": "0.1.0",
  "author": "Me",
  "compat": { "game": "f1_25" },
  "options": [{ "id": "verbose", "type": "toggle", "label": "Verbose", "default": false, "binds": "native" }],
  "operations": {
    "database": ["db/changes.json"],
    "native": [{ "dll": "native/my_plugin.dll" }]
  },
  "launcher": {
    "program": { "linux": "bin/helper", "windows": "bin/helper.exe" },
    "sections": [{ "id": "items", "title": "Items", "icon": "package" }]
  }
}
```

## What's in this section

| Page | What it covers |
|---|---|
| [Your first addon](./first-addon) | A complete addon with one setting and a launcher page, built and installed step by step |
| [Templates](./template) | The two repository templates, what each file is and how to build them |
| [Settings](./settings) | Declaring options, reading them in operations, engine plugins and launcher plugins |
| [Packaging and publishing](./publishing) | Building, testing in game, reading logs, packaging a `.swoftyaddon` and releasing it |
| [Addon manifest](./manifest) | Every `swofty.json` field |

## What the launcher does with an addon

1. **Install.** `swofty addons install <path>` checks the manifest and copies the addon to `$DATA/addons/<id>/`.
2. **Enable.** Your [profile](/using/profiles) lists the addons it uses, their load order and each addon's settings.
3. **Build.** Press **Play** (or run `swofty build`) and the launcher applies every enabled addon's operations to copies of the game's files, or queues them for the game to apply in memory. It also runs each launcher plugin's build step.
4. **Launch.** The launcher puts the build and the in-game loader into the game folder. The loader starts SwoftyMod, which loads each addon's engine plugin.
5. **Play.** Open **ADDON SETTINGS** in game to see each addon's options. Change one and its engine plugin receives the change right away.
6. **Exit.** Settings you changed in game are saved back to the profile, and the game folder returns to how it was.
