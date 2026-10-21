# Tools

## season.py

Reads the 1990 season pack (`season.json`, `drivers.json`) and the addon's team slots. Python 3.10+, standard library only. The pack folder is `/mnt/work/.f1-1990/source/1990`, or `$F1_1990_SOURCE`.

```sh
python3 addons/f1-1990/tools/season.py            # prints the slots and the round 1 line-up
```

```python
import sys; sys.path.insert(0, "addons/f1-1990/tools")
from season import Season, slots, slot, grid_lineup

s = Season.load()                  # or Season.load("/path/to/1990")
s.year, s.points, s.best_results   # 1990, {1: 9, ..., 6: 1}, 11
s.races                            # [Race(round, name, short_name, date, circuit)], by round
s.race(7)                          # Race 7, French Grand Prix
s.teams["mclaren"]                 # Team(id, name, principal, reputation, colour, prequalifying, car,
                                   #      power, weight, drag, reliability, seats, malus)
s.drivers["ayrton_senna"]          # Driver(id, name, first, last, nationality, country, birth_year,
                                   #        reputation, ratings)
s.absences                         # [Absence(round, team, out, into)], chained absences flattened
s.lineup(round)                    # {team id: [Seat(driver, number, races), Seat(...)]} with that round's absences applied
s.regulars([team ids])             # {team id: [Seat, Seat]}: each seat's driver with the most starts (races = starts)
s.stand_ins([team ids])            # [(driver, team id)]: drivers who started in those teams but aren't regulars
s.substitutions([team ids])        # absences for those teams
s.normalised(driver, rating)       # rating scaled to 0..1 over the pack's 44 drivers

slots()                            # the 13 teams in the addon: [Slot(team, team_row, vehicle, asset)]
slot("benetton"), slot("renault_26")
grid_lineup()                      # [(Slot, [Seat, Seat])] for the 13 teams: season regulars
grid_lineup(round=1)               # the same with one round's line-up
```

`power`, `weight` and `drag` are the pack's `power_scalar`, `weight_scalar` and `drag_scalar` with the sign flipped (Ferrari: power 1.08, weight 0.92). `reliability` is `-vehicle_reliability` (Ferrari 0.477). `malus` holds the raw values.

`nationality` is the game's nationality label (`britain`, `northern_ireland` for Donnelly); `country` is the pack's ISO code.

### Team slots

Each 1990 team takes one 2026 team row. `asset` is the car folder the vehicle row loads (`f1_2026_vehicle_package/teams/<asset>/`); `team12_2026` and `team13_2026` are Custom Grid Size's copies of the Williams and Haas rows, so they share those folders.

| 1990 team | Team row | Vehicle | Car folder |
|---|---|---|---|
| McLaren | `mclaren_26` | `mclaren_2026` | `mclaren` |
| Ferrari | `ferrari_26` | `ferrari_2026` | `ferrari` |
| Williams | `williams_26` | `williams_2026` | `williams` |
| Benetton | `renault_26` | `lotus_2026` | `lotus` |
| Tyrrell | `mercedes_26` | `mercedes_2026` | `mercedes` |
| Lotus | `redbull_26` | `redbull_2026` | `redbull` |
| Leyton House (`march`) | `forceindia_26` | `force_india_2026` | `force_india` |
| Larrousse | `team12_26` | `team12_2026` | `williams` |
| Ligier (`ligier_prost`) | `sauber_26` | `sauber_2026` | `sauber` |
| Minardi | `tororosso_26` | `toro_rosso_2026` | `toro_rosso` |
| Brabham | `gm_cadillac_26` | `gm_cadillac_2026` | `gm_cadillac` |
| Dallara | `team13_26` | `team13_2026` | `haas` |
| Arrows | `haas_26` | `haas_2026` | `haas` |

## grid.py

Writes `db/grid.json`, `db/grid_standings.json` (the driver standings) and `lang/grid.json` from the season pack and the stock database. Every op adds a row or sets a field of an added row; no stock row changes, only the row counts of the tables it adds to.

```sh
python3 addons/f1-1990/tools/grid.py <stock gamedatabases.2025.bin> -v
```

