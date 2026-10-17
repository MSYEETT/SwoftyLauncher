# Game database (GVDB)

`game_package/gamedatabases/gamedatabases.2025.bin` holds the game's values: tracks, teams, drivers, cars, grids, career data, AI tuning, menus and rules. The game loads it whole at startup. It is one flat list of 742,857 records in the shipping game, each a hashed key, a type tag and a text value.

| Page | Contents |
|---|---|
| [File format](/reference/gamedb/format) | Header, records, key hashing, value masking, value types, path naming, row counts, datasets |
| [How the game reads it](/reference/gamedb/reading) | The lookup API in `F1_25.exe`, row counts, label lookups, missing-key crashes |
| [Modding recipes](/reference/gamedb/recipes) | Editing values, adding rows, grid size, adding a track, checking a build |
| [Tools](/reference/gamedb/tools) | The formats crate, the launcher's database stage, the Python tools |
| [Table catalogue](/reference/gamedb/catalogue) | Every table family with row counts and scope (generated) |
| [Key schema](/reference/gamedb/schema) | Fields, types and examples for the main tables (generated) |

For addon files that change the database, see [Database edits](/reference/ops/database).

## In short

- A key is `fnv1a64(path)`, where `path` is a dotted name such as `tracks.Track_3.numberOfCorners`. The file stores only the hash. Names come from the game's code and from dictionaries.
- Tables are rows named `<container>.<Row>_<i>`. The row count is a separate key, `<container>._<Row>_instancecount_`. The game reads rows `0` to count−1 and ignores the rest.
- Every row has a `_metadata_` field holding its label (`bahrain`, `mclaren_26`, `f1_2026`). The game finds rows by label with a scan bounded by the row count.
- A key the game reads must exist. A missing key does not read as zero: the lookup returns a garbage pointer and the game crashes.
- Adding a row means adding every key a stock row of that table has, then raising the count.

## Glossary

| Term | Meaning |
|---|---|
| Record | One entry in the file: a key, a type tag and a text value |
| Key | `fnv1a64(path)`, the 64-bit FNV-1a hash of the record's dotted path. The file stores only the hash |
| `key_hi`, `key_lo` | The high and low 32 bits of the key. The file is sorted by `key_hi` |
| Value mask | Each value byte is stored plus a byte of the 30-character key `tG5MYmBZEZZnrH7yg2qTC4qj42Jcq9`, at a position that runs on through the whole file |
| Type tag | The byte that says how the game parses the value text: int, float, bool, list, curve and others |
| Path | The dotted name a key is hashed from, such as `tracks.Track_3.numberOfCorners` |
| Container | The part of a path that holds tables or settings: `tracks`, `entrants.teams`, `rules.stop_go_penalty` |
| Table | Numbered rows under one container: `entrants.teams.team_0` … `team_357`. Written `entrants.teams.team_N` in these docs |
| Row word | The name before a row's number: `team` in `team_5` |
| Row | One numbered entry of a table, with fields under it |
| Field | A value under a row: `entrants.teams.team_5.display_name_code` |
| Sub-object | A group of fields under a row without a number: `ai_vehicle_config.drafting` |
| Sub-table | A table nested under a row: `entrants.entrant_lists.entrant_list_34.entrant_N`, or a one-row table such as `Daylight_0` |
| Table family | All instances of a nested table, such as the `entrant_N` table of each of the 68 entrant lists |
| `_metadata_` | The field holding a row's label |
| Label | The name of a row (`bahrain`, `mclaren_26`). Rows in other tables refer to a row by its label |
| `_instancecount_` | The key `<container>._<Row>_instancecount_` holding a table's row count. The game reads rows 0 to count−1 |
| Stat id | `stat_id` or `statId`: a numeric id unique in its table |
| Dataset | A row of `track_data.game_setup.TrackDataSet_N` (`f1_2025`, `sprint_f1_2026` …) holding per-season track settings, with a parent to fall back to |
| Content set | A row of `content_sets.content_set_N` listing the tracks, grids and modes an edition offers |
| Entrant list | A grid: a row of `entrants.entrant_lists.entrant_list_N` with one `entrant_N` row per seat |
| Entrant setup mode | How a mode builds its grid from entrant lists (`max_entrants`, `max_teams`) |
| World variant | A row of `track_data.track_layers.TrackLayer_N` naming the world archive a mode loads on every track |
| Per-track table | A table with one row per track, labelled with the track id (`canned_events.Per-race DNFs.Per-race DNF_N`). Adding a track needs a row in each |
| Per-team, per-driver, per-vehicle table | A table whose rows are labelled with team, driver or car labels (kits, showroom, accolades). New teams and cars have no rows there |
| Per-season table | A table whose rows are labelled by season or dataset (`f1_2026`, `season_2026`) |
| Stack-built name | A name the exe writes to the stack in pieces, so it never appears whole in the exe's strings (`CareerHubFirstVi` + `sit`) |
| Placeholder `{hash}` | A path part whose name is unknown, written as its FNV-1a hash |
| Missing-key sentinel | The garbage pointer `0x08000f000800000e` a lookup of an absent key returns. Dereferencing it crashes the game |

