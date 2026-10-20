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

Writes `db/grid.json` (database ops) and `lang/grid.json` (`lang.set` strings) from the season pack. Run it after changing the pack or the tables in it; `-v` prints the driver ids and ratings.

```sh
python3 addons/f1-1990/tools/grid.py -v
```

What it writes:

| Part | Ops |
|---|---|
| 13 teams | `set` on each slot's team row: names (`lng_f1990_team_<team>`, `_short`), code, the seven colours, base, nationality, and a 1990 description and hashtag on the 11 rows that carry `story_info`. `set` on its vehicle row: `display_name`, `chassis_name`, `engine_name_override`. Team principal: the boss's `entrants.characters` row gets `lng_f1990_boss_<team>_full/_first/_last`; Larrousse and Dallara get their own boss rows (`andreas_seidl`, `franz_tost`) instead of sharing Williams' and Haas'. `lang.set` on the stock `lng_<team>_boss` keys, which the career's email senders use |
| 28 drivers | Senna, Prost and Mansell keep their stock rows (`set` car number, seasons, AI traits). 25 new drivers (the other 23 season regulars, plus the stand-ins Foitek, Morbidelli, Schneider, Moreno and Herbert) get a full `entrants.drivers` row copied from `jarno_opmeer`, as Driver Manager builds them |
| Looks | One `ai_driver_customisations` row per driver (`f1990_<driver>_look`): the team's 2026 suit and gloves, a stock helmet (the classic helmets for Senna, Prost and Mansell) |
| Ratings | No new `driver_statistics` holders: a race start aborts once the table holds more than 128 (stock 117). Each new driver takes a spare stock holder (`SPARE_HOLDERS`: drivers no 2026 list uses) by `set` on its `driver` field and curves; Senna, Prost and Mansell get `set` on their own holders (`aryton_senna`, `alain_prost`, `nigel_mansell`) |
| Entrants | One `entrants.entrant_lists.entrants` row per driver (`f1990-<driver>`): driver, look, car number |
| Grid | The 26 rows of `f1_2026` (22 stock, 4 from Custom Grid Size at 26 cars) point at the season regulars (each seat's driver with the most starts), with 1990 numbers and `team_position` 0/1 |
| Free agents | `f1_2026_driver_pool`: Senna's row becomes Foitek; Morbidelli, Schneider, Moreno and Herbert are added |
| Standings | `f1_teams_2025` lists the stock team rows in 1989 constructors' order; `f1_drivers_2025` lists 22 of the drivers in 1989 order |

### Driver ids

| Ids | Use |
|---|---|
| 18, 37, 60, 61, 104-108, 119, 120, 124 | The 12 driver ids the game leaves free below 208 |
| 253 down to 238 | The rest, from the top of Driver Manager's block (208-253) |
| 1990-2017 | Look (`driver_customisation`) stat ids |

Driver Manager allocates upward from 208 and checks only the stock database, so it reaches 238 after 30 drivers.

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

## check_grid.py

Checks a built database against `db/grid.json`: the 26 `f1_2026` seats resolve to a team, a car, an entrant, a driver and a look; car numbers, driver stat ids, look stat ids and driver labels are unique; no free agent is also seated; the statistics table holds at most 128 holders and each seated driver exactly one; every added row has every listed field its source row has, and no more.

```sh
python3 addons/f1-1990/tools/check_grid.py <overlay>/game_package/gamedatabases/gamedatabases.2025.bin
```

`swofty build` reads the database in the game folder. While a test build is installed there, build with `--game` pointing at a folder holding a copy of `F1_25.exe` and the stock `game_package/gamedatabases/gamedatabases.2025.bin`, and a copy of Custom Grid Size without its `erp` ops.

Put the database on the build route (`"build_assets": ["db"]` in `$DATA/config.json`, or build without the in-game loader) to get the file.

## build_cars.py

Writes `erp/cars/vtf{,_ers_zeroed}{,_13in,_13in_raised}/` (fragment 0 = BXML, 1 = TSMD stats of each 2026 team handling file), `erp/cars/handling.json` (`replace-resource` ops for every archive that carries each file, keyed by the `ers` and `tyre_size` options) and `db/physics.json`. Reads the car slots and scalars through `season.py`, and the stock VTFs and game database from `--game`. Without `--write` it builds into `build/` and exits 1 when that differs from the shipped files. Tuning constants: `BASE_HP`, `BASE_MASS`, `SPREAD` (scales every scalar's distance from 1), `FAMILIES` (rev limit, peak-power rpm, torque peak per engine), `ENGINES`, `AUDIO`, `TYRES_13`, `BEAD_13`, `ALLOY_13`.
