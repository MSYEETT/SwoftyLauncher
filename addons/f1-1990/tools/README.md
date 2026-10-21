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

Not in the v2 manifest: it replaces stock 2026 textures. The v2 plan (new contexts and textures for the 1990 rows) is in `/mnt/work/.f1-1990/v2/teams-career.md` §6.

Writes `erp/frontend.json` and its sources under `erp/frontend/` (about 3 MB) from the season pack, `grid.py`'s row choices and the stock archives in `--game`. Needs Pillow and lz4 (`nix-shell -p "python3.withPackages (ps: [ ps.pillow ps.lz4 ])"`), `fc-match` and rnd-tyres-thermal's `bc7enc.py` (mode-6 encoder, white on alpha). `JOBS` sets the encoder processes (default 8).

```sh
python3 addons/f1-1990/tools/frontend.py --game <game folder> [--preview <dir>]
```

| Part | Ops |
|---|---|
| Driver Select names | The backdrop is a texture on `mfe_2025_main_menu_driver_select_name.idf` in `environment_package/second_world/main_front_end/wep/main_front_end.erp`, picked by material context = the driver row label (`...driver_select_name.emb?context=<label>` → `mfe_2025_driver_select_<name>_a.material` → `f1_drivername_<name>.tif`, BC7 2048x512). A missing context keeps the previous name. Each of the 31 rows the grid uses gets the 1990 driver's name (first name filled, surname outlined, right-aligned, DejaVu Sans Bold stretched 1.25): the 22 rows with a context reuse their own texture; `ayrton_senna`, `alain_prost`, `nigel_mansell` and the six `OTHER_ROWS` get a new context (`add-resource`) copied from an unused one (Zenith, Konnersport, Doohan, Tsunoda, F2); `lewis_hamilton` moves to an unused F2 slot of the same size, and its texture, which `default` also uses, is left blank |
| Team Select names | The same with `mfe_2025_main_menu_vehicle_select_name.emb?context=<team row>` and `f1_teamname_<team>.tif` (2048x256, outlined): the 13 slots' rows show the team's short name; `team12_26` and `team13_26` get contexts copied from F2 teams |
| Textures | `replace-resource` fragment 1 (the mips in the archive, same size) and fragment 2 (the mip-file descriptor, same length, pointing at `names/f1990_drv_names~...` or `names/f1990_tmnames~...`); `add-mipmaps` installs the LZ4 mip files |
| Tabs | `persistent.erp: screen_groups.uicontentgroup` without `header_f2` and `header_icon` in the `driver_career_driver_select_header` group (the Driver Career Choose Driver screen), behind `hide_f2_icon_tabs` (default on). The tool checks the stock resource round-trips byte-exact through `tools/pkg.py` and that the edit differs only by the two entries. The resource shrinks, so the build writes `persistent.erp` into the game files on both routes |

## badges.py

Not in the v2 manifest: it replaces stock 2026 slices. v2 appends slices for the 1990 rows instead (teams-career.md §6).

Writes `erp/badges.json` and two archives under `erp/badges/` (`.erpk`; the repo ignores `*.erp`) with placeholder 1990 team badges: the short name (the three-letter code at 64 px and below) in DejaVu Sans Condensed Bold on the team's primary colour, a secondary-colour stripe along the bottom.

```sh
python3 addons/f1-1990/tools/badges.py --game <game folder> [--baseline <launcher baseline>/<build>] [--sheet badges.png]
```

Each team badge list is a name-keyed list in `databases.erp: overrides.uidb` whose entry at index i names slice i of a single-mip BC7 texture array (`ui_package/art/texture_arrays/overrides.uidb/overrides.<list>.texarray` and `.texarray.image`). The tool checks every UI archive that carries an array holds the same stock array, encodes the badge for each slice it replaces (rnd-tyres-thermal's `bc7enc.encode_rgba`) and writes the arrays into `erp/badges/badges.erpk`; `put-resource` copies them into every carrier. `--baseline` reads stock archives from the launcher's baseline store when the game folder holds installed ones.

| List | Key | Size |
|---|---|---|
| `team_logos.f1_2026`, `team_logos.driver_career_2024_team_select`, `teams.mp_team_select_logos`, `teams.badges_broadcast`, `broadcast_ui_generic.grid_team_logos`, `broadcast_ui_generic.team`, `teams.fanzone_expressions`, `teams.fanzone_expressions_large`, `teams.fanzone_room`, `teams.teams_f1_2024_gp_voting` | team row | 272x156, 148x148, 700x640, 24x24, 180x180, 220x132, 28x28, 180x180, 560x484, 560x296 |
| `teams.driver_select_team_logos`, `teams.career_team_select_logo_small` | `livery_name`, `team_select_<livery_name>` | 64x64, 268x136 |
| `osd.timing_tower.team_badges_hud`, `_broadcast` | `timing_tower_texture_path` | 24x24 |

At 26 cars the arrays keyed by team row, and the timing tower ones, grow by Larrousse and Dallara (`erp/badges/badges26.erpk`, `put-resource` with `"when": {"cars": 26}`), and a `patch-pkg` appends `team12_26`/`team13_26` (and `larrousse_90`/`dallara_90`, which `grid.py` sets as those rows' `timing_tower_texture_path`) to the lists. A grown array changes its archive's layout, so at 26 cars the build writes those archives into the game files.

## check_grid.py

```sh
python3 addons/f1-1990/tools/check_grid.py <stock db> <db built without grid.json> <db built with it> --cars 22|26 [--lists <driver-manager cache>/game-lists.json]
```

Fails when a grid op changes a stock record other than a row count, or when a seat, entrant, driver, holder, look, list, setup mode, standings or data-group team row is missing or doubled. Lists the `<team>_1990` vehicle rows the build lacks (cars lane).

## build_cars.py

Writes `cars/handling/<ers>_<tyre_size>/<folder>_handling.erp` (one archive per 1990 car and option pair, holding its handling file under `eaid://f1_1990_vehicle_package/teams/<stem>/handling/<stem>.vtf`, and also under the folder name when the stem differs), `cars/handling.json` (the `file.put` ops keyed by `ers` and `tyre_size`) and `db/cars.json` (the `f1_1990` package row and the 13 vehicle rows). Reads the car table from `cars.py`, the team scalars through `season.py`, the vehicle field list from Custom Grid Size's `db/grid.json`, and the stock 2026 handling files from `--game`. Without `--write` it builds into `build/` and exits 1 when that differs from the shipped files. Tuning constants: `BASE_HP`, `BASE_MASS`, `SPREAD`, `FAMILIES`, `TYRES_13`, `BEAD_13`, `ALLOY_13`.

## cars.py

The 1990 car table: team, vehicle row (`<token>_1990`, the grid lane's tokens), performance tier, car folder (asset_filename and AI data), donor 2026 folder, resource stem, stat_id, chassis, engine. Shared by `build_cars.py` and the car pack's `build_pack.py`.
