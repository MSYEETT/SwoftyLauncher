# Game database (GVDB)

`game_package/gamedatabases/gamedatabases.2025.bin` stores game values, including teams and tracks.

For mod changes, use [database operations](/modding/database-ops).

## How values are stored

Each record has a numeric key, a type and a text value. Keys come from paths such as `tracks.Track_3.numberOfCorners`. The file does not store the path names.

Rows have a `_metadata_` label. Tables have an `_instancecount_` value that tells the game how many rows to read. Adding rows must update that count too.

## Tools

- `packages/formats/src/gvdb.rs`: Rust reader and writer.
- `tools/gamedb.py`: Python reader, writer and inspection tool.

Writers must preserve record order for equal sort keys and rewrite the value encoding after edits. An unchanged file should write back with identical bytes.

## Test

```sh
F1_25_DIR="/path/to/F1 25" cargo test -p swofty-formats
```

This checks real game files when `F1_25_DIR` is set.
