---
title: What an addon is
description: An addon is a .swoftyaddon — a zip with a manifest — that can ship data, an engine plugin, a launcher plugin, or all three.
---

# What an addon is

You hand out your addon as a single `.swoftyaddon` file: a zip with a `swofty.json` inside. You'll find the exact format in [the manifest](./manifest), but roughly you can think of it as a listing of what SwoftyLauncher loads for your addon. There are three different kinds of thing an addon can be, and it can be several of them at once, depending on what its manifest declares:

- **Data** — definitions in this part of the manifest edit the game's files, such as a livery, a database value or a line of text. No code runs here; the build merges your changes into copies of those files. On its own this is the same as a [mod](/reference/mods/), but you can combine it with the next two kinds.
- **An engine plugin** — a native DLL loaded *inside* F1 25, where it hooks the game through the mixin system. This lets you read telemetry, draw an overlay or change how the game behaves.
- **A launcher plugin** — a small program the launcher runs to add a page to the launcher. The built-in Team Simplified addon, for example, is what adds the **Mods** page.

## Initialising an Addon

To get an addon installed, you only need a folder with one file:

```
my-addon/
└── swofty.json
```

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

Install that folder directly:

```sh
swofty addons install ./my-addon
```

You'll see it on the launcher's **Addons** page straight away. With no content, it won't do anything yet. To give it something to load, fill in the manifest.

## Example manifests

You've seen the minimal manifest. To ship content you add one of these blocks to it. For every field, and for building a manifest in depth, see [the manifest](./manifest).

**[Data](/reference/mods/)** — the same thing a mod ships. Edits go under [`operations`](./manifest#operations):

```json
"operations": { "database": ["db/livery.json"], "erp": ["erp/car.json"] }
```

**An [engine plugin](./native-plugins)** — a DLL that runs in the game, under [`operations.native`](./manifest#operations):

```json
"operations": { "native": [{ "dll": "native/hud.dll" }] }
```

**A [launcher plugin](./launcher-api)** — a helper that adds a page, under [`launcher`](./manifest#launcher):

```json
"launcher": {
  "program": { "linux": "bin/helper", "windows": "bin/helper.exe" },
  "sections": [{ "id": "mods", "title": "Mods" }]
}
```

**[Settings](./settings)** — toggles, choices and sliders, under [`options`](./manifest#options):

```json
"options": [{ "id": "verbose", "type": "toggle", "label": "Verbose", "default": false }]
```

## Package it

When you're ready to share it, zip the folder's *contents* and give the archive the `.swoftyaddon` extension. The manifest has to sit at the root of the zip, so change into the folder before zipping it:

```sh
cd my-addon
zip -r ../me.my-addon.swoftyaddon .
```

Share that file. People can drag it onto the launcher to install it, or run `swofty addons install me.my-addon.swoftyaddon`. See [Package & distribute](/reference/mods/packaging) for the details.

## What the launcher does with it

The manifest tells the launcher what to load, but the work happens at different points. Here's where your data, plugins and settings come into play:

1. **Install.** `swofty addons install <path>` checks the manifest and copies the addon to `$DATA/addons/<id>/`.
2. **Enable.** Your [profile](/using/profiles) lists the addons it uses, their load order and each addon's settings.
3. **Build.** Press **Play** (or run `swofty build`) and the launcher merges every enabled addon's data into a copy of the game's files. It also runs each launcher plugin's build step.
4. **Launch.** The launcher puts the build and the in-game loader into the game folder. That loader starts SwoftyMod, which loads each addon's engine plugin.
5. **Play.** Open the in-game **ADDON SETTINGS** screen to see each addon's options. Change one and its engine plugin receives the change right away.
6. **Exit.** When you leave the game, any settings you changed there are saved back to the profile. The game folder returns to exactly how it was.

To build a tiny addon end to end, follow [Your first addon](./first-addon). If you'd rather start from something you can trim down, copy [a template](./template).
