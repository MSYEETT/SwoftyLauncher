# F1 1990

Adds a 1990 Driver Career next to the 2025 and 2026 ones. 1990 careers live in the normal profile save, in their own slot list (data group `f1_career_1990`).

## Grid (db/grid.json, db/grid_standings.json, lang/grid.json, tools/grid.py)

The 1990 teams (`<team>_1990`), drivers (`f1990_<driver>`), their looks, holders and entrants, and the entrant lists `f1_1990`, `f1_1990_driver_pool` and `f1_career_1990` are new rows; no 2025 or 2026 row changes. The **Cars** setting (`cars`) is 22 (default: the 11 strongest teams) or 26 (adds Larrousse and Dallara; needs 26-car track layers). See `tools/README.md`.

## Front end and tracks (erp/frontend.json, erp/badges.json, erp/tracks.json, art/logos/, tools/frontend.py, tools/badges.py, tools/logo_art.py, tools/tracks.py)

The 1990 rows get their own Driver Select and Team Select backdrop names and their period team logos, added next to the stock art, and the `f1_1990` setup mode loads its own track worlds (26-car garages and grid slots when `cars` is 26). See `tools/README.md`.

The logos (`art/logos/`, sources and licences in `art/logos/SOURCES.md`) fill a slice per 1990 team in 22 UI texture arrays, each keyed by the name the game builds from the team row (`entrants.ui.asset_templates.team.textures`, `[TEAM]` = `<team>_1990`):

| surface | arrays |
|---|---|
| Team Select (tiles, cards), grid rundown, timing towers, standings and post-race lists, Driver Select, R&D and Team HQ tiles, career team logos, career rewards | `team_logos.f1_2026`, `teams.mp_team_select_logos`, `broadcast_ui_generic.grid_team_logos`, `osd.timing_tower.team_badges_hud` / `_broadcast`, `teams.badges_broadcast`, `teams.driver_select_team_logos`, `team_logos.driver_career_2024_team_select`, `driver_career_rewards.recognition.team_icons` |
| contracts: career hub offers, negotiation, Team HQ cards and backdrop, holding screen banner; press interview pill | `teams.team_contract_logo`, `team_hq.contract_negotiation_logos`, `team_hq.contract_logos_mid`, `team_hq.contract_logos_large`, `team_hq.contract_holding_screen_bgs`, `press_interviews.pill_bg` |
| F1 World fan zone | `teams.fanzone_room`, `teams.teams_f1_2024_gp_voting`, `teams.fanzone_expressions(_large)` |
| blank, as for the stock F1 teams | `teams.career_laptop_header` (the career laptop header; the garage monitor shows its logo only when this key exists), `broadcast_ui_generic.team`, `teams.career_team_select_logo_small` |

