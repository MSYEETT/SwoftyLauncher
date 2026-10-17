# Development

Build from the repository root. See the [design notes](/design/) for current progress and planned work.

## Source folders

| Path | Contents |
|---|---|
| `apps/launcher` | Desktop app |
| `apps/cli` | `swofty` command |
| `packages/core` | Addon management, profiles, launch setup and restore |
| `packages/formats` | Game file readers and writers |
| `packages/sdk` | Plugin SDK (MIT) |
| `native/` | In-game loader |
| `addons/swoftymod` | Built-in addon |
| `addons/custom-grid-size` | Custom Grid Size addon |
| `templates/addon` | Sample addon (MIT) |
| `tools/` | Python tools for game files |
| `re/` | Game research and signatures |
| `docs/` | This site |

## Build and check

On NixOS, `nix develop` provides the development tools and libraries.

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
cargo build -p swofty-launcher
```

The launcher is built separately from the default workspace members.

## Test with game files

These tests only read your game files. They are skipped when `F1_25_DIR` is unset.

```sh
F1_25_DIR="/path/to/F1 25" cargo test -p swofty-formats
```

To compare with the Python tools, set `F1_25_DIR` in your shell, then run:

```sh
python3 tools/gamedb.py roundtrip "$F1_25_DIR/game_package/gamedatabases/gamedatabases.2025.bin"
python3 tools/erpw.py rebuild "$F1_25_DIR/2025_asset_groups/ui_package/databases.erp" /tmp/databases.erp
```

The ADDON SETTINGS build is compared byte for byte with a reference build of the same session (`packages/core/tests/fixtures/menu_session.json`). `F1_25_DIR` must hold unmodified UI archives.

```sh
F1_25_DIR="/path/to/F1 25" SWOFTY_MENU_ORACLE=/path/to/reference cargo test -p swofty-core --test real_menus
```

With `SWOFTY_MAPS_DIR` set to a folder holding an imported `bahrain24` package, the same file checks that the map's edits and ADDON SETTINGS are written into one `databases.erp` and `persistent.erp`.

## Native loader

```sh
native/build.sh
native/test/run_wine_test.sh
```

The build uses Zig to produce Windows DLLs in `native/dist/`. The Wine test uses a small test executable to check the loader and ADDON SETTINGS menu entry.

## Documentation

```sh
pnpm install
pnpm docs:dev
pnpm docs:build
```

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/). The commit hook checks Rust formatting and clippy.
