# F1 1990

Turns the F1 2026 season into the 1990 season in a profile of its own.

## Grid size

The **Cars** setting (`cars`) is 22 (default) or 26. At 22 the grid is the 11 teams on the stock 2026 team rows and the addon needs no other addon. At 26 Larrousse and Dallara join on Custom Grid Size's two extra teams: enable Custom Grid Size with Cars at 26. The grid ops are in `db/grid.json`, written by `tools/grid.py` (see `tools/README.md`).

## Front end (erp/frontend.json, erp/frontend/, tools/frontend.py)

The Driver Select and Team Select backdrops show the 1990 driver and team names, from textures `tools/frontend.py` renders over the stock ones in `main_front_end.erp`. **Hide the F2 and Icons tabs** (`hide_f2_icon_tabs`, default on) removes those two tabs from the Choose Driver screen by replacing `screen_groups.uicontentgroup` in `persistent.erp`, which the game loads at boot; turn it off if the game stops at boot.

## Rules (db/rules.json, db/rules_text.json, tools/rules.py)

`tools/rules.py [stock gamedatabases.2025.bin]` writes both op files from `tools/rules_fields.json` (the field lists of the cloned rows) and the stock database (the option tag lists it extends). The 1990 rules live in their own rows; 2025 and 2026 rows keep their stock values, and stock rows only gain the `f1_1990` name in lists and higher row counts.

- Setup mode `f1_1990` (stat_id 16), a clone of `f1_26` with option tag `gamesetup_f1_1990`, no tyre warmers, DRS lap group `f1_1990` (opens on lap 200), `tyre_regulations.allow_strategy_mandatory_stop_check` False, and the grid cutscene and grid rundown off unless `grid_cutscene` or `grid_rundown` is on. Its entrant setup mode stays `f1_26` until the grid lane points it at its own.
- Weekend (default structure): P1, P2, Friday qualifying (a single sprint-shootout session), Saturday qualifying (a single qualifying session), race. Full qualifying runs the single sessions; the race grid comes from Saturday qualifying; the race scores with `f1_1990_race` (9-6-4-3-2-1, no fastest-lap point).
- Option tag `gamesetup_f1_1990`: the `gamesetup_f1` restrictions plus safety car Off only, parc fermé Off only, standard weekend only and no full qualifying. The 38 options shown for `gamesetup_f1` are also shown for it.
- Track dataset `f1_1990` (parent `f1_2026`, stat_id 20, default format only): on every 2026 track C4 for all three dry compounds, presets `f1_1990` and `f1_1990_mandatory`, no required pit stop and the default weekend format; the 16 calendar tracks also carry the 1990 dates from `tools/career.py`.
- Compound class `f1_1990` (stat_id 12) with the dry options shown as Supersoft (`Supersoft_f1_1990` and two twins for the medium and harder slots). Allocation `f1_1990`: `soft_sets` new sets in P1, P2, Friday qualifying, Saturday qualifying and the race. Mandatory `f1_1990_mandatory`: one softer set in the shootout sessions. `no_tyre_allocation` turns the allocation screen off for this class.
- The 2026 track layer and the track lighting probes also list `f1_1990`.

Not scoped to 1990 (global rows, left stock): the safety-car and parc-fermé option defaults, the short-race restriction (very short races grey out red flags, formation lap and safety car) and the session-name strings (Friday qualifying shows the stock sprint-qualifying names).

## Cars (db/physics.json, erp/cars/, files/audio/, tools/build_cars.py)

`tools/build_cars.py` writes the handling files and both op files from the stock 2026 team archives and the season pack.

| Car folder | 1990 team(s) | Engine | Power | Mass |
|---|---|---|---|---|
| ferrari | Ferrari | V12, 13,500 rpm | 702 hp | 529 kg |
| mclaren | McLaren | V10, 13,300 rpm | 669 hp | 558 kg |
| williams | Williams; at 26 cars also Larrousse (T12) | V10, 13,300 rpm | 655 hp (26 cars: 658) | 571 kg (568) |
| lotus | Benetton | V8, 12,500 rpm | 657 hp | 569 kg |
| redbull | Lotus | V12, 13,500 rpm | 653 hp | 572 kg |
| mercedes | Tyrrell | V8, 12,500 rpm | 635 hp | 588 kg |
| toro_rosso | Minardi | V8, 12,500 rpm | 629 hp | 593 kg |
| force_india | Leyton House | V8, 12,500 rpm | 627 hp | 595 kg |
| sauber | Ligier | V8, 12,500 rpm | 627 hp | 595 kg |
| gm_cadillac | Brabham | V8, 12,500 rpm | 619 hp | 603 kg |
| haas | Arrows; at 26 cars also Dallara (T13) | V8, 12,500 rpm | 621 hp (26 cars: 618) | 601 kg (604) |

