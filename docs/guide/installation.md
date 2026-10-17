# Installation

F1 Mod Manager is a desktop app for F1 25. Modded play is offline only.

## Requirements

- F1 25 on Steam, including the F1 2026 season content.
- Windows 10 or 11 (x86-64), or Linux with Steam and Proton.
- Free disk space for copies of the files your mods change.

Steam Deck, NixOS and Flatpak Steam are supported. macOS is not supported.

## Get the app

Release downloads are not available yet. For now, [build from source](/reference/development).

The app is in development. Some controls and mod features are unfinished; see the [current status](/design/#current-status).

## Set up

Continue with [Windows](/guide/windows) or [Linux and Steam Deck](/guide/linux).

## Where your files are stored

The docs use `$DATA` to mean this folder:

| Platform | Folder |
|---|---|
| Linux | `$XDG_DATA_HOME/swofty`, usually `~/.local/share/swofty` |
| Flatpak Steam | `~/.var/app/com.valvesoftware.Steam/data/swofty` |
| Windows | `%LOCALAPPDATA%Swofty` |

It holds your mods, maps, profiles, backups and session logs. Keep it until you have [restored the game](/using/profiles-and-restoring).
