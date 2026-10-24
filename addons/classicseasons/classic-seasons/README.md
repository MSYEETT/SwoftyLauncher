# Classic Seasons

Adds the F1 2016 to F1 2024 seasons to the Grand Prix and Time Trial pickers. Each season has its own teams, drivers, car numbers, cars, team logos and name backdrops.

Classic Seasons is ten addons in `addons/classicseasons/`. This folder is the shared addon, `swofty.classic-seasons`: the plugin that registers the season boxes, the new drivers, team logos, name backdrops and box art. Each season is its own addon, `swofty.classic-seasons-<year>` in `../classic-seasons-<year>/`, which requires the core and holds that season's cars, rows and text. Any set of seasons installs on top of the core, and a season that is not installed has no box.

Cars: fanfreluche (Fanfreluche aka totolab), "F1 <year> cars for F1 25", used with permission.

Suits: "F1 2016 - suits mod" (author unknown) and "2018 Redbull for F1 25" (author unknown), used with permission.

The car archives are not in the repository. The built packages, each season's `.swoftyaddon` with its cars and the shared addon's, are on the `classic-seasons-<version>` release; install the shared addon and any seasons through the launcher.

## Layout

| Path | Holds |
|---|---|
| `data/seasons.json` | Teams, team names and line-ups per season, new drivers |
| `data/stock_looks.json` | Stock helmet, suit and glove looks used as placeholders |
| `tools/gen.py` | Writes the core's `db/core.json`, `lang/core.json` and `native/src/seasons.hpp`, and each season addon's `swofty.json`, `db/season.json`, `lang/season.json` and `assets/cars.json` |
| `tools/layout.py` | Season addon folders and the asset ops that ship a season's cars |
| `tools/build_cars.py` | Builds a season addon's `cars/` from the season zips, and an extra team's car (`car` in `data/seasons.json`) into `cars/extra/` |
| `tools/carperf.py` | Shadow caster rebuild and texture cap, run by `build_cars.py` |
| `tools/handling.py` | Handling files: wheel positions from the handling file, run by `build_cars.py` |
| `tools/tyres.py` | `Tyres` overrides naming a season's tyre set, run by `build_cars.py` |
| `tools/build_incardriver.py` | Per-season in-car driver archives (2016-2021), in the season addon's `cars/incardriver/` |
| `tools/logos.py`, `tools/frontend.py` | Team logo arrays (`erp/logos/`) and name backdrops (`erp/frontend/`) |
| `tools/tiles.py` | Season box art (`erp/tiles/`): one `tiles_solo_6x6` slice per season from `art/tiles/src/<year>.dds` |
| `data/real_suits.json` | Real suits from mods: each one's season, team, driver, base suit and archives |
| `tools/suit_sources.py` | The suit each team-season and driver of 2016 to 2021 wears |
| `tools/build_real_suits.py` | The real suits from mods, relabelled, in `suits/`, and their contact sheet `art/suits/real.png` |
| `tools/build_suits.py` | Generated team suits and gloves of 2016 to 2021, in the season addon's `suits/`, and the season contact sheets |
| `art/suits/<year>.png` | Half-size contact sheet of a season's suits; the full one is the season addon's `suits/preview.png` |
| `tools/bcn.py`, `tools/suitgeo.py` | BC1, BC3, BC5 and BC7 textures and `.mipmaps` levels; mesh streams, decal slots and the suit renders, used by `build_suits.py` |
| `native/` | The plugin that adds the picker boxes |
| `../classic-seasons-<year>/cars/` | Built car archives of a season (not committed) |
| `suits/` | Built real suit archives, their `.mipmaps` sidecars, `meta.json` and `preview.png` (not committed) |
| `../classic-seasons-<year>/suits/` | Built generated suit archives of a season, their `meta.json` and `preview.png` (not committed) |
| `data/setup_preset_fields.json` | The 421 keys of a `car_setup.presets` row, copied for every season package |
| `tools/team_colours.py` | Writes `data/team_colours.json`: three team-wear colours per team, sampled from its car's paint texture (`<folder>_paint_d.tif`, or `_paint_c.tif` when `paint_d` is an 8 px placeholder), or the team's `team_wear` hex list in `data/seasons.json`; `--sheet <png>` draws a swatch sheet |

## Rows

Shared addon:

