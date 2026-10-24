---
title: Templates
description: Three starting points — templates/launcher-addon for a launcher plugin, templates/addon for data and an engine plugin, templates/shader-pack for colour looks. What each file is and how to build it.
---

# Templates

Pick the repository template that fits what you want to build. Copy it, change `id` in `swofty.json`, then build it. All three templates are MIT-licensed; you can use any licence for your addon.

| Template | Start here for |
|---|---|
| [`templates/launcher-addon`](https://github.com/Swofty-Developments/SwoftyLauncher/tree/main/templates/launcher-addon) | A [launcher plugin](./launcher-api): a sidebar page, an importer, a build hook and a command. |
| [`templates/addon`](https://github.com/Swofty-Developments/SwoftyLauncher/tree/main/templates/addon) | [Data](/reference/data/) and an [engine plugin](./native-plugins): database and UI edits driven by settings, plus a C++ plugin. |
| [`templates/postfx-pass`](https://github.com/Swofty-Developments/SwoftyLauncher/tree/main/templates/postfx-pass) | A [post-process pass](./postfx): a vignette with saturation, with settings for both. |
| [`templates/shader-pack`](https://github.com/Swofty-Developments/SwoftyLauncher/tree/main/templates/shader-pack) | A [shader pack](./shader-packs): three colour looks, with settings to pick one and its strength. |

## templates/launcher-addon

This template gives you a Rust helper program with just one dependency, `serde_json`. It talks to the launcher over stdin and stdout, answering `hello`, `view`, `event`, `import`, `build`, `command` and `summary`.

| Path | What it is |
|---|---|
| `swofty.json` | The manifest: `launcher` names the helper for each OS, the `Example` section, the `exmod` import extension, the build hook and the `example` command. |
| `Cargo.toml` | One binary, `example-helper`, depending on `serde_json`. |
| `src/main.rs` | The helper: an `Example` page listing imported `.exmod` files with a switch for each, an importer, a build hook and the `example` command. |
| `README.md` | Build and install steps. |

When you drop `.exmod` files, the importer copies them into your addon's `addon-data` folder. The page lists imported `.exmod` files with a per-profile switch for each, so you can choose which items to turn on. When you build, the build hook writes those items' names to `enabled.txt` and installs it with `file.put`. Run `swofty example` to print each item and whether it's on.

To build and install the helper, run:

```sh
cargo build --release
mkdir -p bin
cp target/release/example-helper bin/
swofty addons install .
swofty addons enable example.launcher-addon
```

On Windows, build `example-helper.exe` and copy it to `bin/`. Ship both binaries in `bin/`.

## templates/addon

Use this template to connect settings to both data edits and an engine plugin. Your settings drive `gamedatabases` and UI edits; the C++ engine plugin reads those settings too.

| Path | What it is |
|---|---|
| `swofty.json` | The manifest: a toggle, a choice and two sliders (one `db`, the rest `native`), and `database`, `ui` and `native` operations. |
| `db/example.json` | Database operations: a `set` whose value comes from the `penalty_seconds` setting, and a `set` through a row selector. |
| `ui/example.pkgpatch.json` | A `!pkg` patch that clones a tooltip entry in `tool_tips.uidb`. |
| `native/src/sample_plugin.cpp` | The engine plugin: reads its settings, reacts to in-game changes with `SWOFTY_ON_CHANGE` and runs one signature scan. |
| `native/include/` | The plugin C ABI (`swofty_api.h`) and the C++ layer (`swofty/*.hpp`). |
| `native/build.sh` | Builds `native/sample_plugin.dll` with `zig c++` for `x86_64-windows-gnu`. |
| `README.md` | Build and install steps. |

Build the plugin, then install and enable the addon:

```sh
native/build.sh
swofty addons install .
swofty addons enable example.sample-mod
```

If you only need data edits, remove the `native` entry from `swofty.json` and the `native/` directory. You can then install the folder as it is, without a compiler.

## templates/postfx-pass

Use this template for a [post-process pass](./postfx): a vignette with saturation, and settings for both.

| Path | What it is |
|---|---|
| `swofty.json` | The manifest: the `postfx` experimental feature, `vignette` and `saturation` sliders (both `db`), and the `postfx` block that names the shader and passes the sliders to it. |
| `fx/vignette.hlsl` | The pass. |
| `README.md` | How to change the pass, and install steps. |

The build compiles the pass with the launcher's dxc; see [Build](./postfx#build).

```sh
swofty addons install .
swofty addons enable example.vignette-pass
```

## templates/shader-pack

Use this template for a [shader pack](./shader-packs): colour looks for the race, picked with a setting.

| Path | What it is |
|---|---|
| `swofty.json` | The manifest: the `shaders` category and experimental feature, a `look` choice and a `strength` slider (both `db`), and the `shader` block that names the look's `.cube` file. |
| `looks/monochrome.cube`, `looks/overcast.cube`, `looks/noir.cube` | The three looks, 17×17×17 each. |
| `looks/make_looks.py` | The script that wrote the three `.cube` files. |
| `README.md` | How to make your own look, and install steps. |

Install and enable it as it is; it needs no compiler:

```sh
swofty addons install .
swofty addons enable example.monochrome-looks
```

## Share

You can share the folder directly, or zip its contents with `swofty.json` at the root and give the archive the `.swoftyaddon` extension. Include your changes and compiled binaries, never copies of the game's archives, database or executable. See [Packaging and publishing](./publishing).

## Next

- [Your first addon](./first-addon): a launcher plugin written from scratch, step by step.
- [Data addons](/reference/data/), [Engine plugins](./native-plugins) and [Launcher plugins](./launcher-api): each path in full.
- [Settings](./settings): the template's four settings, explained.