- Handling: no `EnergyRecoverySystem` section and no ERS parts in the R&D upgrade blocks; naturally aspirated engine (`TurboMode Off`); power = 650 hp × the team's power scalar, mass = 575 kg × its weight scalar, drag = the stock car's drag × its drag scalar; at 26 cars, teams sharing a car folder get the mean of their scalars (the `cars` option picks the handling file). Downforce, tyres, suspension and brakes stay stock. The final drive keeps the stock top-gear speed at the rev limit; fuel use per second at full power stays stock. No moving front wing. No DRS hardware: the four DRS failure and joint keys are removed and the rear-wing flap angle, open/close speed and DRS-open wing factors are set to the safety car's values (0, 1 and 1.0), and `radUpgradeDRS` is empty.
- Database: `hud_dial` (option, default `f2`) and `drs_capable False` on the 13 vehicle rows and the 11 team rows of `f1_2026_equal_performance`; `is_26_aero_regulations False` on the `f1_2026` package (option `classic_aero`); every power-unit component in both component sets gets allocation 99, no grid drops and no wear.
- Option `ers`: `removed` (default) or `zeroed`, which keeps the ERS section with no harvest, deploy, MGU-K braking or override.
- Option `tyre_size`: `stock` (default), `13in` or `13in_raised`. `13in` matches the MP4/5B 13-inch shared tyre mesh: TyreRadius 0.323 m front and 0.330 m rear (stock 0.351 / 0.354), AlloyRadius 0.182 m at its 0.175 m bead, the tyre damage node graphs remapped onto the new bead, tread and width, and the final drive scaled by the rear radius so road speed per engine rev is unchanged. TyreWidth, tyre mass, inertia and compression stay stock. `13in_raised` also adds the radius drop (0.028 m front, 0.024 m rear) to each axle's RideHeight and setup ride-height range.
- Engine sounds: the V12, V10 and V8 bundles of the F1 25 Engine Sound Pack replace `ferrari_2026.bdl`, `renault_2026.bdl` and `ford_2026.bdl` (V8 also `mercedes_2026.bdl`), and each 2026 team's `f1_m_<team>_26.bundle_name` points at its 1990 engine. T12 takes the Lotus row's `audio_id`, so Larrousse plays the V12.
- Grand Prix with equal performance on uses the `f1_2026_equal_performance` cars, whose handling files keep ERS and DRS.

```sh
python3 addons/f1-1990/tools/build_cars.py --game "<F1 25>"          # compare with the shipped files
python3 addons/f1-1990/tools/build_cars.py --game "<F1 25>" --write  # replace them
```

The stock 2026 team archives and game database must be the stock files.

## Driver Career (db/career.json, db/career-lang.json, tools/career.py, tools/check_career.py)

The 1990 career is a third career content next to the stock 2025 and 2026 ones. Its rows are full-key clones of the
2026 rows; no 2025 or 2026 row changes.

| Row | Name | Cloned from |
|---|---|---|
| career data group, in all eight data-group tables | `f1_career_1990` | `f1_career_2026` |
| mode row | `driver_career_1990` (statId 81, save container `driver_career_1990`) | `driver_career_26` |
| championship | `F1_driver_career_1990` (statId 35, setup mode `f1_1990`, entrant list and setup data `f1_career_1990`) | `F1_driver_career_26` |
| event list | `F1_career_1990`: one season, five season events (no New Season Grid Reveal) | `F1_career_26` |
| track list | `F1_1990`: the 16 rounds in order (Hockenheim on Shanghai) | `F1_2026_16` |
| season lengths | `F1_1990_Long`, `_Medium`, `_Short`, all 16 rounds, no transfer window, setup mode `f1_1990` | `F1_2026_*` |
| track selection | `F1_1990` | `F1_2026` |

Content set `2026` lists `driver_career_1990` and `f1_1990`. The career data group is the key the career slot list
filters on, so 1990 careers have their own slots. The **Career year** setting starts the career in 1990 (default) or
2026.

Strings are new keys: `lng_f1990_career_content(_description)` for the third Career tile, `lng_f1990_season`,
`lng_f1990_championship_season(_loading_screen)`, `lng_f1990_trophy_drivers_champion`, `lng_f1990_season_length`, and
per round `lng_f1990_gp_<code>` (long Grand Prix name) and `lng_f1990_gp_<code>_short`. `career.TRACK_NAMES` and
`career.TRACK_DATES` give the keys and the 1990 dates per track for the `f1_1990` track dataset.

The agent is Flavio Briatore in every career: her strings and her character row are shared by all careers, and no data
names the agent per career. Her English speech banks are replaced with silent copies (`files/speech/`,
`tools/mute_agent.py`), which also mutes her in 2025 and 2026 careers.

```sh
python3 addons/f1-1990/tools/career.py <stock gamedatabases.2025.bin> <dir of language_<code>.tsv>
python3 addons/f1-1990/tools/check_career.py <stock gamedatabases.2025.bin> <built gamedatabases.2025.bin>
```

`check_career.py` fails when a key of a stock 2025/2026 career row, points preset, track dataset, content set or car
group changed (other than the row counts, content set 11's two lists and the agent's name fields), or a 1990 row is
missing.