| Part | Rows |
|---|---|
| 13 teams | `<team>_1990` (`mclaren`, `ferrari`, `williams`, `benetton`, `tyrrell`, `lotus`, `leytonhouse`, `larrousse`, `ligier`, `minardi`, `brabham`, `dallara`, `arrows`), stat_id 502–514: every field of the 2026 donor (Larrousse from Williams, Dallara from Haas) plus names, code, colours, base, nationality, story text, `livery_name` and `timing_tower_texture_path` = the row label, and a boss row `f1990_boss_<team>` |
| 31 drivers | `f1990_<driver>`, stat ids from the free low block and 235–253 (Driver Manager keeps 208–234): every field of a donor row (Senna, Prost and Mansell from their stock rows, the rest from 2026 and old F2 drivers) plus names, code, number, nationality, head, birth date, seasons, voice and AI traits; a statistics holder (one `default` data set), a look `f1990-<team>_<driver>` in the official folder (era-neutral suit), and an entrant `f1990-<driver>` |
| Lists | `f1_1990` (the grid: 22 seats, 26 with `cars` = 26, `team`/`vehicle` = `<team>_1990`), `f1_1990_driver_pool` (the five stand-ins), `f1_career_1990` (references both); entrant setup mode `f1_1990` (`max_entrants` = `cars`); standings `f1_teams_1989` and `f1_drivers_1989` |
| Kits | `2026-f1990_<team>` in the 2026 team-customisation year, `linked_team` the 1990 row, the team's looks |
| Career data group | In the `f1_career_1990` rows the career lane clones: the team rows of accolades, primary sponsors, recognition and resource points move from `<team>_26` to `<team>_1990`, plus Larrousse and Dallara |

