# Development

## Layout

| Path | Contents |
|---|---|
| `apps/launcher` | The GPUI desktop app, `swofty-launcher` |
| `apps/cli` | The `swofty` CLI |
| `packages/formats` | GVDB, ERP and `!pkg` read and write |
| `packages/core` | Manifests, the store, the resolver, Steam and Proton discovery |
| `packages/sdk` | The native plugin C ABI and its Rust types (MIT) |
| `native/` | The in-game loader: `version.dll` (C) and `swofty_core.dll` (C++23, safetyhook) |
| `mods/swoftymod` | The built-in SwoftyMod |
| `templates/mod` | The sample mod (MIT) |
| `re/` | The reverse-engineering knowledge base: names, addresses, structs, signatures, protocol |
| `tools/` | The Python tools, the byte-identity oracle for the Rust formats |
| `docs/` | This site |

## Development shell

```sh
nix develop
```

`flake.nix` provides the Rust toolchain dependencies, the libraries GPUI needs on Linux (Vulkan, Wayland, X11, fontconfig), zig, Wine, Node.js and pnpm.

## Rust

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
```

These cover the CLI and the libraries, the workspace's default members. The launcher builds on its own:

```sh
cargo build -p swofty-launcher
```

## Format tests against real files

The round-trip tests for the GVDB and ERP read the game's own files. Point `F1_25_DIR` at the game folder or a pristine copy; the tests only read it.

```sh
F1_25_DIR="/path/to/F1 25" cargo test -p swofty-formats
```

Without `F1_25_DIR` they are skipped. The Rust output must match the Python tools byte for byte:

```sh
python3 tools/gamedb.py roundtrip "$F1_25_DIR/game_package/gamedatabases/gamedatabases.2025.bin"
python3 tools/erpw.py rebuild "$F1_25_DIR/2025_asset_groups/ui_package/databases.erp" /tmp/databases.erp
```

## The native loader

```sh
native/build.sh
native/test/run_wine_test.sh
```

`build.sh` cross-builds `version.dll` and `swofty_core.dll` with `zig cc` and `zig c++ -std=c++23` for `x86_64-windows-gnu`, statically linked, into `native/dist/`. It enters `nix shell nixpkgs#zig` if zig is missing. The build is deterministic.

`run_wine_test.sh` builds a synthetic `F1_25.exe` that carries the hook signatures and the home-menu instruction sequence, runs it under Wine with the built bundle, and checks the tile order, renumbering, label, id and action.

## Documentation

```sh
pnpm install
pnpm docs:dev
pnpm docs:build
```

## Commits

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/), checked by commitlint. lefthook runs rustfmt and clippy on staged Rust files before each commit.
