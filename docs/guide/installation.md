# Installation

Swofty Launcher for F1 25 is a desktop app for modding F1 25. Modded play is offline only.

## Requirements

- F1 25 on Steam, including the F1 2026 season content.
- Windows 10 or 11 (x86-64), or Linux with Steam and Proton.
- Free disk space for copies of the files your mods change.

Steam Deck, NixOS and Flatpak Steam are supported. macOS is not supported.

## Get the app

Download the archive for your platform from [Releases](https://github.com/Swofty-Developments/SwoftyLauncher/releases) and extract it:

| Platform | Archive |
|---|---|
| Windows | `swofty-launcher-<version>-windows-x86_64.zip` |
| Linux and Steam Deck | `swofty-launcher-<version>-linux-x86_64.tar.gz` |

`bin/` holds the launcher, `swofty-launcher`, and the command line, `swofty`. `loader/` holds the in-game loader and stays next to `bin/`. To run the latest code, [build from source](/reference/development).

[0.2.0](/releases/0.2.0) lists what is new.

The app is in development. Some controls and mod features are unfinished; see the [current status](/design/#current-status).

## Set up

Continue with [Windows](/guide/windows) or [Linux and Steam Deck](/guide/linux).

## Where your files are stored

The docs use `$DATA` to mean this folder:

| Platform | Folder |
|---|---|
| Linux | `$XDG_DATA_HOME/swofty`, usually `~/.local/share/swofty` |
| Flatpak Steam | `~/.var/app/com.valvesoftware.Steam/data/swofty` |
| Windows | `%LOCALAPPDATA%\Swofty` |

It holds your addons, maps, profiles, backups and session logs. Keep it until you have [restored the game](/using/profiles-and-restoring).
