# Example launcher addon

An addon with a launcher page, an importer and a build hook, run by a helper program. The protocol is in `docs/design/addons-and-mods.md`, section 2, Third-party addons.

| Path | What it is |
|---|---|
| `swofty.json` | The manifest. `launcher` names the helper per platform, the `Example` section, the `exmod` import extension, the build hook and the `example` command. |
| `src/main.rs` | The helper. It answers `hello`, `view`, `event`, `import`, `build`, `command` and `summary` over stdin and stdout. |

The page lists the imported `.exmod` files with a switch each. The switches are per-profile state. The build hook writes the names of the items that are on to `enabled.txt` in the cache folder and installs it as `example-launcher-addon/enabled.txt`. `swofty example` prints each item and whether it is on.

## Build and install

```sh
cargo build --release
mkdir -p bin
cp target/release/example-helper bin/
swofty addons install .
swofty addons enable example.launcher-addon
```

On Windows, build `example-helper.exe` and copy it to `bin/`. Ship both binaries in `bin/` for players on both platforms.
