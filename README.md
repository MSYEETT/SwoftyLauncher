# F1 Mod Manager

![The mod library in the Swofty Mod Launcher](docs/public/screenshots/library.png)
<!-- TODO: docs/public/screenshots/library.png -->

F1 Mod Manager installs mods and maps for F1 25 and launches the game modded and offline, on Windows and on
Linux through Steam and Proton. It is made of the Swofty Mod Launcher desktop app, the `swofty` command, a
native loader that runs inside the game, and SwoftyMod, the built-in mod that adds MOD SETTINGS to the game.

## Features

* **Mods and maps, kept apart**: a mod changes the game database, assets or UI, or adds a native plugin; a map is
  a community track package, imported from its zip without repacking
* **One mod format**, `swofty.json`, with declarative database, ERP, UI and file operations and native plugins
* **Database edits by name**: row selectors such as `entrants.teams.team[_metadata_=gm_cadillac_26]`,
  `value_from`, and an `add-row` that keeps each table's `_instancecount_` equal to its row count
* **Map import** from the community track-package format: `manifest.json`, `<code>.erp` and `ui/*.json`
* **SwoftyMod, built in and always on**: MOD SETTINGS directly under Career, the F9 (R3) prompt, the
  "Launched with Swofty Mod Launcher" panel and a settings page of its own
* **Profiles** with load order, requirements and conflict checks
* **Offline only**: mods load only in a session the launcher prepared, with no anticheat present, and the loader
  files are in the game folder only for that session
* **Live logs**: `swofty run` streams the loader log and Wine errors, and names the game function at a crash
  address from the reverse-engineering knowledge base
* **Byte-identical formats**: GVDB, ERP and `!pkg` read and write in Rust and round-trip the game's files byte for
  byte, checked against the Python tools
* A **Zed-like desktop app** built on GPUI, and a CLI that covers every function headless, including Steam Deck
  Game Mode
* A **sample mod** and a C plugin SDK, both under the MIT licence

F1 Mod Manager is pre-release. The formats, the store, the resolver, Steam discovery, map import and the
`swofty` command's `run`, `logs`, `mods`, `maps`, `verify` and `doctor` work. The overlay builder covers database
operations only, and the launcher's views are placeholders. In the loader, the MOD SETTINGS tile works; the
safety gate, the file redirection and the offline guard are not written yet.

## Installation

Install and set up F1 Mod Manager by following the documentation at
[swofty-developments.github.io/F1ModManager](https://swofty-developments.github.io/F1ModManager/).

## Screenshots

### Library

The library lists mods and maps separately. SwoftyMod is pinned at the top and has no switch.

![The mod library](docs/public/screenshots/library.png)
<!-- TODO: docs/public/screenshots/library.png -->

![The map library with an imported track](docs/public/screenshots/maps.png)
<!-- TODO: docs/public/screenshots/maps.png -->

### Play

Play builds the profile, prepares the session and starts the game. A banner stays up while a modded session is
running.

![The Play view with the session banner](docs/public/screenshots/play.png)
<!-- TODO: docs/public/screenshots/play.png -->

### In game

MOD SETTINGS sits directly under Career, and the bottom-left panel shows that the game was launched with Swofty
Mod Launcher.

![The home screen with MOD SETTINGS under Career](docs/public/screenshots/in-game-home.png)
<!-- TODO: docs/public/screenshots/in-game-home.png -->

![The SwoftyMod settings page](docs/public/screenshots/in-game-settings.png)
<!-- TODO: docs/public/screenshots/in-game-settings.png -->

### Terminal

`swofty run` follows the game and prints a crash summary when it exits.

![swofty run following a session](docs/public/screenshots/swofty-run.png)
<!-- TODO: docs/public/screenshots/swofty-run.png -->

## Supported platforms

* **Windows** 10 and 11, with Steam
* **Linux** with Steam and Proton, including Steam Deck (SteamOS), NixOS and Flatpak Steam
* **F1 25** from Steam (app 3059520), build 25251167, including the F1 2026 season content

Modded sessions are single-player and offline. Steam's own Play button stays vanilla on Windows.

## Repository

| Path | Contents |
|---|---|
| `apps/launcher` | Swofty Mod Launcher, the GPUI desktop app |
| `apps/cli` | The `swofty` command |
| `packages/formats` | GVDB, ERP and `!pkg` readers and writers |
| `packages/core` | Manifests, operations, store, resolver, Steam and Proton discovery, crash symbolisation |
| `packages/sdk` | The native plugin C ABI and its Rust types (MIT) |
| `native` | The in-game loader: `version.dll` (C) and `swofty_core.dll` (C++23, safetyhook) |
| `mods/swoftymod` | SwoftyMod's manifest and UI patches |
| `templates/mod` | The sample mod (MIT) |
| `re` | The reverse-engineering knowledge base: names, addresses, structures and signatures |
| `tools` | The Python tools, the byte-identity oracle for `packages/formats` |
| `docs` | The documentation site |

The repository holds no game data. Tests that need the game's files read them from `F1_25_DIR` and skip when it
is unset.

## Contributing

```bash
nix develop                     # on NixOS: GPUI libraries, zig, Wine, pnpm
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test                      # F1_25_DIR=<game folder> also runs the real-file round trips
cargo run -p swofty-launcher
native/build.sh                 # version.dll and swofty_core.dll, with zig
native/test/run_wine_test.sh    # the loader against a synthetic F1_25.exe under Wine
pnpm install
pnpm docs:dev
```

CI runs the same checks on pull requests, plus the launcher build on Linux and Windows, the docs build and the
native Wine test. lefthook runs rustfmt and clippy on staged files before each commit. Commit messages follow
[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).

## Licence

F1 Mod Manager is free software under the GNU Affero General Public License, version 3 or later. See
[LICENSE](LICENSE). The plugin SDK in `packages/sdk` and the mod template in `templates/` are under the MIT
licence, so mods built on them can use any licence.
