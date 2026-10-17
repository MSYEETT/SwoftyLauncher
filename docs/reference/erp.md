# ERP archives

Most of F1 25's assets live in `.erp` archives under `2025_asset_groups/`. An ERP is an index of named resources, each made of one or more fragments, followed by the fragment data.

`packages/formats` reads and writes ERPs. `tools/erp.py` and `tools/erpw.py` are the reference implementation and the byte-identity oracle.

## Layout

All integers are little-endian.

```
0x00  "ERPK"
0x04  u32  version                 3 in F1 25 (4 adds two bytes per entry, below)
0x08  ...  (copied unchanged)
0x10  u64  info_offset             0x30
0x18  u64  info_size               bytes from 0x30 to the data
0x20  u64  data_offset             0x30 + info_size
0x28  ...  (copied unchanged)
0x30  u32  resource_count
0x34  u32  fragment_count          total over all resources
0x38  entry[resource_count]
      data
```

Each index entry:

```
u32  entry_size                    bytes after this field
u16  name_length                   including the NUL
u8   name[name_length]             "eaid://ui_package/screens/fe_home.ui\0"
u8   type[16]                      NUL-padded, such as "UIDF" or "GfxTexture"
u8   unknown[4]                    6 bytes when version >= 4
u8   fragment_count
fragment[fragment_count]:
  u8   name[4]                     such as "temp", "syst"
  u64  offset                      from data_offset
  u64  size                        decompressed size
  u32  flags
  u8   compression                 0x11 zstd, 0x91 stored
  u64  packed_size                 bytes in the archive
u8   hash[16]                      MD5 of the concatenated decompressed fragments
```

## Facts the writer relies on

- Fragment data is contiguous, in index order, with no gaps.
- Compression `0x11` is zstd. `0x91` is stored: `size == packed_size` for every such fragment.
- A resource's hash is the MD5 of all its fragments, decompressed and concatenated.
- Rebuilding the index and data from the parsed archive, with offsets reassigned in index order, reproduces the archive byte for byte. This was checked on `databases.erp`, `persistent.erp` and `flow_frontend.erp` (165 MB).

## Writing

To replace a fragment, the writer recompresses it if it was zstd (level 19), updates `size` and `packed_size`, recomputes the resource's MD5, reassigns every fragment offset in index order, and rewrites `info_size` and `data_offset`. Untouched fragments are copied as stored, so their bytes do not change.

## Resource types

UI resources (`UIDF`, `UIDB`, `UIRO`, `UIPR`, `UIMD`, `UCON`, ...) are [`!pkg`](/reference/pkg). World files (`.wep`), layers and track markup use other formats; layers are `!pkg` variants (`!pkgWRLD`, `!pkgINST`), and track markup and track space data are binary XML.

## Tests

```sh
F1_25_DIR="/path/to/F1 25" cargo test -p swofty-formats
```

With `F1_25_DIR` set, the round-trip tests rebuild real archives and compare them byte for byte. Without it they are skipped.
