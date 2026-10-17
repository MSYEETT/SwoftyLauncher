# File format

`game_package/gamedatabases/gamedatabases.2025.bin`. The shipping file is 19,341,077 bytes with 742,857 records. All integers are little-endian. Everything on this page is proven from the file and the exe unless marked inferred.

## Layout

```
u32  count                   // number of records (0x000B55C9 in the shipping file)
record[count]:
  u32  key_lo                // low 32 bits of the key
  u32  key_hi                // high 32 bits of the key
  u8   type                  // value type tag
  u32  len                   // value length in bytes
  u8   value[len]            // masked UTF-8 text
  u8   0x00                  // terminator: not counted in len, not masked
```

- A record is `14 + len` bytes. The last record ends at the end of the file. There is no section table, string table, index, trailer or checksum.
- Records are sorted ascending by `key_hi` only. 50 `key_hi` values occur more than once; records with the same `key_hi` keep their original order. All 742,857 full keys are unique.
- A writer that sorts stably by `key_hi` and re-masks the values reproduces the shipping file byte for byte.

## Keys

`key = fnv1a64(path)`, the 64-bit FNV-1a hash of the UTF-8 path:

```
h = 0xcbf29ce484222325
for each byte c: h = (h ^ c) * 0x100000001b3   (mod 2^64)
```

`key_hi = key >> 32`, `key_lo = key & 0xffffffff`. Example: `tracks.Track_3.numberOfCorners` hashes to the key of the record holding `15`.

FNV-1a can be run backwards with the inverse prime `0xce965057aff6957b`: `h_prev = (h * inv) ^ c`. Peeling a known suffix off a key gives the hash of the path before it. This is how rows and tables are found without knowing their full paths (`fnv1a64_unwind` in the formats crate).

## Value masking

Each value byte is stored plus one byte of a 30-byte repeating key:

```
MASK  = "tG5MYmBZEZZnrH7yg2qTC4qj42Jcq9"
plain[i] = enc[i] - MASK[(10 + S + i) % 30]    (mod 256)
```

`S` is the sum of `len` over all earlier records. The stream skips record headers and terminators. Changing the length of one value shifts the mask of every value after it, so an edit rewrites every later byte of the file.

## Value types

Values are text. The type tag says how the game parses the text. Counts are for the shipping file.

| Tag | Name | Records | Text form | Examples |
|---|---|---|---|---|
| 0 | int | 128,932 | decimal integer | `30`, `100000` |
| 1 | float | 172,067 | decimal number | `17.5`, `0.009000003`, `-1` |
| 2 | string | 152,616 | text, often a label or a localisation key | `mercedes`, `lng_track_usa_f2_title` |
| 3 | enum | 47,290 | token | `Cinematic`, `championship` |
| 4 | id | 82,309 | name or row label | `styria`, `sophie_levasseur_2026` |
| 5 | bool | 65,476 | `True` or `False` | |
| 6 | time | 4,337 | `HH,MM` | `11,30` |
| 9 | curve | 24,709 | `@v2@minx:…,maxx:…,x:y,…` | |
| 13 | list | 24,336 | comma-joined values, up to 7,040 bytes | `6,8` |
| 14 | vector | 8,476 | comma-joined floats | `0,-0.6,0` |
| 7 | (unnamed) | 4,841 | comma-joined integers | `12,30`, `28,40,0` |
| 8 | (unnamed) | 6,868 | float or comma-joined floats | `0,0,0,0`, `0.31,0.4,0.53` |
| 11 | colour | 15,508 | signed 32-bit ARGB integer (inferred: every `*colour` field uses it) | `-13249833` (`0xFF35CED7`) |
| 12 | (unnamed) | 2,622 | integer | `2`, `597` |
| 17 | (unnamed) | 12 | empty | |
| 18 | (unnamed) | 2,458 | comma-joined tokens | `game_state`, `driver_names,on` |

Names for tags 0–6, 9, 13 and 14 are inferred from the values; `True`/`False` for tag 5 is proven by the mask derivation. Tags 7, 8, 12, 17 and 18 are described by shape only. A new key must use the tag of the same field on a stock row.

- Text is UTF-8 without a terminator inside `len`. 23 values contain non-ASCII bytes.
- 86,764 values are empty (`len` 0). An empty value is a value: the key exists.
- Lists and vectors have no spaces after the commas.

## Paths

