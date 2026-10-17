# Design notes

This is a contributor overview of F1 Mod Manager. It covers the intended app and the work still needed. For setup, start with [Installation](/guide/installation).

## Goals

- Manage mods and community maps in one desktop app.
- Keep modded play offline only, with multiplayer off.
- Switch profiles and restore the original game in one click.
- Change mod options through MOD SETTINGS under Career.
- Support Windows and Linux through Steam and Proton.

## Main parts

| Path | Role |
|---|---|
| `apps/launcher` | Desktop interface, built with Rust and GPUI |
| `apps/cli` | Terminal commands and Linux Steam launch wrapper |
| `packages/core` | Mods, maps, profiles, backups and launch setup |
| `packages/formats` | Game file readers and writers |
| `packages/sdk` | Plugin API |
| `native/` | In-game loader and menu entry |
| `mods/swoftymod` | Built-in menu changes |

Keep the interface separate from the shared core. Use the Python tools in `tools/` to check the Rust format code.

## Mods and maps

Mods use a [`swofty.json` manifest](/modding/manifest). It lists database changes, assets, UI patches, loose files and plugins.

Maps use the existing community track-package format. Import the package unchanged and use its track code in the profile.

Apply changes in load order. Report missing requirements and conflicts before launch.

## Profiles and saves

Each profile holds its mods, maps and options. The intended design gives each profile separate saves under `$DATA/saves/<profile>/`, away from vanilla saves and Steam Cloud.

Save isolation is not implemented yet. The built-in `vanilla` profile contains no mods and cannot be edited.

## Launch and restore

The intended flow is:

1. Check the game and profile.
2. Prepare the changed files and record backups.
3. Start an offline modded session.
4. Show logs while the game runs.
5. Restore the original files when it exits.

Record changes before applying them so interrupted sessions can be recovered. Never restore an old build's files over a new one. See [Backups and recovery](/reference/restore).

Linux uses a Steam launch option. Windows uses a separate modded launch. Normal Steam launches remain vanilla when the manager's launch option is absent and the game has been restored.

## In-game settings

SwoftyMod adds MOD SETTINGS below Career, an F9/R3 prompt and a launcher panel.

The settings page has a section per mod, built each session from the mods' options. Database settings need a restart; plugin settings apply immediately.

## Current status

The repository is pre-release:

- Mod installation, profiles, map import and command-line logs are implemented.
- Database operations can be built. ERP, UI and loose-file build steps are skipped.
- Backup and restore code has tests for interrupted changes and game updates.
- The native menu entry is tested with a test executable under Wine. It currently opens the game's Settings page.
- Desktop controls and the full launch, build and restore flow still need connecting.
- Mod settings work in the launcher and the command line. The in-game page is generated; saving in-game changes waits on the native bridge's in-game test.
- Save isolation, offline enforcement and plugin approval are unfinished.

These remaining steps must be completed before the full player workflow is ready.

## Contributor checks

Keep changes compatible with the game build they target. Native hooks must use verified signatures and stop if a match is missing or ambiguous.

Follow `re/PROTOCOL.md` for game research. See [Development](/reference/development) for build and test commands.
