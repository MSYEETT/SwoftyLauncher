# Game database (GVDB)

`game_package/gamedatabases/gamedatabases.2025.bin` is the game value database: one flat, sorted hash map of about 743,000 records. It has no sections, tables, schema or string table. Table and row structure exists only in the path strings, which the file does not contain.

`packages/formats` reads and writes it. `tools/gamedb.py` is the reference implementation and the byte-identity oracle.

## Layout

All integers are little-endian.

```
u32  count
record[count]:
  u32  key_lo        low 32 bits of FNV-1a-64(path)
  u32  key_hi        high 32 bits; the file is sorted ascending by key_hi
  u8   type          value type tag
  u32  len           length of the masked value
  u8   value[len]    masked UTF-8 text
  u8   0x00          terminator, not counted in len, not masked
```

A record is `14 + len` bytes. The parse ends exactly at end of file, with no trailer or checksum.

## Keys

`key = fnv1a64(path)`, with basis `0xcbf29ce484222325` and prime `0x100000001b3`, over a dotted path such as `tracks.Track_3.numberOfCorners`.

- Table rows are `<container>.<Table>_<index>`, and rows nest: `entrants.entrant_lists.entrant_list_34.entrant_21.team`.
- Every table row has a `_metadata_` field holding its label (`bahrain`, `mclaren_driver_01`).
- Every table stores its row count in `<container>._<row>_instancecount_`. The game reads rows `0..count-1` only. See [the row-count rule](/modding/database-ops#the-row-count-rule).
- FNV-1a can be run backwards one byte at a time, so a known suffix can be peeled off a key. Tables whose path is still unknown are named by a `{hash}` placeholder.

## Value masking

Values are masked by adding a 30-byte repeating key stream that runs continuously across the whole file:

```
MASK  = "tG5MYmBZEZZnrH7yg2qTC4qj42Jcq9"
plain[i] = enc[i] - MASK[(10 + S + i) % 30]    (mod 256)
```

`S` is the total `len` of all earlier records. The stream skips headers and terminators, so any edit changes every masked byte after it.

## Types

| Tag | Shape |
|---|---|
| 0 | integer |
| 1 | float |
| 2 | string or reference |
| 3 | enum token |
| 4 | id or name |
| 5 | bool, `True` or `False` |
| 6 | time, `HH,MM` |
| 9 | curve, `@v2@minx:..,maxx:..,x:y,...` |
| 13 | list, comma-joined |
| 14 | vector, `0,-0.6,0` |
| 7, 8, 11, 12, 17, 18 | not yet classified |

New fields copy their type tag from a sibling row; tags are never guessed.

## Writing

The writer sorts records by `key_hi` with a stable sort, so the 50 `key_hi` values that repeat keep their order. It rewrites `count` and re-masks the whole stream. Reading the pristine file and writing it back is byte-identical.

The game probably binary-searches on `key_hi` and then checks `key_lo`. The load buffer may have a fixed size, so the build warns as the file grows.

## Tests

```sh
F1_25_DIR="/path/to/F1 25" cargo test -p swofty-formats
```
