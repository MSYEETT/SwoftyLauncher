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

Every op on Custom Grid Size's rows (`team12_*`, `team13_*`, their seats and the two boss rows `andreas_seidl`, `franz_tost`) carries `"when": {"cars": 26}`. At the default `cars` = 22 the grid is the 11 teams on the stock 2026 team rows (no Larrousse, no Dallara) and the addon builds without Custom Grid Size; at 26 Custom Grid Size must be enabled with Cars at 26. `build_cars.py` gates its five ops on those rows the same way.

| Part | Ops |
|---|---|
| 13 teams | `set` on each slot's team row: names (`lng_f1990_team_<team>`, `_short`), code, the seven colours, base, nationality, and a 1990 description and hashtag on the 11 rows that carry `story_info`. `set` on its vehicle row: `display_name`, `chassis_name`, `engine_name_override`. Team principal: the boss's `entrants.characters` row gets `lng_f1990_boss_<team>_full/_first/_last`; Larrousse and Dallara get their own boss rows (`andreas_seidl`, `franz_tost`) instead of sharing Williams' and Haas'. `lang.set` on the stock `lng_<team>_boss` keys, which the career's email senders use |
| 31 drivers | No new driver, look or statistics rows (see [Why rows are reused](#why-rows-are-reused)). Senna, Prost and Mansell keep their stock rows (`set` car number, seasons, AI traits). The other 28 (23 season regulars plus the stand-ins Foitek, Morbidelli, Schneider, Moreno and Herbert) each take a stock driver row no list in the 1990 profile uses, by `set` on its names, code, number, nationality, head, birth date, seasons, voice and AI traits: a 2026 F1 driver of the same team row where one is free (`PACKAGES`), otherwise one of six old F2 drivers (`OTHER_ROWS`) |
| Looks | A reused 2026 F1 driver keeps his `2026-<team>_<driver>` look; Senna, Prost, Mansell and the `OTHER_ROWS` drivers take a 2026 F2 look (`SPARE_LOOKS`). Each look gets `set` on helmet, suit, gloves (the team's 2026 kit) and `linked_driver_stat_id` |
| Ratings | `set` on the curves of the reused driver's own `driver_statistics` holder, in every data set (2 for the 2026 F1 drivers) |
| Entrants | A reused 2026 F1 driver keeps his `f1_2026-<driver>` entrant (`set` car number and look); the other 9 drivers get a new `entrants.entrant_lists.entrants` row (`f1990-<driver>`) |
| Grid | The 26 rows of `f1_2026` (22 stock, 4 from Custom Grid Size at 26 cars) point at the season regulars (each seat's driver with the most starts), with 1990 numbers and `team_position` 0/1 |
| Free agents | `f1_2026_driver_pool`: Senna's row becomes Foitek; Morbidelli, Schneider, Moreno and Herbert are added |
| Standings | `f1_teams_2025` lists the stock team rows in 1989 constructors' order; `f1_drivers_2025` lists 22 of the drivers in 1989 order |

### Why rows are reused

A 26-car race start aborts (heap error 0x16 in `Ne_RobinHoodU8_IncreaseSize` from `Sess_Veh_UpdateWheelMap`) when the profile adds too much to the database, independent of which drivers race:

- with only the new driver rows and 11 new statistics holders (128 holders) the race starts; with 12 (129 holders) it aborts;
- with the holders kept at 117 but the new driver, look and entrant rows added, it still aborts.

So the grid adds no driver, look or statistics rows and needs no new stat ids. Results: `/mnt/work/.f1-1990/addon/results/10-grid-race-crash.md`.

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
