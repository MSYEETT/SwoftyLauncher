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
| Performance tier | `performance_tier` on each slot's vehicle row from the pack's team reputation: TOP_TEAM and MIDFIELD_HIGH 0 (McLaren, Ferrari, Williams, Benetton), MIDFIELD 1, MINNOW and SUPER_MINNOW 2 (Ligier, Larrousse) |
| Heads and helmets | Every head and helmet is one the game ships an asset for: the `alain_prost`, `nigel_mansell`, `pedro_piquet` and `giuliano_alesi` heads and the `classic-prost01`/`classic-mansell01` helmets have database rows but no assets (the driver renders headless), so Prost, Mansell, Piquet and Alesi get generic heads and Prost and Mansell stock helmets |
| Looks | A reused 2026 F1 driver keeps his `2026-<team>_<driver>` look; Senna, Prost, Mansell and the `OTHER_ROWS` drivers take a 2026 F2 look (`SPARE_LOOKS`). Each look gets `set` on helmet, suit and gloves (`SUIT`, the plain customisable `tier01_livery76`, so no 2026 sponsors) and `linked_driver_stat_id` |
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

### Front-end art still on 2026

`frontend.py` (backdrop names, tabs) and `badges.py` (team badges) cover the rest of the front end; these remain.

| What | Where it comes from |
|---|---|
| Driver picker tiles with no 1990 badge | `teams.driver_select_team_logos` and `teams.career_team_select_logo_small` are keyed by the team row's `livery_name`: Brabham's row (`cadillac`) has no entry, and at 26 cars Larrousse and Dallara share Williams' and Arrows' keys. `livery_name` also picks the car livery |
| Suit colours | Generic suits are colour-customisable (`nb_colours` 3 or 4); their diffuse is composed at run time (`charname_livery_v2_d`), so the colour an AI driver gets isn't in the data. 1990 kits need new `character_package/team_livery/1990-<team>` packages |

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

Checks a built database against `db/grid.json`: the 26 `f1_2026` seats resolve to a team, a car, an entrant, a driver and a look; car numbers, driver stat ids, look stat ids and driver labels are unique; no free agent is also seated; the statistics table holds at most 128 holders and each seated driver exactly one; every added row has every listed field its source row has, and no more.

```sh
python3 addons/f1-1990/tools/check_grid.py <overlay>/game_package/gamedatabases/gamedatabases.2025.bin [--lists <driver-manager cache>/game-lists.json]
```

With `--lists` (Driver Manager's `game-lists.json`, whose `missing` list names the heads and kit the game has no asset for) it also checks each seated driver's head, helmet, suit and gloves.

`swofty build` reads the database in the game folder. While a test build is installed there, build with `--game` pointing at a folder holding a copy of `F1_25.exe` and the stock `game_package/gamedatabases/gamedatabases.2025.bin`, and a copy of Custom Grid Size without its `erp` ops.

Put the database on the build route (`"build_assets": ["db"]` in `$DATA/config.json`, or build without the in-game loader) to get the file.

## build_cars.py

Writes `erp/cars/vtf{,_ers_zeroed}{,_13in,_13in_raised}/` (fragment 0 = BXML, 1 = TSMD stats of each 2026 team handling file), `erp/cars/handling.json` (`replace-resource` ops for every archive that carries each file, keyed by the `ers` and `tyre_size` options, and by `cars` for the Williams and Haas folders, whose 26-car files are `<car>.26.vtf.*`) and `db/physics.json`. Reads the car slots and scalars through `season.py`, and the stock VTFs and game database from `--game`. Without `--write` it builds into `build/` and exits 1 when that differs from the shipped files. Tuning constants: `BASE_HP`, `BASE_MASS`, `SPREAD` (scales every scalar's distance from 1), `FAMILIES` (rev limit, peak-power rpm, torque peak per engine), `ENGINES`, `AUDIO`, `TYRES_13`, `BEAD_13`, `ALLOY_13`.
