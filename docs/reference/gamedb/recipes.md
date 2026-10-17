# Modding recipes

These rules come from built and runtime-tested variants of the database (the 24 to 30 car work and the first track activations). Each one cost a crash or a silent no-op to learn.

## Editing a value

1. Find the key in the [catalogue](/reference/gamedb/catalogue) or `tools/gamedb/catalog/keys.tsv.gz`. Keep the type the stock key has.
2. Write the value in the stock form: `True`/`False` for bools, `HH,MM` for times, comma-joined lists and vectors with no spaces.
3. Express it as a [database operation](/modding/database-ops):

```json
{ "op": "set", "select": "tracks.Track[_metadata_=bahrain]", "field": "numberOfCorners", "value": "16" }
```

- `select` names a row by path (`tracks.Track_3`), by label (`tracks.Track[_metadata_=bahrain]`) or by hash placeholder (`{446668bc25ba571c}[_metadata_=f1_26]`). Use labels; row numbers can change between game builds.
- `value_from` copies a value from another key: `"value_from": "{446668bc25ba571c}[_metadata_=myteam_26].worldEri"`.
- `set` on a key that does not exist stops the build with `no such key`. `"create": true` with `"type": <tag>` adds it.
- Strings in an operation can hold <span v-pre>`{{option}}`</span> placeholders and a `when` condition, filled from the mod's [settings](/modding/manifest#options) at build time.

The launcher applies database changes in this order: map activation (one per enabled map), then each mod's operations in load order. A later write to the same key wins. It then writes the file sorted by `key_hi`.

## Adding a row

