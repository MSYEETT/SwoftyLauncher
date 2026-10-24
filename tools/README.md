# Python tools

These are the research tools the formats were worked out with. They are the byte-identity oracle for
`packages/formats`: the Rust readers and writers must produce the same bytes as these scripts for the same input
and the same edit. They are not shipped, except Swofty UI.

| Tool | Format or job |
|---|---|
| `gamedb.py` | `gamedatabases.2025.bin` (GVDB): dump, round trip, get, set |
| `erp.py`, `erpw.py`, `erpx.py`, `erpdump.py` | ERP archives: index, byte-identical rebuild, fragment replace and extract |
| `pkg.py`, `uinode.py`, `uitree.py` | `!pkg` node graphs: read, write, djb2 node ids, tree view |
| `idf.py` | `.idf` UI scenes (`eScene`): tree, params, geoms, round trip, clone a slot, move a slot, check and retarget `eXRef` references |
| `swofty-ui/` | [Swofty UI](swofty-ui/README.md), the scene viewer and editor that release archives ship as `bin/swofty-ui` |
| `build_home_tile_mod.py`, `swofty_settings.py` | The reference implementation of SwoftyMod's UI edits and settings page |
| `modswap.py` | The pristine-verify, install and restore pattern behind the journal |
| `vtf.py` | Car handling files (`.vtf`, bxml plus TSMD): read, write, add a track's ERS settings |
| `gfx_material.py` | `.material` (GfxMaterialRes) in the F1 25 and F1 23 layouts: read, write, dump, convert F1 23 materials to F1 25 (`convert`, `convert-erp`). `gfx_material_layouts.json` holds the F1 25 layout of each shader and is rebuilt with `catalog <game>/2025_asset_groups` ([format](../re/db/subsystems/gfx_material.md)) |
| `map_check.py` | Checks a profile built with tracks against the game it was built from, using the build's `plan.json` |
| `track_keys.py` | The game-DB names `F1_25.exe` reads, stack-built ones included, and the keys they resolve to on a track's rows |
| `gdb_hashsim.py` | Replays the game's hash-map load of `gamedatabases.2025.bin` and prints the missing-key sentinel |
| `rtassets.py` | ERP and LNGT writers for the runtime assets Wine test (`native/test/run_runtime_assets_test.sh`) |
| `gamedb/catalog.py` | The table and key catalogue of the game database (`gamedb/catalog/`), rerunnable against a game folder |
| `gamedb/deepkeys.py` | Finds the keys under a game-database row that no catalogue names, by hashing candidate paths from the exe's identifiers, the catalogue and table labels |
| `ui_catalog.py nodes` | The UI node catalogue (`uicatalog/catalog.json`): every node type, property, command, event, pin and layout binding in the stock `.ui` files, and every scene element type and param in the stock `.idf` scenes. `packages/core` checks UI edits against it |
| `pack_addon.py` | Packages an addon folder as a `.swoftyaddon`; `native/build.sh` uses it for the addons a release bundles |

`packages/formats/tests/real_files.rs` runs the comparison:

```sh
F1_25_DIR=<game folder or a pristine copy> cargo test --release -p swofty-formats --test real_files
```

Check a build with tracks (needs `zstandard` or the `zstd` command):

```sh
python3 tools/map_check.py <overlay folder> --game <game folder or pristine copy> --maps <track packages folder>
```

Rebuild the UI node catalogue (archives the launcher has replaced are read from `--stock` folders, checked against the baseline's sha256):

```sh
F1_25_DIR=<game folder> python3 tools/ui_catalog.py nodes --baseline ~/.local/share/swofty/baseline/<build>/baseline.json [--stock <pristine copy>]
```

The reader-key part of the check and `track_keys.py` need `numpy`, `pefile` and `capstone`, and an `F1_25.exe` whose build is listed in `track_keys.BUILDS`.

Scripts that read or write game files take them from `work/`, which is ignored by git, or from `F1_25_DIR`. Never
commit their output.