- The new drivers, their statistics holders and the head overrides.

Each season:

- `game_setup.scenarios` row `cs_f1_<year>` (setup mode `swofty_f1`, swofty.core's host setup mode for addon boxes), entrant list `cs_f1_<year>` (a copy of `f1_2025`, 20 entrants, or of `f1_2026` with the 2025 season, leaderboard and UI path for 22), team standings `cs_f1_teams_<year>`. A 22-car season sets `swofty_f1`'s `max_entrants` to 22.
- `list-append` of `cs_f1_<year>` onto the `swofty_f1` entrant setup mode's `entrant_lists` and `selectable_entrant_lists`, and onto the 2025 and 2026 content sets.
- Teams and vehicles `cs<yy>_<key>` (`key` defaults to the team's `folder`). A team's `team_wear_primary_colour`, `team_wear_secondary_colour` and `team_wear_tertiary_colour` come from `data/team_colours.json`. Vehicle package `cs_<year>`, and `cx_<year>` for a team with its own car archive.
- A `car_setup.presets` row for each vehicle package (`cs_<year>_vehicle_package`, `cx_<year>_vehicle_package`), copied from `f1_2025_vehicle_package`: the game looks up the AI and player car setups by package name.
- A `pit_lane.pit_times` row per team, copied from its stock team.
- Driver looks `<year>-cs_<key>_<driver>` (stat ids from 420, numbered across every season) and pool entrants `cs<year>-<driver>`.
- 2016 to 2021: each driver look wears the suit and gloves of [Suits](#suits). A season copies the suit and gloves rows of each generated suit it ships, and the shared addon those of each real suit from a mod, with its helmet row. The copies are labelled with the suit's label and take the base suit's rows with the suit's asset group, models and `material_override`. Item ids (`tools/gen.py`): `REAL_SUIT_IDS` for the real suits, 974 up in `data/real_suits.json` order; `SUIT_IDS` for the generated ones, the next ids, one per team-season of 2016 to 2021 in season and team order; `HELMET_IDS`, 868 up, for the helmets. The stock ids are 448 to 973 for suits and gloves and 315 to 867 for helmets. Each suit and gloves copy sets `colour_1_default` to `colour_4_default` to `0`, a helmet copy the base helmet's values, and the file's `fields` map lists the four keys for suit, gloves and helmet rows.

New drivers (Nasr, Gutiérrez, Vandoorne, Hartley, Sirotkin, Haryanto) use driver stat ids 225 to 230.

With a season of more than ten teams, the shared addon adds a `track_data.track_layers` row for `swofty_f1` (seasons 2023 to 2025) with the `f1_26` worlds, which have an eleventh garage, and removes `swofty_f1` from the `f1` track layer.

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

The years build independently, so `--years` ranges can run in parallel. `build_cars.py` empties a season's `cars/`, so `build_incardriver.py` runs after it.

2016's Manor is `sauber.erp` in the downloads folder: fanfreluche's Manor car in the Sauber slot. It builds into `cx_2016_vehicle_package` under `teams/sauber/`, with the same passes and its shared references on the 2016 shared archive.

## Suits

A team-season of 2016 to 2021 wears, first found:

1. the real team suit of its own season;
2. the real team suit of the nearest season with the same team name in `data/seasons.json` (Force India and Racing Point are different teams), the earlier season on a tie;
3. its generated suit.

A real suit is a mod's suit in `data/real_suits.json` that `build_real_suits.py` has built, or a stock F1 25 suit of a season in its `stock_seasons` (2024: `2024-<stock team>` and `2024-<stock team>_<driver>`). Only team suits decide the season. Every driver of the team-season wears that season's kit: his own suit of that season and team when there is one, else that season's team suit. A mod driver suit brings its helmet. `"suit": {"source": "<label>"}` or `"source": "generated"` on a team in `data/seasons.json` sets the team's suit. The season contact sheets name the suit of every team and driver.

### Real suits

`build_real_suits.py --game <F1 25> --mods <folder holding the extracted mods>` builds each mod suit of `data/real_suits.json` into `suits/`:

- The label is `cs<yy>-<stock team>` (a team suit) or `cs<yy>-<stock team>_<driver slot>` (a driver suit), `<yy>` the suit's season. It has the length of the base label it replaces, `2025-<stock team>` or `2025-<stock team>_<driver slot>`.
- The archive holds every resource of the mod archive whose name holds the base label (a driver suit: the base team label), renamed byte for byte. The stock base archive holds the shared resources, and the asset group lists it first.
- A `.mipmaps` sidecar the mod ships, that holds the levels its texture names and that differs from the stock file goes to `suits/sidecars/` under the relabelled path. A sidecar the mod does not ship keeps its stock path when the texture's levels are the stock ones; otherwise the texture keeps only its embedded levels.
- A helmet archive takes the suit archive's copy of each resource both hold.
- `art/suits/real.png` shows each real suit's front and back render, `livery_v2_d` and `logo_01_d`.

| Mod archive | Season, team, driver | Label |
|---|---|---|
| F1 2016 - suits mod, `team_livery/2025-<team>/2025-<team>.erp` (10 teams) | 2016, the team the archive replaces | `cs16-<team>` |
| 2018 Redbull for F1 25, `team_livery/2025-redbull` | 2018 Red Bull Racing | `cs18-redbull` |
| 2018 Redbull for F1 25, `drivers/male/2025-redbull_max_verstappen` | 2018 Red Bull Racing, Max Verstappen | `cs18-redbull_max_verstappen` |
| 2018 Redbull for F1 25, `drivers/male/2025-redbull_yuki_tsunoda` | 2018 Red Bull Racing, Daniel Ricciardo | `cs18-redbull_yuki_tsunoda` |

### Generated suits

`build_suits.py` builds one suit per team-season that wears no real suit, from an F1 25 team suit (the base) and the team's car:

| Field of the team's `suit` in `data/seasons.json` | Default |
|---|---|
| `base` | `2025-<stock team>`, the stock team of the team's `folder` |
| `design` | `tier01_livery76`, a customisable suit design whose panel masks paint the suit |
| `colours` | The team's `team_wear` in `data/seasons.json`, else its `team_wear` colours in `data/team_colours.json`; without them, three colours from the car's `paint_d` and opaque `decal_da` texels: the most common, then the most common that differ from those before |
| `sponsors` | The largest separate shapes of the car's `decal_da`, as `[x, y, w, h]` rectangles in pixels of the 2048 atlas |

- The suit's label is `cg<yy>-<stock team>`, or `cg<yy>-<key>` padded with `_` to the base label's length for a team with its own `key` (2016 Manor: `cg16-manor_`). It has the base label's length.
- The suit archive holds every resource of the base archive whose name holds the base label, renamed byte for byte. The shared body, glove and template resources stay in the stock base archive, which the suit's asset group lists before the suit archive.
- `livery_v2_d` (2048), `boots_d` (1024 by 512) and `glove_d` (1024) are the design's `_m` masks painted with the colours, as the game paints a customisable suit: white, then colour 1 by red, colour 2 by green and colour 3 by blue. The boots take darker colours.
- `logo_01_d` (1024) and `glove_logo_d` (512) are new decal atlases. Each decal of the base suit's logo meshes keeps its place on the body. Its atlas rectangle, the second texture coordinate pair of the decal mesh, gets the team logo (the two largest torso decals, and the largest near-square one), a sponsor, or nothing. A logo on a background of similar lightness turns white or black. The team logo is the brand art of `art/logos/<brand>/` or the stock team's mark from `teams.driver_select_team_logos`.
- `logo_01_n` and `glove_logo_n` become flat. Every other texture keeps the stock `.mipmaps` sidecar of the base suit.
- The contact sheet (`suits/preview.png`, and `art/suits/<year>.png` at half size) shows each team's front and back render (the male body, boots, gloves and decals), the painted `livery_v2_d` and the `logo_01_d` atlas with its decal rectangles: blue for the team logo, yellow for a sponsor, grey for an empty one.

```sh
python3 tools/build_real_suits.py --game <F1 25> --mods <folder holding the extracted mods>
python3 tools/build_suits.py --game <F1 25> --years 2016-2021 [--cars <folder holding classic-seasons-<year>/cars>]
python3 tools/gen.py
```

It needs numpy, Pillow, SciPy and lz4, `node` (BC7 decoding) and the season's built cars.

## Packages

```sh
for a in ../classic-seasons ../classic-seasons-20*; do python3 ../../../tools/pack_addon.py "$a" dist; done
```
