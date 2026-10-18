# Installation

To get started, you'll download the launcher, extract it, then follow the setup steps for your platform. You'll play modded games in single-player, offline.

## Requirements

- F1 25 on Steam, with the F1 2026 season content installed.
- Windows 10 or 11 (x86-64), or Linux with Steam and Proton. Steam Deck, NixOS and Flatpak Steam work. macOS does not.
- Free disk space: your content changes copies of the game files, so you'll need room for those copies.

## Download and extract

1. Open [Releases](https://github.com/Swofty-Developments/SwoftyLauncher/releases).
2. Download the archive for your platform:

   | Platform | Archive |
   |---|---|
   | Windows | `swofty-launcher-<version>-windows-x86_64.zip` |
   | Linux and Steam Deck | `swofty-launcher-<version>-linux-x86_64.tar.gz` |

3. Extract it where you want to keep the launcher, such as `Documents\SwoftyLauncher` or `~/SwoftyLauncher`.

Inside, you'll find `bin/`, which holds the launcher `swofty-launcher` and the command line `swofty`. The in-game loader lives in `loader/`; keep that folder next to `bin/`.

Now follow the setup steps for [Windows](/guide/windows) or [Linux and Steam Deck](/guide/linux).

## Updating

When you open the launcher, it checks for a new release, at most once every 6 hours. If there's one available, you'll see its version in a bar above **Play**, with **Update**, **Release notes** and **Skip this version**.

1. Close F1 25 first: the launcher won't update while the game runs.
2. Select **Update**. It downloads the archive, checks it against the release's `SHA256SUMS`, and replaces `bin/`, `loader/` and, on Linux, `share/`.
3. Select **Restart**.

If a step fails, you'll see the error and the old files stay in place. An update interrupted partway is rolled back the next time you start the launcher.

If you built from source or installed through Nix, you'll see **Download** instead; it opens the release page. From a terminal, `swofty update` does the same.

## Where your files are stored

When these docs refer to `$DATA`, they mean the folder below for your platform:

| Platform | Folder |
|---|---|
| Linux | `$XDG_DATA_HOME/swofty`, usually `~/.local/share/swofty` |
| Flatpak Steam | `~/.var/app/com.valvesoftware.Steam/data/swofty` |
| Windows | `%LOCALAPPDATA%\Swofty` |

Your addons, mods, profiles, backups and session logs live here. Because it holds your backups, keep this folder until you've [restored the game to stock](/using/profiles-and-restoring).
