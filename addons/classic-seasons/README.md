# Classic Seasons

Adds the F1 2016 to F1 2024 seasons to the Grand Prix and Time Trial pickers. Each season has its own teams, drivers, car numbers, cars, team logos and name backdrops.

Classic Seasons is ten addons. This folder is the shared addon, `swofty.classic-seasons`: the plugin that registers the season boxes, the new drivers, team logos, name backdrops and box art. Each season is its own addon, `swofty.classic-seasons-<year>` in `../classic-seasons-<year>/`, which requires the core and holds that season's cars, rows and text. Any set of seasons installs on top of the core, and a season that is not installed has no box.

Cars: fanfreluche (Fanfreluche aka totolab), "F1 <year> cars for F1 25", used with permission.

## Layout

| Path | Holds |
|---|---|
| `data/seasons.json` | Teams, team names and line-ups per season, new drivers |
| `data/stock_looks.json` | Stock helmet, suit and glove looks used as placeholders |
| `tools/gen.py` | Writes the core's `db/core.json`, `lang/core.json` and `native/src/seasons.hpp`, and each season addon's `swofty.json`, `db/season.json`, `lang/season.json` and `assets/cars.json` |
| `tools/layout.py` | Season addon folders and the asset ops that ship a season's cars |
| `tools/build_cars.py` | Builds a season addon's `cars/` from the season zips |
| `tools/carperf.py` | Shadow caster rebuild and texture cap, run by `build_cars.py` |
| `tools/handling.py` | Handling files: wheel positions from the handling file, run by `build_cars.py` |
| `tools/tyres.py` | `Tyres` overrides naming a season's tyre set, run by `build_cars.py` |
| `tools/build_incardriver.py` | Per-season in-car driver archives (2016-2021), in the season addon's `cars/incardriver/` |
| `tools/logos.py`, `tools/frontend.py` | Team logo arrays (`erp/logos/`) and name backdrops (`erp/frontend/`) |
| `tools/tiles.py` | Season box art (`erp/tiles/`): one `tiles_solo_6x6` slice per season from `art/tiles/src/<year>.dds` |
| `native/` | The plugin that adds the picker boxes |
| `../classic-seasons-<year>/cars/` | Built car archives of a season (not committed) |

## Rows

Shared addon:

- The new drivers, their statistics holders and the head overrides.

Each season:

- `game_setup.scenarios` row `cs_f1_<year>` (setup mode `swofty_f1`, swofty.core's host setup mode for addon boxes), entrant list `cs_f1_<year>` (20 entrants), team standings `cs_f1_teams_<year>`.
- `list-append` of `cs_f1_<year>` onto the `swofty_f1` entrant setup mode's `entrant_lists` and `selectable_entrant_lists`, and onto the 2025 and 2026 content sets.
- Teams and vehicles `cs<yy>_<folder>`, vehicle package `cs_<year>`.
- Driver looks `<year>-cs_<folder>_<driver>` (stat ids from 420, numbered across every season) and pool entrants `cs<year>-<driver>`.

New drivers (Nasr, Gutiérrez, Vandoorne, Hartley, Sirotkin) use driver stat ids 225 to 229.

## Plugin

At load it registers one box per season with `swofty::era::add_box` (`swofty/era.hpp`): entrant list `cs_f1_<year>`, title `lng_cs_f1_<year>`, image `#overrides.home_screen.tiles_solo_6x6[cs_f1_<year>]`, in year order. swofty.core builds them in the Addons row of the Grand Prix and Time Trial era picker, skips a season that is not installed and narrows `swofty_f1` to the pressed season.

## Car archives

`build_cars.py`:

- Renames each archive's `eaid://f1_2025_vehicle_package/` resources to `eaid://cs_<year>_vehicle_package/` (same length). A season without its own `common.erp` (2022, 2023) renames only `teams/` and lists the stock `common.erp` in its asset groups.
- Moves the `.mipmaps` sidecars the zip ships to the season's folder.
- Rebuilds every shadow caster over 20,000 triangles from a lower LOD of the same part, and fails a car with a caster over 40,000.
- Caps every non-livery texture and shipped sidecar at 1024 px.
- Moves the cockpit camera of a season with `cockpit_camera` (`up` and `forward`, in metres) in `data/seasons.json`, and sets the TV pod cameras' near clip of a season with `pod_camera` (`near_clip`, in metres) (tools/cameras.py).
- Turns off `UseModelPositions` in every handling file, so the wheels sit at the handling file's positions instead of the model's (tools/handling.py).
- Renames the tyre set (`shared/tyres/`) of a season with its own `common.erp` with the rest, and adds a `Tyres` override naming it (`tyre model`, `tyre wet model`, `material`, `wet material`) to every car and showroom model layer (tools/tyres.py). The car templates in `eaid://wote/types` name the stock tyre set.

```sh
python3 tools/build_cars.py --years 2016-2024
python3 tools/build_incardriver.py
python3 tools/gen.py
native/build.sh
```

The years build independently, so `--years` ranges can run in parallel.
