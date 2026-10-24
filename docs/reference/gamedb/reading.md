# How the game reads it

Addresses are for `F1_25.exe` build `d873a1c8…` and come from decompiles in the [RE knowledge base](https://github.com/Swofty-Developments/SwoftyLauncher/tree/main/re/db) (`re/db/functions.tsv`). Statements are proven by decompile or by a runtime crash unless marked inferred.

## Loading

`Gdb_Load_GameDatabase` (`0x1445a0350`) reads the file named by the config value `GameDatabase.fileName` (default `gamedatabases.bin`, resolved to the `.2025.bin` file) and inserts every record, in file order, into one hash map (`Gdb_HashMap_Insert`, `0x1445a12d0`). The map is a martinus `robin_hood` table: multiplier `0xc4ceb9fe1a85ec53`, 80% maximum load, 2^20 buckets for the shipping file. It grows on demand. The record count has no fixed limit.

## The lookup API

| Function | Address | What it does |
|---|---|---|
| `Gdb_Lookup_Key` | `0x1445a22f0` | Hashes a full path and probes the map |
| `Gdb_Init_TableCursor` | `0x1445a25c0` | Builds `<container>._<row>_instancecount_`, reads it and caches the row count in the cursor (`+0x200`). A missing count key gives 0 rows |
| `Gdb_Get_RowByIndex` | `0x1445a28f0` | Builds the path of row `i` (`<container>.<row>_<i>`) |
| `Gdb_Find_RowByLabel` | `0x1445a2bd0` | Finds a row by label (below) |
| `Gdb_Get_FieldInt` | `0x1445a2cf0` | Builds `<rowpath>.<field>`, hashes it, probes the map and returns a pointer to the value slot |
| `Gdb_Get_RowName` | `0x1445a3900` | Returns a row's path |

Readers build every path at run time from a container, a row word and a field name, with format strings such as `%s.%s_%d` and `%s.%s.%s_%d`. The callers of these six functions are the game's database readers. `tools/track_keys.py` lists the 8,286 names they use.

### Stack-built names

Many names never appear whole in the exe. The compiler writes them to the stack in pieces: a 16-byte constant from `.rdata` plus an immediate. Examples:

| Name | Pieces |
|---|---|
| `CareerHubFirstVisit` | `CareerHubFirstVi` (`.rdata 0x1457e8080`) + `sit` |
| `ai_vehicle_config` | `ai_vehicle_confi` + `g` |

A string search of the exe misses these. `track_keys.py` emulates the stack writes in every reader to recover them.

## Row counts

Every table walk starts with `Gdb_Init_TableCursor`. Loops run `i = 0 .. count−1` over `<row>_<i>`. Rows at or above the count are never read.

Example: the F1 2026 grid is `entrants.entrant_lists.entrant_list_34`, whose `_entrant_instancecount_` is 22. The entrant-list parser (`Sess_Parse_EntrantList`, `0x142453c40`) reads entrants 0–21 only. Adding `entrant_22` and `entrant_23` without raising the count leaves the grid at 22.

## Label lookups

`Gdb_Find_RowByLabel` scans rows `0 .. count−1` of a table and compares the FNV-1a-32 hash of each row's `_metadata_` value with the FNV-1a-32 hash of the wanted label. On a miss it sets the row path to `Invalid` and clears the found flag.

- A row at or above the count cannot be found by label.
- References between tables are labels: an entrant names its `team`, `vehicle` and `driver`; a track names its `track_resource`; a dataset row names its `track`.

## Missing keys

A lookup of a key that does not exist does not return null. The not-found path of `Gdb_Get_FieldInt` (`0x1445a2f7e`) returns `&info[8]`, a pointer into the map's metadata bytes. Callers read that as a record pointer and only check it for null. In the shipping database `info[8..15]` is `0e 00 00 08 00 0f 00 08`, so the "record pointer" is `0x08000f000800000e`. Dereferencing it raises an access violation.

`tools/gdb_hashsim.py` replays the loader and prints this sentinel for any database. The stock and modded files give the same value; the shipping game never looks up a missing key.

**Rule: every key the game reads must exist.** The build skips `remove-record` ops on stock keys unless they set `allow_stock_removal` ([Removing stock records](/reference/ops/database#removing-stock-records)). Crashes found so far:

| Case | Key | Where |
|---|---|---|
| New vehicle row with 28 of its 246 keys | `entrants.vehicles.vehicle_413.ai_vehicle_config.exit_fallibility.max_yaw_angle_deg` | `Sess_Parse_AiHandlingConfig` (`0x1406828a0`), fault at `0x1406831f7`, during startup |
| New team row above `_team_instancecount_` | `Invalid.stat_id` after a failed label lookup | team branch of `Sess_Parse_Entrant` (`0x142452e60`), fault at `0x14245323c` |
| Imported mod made for game version 1.0.138 on 1.0.141, which removed 14,618 stock records | Not identified | `Sess_Apply_ContentAvailability` reads the sentinel `0x08000f000800000e`, during startup |
| Copied customisation item row without its colour defaults | `customisation.custom_avatar.gloves.Gloves_N.colour_1_default` | `Game_Build_CustomisationCatalogue` (`0x1414059c0`), called from `Sess_Apply_ContentAvailability`, fault at `0x1414078fa`, during startup ([Colour defaults of customisation items](/reference/ops/database#colour-defaults-of-customisation-items)) |
| Driver Manager built before a mod made for game version 1.0.138: the mod relabelled the entrant list Driver Manager had added a free agent to and lowered `entrants.entrant_lists.entrants._entrant_instancecount_` below the driver's entrant row | `Invalid.driver` after a failed label lookup of `dm_208` in `entrants.entrant_lists.entrants` | `0x1424de3e0` under `Sess_Parse_Entrant`, fault at `0x1424de416`, at startup under `Sess_Apply_ContentAvailability` |
| Cloned track row missing stack-built sub-tables | `Daylight_0.start`, `Daylight_0.end`, `CareerHubFirstVisit_0.timeofday`, `CareerHubLastVisit_0.timeofday` | track reader `0x142e4c640`, fault at `0x142e53c0b`, at startup under `Sess_Apply_ContentAvailability` |

### Checked and unchecked reads

| Reader | Behaviour on a missing key or label |
|---|---|
| Entrant parser, team label (`0x142452e60`) | Unchecked: crash |
| Entrant parser, driver label | Checked |
| Entrant list parser, `entrant_reference` label (`0x1424de3e0`) | Unchecked: crash |
| Vehicle AI config (`0x1406828a0`, called from `Sess_Load_VehicleAiConfig` `0x1425027e0`) | Unchecked: crash |
| Track reader `0x142e4c640` (`Daylight`, `CareerHubFirstVisit`, `CareerHubLastVisit`) | Unchecked: crash |
| Per-race DNFs (`0x1409c2b10`): `Failures.*`, `Faults.%s.*`, `Accidents.%s.*`, `LockUps.%s.*` by track label | Unchecked: crash. A new track id needs a row in `canned_events.Per-race DNFs` |
| Pit lane spline generation and slide settings (`0x1434ca550`) | Checked |
| Practice programme per-track settings | Checked |
| Team `story_info` table (`Sess_Load_Teams` `0x142469440`) | A missing count reads as 0 rows |

Readers not in this table are unaudited. Treat them as unchecked.

## Runtime state

- Every team and vehicle row in the file has `available=False`. `Sess_Set_RowsAvailable` (`0x1413e44c0`) and the startup availability pass (`0x1413e4cf0`) set `available` at run time on rows `0 .. count−1` or on rows found by label, from stat-id lists. The startup vehicle AI loop (`Sess_Startup_VehicleAiLoop`, `0x142505f30`) then reads every available vehicle's `ai_vehicle_config`.
- Raising a team or vehicle count puts the new rows into these passes. Every key of the new rows is then read.
- The active content set comes from the config value `ContentSet.currentContentSet`, falling back to `content_sets.current_content_set` (empty in the shipping database). `0x1413d5a60` parses its `available_tracks_N`, `initial_tracks_N` and `sorted_track_order_N`.

## Hashes in code

1,098 full key hashes appear as 64-bit immediates in the exe (`movabs`). The game looks these keys up without building a path. 98 of them have a known name. `catalog.py` marks them `hash` in the `exe` column.

## A null-on-miss hook (not shipped)

A MidHook at `0x1445a2f9d` (signature `4D 85 DB 74 0C 49 29 C9 49 C1 F9 04 4C 89 CA EB 02 31 D2 48 C1 E2 04 48 8D 04 11 48 83 C0 08 48 81 C4 28`, match + `0x1F`) can point `rax` at a zero qword when it equals `*(*(0x146cc4278) + 0xc8) + 8`. Every `Gdb_Get_FieldInt` caller would then see a missing key as null. The same find code is inlined about 88 times elsewhere and those copies are not covered. Swofty Launcher for F1 25 does not install this hook; database mods must keep every read key present.