The file holds no names. Names come from the strings the game builds when it looks keys up (see [How the game reads it](/reference/gamedb/reading)).

| Form | Example |
|---|---|
| Setting | `rules.stop_go_penalty.penalty_duration` |
| Row field | `tracks.Track_3.numberOfCorners` |
| Sub-object of a row (no index) | `tracks.Track_3.ai_track_config.formation_lap.formation_rev_distance` |
| Nested table row | `entrants.entrant_lists.entrant_list_34.entrant_21.driver` |
| Row label | `tracks.Track_3._metadata_` = `bahrain` |
| Row count | `tracks._Track_instancecount_` = `43` |
| Nested row count | `entrants.entrant_lists.entrant_list_34._entrant_instancecount_` = `22` |

- Rows are `<container>.<Row>_<i>`, numbered from 0. The exe formats them with `%s.%s_%d` and `%s.%s.%s_%d`.
- The row count of a table is `<container>._<Row>_instancecount_`, an int. Nested tables have their own count under the parent row.
- Every row has `_metadata_`, its label. Labels are unique within a table.
- One-row sub-tables are common: `tracks.Track_N.Daylight_0`, `entrants.teams.team_N.story_info_0`, `track_data.game_setup.TrackDataSet_N.TrackData_N.TyreData_0`. Each has its own count key set to 1.
- Names may contain spaces and dashes: `canned_events.Per-race DNFs.Per-race DNF_5`, `Session Start Times`.

### Unresolved paths

Some tables sit under a container whose name is not known. The catalogue and [database ops](/modding/database-ops) write them with a hash placeholder:

| Placeholder | Meaning |
|---|---|
| `{827bc45f3d492762}5.entrant_0.team` | `{H}` is `fnv1a64("<container>.<Row>_")`; `5` is the row number |
| `{78c616c2de9963a2}.DriverStatisticsHolder_3` | `{C}` is `fnv1a64("<container>")`; the row word is known |

`{hex}rest` means "continue the FNV-1a hash `hex` with `rest`". A table's row count key needs the container's real path or hash and the row word, so row additions need one of those.

## Datasets

Track data is split into datasets, rows of `track_data.game_setup.TrackDataSet_N` (20 rows):

| Datasets | `parent_data_set` |
|---|---|
| `short` | none |
| `f1`, `f2` | `short` |
| `classic`, `f1_2023`, `sprint_f1_2023`, `f1_2024`, `sprint_f1_2024`, `f1_2025`, `sprint_f1_2025`, `multiclass`, `sprint_multiclass`, `f1_2026` | `f1` |
| `sprint_f1_2026` | `f1_2026` |
| `f2_2023`, `f2_2024`, `f2_2025`, `f2_2025_sprint`, `f2_2026`, `f2_2026_sprint` | `f2` |

- Each dataset has `TrackData_N` rows with `_metadata_` and `track` (the track id) plus one-row sub-tables: `TyreData` (`MandatoryPitstopsOverride`), `Session Start Times`, `TrackFuelSettings` (`Qualifying.{full,short,one_shot}`, `RaceFuelSettings_N`), `TrackGeneralData`, `TrackTextures`, `TrackSafetyCarData`, `TrackAuthenticWeekendData`, `TrackNameData`, `TrackStats` (`SingleTrackStat_N`), `TrackPitData`, `TrackAudioData`.
- Root datasets (`f1`, `f2`) have full rows. Child datasets override sub-tables. The dataset reader (`0x142ed4870`) falls back to `parent_data_set` only when a dataset's `TrackData` table is empty.

Other season-keyed tables use the same labels without the parent chain:

- entrant lists (`entrants.entrant_lists.entrant_list_N`, 68 rows: `f1_2025`, `f1_2026`, `f2_2025`, `f1cg_f1_2013` …);
- entrant setup modes (`entrants.entrant_setup_modes.entrant_setup_mode_N`, 11 rows: `f1`, `classic`, `f2`, `f1_generic`, `f1_custom_team`, `f1_world`, `f1_elimination`, `multiclass`, `f1_26`, `f1_generic_2026`, `f1_custom_team_2026`);
- content sets (`content_sets.content_set_N`: `2024`, `2025`, `2026`, `vte`, `f1a` …), which list the playable tracks in `available_tracks_N.tracks`;
- the vehicle packages (`f1_2026_vehicle_package` …) named by AI difficulty and car setup rows.
