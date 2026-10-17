# Swofty Launcher for F1 25

A desktop app for modding F1 25 on Windows and Linux through Steam and Proton.

![The Swofty Launcher for F1 25 launcher](docs/public/screenshots/hero-launcher.png)

## Features

- **Mods:** Drop `.serps` packages, mod zips and folders on the launcher's Mods page, and order them per profile.
- **Tracks:** Import community track packages, such as Sepang and Okayama, as mods and race on them as extra tracks.
- **Addons:** Install, enable and disable addons. The app warns when two addons change the same thing.
- **In-game addon settings:** Open **ADDON SETTINGS** in the game's home menu, under Career, to change addon options without leaving the game.
- **Profiles:** Switch between separate sets of addons, mods and settings. The original game is always restorable in one click.
- **Offline-only modded play:** Modded sessions always run offline with multiplayer off. Launching from Steam normally stays completely vanilla.
- **The launcher app:** Manage everything and press Play in one window, with live logs to help if the game crashes. The in-game loader adds the game's own log and crash reports with named call stacks.

## Install

Follow the [installation guide](https://f1.swofty.net/guide/installation).

## Supported platforms

- Windows 10 and 11 with Steam.
- Linux with Steam and Proton, including Steam Deck, NixOS and Flatpak Steam.
- F1 25 on Steam, including the F1 2026 season content.

## Contributing

```bash
nix develop                     # development tools on NixOS
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test                      # set F1_25_DIR to test with your game files
cargo run -p swofty-launcher
native/build.sh
native/test/run_wine_test.sh
pnpm install
pnpm docs:dev
```

See the [development guide](https://f1.swofty.net/reference/development) for build details.

## Licence

[GNU Affero General Public License, version 3 or later](LICENSE). The plugin SDK in `packages/sdk` and the addon template in `templates/addon` use the MIT licence. The docs theme adapts MIT-licensed CSS from the Geode docs; see [THIRD_PARTY.md](THIRD_PARTY.md).
