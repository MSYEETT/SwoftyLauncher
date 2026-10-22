# Classic Seasons

Adds the F1 2016 to F1 2024 seasons to the Grand Prix and Time Trial pickers. Each season has its own teams, drivers, car numbers, cars, team logos and name backdrops.

Cars: fanfreluche (Fanfreluche aka totolab), "F1 <year> cars for F1 25", used with permission.

## Layout

| Path | Holds |
|---|---|
| `data/seasons.json` | Teams, team names and line-ups per season, new drivers |
| `data/stock_looks.json` | Stock helmet, suit and glove looks used as placeholders |
| `data/f1_mode_lists.json` | Every stock game setup mode list that holds `f1` |
| `tools/gen.py` | Writes `db/seasons.json`, `lang/seasons.json` and `native/src/seasons.hpp` from the data |
| `tools/build_cars.py` | Builds `cars/<year>/` and `assets/cars.json` from the season zips |
| `tools/carperf.py` | Shadow caster rebuild and texture cap, run by `build_cars.py` |
| `tools/tyres13.py` | 2016-2021 tyres: the stock 2025 tyre reshaped to the period size, run by `build_cars.py` |
| `tools/build_incardriver.py` | Per-season in-car driver archives (2016-2021) |
| `tools/build_compounds.py` | Per-season tyre compound archives for seasons with their own `common.erp` |
| `tools/logos.py`, `tools/frontend.py` | Team logo arrays (`erp/logos/`) and name backdrops (`erp/frontend/`) |
| `native/` | The plugin that adds the picker boxes |
| `cars/` | Built car archives (not committed) |

## Rows

- One game setup mode `cs_f1` (stat id 20) with one entrant setup mode `cs_f1` that lists every season's entrant list. `cs_f1` is added to every game setup mode list that holds `f1`.
- Per season: `game_setup.scenarios` row `cs_f1_<year>` (setup mode `cs_f1`), entrant list `cs_f1_<year>` (20 entrants), team standings `cs_f1_teams_<year>`.
- Teams and vehicles `cs<yy>_<folder>`, vehicle package `cs_<year>`.
- Driver looks `<year>-cs_<folder>_<driver>` (stat ids from 420) and pool entrants `cs<year>-<driver>`.

New drivers (Nasr, Gutiérrez, Vandoorne, Hartley, Sirotkin) use driver stat ids 225 to 229.

## Plugin

- At `site_FeTile_EraTilesBuilt` it adds one box per season after the stock boxes, for each season whose entrant list the screen's mode offers under `cs_f1`.
- `Game_F1WorldActivityContainer_SelectEntrantList` is wrapped: when a season box is pressed, the `cs_f1` entrant setup mode offers only that season's list until another box is pressed or the boxes are built again. Time Trial then loads only that season's cars, and the Grand Prix takes that season's line-up.

## Car archives

`build_cars.py`:

- Renames each archive's `eaid://f1_2025_vehicle_package/` resources to `eaid://cs_<year>_vehicle_package/` (same length). A season without its own `common.erp` (2022, 2023) renames only `teams/` and lists the stock `common.erp` in its asset groups.
- Moves the `.mipmaps` sidecars the zip ships to the season's folder.
- Rebuilds every shadow caster over 20,000 triangles from a lower LOD of the same part, and fails a car with a caster over 40,000.
- Caps every non-livery texture and shipped sidecar at 1024 px.
- Swaps the 2016-2021 tyres for the stock 2025 tyre reshaped to the period diameter (660 mm for 2016, 670 mm for 2017-2021) and widened to clear the season's rims.

```sh
python3 tools/build_cars.py --years 2016-2024
python3 tools/build_incardriver.py
python3 tools/build_compounds.py
python3 tools/gen.py
native/build.sh
```
