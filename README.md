# F1 Mod Manager

A desktop app for modding F1 25 on Windows and Linux through Steam and Proton.

## Features

- **Maps:** Import community track packages, such as Sepang and Okayama, and race on them as extra tracks.
- **Mods:** Install, enable and disable mods. The app warns when two mods change the same thing.
- **In-game mod settings:** Open **MOD SETTINGS** in the game's home menu, under Career, to change mod options without leaving the game.
- **Profiles:** Switch between separate sets of mods and maps, each with its own saves. The original game is always restorable in one click.
- **Offline-only modded play:** Modded sessions always run offline with multiplayer off. Launching from Steam normally stays completely vanilla.
- **The launcher app:** Manage everything and press Play in one window, with live logs to help if the game crashes.

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

[GNU Affero General Public License, version 3 or later](LICENSE). The plugin SDK in `packages/sdk` and the mod template in `templates/mod` use the MIT licence. The docs theme adapts MIT-licensed CSS from the Geode docs; see [THIRD_PARTY.md](THIRD_PARTY.md).
