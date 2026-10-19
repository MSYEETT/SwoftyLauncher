---
title: Installation
description: Install Swofty Launcher for F1 25 with the installer or by hand, and find the install folder and the data folder.
---

# Installation

Install the launcher with the installer or by hand, then follow the setup steps for [Windows](/guide/windows) or [Linux and Steam Deck](/guide/linux). You'll play modded games in single-player, offline.

## Requirements

- F1 25 on Steam, with the F1 2026 season content installed.
- Windows 10 or 11 (x86-64), or Linux with Steam and Proton. Steam Deck, NixOS and Flatpak Steam work. macOS does not.
- Free disk space: your content changes copies of the game files, so you'll need room for those copies.

## Install with the installer

1. Download the installer for your platform:

   | Platform | Download |
   |---|---|
   | Windows | [SwoftyLauncherInstaller.exe](https://github.com/Swofty-Developments/SwoftyLauncher/raw/main/installer/SwoftyLauncherInstaller.exe) |
   | Linux and Steam Deck | [SwoftyLauncherInstaller-linux.zip](https://github.com/Swofty-Developments/SwoftyLauncher/raw/main/installer/SwoftyLauncherInstaller-linux.zip), then extract it |

2. Run `SwoftyLauncherInstaller.exe` on Windows, or `SwoftyLauncherInstaller` on Linux and Steam Deck. Windows may show **Windows protected your PC**; select **More info**, then **Run anyway**.
3. Under **Install to**, keep the default folder, or select **Browse…** to choose another.
4. Leave **Add a desktop shortcut** ticked if you want one.
5. Select **Install**.

The installer downloads the latest release, checks it against the release's `SHA256SUMS` and extracts it into that folder. It adds **Swofty Launcher** to the Start menu on Windows, or **Swofty Launcher for F1 25** to the app menu on Linux, adds the desktop shortcut if you ticked it, then opens the launcher.

| Platform | Default folder |
|---|---|
| Windows | `%LOCALAPPDATA%\Programs\SwoftyLauncher` |
| Linux and Steam Deck | `~/SwoftyLauncher` |

When you browse to a folder that isn't empty, isn't named `SwoftyLauncher` and holds no install, the installer uses a `SwoftyLauncher` folder inside it. With **Flatpak Steam**, choose a folder inside `~/.var/app/com.valvesoftware.Steam/`, such as `~/.var/app/com.valvesoftware.Steam/SwoftyLauncher`.

You only need the installer once. After that, open the launcher from the Start menu or app menu, and it [updates itself](/using/updating).

## Install by hand

1. Open [Releases](https://github.com/Swofty-Developments/SwoftyLauncher/releases).
2. Download the archive for your platform:

   | Platform | Archive |
   |---|---|
   | Windows | `swofty-launcher-<version>-windows-x86_64.zip` |
   | Linux and Steam Deck | `swofty-launcher-<version>-linux-x86_64.tar.gz` |

3. Extract it where you want to keep the launcher, such as `Documents\SwoftyLauncher` or `~/SwoftyLauncher`. With **Flatpak Steam**, extract it inside `~/.var/app/com.valvesoftware.Steam/`.

A hand install adds no menu entries or shortcuts; open the launcher from `bin/`.

## The install folder

Either way, the install folder holds:

| Path | Contents |
|---|---|
| `bin/swofty-launcher` | The launcher |
| `bin/swofty` | The command line |
| `bin/swofty-ui` | [Swofty UI](/reference/ui/swofty-ui) |
| `loader/` | The in-game loader; keep it next to `bin/` |
| `addons/` | The [addons that ship with the launcher](/using/bundled-addons), installed switched off |

On Windows, the files in `bin/` end in `.exe`. The setup guides call this folder the install folder.

## Where your files are stored

When these docs refer to `$DATA`, they mean the folder below for your platform:

| Platform | Folder |
|---|---|
| Linux | `$XDG_DATA_HOME/swofty`, usually `~/.local/share/swofty` |
| Flatpak Steam | `~/.var/app/com.valvesoftware.Steam/data/swofty` |
| Windows | `%LOCALAPPDATA%\Swofty` |

Your addons, mods, profiles, backups and session logs live here, apart from the install folder. It holds your backups, so keep this folder until you've [restored the game to stock](/using/profiles-and-restoring).

## Updating

The launcher updates itself. See [Updating](/using/updating).

## Next

- Set up on [Windows](/guide/windows) or [Linux and Steam Deck](/guide/linux).
