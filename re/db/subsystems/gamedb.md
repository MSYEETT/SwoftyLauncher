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
- 2026-10-17 12:47 b26crash: Gdb_Get_FieldInt proto should be GdbValue **(row, name, mode): it returns the value-slot pointer; mode 0 miss returns &info[8] (end sentinel), mode 1 goes through Gdb_Lookup_Key (null on miss). Sub-row names are often stack-built (16-byte rodata chunk + imm tail, e.g. 'ai_vehicle_confi'+'g', 'story_in'+'fo') so they never show in strings; work/exe/b26crash/sw/sweep2.c does prefix x mid x leaf sweeps against the key set. Bools parse as _stricmp(v,'true')==0 (Gdb_Unk_1445a12d0 case 5); available is forced True at runtime by Sess_Set_RowsAvailable / Sess_Apply_ContentAvailability.
- 2026-10-19 13:27 ea-port: EA port: gamedb.table_cursor_init (Gdb_Init_TableCursor) cannot be relocated statically on EA (on-disk exe fully packed, see core.md). dbread.cpp already has a build-independent probe fallback gated by same_build(); on EA it also needs its S_GLOBAL/S_INSERT scans pointed at section .rdata (not .xtls) and EA patterns from a decrypted-memory dump.
- 2026-10-19 13:56 ai-re: ai lane: the 5 ai.general_ai_settings.* db_keys rows (readers 0x1405fb920, 0x140614d70) are AI-feature keys; reader 0x1405fb920 is now Ai_Resolve_GeneralSettings (L2) and 0x140614d70 is Ai_Update_VehicleDriver (L1). Feel free to flip their subsystem col gamedb->ai, or leave as gamedb discovery rows (cross-referenced from subsystems/ai.md#general-settings).
- 2026-10-19 15:13 rnd-tyres: Career department row count: Career_Load_DepartmentEntries (0x1412326e0) gets its loop bound from Gdb_Init_TableCursor via the key <container>._UpgradeDepartmentEntry_instancecount_. The two REAL dept containers the runtime loader reads are {5e21a18f9f59dbf6} and {1f304d36c3c2b947} (instancecount=4, rows UpgradeDepartmentEntry_0..3 = Aero/Chassis/Durability/Powertrain). Confirmed at runtime: rnd-probe logged db_count=4, idx 0..3, two rsi objects. Count-store site site_Gdb_DeptCountStore (0x1445a28df) + sig gamedb.dept_count_store. NOTE: the static containers {016caf2bb4d4f128}/{3121442cecc7bb08} (rows _4..7, NO instancecount key) are NOT read by this loader.