## Table catalogue

`tools/gamedb/catalog.py` builds the catalogue from a game folder. It walks the database from the names `F1_25.exe` reads (8,286, stack-built ones included) plus `tools/gamedb/names.txt`, and writes:

| File | Contents |
|---|---|
| `tools/gamedb/catalog/tables.tsv` | One line per table family: path, row word, prefix hash, instances, rows, `_instancecount_`, field count, scope, label examples, meaning, evidence, source |
| `tools/gamedb/catalog/keys.tsv.gz` | One line per schema key (`Table_N.field`): type, rows holding it, whether the exe reads it, value range, unit, examples, meaning, evidence, source. Exe names with no match in the database are listed with `in_db = no` |
| `tools/gamedb/catalog/exe_names.tsv` | Every name the exe's database readers use, and whether the database has it |
| `tools/gamedb/catalog/summary.json` | The totals below |

Totals for the shipping database (game build `d873a1c8…`):

| | |
|---|---|
| Records | 742,857 |
| Records with a known name | 566,078 (76%) |
| Table families | 2,080 (823 with a full path) |
| Table instances / rows | 11,209 / 60,782 |
| Schema keys (`Table_N.field` form) | 16,502 |
| Schema keys whose dotted path a reader uses | 1,888 |
| Names the exe's database readers use | 8,286 |
| Exe names found in the database | 4,351 |
| Exe names not found among the named keys | 3,935 |
| Full key hashes held as 64-bit immediates in the exe | 1,098 (98 named) |
| Table meanings: proven / inferred / unknown | 39 / 1,512 / 529 |
| Key meanings: proven / inferred / unknown | 4,138 / 12,364 / 0 |

The 176,779 unnamed records sit under paths no known name reaches. The 3,935 unmatched exe names include format strings, prefixes, keys of other databases the same API reads (such as `.uidb` overrides) and keys that are absent from the shipping database.

A family groups the tables that share a path once row numbers are replaced by `N`: the 68 entrant lists each have an `entrant_N` table, and all 68 form `entrants.entrant_lists.entrant_list_N.entrant_N`.

Facts in the catalogue come from the data (rows, fields, types, counts, ranges) or the exe (names, key hashes). The `scope` column (per track, team, driver, vehicle or season) is inferred from row labels.

Every table and key has a `meaning`, an `evidence` tag and a `source`:

| Evidence | Meaning came from |
|---|---|
| `proven` | How the exe uses the key (a named reader in `re/db/functions.tsv`) or an in-game test |
| `inferred` | The research notes, the name, or the values (for example, values that are all labels of `entrants.teams.team_N`) |
| `unknown` | Nothing: the table's row word is not known |

Curated meanings live in `tools/gamedb/meanings.tsv` as glob patterns over table and key paths (`entrants.vehicles.vehicle_N.ai_vehicle_config.*`); the most specific pattern wins. Keys no pattern covers get a meaning from their name, unit suffix and values. Add a line to `meanings.tsv` and rerun the generator to record a new finding.

Regenerate it after a game update:

```sh
nix-shell -p 'python3.withPackages(ps: [ps.numpy ps.pefile ps.capstone])' --run \
  'python3 tools/gamedb/catalog.py --game "/path/to/F1 25" --docs docs/reference/gamedb'
```

A new `F1_25.exe` build needs its database API and reader addresses in `tools/track_keys.py` (`BUILDS`). `--db` and `--exe` take the two files directly; `--no-immediates` skips the scan for key hashes in the exe.

## Test

```sh
F1_25_DIR="/path/to/F1 25" cargo test -p swofty-formats
```

This reads and rewrites the real database when `F1_25_DIR` is set. An unchanged database writes back byte for byte.
