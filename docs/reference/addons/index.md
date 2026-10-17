---
title: Creating addons
description: What an addon contains and what happens to it from install to the end of a session.
---

# Creating addons

An addon is a folder with a [`swofty.json`](./manifest) manifest at its root. It adds code, settings and in-game screens to F1 25, and can carry the same content a [mod](/reference/mods/) does.

| Part | Manifest field | Page |
|---|---|---|
| Name, version, author, requirements | `id`, `name`, `version`, `requires`, `conflicts`, `compat` | [Addon manifest](./manifest) |
| Settings | `options` | [Settings](./settings) |
| Native plugins | `operations.native` | [Native plugins](./native-plugins), [Mixin system](./hooking) |
| In-game screens | `operations.screens` | [In-game UI](./in-game-ui) |
| Content | `operations.database`, `operations.erp`, `operations.ui`, `operations.files`, `operations.assets` | [Creating mods](/reference/mods/), [Asset API](./asset-api) |
| Launcher sections, importers, build hooks (built-in addons) | | [Launcher API](./launcher-api) |

Start with [Your first addon](./first-addon), or copy [the addon template](./template).

## Lifecycle

1. **Install.** `swofty addons install <path>`, or **Install addon…** in the launcher, checks the manifest and copies the folder to `$DATA/addons/<id>/`.
2. **Enable.** A [profile](/using/profiles) lists its addons in load order, with each addon's setting values.
3. **Build.** `swofty build`, or **Play**, applies the enabled addons' operations to copies of the game's files in `$DATA/cache/overlay/<profile>/<buildhash>/`, and writes the session's `swofty/settings.json`. See [`swofty build`](/reference/cli#swofty-build).
4. **Launch.** The build and the in-game loader are installed into the game folder through the journal. The loader starts SwoftyMod, which loads each addon's plugins and registers its screens.
5. **Play.** **ADDON SETTINGS** shows each addon's settings. A change reaches the addon's plugins at once.
6. **Exit.** Settings changed in game are saved to the profile, and the game folder returns to its original files.

## In-game loader {#in-game-loader}

A modded launch places the loader in the game folder and removes it when the game exits:

| File | Role |
|---|---|
| `version.dll` | Loads every DLL in `swofty/addons/core/`, then `swofty/addons/*.dll`. It also loads `swofty/mods/core/` and `swofty/mods/*.dll` with them, in name order |
| `swofty/addons/core/swofty_core.dll` | SwoftyMod: the ADDON SETTINGS entry, the settings bridge, plugin mixins, addon-added special liveries reported as owned, and the crash log |
| `swofty/addons/core/swofty_gamelog.dll` | Copies the game's own log into `loader.log` |
| `swofty/addons/core/bindings.json`, `symbols.tsv`, `log_channels.tsv` | Game bindings for plugins, function names for crash reports, and the game's log channels |
| `swofty/settings.json` | The profile's addon settings for the session under `addons`, the `ui` block listing the screens to register, and the `owned_items` block listing the ownership keys of unlock rows that addons add |
| `swofty/ui/*.ui` | ADDON SETTINGS and addon screens, registered by `swofty_core.dll` |

With the loader, the build also replaces four archives in `2025_asset_groups/ui_package/`: `flow_frontend.erp`, `persistent.erp`, `databases.erp` and `flow_persistent.erp`. They are built from the game's own copies for the profile's addons and tracks. `swofty restore` puts the originals back.

Everything logs to `<game>/swofty/loader.log`. The loader moves the previous log to `loader.prev.log` when it attaches. A session shows and copies `loader.log` only when the loader wrote a new one during that session. `swofty run` shows its lines as `[loader]` and `[core]`, and the launcher console shows them on the Game tab.

- **Crash log:** an exception writes its code, address, registers and a named stack. `ExitProcess` and `TerminateProcess` calls are logged with the caller's stack.
- **Game log:** warnings and errors from every engine channel, and info from the UI and flow channels. Set `SWOFTY_GAME_LOG` to `off`, `info`, `warn` or `error`, and `SWOFTY_GAME_LOG_CHANNELS` to a channel list, in the launch environment to change it.
- **Settings bridge:** changes made in ADDON SETTINGS reach plugins at once and are saved to the profile when the game exits.
