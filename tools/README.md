# Python tools

These are the research tools the formats were worked out with. They are the byte-identity oracle for
`packages/formats`: the Rust readers and writers must produce the same bytes as these scripts for the same input
and the same edit. They are not shipped.

| Tool | Format or job |
|---|---|
| `gamedb.py` | `gamedatabases.2025.bin` (GVDB): dump, round trip, get, set |
| `erp.py`, `erpw.py`, `erpx.py`, `erpdump.py` | ERP archives: index, byte-identical rebuild, fragment replace and extract |
| `pkg.py`, `uinode.py`, `uitree.py` | `!pkg` node graphs: read, write, djb2 node ids, tree view |
| `build_home_tile_mod.py`, `swofty_settings.py` | The reference implementation of SwoftyMod's UI edits and settings page |
| `modswap.py` | The pristine-verify, install and restore pattern behind the journal |
| `vtf.py` | Car handling files (`.vtf`, bxml plus TSMD): read, write, add a track's ERS settings |
| `map_check.py` | Checks a profile built with maps against the game it was built from, using the build's `plan.json` |

`packages/formats/tests/real_files.rs` runs the comparison:

```sh
F1_25_DIR=<game folder or a pristine copy> cargo test --release -p swofty-formats --test real_files
```

Check a build with maps (needs `zstandard` or the `zstd` command):

```sh
python3 tools/map_check.py <overlay folder> --game <game folder or pristine copy> --maps <maps folder>
```

Scripts that read or write game files take them from `work/`, which is ignored by git, or from `F1_25_DIR`. Never
commit their output.
