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

[0.2.7](/releases/0.2.7) lists what is new.

The app is in development. Some controls and mod features are unfinished; see the [current status](/design/#current-status).

## Set up

Continue with [Windows](/guide/windows) or [Linux and Steam Deck](/guide/linux).

## Updating

The launcher checks for a new release when it starts, at most once every 6 hours. When one is out, a bar above **Play** shows its version with **Update**, **Release notes** and **Skip this version**.

**Update** downloads the archive for your platform, checks it against the release's `SHA256SUMS` and replaces `bin/`, `loader/` and, on Linux, `share/` in the extracted folder. Select **Restart** to start the new version. Close F1 25 first; the launcher does not update while the game runs. If a step fails, the old files stay in place and the launcher shows the error. If an update is cut off partway, for example by a power loss, the next start of the launcher or `swofty update` puts the previous version back.

The launcher updates itself only when it runs from an extracted release and can write to that folder. A build from source or a Nix install shows **Download**, which opens the release page.

`swofty update` does the same from a terminal. To turn the check off, set `"updates": { "check": false }` in `$DATA/config.json`. See [Command line](/reference/cli#swofty-update).

## Where your files are stored

The docs use `$DATA` to mean this folder:

| Platform | Folder |
|---|---|
| Linux | `$XDG_DATA_HOME/swofty`, usually `~/.local/share/swofty` |
| Flatpak Steam | `~/.var/app/com.valvesoftware.Steam/data/swofty` |
| Windows | `%LOCALAPPDATA%\Swofty` |

It holds your addons, mods, profiles, backups and session logs. Keep it until you have [restored the game](/using/profiles-and-restoring).
