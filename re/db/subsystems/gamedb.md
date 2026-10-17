# gamedb: GVDB game value database (code side)

Owner: see `subsystems.tsv`. Data-side format: research/gamedb.md. Cap analysis: research/grid24-cap.md §1.

## Map
- `Gdb_Load_GameDatabase` (0x1445a0350) references `game_package/gamedatabases/`, `gamedatabases.bin`, `GameDatabase.fileName`.
- Keys are `fnv1a64(dotted path)`. 1,108 code sites load key hashes as 64-bit immediates (`db_keys.tsv`, L1, from `work/exe/db_key_immediates.txt`); 92 have known paths.
- **Row counts** {#instancecount}: `Gdb_Init_TableCursor` (0x1445a25c0) builds `<container>._<row>_instancecount_` at runtime, looks it up with `Gdb_Lookup_Key`, caches it at `GdbTableCursor.row_count` (+0x200, < 0 = not cached). Missing key = 0 rows. Every tool that adds DB rows must bump the matching `_instancecount_` key.
- Field reads by name: `Gdb_Get_FieldInt(row, "max_entrants", 0)`; rows by label: `Gdb_Find_RowByLabel` (independent of row counts).

## Open
- The eleven `Gdb_Unk_1445a6xxx` functions (labelled vt0..vt35 by the exe agent): which object's vtable.
- `Gdb_Unk_14305c1f0` was labelled onGameValueDbLoaded but its strings are network logging.

## Inbox
