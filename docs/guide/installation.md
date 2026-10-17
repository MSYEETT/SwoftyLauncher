# Installation

F1 Mod Manager has three parts:

- **Swofty Mod Launcher** (`swofty-launcher`), the desktop app: library, profiles, setup and Play.
- **`swofty`**, the [command line tool](/reference/cli). It does everything the app does, headless, and it is the Steam launch wrapper on Linux.
- **The in-game loader**, a `version.dll` proxy and `swofty_core.dll`. The launcher places them in the game folder for a modded session and removes them when the game exits.

## Requirements

| | |
|---|---|
| Game | F1 25 on Steam (app 3059520), including the F1 2026 season content |
| Windows | Windows 10 or 11, x86-64, Steam running |
| Linux | Steam with Proton, including Steam Deck, Flatpak Steam and NixOS |
| Disk | Room for the overlay cache under the data directory, roughly the size of the files your mods change |

macOS is not supported.

## Download

<div class="todo">TODO: release downloads. Builds for Windows and Linux are attached to each tagged release on GitHub.</div>

Until the first release, build from source as described in [Development](/reference/development).

## Data directory

Everything the launcher keeps lives in one directory, `$DATA`:

| Platform | `$DATA` |
|---|---|
| Linux | `$XDG_DATA_HOME/swofty` (`~/.local/share/swofty`) |
| Linux, Flatpak Steam | `~/.var/app/com.valvesoftware.Steam/data/swofty` |
| Windows | `%LOCALAPPDATA%\Swofty` |

Its layout:

| Path | Contents |
|---|---|
| `mods/<id>/` | Installed mods |
| `maps/<track_code>/` | Imported track packages, unchanged |
| `profiles/` | Profiles |
| `cache/overlay/<profile>/<buildhash>/` | Built files for a profile and game build |
| `baseline/<build_id>/baseline.json` | Original sizes and hashes of the files the launcher changed |
| `baseline/objects/<sha256>` | Original contents of those files |
| `saves/<profile>/` | Modded saves, per profile |
| `state/journal/` | The restore journal |
| `state/requests/` | Changes made in game, waiting to be merged into a profile |
| `sessions/<id>/` | Session files, status, logs and crash dumps |
| `config.json` | Settings, including launch affinity and environment |
| `bin/` | The `swofty` binary used by the Steam launch option |

## Next

- [Windows](/guide/windows)
- [Linux and Steam Deck](/guide/linux)
