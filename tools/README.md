# Python tools

These are the research tools the formats were worked out with. They are the byte-identity oracle for
`packages/formats`: the Rust readers and writers must produce the same bytes as these scripts for the same input
and the same edit. They are not shipped, except Swofty UI.

| Tool | Format or job |
|---|---|
| `gamedb.py` | `gamedatabases.2025.bin` (GVDB): dump, round trip, get, set |
| `erp.py`, `erpw.py`, `erpx.py`, `erpdump.py` | ERP archives: index, byte-identical rebuild, fragment replace and extract |
| `pkg.py`, `uinode.py`, `uitree.py` | `!pkg` node graphs: read, write, djb2 node ids, tree view |
| `idf.py` | `.idf` UI scenes (`eScene`): tree, params, geoms, round trip, clone a slot, move a slot |
| `swofty-ui/` | [Swofty UI](swofty-ui/README.md), the scene viewer and editor that release archives ship as `bin/swofty-ui` |
| `build_home_tile_mod.py`, `swofty_settings.py` | The reference implementation of SwoftyMod's UI edits and settings page |
| `modswap.py` | The pristine-verify, install and restore pattern behind the journal |
| `vtf.py` | Car handling files (`.vtf`, bxml plus TSMD): read, write, add a track's ERS settings |
| `map_check.py` | Checks a profile built with maps against the game it was built from, using the build's `plan.json` |
| `track_keys.py` | The game-DB names `F1_25.exe` reads, stack-built ones included, and the keys they resolve to on a track's rows |
| `gdb_hashsim.py` | Replays the game's hash-map load of `gamedatabases.2025.bin` and prints the missing-key sentinel |
| `rtassets.py` | ERP and LNGT writers for the runtime assets Wine test (`native/test/run_runtime_assets_test.sh`) |
| `gamedb/catalog.py` | The table and key catalogue of the game database (`gamedb/catalog/`), rerunnable against a game folder |

`packages/formats/tests/real_files.rs` runs the comparison:

```sh
F1_25_DIR=<game folder or a pristine copy> cargo test --release -p swofty-formats --test real_files
```

Check a build with maps (needs `zstandard` or the `zstd` command):

```sh
python3 tools/map_check.py <overlay folder> --game <game folder or pristine copy> --maps <maps folder>
```

The reader-key part of the check and `track_keys.py` need `numpy`, `pefile` and `capstone`, and an `F1_25.exe` whose build is listed in `track_keys.BUILDS`.

Scripts that read or write game files take them from `work/`, which is ignored by git, or from `F1_25_DIR`. Never
commit their output.
