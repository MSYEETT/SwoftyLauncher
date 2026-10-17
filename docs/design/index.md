# Design notes

This is a contributor overview of Swofty Launcher for F1 25. It covers the intended app and the work still needed. For setup, start with [Installation](/guide/installation).

## Goals

- Manage addons and community maps in one desktop app.
- Keep modded play offline only, with multiplayer off.
- Switch profiles and restore the original game in one click.
- Change addon options through ADDON SETTINGS under Career.
- Support Windows and Linux through Steam and Proton.

## Main parts

| Path | Role |
|---|---|
| `apps/launcher` | Desktop interface, built with Rust and GPUI |
| `apps/cli` | Terminal commands and Linux Steam launch wrapper |
| `packages/core` | Addons, maps, profiles, backups and launch setup |
| `packages/formats` | Game file readers and writers |
| `packages/sdk` | Plugin API |
| `native/` | In-game loader and menu entry |
| `addons/swoftymod` | Built-in menu changes |

Keep the interface separate from the shared core. Use the Python tools in `tools/` to check the Rust format code.

## Addons and maps

Addons use a [`swofty.json` manifest](/reference/addons/manifest). It lists database changes, assets, UI patches, loose files and plugins.

Maps use the existing community track-package format. Import the package unchanged and use its track code in the profile.

Apply changes in load order. Report missing requirements and conflicts before launch.

[Addons and mods](/design/addons-and-mods) plans content mods, the Launcher API, the Asset API and the built-in Team Simplified addon.

## Profiles and saves

Each profile holds its addons, maps and options. The intended design gives each profile separate saves under `$DATA/saves/<profile>/`, away from vanilla saves and Steam Cloud.

Save isolation is not implemented yet. The built-in `vanilla` profile contains no addons and cannot be edited.

## Launch and restore

The intended flow is:

1. Check the game and profile.
2. Prepare the changed files and record backups.
3. Start an offline modded session.
4. Show logs while the game runs.
5. Restore the original files when it exits.

Record changes before applying them so interrupted sessions can be recovered. Never restore an old build's files over a new one. See [Backups and recovery](/reference/restore).

Linux uses a Steam launch option. Windows uses a separate modded launch. Normal Steam launches remain vanilla when the launcher's launch option is absent and the game has been restored.

## In-game settings

SwoftyMod adds ADDON SETTINGS below Career, an F9/R3 prompt and a launcher panel.

ADDON SETTINGS has a tile per enabled addon and map and a page for each, built from the player's game files and the profile when the profile is built. Database settings need a restart; plugin settings apply immediately.

## Current status

The repository is pre-release:

- Addon installation, profiles, map import and command-line logs are implemented.
- Database operations, `!pkg` edits, resource copies and loose files can be built. Other addon ERP and UI build steps are skipped.
- When the in-game loader's core has the feature, asset groups, changed ERP resources, database edits and language strings go into `swofty/assets.json` and apply in memory. A Grand Prix has loaded a track through asset groups and database edits. The file overlay is experimental.
- Enabled maps are activated: database rows, track archive, menu art, minimap and ERS. `swofty apply` installs them through the journal. Neither has been tested in game yet.
- Backup and restore code has tests for interrupted changes and game updates.
- The native menu entry is tested with a test executable under Wine. It currently opens the game's Settings page.
- Play in the launcher builds the profile, installs it with the in-game loader, and rolls both back when the game exits. This flow has not been tested in game yet.
- The built-in Team Simplified addon imports `.serps` mods into a library and keeps each profile's order and switches in its **Mods** section and `swofty mods`. It does not load them into the game yet.
- Addon settings work in the launcher and the command line. The in-game page is generated; saving in-game changes waits on the native bridge's in-game test.
- Save isolation, offline enforcement and plugin approval are unfinished.

These remaining steps must be completed before the full player workflow is ready.

## Contributor checks

Keep changes compatible with the game build they target. Native hooks must use verified signatures and stop if a match is missing or ambiguous.

Follow `re/PROTOCOL.md` for game research. See [Development](/reference/development) for build and test commands.