Not covered: the garage boards and pit perches of the 1990 track worlds still show the 2026 team of each garage slot (`garage_board_f1.emb?context=2026-<team>` and `pit_perch_<team>` are placed per slot in each track's layers).

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
- Handling: each car's handling file is built from its donor's stock 2026 handling file and shipped as `cars/handling/<ers>_<tyre_size>/<folder>_handling.erp`; `cars/handling.json` puts the one matching the options at `f1_1990_vehicle_package/teams/<folder>/wep/<folder>_handling.erp`, which the car's asset group lists. No EnergyRecoverySystem section (or zeroed, option `ers`), naturally aspirated engine, power = 650 hp × the team's power scalar, mass = 575 kg × its weight scalar, drag × its drag scalar, no moving front wing, no DRS hardware, 13-inch tyres (TyreRadius 0.323/0.330, AlloyRadius 0.182, damage graphs remapped, final drive scaled); `13in_raised` adds the radius drop to the ride heights. The R&D upgrade blocks carry the team's ceilings from `tools/rnd.py`: the Engine Power deltas scaled to the supplier's bhp (× bhp × 745.7 / 53988), the fuel-consumption multipliers at 12/15 (works) or 8/15 (customer) of their stock effect, and the weight-reduction mass set to the team's weight target.
- Engine sounds (`db/audio.json`, `tools/build_audio.py`, `tools/audio.py`): a car's engine sound is the `audio.audio_car_data.<switchName>` row named by the AudioF1Car `switchName` in its carmodel layer. The car pack gives each 1990 car its own label (`f1_m_<name>_90`, the donor's with `_90` for `_26`; Larrousse `f1_m_larrouss_90`, Dallara `f1_m_dall_90`), and `db/audio.json` adds those 13 rows (put-record, the donor's values) with `bundle_name` `v12_1990.bdl` (Ferrari, Lotus, Larrousse), `v10_1990.bdl` (McLaren, Williams), `v8j_1990.bdl` (Leyton House, Brabham) or `v8_1990.bdl` (the Ford teams), plus four `audio.audio_bundle_settings` rows for those bundles. `operations.files` adds the four `.bdl` files from the F1 25 Engine Sound Pack under those new names. No stock audio row or bundle changes.
- B-spec cars: every team also has `<token>_1990b` (package `f1_1990b`, folder `f1_1990_vehicle_b_specs`, stat_id A + 13 = 526–538), a full copy of its A row with the same car folder, so the same AI data, handling archive and common.erp; only the model archive differs. The B rows and the `f1_1990b` package carry `available True`: the startup vehicle loader (0x142505f30) only registers available vehicles, and the content pass (0x1413e4cf0) makes available only the vehicles an entrant list names, which no B car is. `native/f1_1990_bspec.dll` (option `bspec_cars`) moves a team's career entrant rows to the B vehicle once its R&D upgrade `f1990_bspec*` is Completed, checked on career load and every hub visit, 1990 careers only (`/mnt/work/.f1-1990/addon/native-requests.md` N11, `/mnt/work/.f1-1990/v2/rnd-bspec-interface.md`).

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
| mode row | `driver_career_1990` (statId 81, save container `driver_career_1990`, game setup modes `f1_1990`) | `driver_career_26` |
| championship | `F1_driver_career_1990` (statId 35, setup mode `f1_1990`, entrant list and setup data `f1_career_1990`) | `F1_driver_career_26` |
| event list | `F1_career_1990`: one season, five season events (no New Season Grid Reveal) | `F1_career_26` |
| track list | `F1_1990`: the 16 rounds in order (Hockenheim on Shanghai) | `F1_2026_16` |
| season lengths | `F1_1990_Long`, `_Medium`, `_Short`, all 16 rounds, no transfer window, setup mode `f1_1990` | `F1_2026_*` |
| track selection | `F1_1990` | `F1_2026` |

Content set `2026` lists `driver_career_1990` and `f1_1990`. The game registers a game setup mode only when the
content set lists it and an available mode row names it in `game_setup_modes`; no stock mode row names `f1_1990`, so the
1990 mode row does. Without it the 1990 championship has no setup mode, every 1990 team fails the championship's team
check and starting the career crashes. The career data group is the key the career slot list filters on, so 1990
careers have their own slots. The **Career year** setting starts the career in 1990 (default) or
2026.

The Choose Driver screen finds its F1, F2 and Icons drivers through the content set's official entrant lists named
`F1CAREER_<suffix>`, `F2CAREER_<suffix>` and `ICONS_<suffix>`, with the suffix of the Career tile that opened the wizard
(`25`, `26`, and `90` for the 1990 tile). Content set `2026` gains `F1CAREER_90` (`f1_1990`), `F2CAREER_90` (the 2026
F2 list; the 1990 wizard has no F2 start) and `ICONS_90` (`f1_1990_driver_pool`); the stock rows stay as they are.

The 1990 wizard has no F2 start, team, season length, driver icons or driver type choice (`native/f1_1990_wizard.dll`,
see `native/README.md`). `tools/wizard.py --game <F1 25>` writes `erp/wizard.json`: the route
`career_wizard_driver_career_1990`, a copy of the content select's `career_wizard_driver_career_to_f1` that opens the
settings step instead of the season length step, and the copies that take Choose Driver's back to the settings step
and the settings step's back to the Career tiles. Only the 1990 wizard fires them; stock routes stay as they are.

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

## R&D (db/rnd.json, db/rnd-lang.json, data/rnd-strings.json, data/rnd-art.json, tools/rnd.py, tools/check_rnd.py)

A 1990 career reads its R&D from `upgrades.1990`; 2025 and 2026 careers keep `upgrades.2026` and their rows stay stock.
The path is the `upgrades_path` of the career's ComUpgrades component, so the 1990 championship lists its own shared
component set, `shared_driver_career_1990_components`: ComUpgrades on `upgrades.1990`, ComResourcePoints and
ComAccolades on `f1_career_1990`, ComSchedule on `F1_career_1990`. Under `upgrades.1990` the engine supplier trees,
delivery schedules, regulation change schedules and R&D challenges are copies of the 2026 tables.

| Part | Rows |
|---|---|
| Trees | `trees.<token>` for the 12 tokens the game knows; `teams_linked_to_0.links` names each team's `_1990` and `_1990b` vehicles (Lotus and Larrousse share `redbull`) |
| Departments | Aerodynamics 6 upgrades, Chassis 6 (with `f1990_bspec_package`, the B-spec milestone), Engine 7 for the works teams (McLaren, Ferrari, Williams, Benetton) or 5, Durability 6. No DRS, ERS or turbo rows |
| Economy | 400 / 800 / 1400 RP, 7+7 / 14+10 / 24+14 days of research and development, no facility levels |
| Metadata | per team: the engine supplier's Engine Power ceiling (24 to 75 bhp), the weight target towards 500 kg, fuel consumption, a drag cost on downforce |
| Resource points | the `f1_career_1990` team rows, by the season pack's reputation: McLaren 1610 + 420 a week down to Ligier 750 + 220 |
| Text | `lng_rnd_f1990_*` names and descriptions; `data/rnd-strings.json` holds the 1990 department names on stock keys for the runtime string override; `data/rnd-art.json` maps each part to a stock card frame |

The qualifying line (`f1990_quali_rev_limit`, `f1990_quali_engine`) needs native request N12 to count only in
qualifying; without it, it is ordinary engine power. Design and numbers: `/mnt/work/.f1-1990/v2/rnd-design.md`.

## R&D look (erp/rnd_look.json, erp/rnd_look/rnd_look.erpk, art/rnd/, data/rnd-look.json, tools/rnd_look.py)

In 1990 careers each R&D department has a 1990 colour and a period icon; 2025 and 2026 careers keep the stock look.
Option `rnd_look_1990` (default on).

| department (code) | colour | icon (`art/rnd/`) |
|---|---|---|
| Aerodynamics (0) | Rothmans / Tyrrell blue `#2F74E8` | `aero.svg`: car front with the raised nose and anhedral wing |
| Chassis & Suspension (1) | Benetton green `#22A45D` | `chassis.svg`: pushrod coil-over on the carbon tub |
| Reliability & Gearbox (2) | Camel / Lotus yellow `#F6C21C` | `reliability.svg`: gear with an H-pattern gate and a spanner |
| Engine (3) | Ferrari / Marlboro red `#E2231A` | `engine.svg`: V engine with intake trumpets |
| Tyres (4, rnd-tyres-thermal) | orange `#FF7800`, rnd-tyres' own | `tyres.svg`: tall-sidewall slick on a 13-inch rim |

`erp/rnd_look.json` adds the uidb package `overrides.career.f1990_rnd`, a clone of the stock `rnd_om_xm` package whose
`rnd_dept_icons_small` (48 px white, alpha 255 + 128), `rnd_wheel_icon` (60 px coloured), `rnd_department_strips`
(1112 x 4) and `dept_card_icons` (148 px coloured with a glow) use new texture arrays: aero, chassis, maintenance,
engine, tyres and error_department, plus the stock regulation and shutdown wheel icons. The arrays are new resources in
`flow_career_hub.erp`, `flow_r_and_d.erp` and `flow_team_hq.erp` (where the stock ones are), so no stock array, list or
slice changes and nothing collides with arrays rnd-tyres-thermal grows. New resources change the archives' layout, so
the build writes those archives into the game files. `native/f1_1990_rndlook.dll` points the department atlases at
the package and writes the colours, only while a 1990 career is loaded (`native/README.md`).

```sh
python3 addons/f1-1990/tools/rnd_look.py --game <F1 25> [--baseline <launcher baseline>/<build>] [--preview <png>]
```

It needs Pillow, numpy, resvg (or nix), zstd and node. rnd_look.json must run before rnd-tyres-thermal's uidb edits,
which its `load_after` on this addon gives.

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