A new row must have **every key** a stock row of the same table has, including keys of its sub-objects and nested tables. The game reads them and [crashes on a missing one](/reference/gamedb/reading#missing-keys).

| Table | Keys per row |
|---|---|
| `entrants.vehicles.vehicle_N` | 246: 28 top-level and 218 under `ai_vehicle_config` (202 read by `Sess_Parse_AiHandlingConfig`) |
| `entrants.teams.team_N` | 30: 26 top-level, `_story_info_instancecount_` and `story_info_0.{_metadata_,description,hashtag}` |
| `entrants.entrant_lists.entrant_list_N.entrant_N` | 10 |
| `tracks.Track_N` | 268 schema keys over the row and its sub-tables (`Daylight`, `CareerHubFirstVisit`, `CareerHubLastVisit`, `TrackCrowds`, `TrackHighlights`, `TrackStats`, `TrackTextures`, `TrackVideo`, `Tyres`, `Weather`) |

Then:

1. Copy every key of a stock row, with its type, to the new row number (the current count). The [key schema](/reference/gamedb/schema) and `keys.tsv.gz` list the keys; `tools/track_keys.py keys` lists a track's.
2. Give the row a new `_metadata_` label, unique in the table.
3. Give it a new `stat_id` (teams, vehicles) or `statId` (tracks) above the stock maximum. Stock maxima: team 499, vehicle 508.
4. Raise the table's `_instancecount_` by one.

Both halves are required:

- A row at or above the count is invisible: no enumeration reads it and no label lookup finds it. An entrant whose `team` names such a row crashes the entrant parser.
- A row inside the count is read at startup. Teams and vehicles inside the count join the availability passes and the vehicle AI loop.

New labels have no rows in side tables keyed by team or vehicle label (kits, showroom, stings, academies, career ratings, DLC unlocks, special liveries, equal-performance entrants). Cars and teams still load without them; no reader of those tables has been audited.

`add-row` copies the listed fields of a source row and raises the count:

```json
{ "op": "add-row", "table": "entrants.teams", "from": "entrants.teams.team[_metadata_=williams_26]",
  "set": { "_metadata_": "team12_26", "stat_id": "500" } }
```

It needs the table's real path (`entrants.teams`, not a `{hash}`) and the row's field list in the file's [`fields` map](/modding/database-ops#operations). Every listed field must exist on the source row. `addons/custom-grid-size/db/grid.json` lists every field of team, vehicle and entrant rows.

## Grid size

How a 2026 Grand Prix builds its grid, all of it data:

1. The GP game mode row names game setup mode `f1_26`, which names entrant setup mode `f1_26`.
2. Three `EntrantSetupModeOverride` rows replace `f1_26` with `f1_custom_team_2026`, row 10 of `entrants.entrant_setup_modes.entrant_setup_mode_N`.
3. Row 10's list is `entrant_list_66` (`f1_custom_team_2026`): 2 custom-team seats plus `entrant_list_references` = `f1_2026`. The parser reads the referenced list first, so the pool is list 34's entrants plus 2.
4. The selector takes `min(max_entrants, pool)` entrants (`reuse_entrants` False), then drops teams while there are more than `max_teams` (-1 means no limit).
5. With no player in the custom team, `remove_team_without_players` removes its 2 seats.

Stock values: list 34 (`f1_2026`) has 22 entrants; row 10 has `max_entrants` 24; rows 8 (`f1_26`) and 9 (`f1_generic_2026`) have 22. That gives 22 cars, or 24 with a player-made team.

For N cars:

| Key | Value |
|---|---|
| `entrants.entrant_lists.entrant_list_34._entrant_instancecount_` | N, with `entrant_22 …` rows of 10 keys each |
| `max_entrants` on `f1_26` and `f1_generic_2026` | N |
| `max_entrants` on `f1_custom_team_2026` (row 10) | N + 2 |
| `entrants.teams._team_instancecount_`, `entrants.vehicles._vehicle_instancecount_` | stock + new teams, with complete rows (above) |
| World variant `{446668bc25ba571c}[_metadata_=f1_26]` `worldEri`, `worldAssetGroup` | the `myteam_26` values, from 24 cars |

- Row 10 must be N + 2. The custom seats are drawn first and removed afterwards; N alone fields N − 2.
- The world switch is needed from 23 cars: the `_f1_26_myteam` world has 24 grid slots and 12 garages. More than 24 cars needs extra grid slots, garages and pit boxes in each track's world archive, which is outside the database. The Custom Grid Size mod adds them with [`patch-pkg`](/modding/erp-ops#editing-pkg-resources) operations.
- More than 29 AI cars overruns the game's 30-entry AI vehicle name table (`Sess_Build_AiSystem`). Nothing extends it. The Custom Grid Size mod stops at 30 cars, 29 of them AI when the player drives for a real team.
- Still sized for 22: `f1_2026_equal_performance` (list 35) and the per-team weighted-random grid table. Use realistic performance and a random grid.
- Garages follow `garage_assignment` `PreviousSeasonTeamStandings` from `f1_teams_2025`. Teams without a standing are appended.

## Adding a track

The launcher does this when a map is enabled (`packages/core/src/activation/`). Its table list is `packages/core/src/activation/track_tables.json`. For a new track id it:

1. Clones the slot track's `tracks.Track_N` row with its full schema and sub-tables, sets a new `statId` (max + 1), and raises `tracks._Track_instancecount_` (43 in the shipping file).
2. Adds a track resource row `swofty_<code>` to `tracks.track_resource.TrackResource_N` and names it in the track row's `track_resource`.
3. Clones `TrackData_N` rows in every dataset where the base track has one (19 of 20), raising each dataset's `_TrackData_instancecount_`.
4. Adds a row, cloned from the slot track (or the base track), to every per-track table:

| Table | Rows | Missing row |
|---|---|---|
| `canned_events.Per-race DNFs.Per-race DNF_N` | 44 | crash |
| `pit_lane.spline_generation.track_spline_generation_N` | 45 | checked |
| `pit_lane.track_slide_settings_N` | 43 | checked |
| `practice_programmes_{25,26}.{fuel_management,ers_management,quali_pace,race_pace_evaluation}.per_track_settings` | 39–40 | checked |
| practice tyre wear (2 tables) | 39 | reader not found |
| specialist `track_data_N` (2 tables) | 43 | career and F1 World |
| `replay.highlights.HeroShots.TrackName_N` (with `CameraShot_N`) | 38 | optional |
| tyre wear scales `track_tyre_wear_scale_N` (3 tables) | 31–44 | not traced |
| track trivia `TrackTriviaTrack_N` | 32 | not traced |

5. Adds the code to every `content_sets.content_set_N.available_tracks_N.tracks` list that holds the base track, and to both `track_data.sorted_track_order.sorted_track_order_N.TrackRef_N` orders.
6. Applies the map's overrides to its `TrackData` rows and track overrides to its `Track` row.

Tables keyed by track that are calendars, challenge rounds, F1 World activities, map markers and trophies are not cloned.

## Checking a build

Build without starting the game, then check the result against the pristine game:

```sh
swofty build --profile <name>
python3 tools/map_check.py <overlay folder> --game <pristine game folder>
```

`map_check.py` checks that every key of each source row exists on the new rows, using the names `F1_25.exe` reads. Two more checks:

```sh
# every key the game's track readers use on one track
python3 tools/track_keys.py keys F1_25.exe gamedatabases.2025.bin bahrain
# the missing-key sentinel for a built file
python3 tools/gdb_hashsim.py <overlay>/game_package/gamedatabases/gamedatabases.2025.bin
```

`track_keys.py` and the reader-key part of `map_check.py` need `numpy`, `pefile` and `capstone`, and an exe build listed in `track_keys.BUILDS`.

For a hand-made database, also check:

- the file round-trips (`python3 tools/gamedb.py roundtrip <file>`);
- every new row has every key of its source row, with the same types;
- every label an entrant, track or dataset references resolves inside its table's count;
- the counts read back as intended (`python3 tools/gamedb.py get <file> <path>`).