Field lists: the gamedb catalog plus every known field name present on the row (names.txt, the catalog, Driver Manager's driver fields). The catalog alone has 29 of a driver row's 129 fields.

`check_grid.py` checks a build against stock and a build without the grid ops.

### Ratings

The pack's 14 ratings are scaled to 0..1 across its 44 drivers (`Season.normalised`), then:

| F1 25 | From |
|---|---|
| experience | 0.62 + 0.33 × min(seasons before 1990 / 10, 1) |
| racecraft | 0.70 + 0.27 × mean(race_skill, defending, aggression) |
| awareness | 0.70 + 0.27 × mean(avoidance_of_mistakes, avoidance_of_forced_mistakes, blue_flag_conceding) |
| pace | 0.70 + 0.27 × mean(qualifying_skill, race_skill) |
| focus | 0.70 + 0.27 × mean(stamina, tyre_management, fuel_management, weather_tyre_changes) |
| trait aggression | 0.55 + 0.45 × aggression |
| trait control | 0.55 + 0.45 × mean(race_skill, wet_skill) |
| trait mistakes | 0.55 + 0.45 × mean(avoidance_of_mistakes, avoidance_of_forced_mistakes) |
| trait reaction_speed | 0.55 + 0.45 × start_reactions |
| trait speed_in | 0.55 + 0.45 × qualifying_skill |
| trait speed_out | 0.55 + 0.45 × mean(qualifying_skill, race_skill) |

Each rating curve is flat at its value. `consistency` is not used.

## frontend.py

Writes `erp/frontend.json` and `erp/frontend/`: the Driver Select and Team Select backdrop names for the 1990 rows, added to `main_front_end.erp` as new resources. The backdrop is a texture picked by material context, the driver or team row label (`mfe_2025_main_menu_driver_select_name.emb?context=<label>`, `…vehicle_select_name.emb?context=<label>`). Per row: a texture `f1_drivername_f1990_d_NNN` / `f1_teamname_f1990_NN` (BC7, the template's size and mip layout, loose top mips under `names/f1990_drv_names~…` / `names/f1990_tmnames~…`), its SRV, a material and the context, each a copy of a stock one (`sonny_hayes`, `mercedes`) with the name swapped for one of the same length. Needs Pillow and lz4 (`nix-shell -p "python3.withPackages (ps: [ ps.pillow ps.lz4 ps.numpy ])"`) and rnd-tyres-thermal's `bc7enc.py`.

```sh
python3 addons/f1-1990/tools/frontend.py --game <game folder> [--baseline <launcher baseline>/<build>] [--preview <dir>]
```

## badges.py

Writes `erp/badges.json` and `erp/badges/badges.erpk` (the repo ignores `*.erp`): placeholder 1990 team badges appended to the 14 team-logo texture arrays (the short name, or the three-letter code at 64 px and below, on the team's primary colour with a secondary stripe), with one `overrides.uidb` entry per new slice. Keys: the team row `<team>_1990` (also its `livery_name` and `timing_tower_texture_path`), `team_select_<team>_1990` in the career small logos. Stock slices stay as they are; every carrier archive grows, so the build writes them into the game files.

```sh
python3 addons/f1-1990/tools/badges.py --game <game folder> [--baseline <launcher baseline>/<build>] [--sheet badges.png]
```

## tracks.py

Writes `erp/tracks.json` and `erp/tracks/`: a 1990 world `<track>_f1_1990.wep` in each track's `<track>_f1_26_myteam.erp`, which `grid.py`'s track layer `f1_1990` loads with the stock My Team asset group. The world is the My Team world listing its own copies of the layers Custom Grid Size widens (`…_f1990.layer`); with `cars` at 26 the copies get Custom Grid Size's 26-car edits. The stock worlds and layers stay as they are.

```sh
python3 addons/f1-1990/tools/tracks.py --game <game folder> [--baseline <launcher baseline>/<build>]
```

## check_grid.py

```sh
python3 addons/f1-1990/tools/check_grid.py <stock db> <db built without grid.json> <db built with it> --cars 22|26 [--lists <driver-manager cache>/game-lists.json]
```

Fails when a grid op changes a stock record other than a row count, or when a seat, entrant, driver, holder, look, list, setup mode, standings or data-group team row is missing or doubled. Lists the `<team>_1990` vehicle rows the build lacks (cars lane).

## build_cars.py

Writes `cars/handling/<ers>_<tyre_size>/<folder>_handling.erp` (one archive per 1990 car and option pair, holding its handling file under `eaid://f1_1990_vehicle_package/teams/<stem>/handling/<stem>.vtf`, and also under the folder name when the stem differs), `cars/handling.json` (the `file.put` ops keyed by `ers` and `tyre_size`) and `db/cars.json` (the `f1_1990` package row and the 13 vehicle rows). Reads the car table from `cars.py`, the team scalars through `season.py`, the vehicle field list from Custom Grid Size's `db/grid.json`, and the stock 2026 handling files from `--game`. Without `--write` it builds into `build/` and exits 1 when that differs from the shipped files. Tuning constants: `BASE_HP`, `BASE_MASS`, `SPREAD`, `FAMILIES`, `TYRES_13`, `BEAD_13`, `ALLOY_13`.

## cars.py

The 1990 car table: team, vehicle row (`<token>_1990`, the grid lane's tokens), performance tier, car folder (asset_filename and AI data), donor 2026 folder, resource stem, stat_id, chassis, engine. Shared by `build_cars.py` and the car pack's `build_pack.py`.

## build_audio.py, audio.py

`build_audio.py --game <F1 25> [--write]` writes `db/audio.json`: four `audio.audio_bundle_settings` rows for the 1990 bundles and one `audio.audio_car_data.<label>` row per 1990 car, every field copied from its donor's stock row except `_metadata_` and `bundle_name`. It reads the donor labels from the stock 2026 carmodel layers and fails if any key it writes is already in the stock database. Without `--write` it builds into `build/` and exits 1 when that differs from the shipped file. `audio.py` holds the label rule (`label_1990`), the `switchName` reader and rewriter the car pack's `build_pack.py` uses, and the engine-to-bundle table.

## rnd.py

Writes `db/rnd.json`, `db/rnd-lang.json`, `data/rnd-strings.json` and `data/rnd-art.json` from the stock database, the
season pack and the tables at the top of the file (`TOKENS`, `UPGRADES`, `SIZES`, `SUPPLIER_BHP`, `BUDGET`). `--check`
exits 1 when a shipped file differs from a fresh build.

```sh
python3 addons/f1-1990/tools/rnd.py <stock gamedatabases.2025.bin> [--check]
```

## check_rnd.py

```sh
python3 addons/f1-1990/tools/check_rnd.py <stock db> <db built without rnd.json> <db built with it>
```

Fails when `rnd.json` changes a stock key other than the shared component row count, when a stock R&D key of the
2026, 2022 and My Team prefixes, the stock shared component sets or the 2025/2026 resource-point groups differs from
stock in the full build, or when a 1990 tree, its vehicle list or its B-spec upgrade is missing.
