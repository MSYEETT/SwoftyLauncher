# SwoftyMod

SwoftyMod (`swofty.core`) is the built-in mod. It adds core functionality for Swofty Mod loading and customisation. It is always enabled and cannot be disabled or removed.

It provides:

* **MOD SETTINGS** directly under Career on the home screen, added by a `MidHook` in `swofty_core.dll` (`native/src/core/`)
* the **F9 / R3 prompt** "Mod Settings", a listener and a tooltip entry
* the bottom-left "Launched with Swofty Mod Launcher" panel, with the F9 or R3 glyph and "to open settings"
* the `mod_menu` route
* the MOD SETTINGS page, with a section per installed mod built from `swofty/settings.json` each session
* its own setting, `show_launcher_panel`

This directory holds the recipe only, never game files. The patches in `ui/` are applied to the player's own copy of the game data at build time. `native/swofty_core.dll` is produced by `native/build.sh` and is not committed.

| File | Target |
|---|---|
| `ui/fe_home.pkgpatch.json` | `fe_home.ui` in `flow_frontend.erp` and `screens/fe_home.erp`: F9 listener, tooltip, offline panel text and signals |
| `ui/routes.pkgpatch.json` | `routes_frontend_general.uir` in `persistent.erp`: the `mod_menu` route, a clone of `options` |
| `ui/tooltips.pkgpatch.json` | `tool_tips.uidb` in `databases.erp`: the `mod_menu` and `mod_menu_offline` entries |

The reference implementation of these edits is `tools/build_home_tile_mod.py` (variant `v6`). The settings page is `v8 --settings swofty/settings.json`, built by `tools/swofty_settings.py` from the session settings file. The Rust `!pkg` patch engine replaces both.
