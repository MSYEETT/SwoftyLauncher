# F1 1990

Turns the F1 2026 season into the 1990 season in a profile of its own.

## Grid (db/grid.json, db/grid_standings.json, lang/grid.json, tools/grid.py)

The 1990 teams (`<team>_1990`), drivers (`f1990_<driver>`), their looks, holders and entrants, and the entrant lists `f1_1990`, `f1_1990_driver_pool` and `f1_career_1990` are new rows; no 2025 or 2026 row changes. The **Cars** setting (`cars`) is 22 (default: the 11 strongest teams) or 26 (adds Larrousse and Dallara; needs 26-car track layers). See `tools/README.md`.

## Front end and tracks (erp/frontend.json, erp/badges.json, erp/tracks.json, tools/frontend.py, tools/badges.py, tools/tracks.py)

The 1990 rows get their own Driver Select and Team Select backdrop names and placeholder team badges, added next to the stock art, and the `f1_1990` setup mode loads its own track worlds (26-car garages and grid slots when `cars` is 26). See `tools/README.md`.

## Rules (db/rules.json, db/rules_text.json, tools/rules.py)

`tools/rules.py [stock gamedatabases.2025.bin]` writes both op files from `tools/rules_fields.json` (the field lists of the cloned rows), `tools/career.py` (`TRACK_DATES`, `TRACK_NAMES`) and the stock database (the option tag lists it extends). The 1990 rules live in their own rows; 2025 and 2026 rows keep their stock values, and stock rows only gain the `f1_1990` name in lists and higher row counts. `tools/check_rules.py <stock> <built>` checks a build.

- Setup mode `f1_1990` (stat_id 16), a clone of `f1_26` with option tag `gamesetup_f1_1990`, no tyre warmers, DRS lap group `f1_1990` (opens on lap 200), `tyre_regulations.allow_strategy_mandatory_stop_check` False, and the grid cutscene and grid rundown off unless `grid_cutscene` or `grid_rundown` is on.
- Weekend (default structure): P1, P2, Friday qualifying (a single sprint-shootout session), Saturday qualifying (a single qualifying session), race. Full qualifying runs the single sessions; the race grid comes from Saturday qualifying; the race scores with `f1_1990_race` (9-6-4-3-2-1, no fastest-lap point).
- Option tag `gamesetup_f1_1990`: the `gamesetup_f1` restrictions plus safety car Off only, parc fermé Off only, standard weekend only and no full qualifying. The 38 options shown for `gamesetup_f1` are also shown for it.
- Track dataset `f1_1990` (parent `f1_2026`, stat_id 20, default format only): on every 2026 track C4 for all three dry compounds, presets `f1_1990` and `f1_1990_mandatory`, no required pit stop and the default weekend format; the 16 calendar tracks also carry the 1990 dates (TrackGeneralData) and grand prix names (TrackNameData) from `tools/career.py`.
- Compound class `f1_1990` (stat_id 12) with the dry options shown as Supersoft (`Supersoft_f1_1990` and two twins for the medium and harder slots). Allocation `f1_1990`: `soft_sets` new sets in P1, P2, Friday qualifying, Saturday qualifying and the race. Mandatory `f1_1990_mandatory`: one softer set in the shootout sessions. `no_tyre_allocation` turns the allocation screen off for this class.
- The track lighting probes also list `f1_1990`.

Not scoped to 1990 (global rows, left stock): the safety-car and parc-fermé option defaults, the short-race restriction (very short races grey out red flags, formation lap and safety car), the session-name strings (Friday qualifying shows the stock sprint-qualifying names) and the power-unit component sets (`engine_management.components`), which the game picks by season only: `f1_1990` runs on `season_2026`, so 1990 has the 2026 allocation, wear and grid drops.

## Cars (db/cars.json, cars/, tools/build_cars.py, tools/cars.py)

The 1990 cars live in their own vehicle package, `f1_1990` (stat_id 59). Its `resource_path` is `f1_1990_vehicle_package/teams/[FILENAME]/wep/[FILENAME]` and its `ai_package_folder` stays `f1_2026_vehicle_package`, so the per-track AI data of the 2026 car folders serves the 1990 cars. No 2026 archive, row or file changes. The models, the package's `teams/common.erp` (13-inch tyres) and its tyre compounds come from the private car pack `swofty.f1-1990-cars`, which registers one asset group per car at runtime. Without the car pack, a session with a 1990 vehicle hangs on load.

| Vehicle | Car folder (AI data) | Chassis | Engine | Power | Mass |
|---|---|---|---|---|---|
| mclaren_1990 | mclaren | McLaren MP4/5B | Honda V10, 13,300 rpm | 669 hp | 558 kg |
| ferrari_1990 | ferrari | Ferrari 641 | V12, 13,500 rpm | 702 hp | 529 kg |
| williams_1990 | williams | Williams FW13B | Renault V10 | 655 hp | 571 kg |
| benetton_1990 | lotus | Benetton B190 | Ford V8, 12,500 rpm | 657 hp | 569 kg |
| tyrrell_1990 | mercedes | Tyrrell 019 | Ford V8 | 635 hp | 588 kg |
| lotus_1990 | redbull | Lotus 102 | Lamborghini V12 | 653 hp | 572 kg |
| leytonhouse_1990 | force_india | Leyton House CG901 | Judd V8 | 627 hp | 595 kg |
| larrousse_1990 | myteam_ferrari | Lola LC90 | Lamborghini V12 | 661 hp | 565 kg |
| ligier_1990 | sauber | Ligier JS33B | Ford V8 | 627 hp | 595 kg |
| minardi_1990 | toro_rosso | Minardi M190 | Ford V8 | 629 hp | 593 kg |
| brabham_1990 | gm_cadillac | Brabham BT59 | Judd V8 | 619 hp | 603 kg |
| dallara_1990 | myteam_honda | Dallara F190 | Ford V8 | 615 hp | 606 kg |
| arrows_1990 | haas | Arrows A11B | Ford V8 | 621 hp | 601 kg |

- Vehicle rows: full copies (246 keys) of the donor folder's 2026 row, stat_id 513–525 (Custom Grid Size uses 509–512), `vehicle_package f1_1990`, `hud_dial` from the option (default `f2`), `drs_capable False`, `equal_performance_vehicle` empty, `performance_tier` 0 (McLaren, Ferrari, Williams, Benetton), 1 (Tyrrell, Lotus, Leyton House, Minardi, Brabham, Arrows, Dallara) or 2 (Ligier, Larrousse), and the grid lane's strings `lng_f1990_team_<team>`, `lng_f1990_chassis_<team>`, `lng_f1990_engine_<team>`. The labels use the grid lane's tokens (`leytonhouse_1990`, `ligier_1990`). The engine id stays the donor's.
- Package row: a copy of `f1_2026` with the fields above, `has_cockpit_halo False`, the 2026 animation, crew, garage, pit-stop and pit-wall groups, and `is_26_aero_regulations False` when `classic_aero` is on.
- Handling: each car's handling file is built from its donor's stock 2026 handling file and shipped as `cars/handling/<ers>_<tyre_size>/<folder>_handling.erp`; `cars/handling.json` puts the one matching the options at `f1_1990_vehicle_package/teams/<folder>/wep/<folder>_handling.erp`, which the car's asset group lists. No EnergyRecoverySystem section (or zeroed, option `ers`), naturally aspirated engine, power = 650 hp × the team's power scalar, mass = 575 kg × its weight scalar, drag × its drag scalar, no moving front wing, no DRS hardware, 13-inch tyres (TyreRadius 0.323/0.330, AlloyRadius 0.182, damage graphs remapped, final drive scaled); `13in_raised` adds the radius drop to the ride heights.
- Engine sounds (`db/audio.json`, `tools/build_audio.py`, `tools/audio.py`): a car's engine sound is the `audio.audio_car_data.<switchName>` row named by the AudioF1Car `switchName` in its carmodel layer. The car pack gives each 1990 car its own label (`f1_m_<name>_90`, the donor's with `_90` for `_26`; Larrousse `f1_m_larrouss_90`, Dallara `f1_m_dall_90`), and `db/audio.json` adds those 13 rows (put-record, the donor's values) with `bundle_name` `v12_1990.bdl` (Ferrari, Lotus, Larrousse), `v10_1990.bdl` (McLaren, Williams), `v8j_1990.bdl` (Leyton House, Brabham) or `v8_1990.bdl` (the Ford teams), plus four `audio.audio_bundle_settings` rows for those bundles. `operations.files` adds the four `.bdl` files from the F1 25 Engine Sound Pack under those new names. No stock audio row or bundle changes.

```sh
python3 addons/f1-1990/tools/build_cars.py --game "<pristine F1 25 copy>"          # compare with the shipped files
python3 addons/f1-1990/tools/build_cars.py --game "<pristine F1 25 copy>" --write  # replace them
```

`--game` must hold stock 2026 team archives: a game folder with a test build installed carries modified handling files. `/mnt/work/.f1-1990/data-physics/fakegame-full` is a pristine copy.

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

The agent stays Elena in data: her character row, contact and strings are shared by all careers, and no data names the
agent per career. `data/flavio-strings.json` holds, per language, every stock string that names her rewritten for
Flavio Briatore (sign-offs say Flavio, the intro subtitles Flavio Briatore); the native side applies them, and mutes
her voice, only while a 1990 career is loaded. The character row `flavio` (stat_id 105, names `lng_f1990_agent_*`) is
the 1990 agent for that switch; its message avatar is `message_flavio` (`tools/portrait.py`).

```sh
python3 addons/f1-1990/tools/career.py <stock gamedatabases.2025.bin> <dir of language_<code>.tsv>
python3 addons/f1-1990/tools/check_career.py <stock gamedatabases.2025.bin> <built gamedatabases.2025.bin>
```

## Agent portrait (erp/portrait.json, erp/portrait/portrait.erpk, tools/portrait.py)

`tools/portrait.py` cuts Flavio Briatore's head and shoulders out of a photo (GrabCut for the body, a fitted background
plate for the hair), frames it like the stock portraits and encodes it as a 256x256 single-mip BC7 slice. The slice is
appended as `message_flavio` to the character message-avatar array
(`overrides.characters.message_avatars.texarray`, 87 stock slices), and a `patch-pkg` clones the list entry in
`databases.erp: overrides.uidb`. The four archives that carry the array (`flow_career_hub`, `flow_customisation`,
`flow_st_hub`, `flow_team_hq`) get the grown array by `put-resource`; a grown array changes the archives' layout, so
the build writes them into the game files. Stock slices, Elena's included, are unchanged.

```sh
python3 addons/f1-1990/tools/portrait.py --game <F1 25> --source <photo> [--baseline <launcher baseline>/<build>] [--preview <png>]
```

`check_career.py` fails when a key of a stock 2025/2026 career row, points preset, track dataset, content set or car
group changed (other than the row counts, content set 11's two lists and the agent's name fields), or a 1990 row is
missing.
