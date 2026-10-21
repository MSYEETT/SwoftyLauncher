# F1 1990

Turns the F1 2026 season into the 1990 season in a profile of its own.

## Grid size

The **Cars** setting (`cars`) is 22 (default) or 26. At 22 the grid is the 11 teams on the stock 2026 team rows and the addon needs no other addon. At 26 Larrousse and Dallara join on Custom Grid Size's two extra teams: enable Custom Grid Size with Cars at 26. The grid ops are in `db/grid.json`, written by `tools/grid.py` (see `tools/README.md`).

## Front end (erp/frontend.json, erp/frontend/, tools/frontend.py)

The Driver Select and Team Select backdrops show the 1990 driver and team names, from textures `tools/frontend.py` renders over the stock ones in `main_front_end.erp`. **Hide the F2 and Icons tabs** (`hide_f2_icon_tabs`, default on) removes those two tabs from the Choose Driver screen by replacing `screen_groups.uicontentgroup` in `persistent.erp`, which the game loads at boot; turn it off if the game stops at boot.

## Rules (db/rules.json, db/rules_text.json, tools/rules.py)

`tools/rules.py` writes both op files. In the `f1_26` setup mode:

- Weekend (default structure): P1, P2, Friday qualifying (a single sprint-shootout session), Saturday qualifying (a single qualifying session), race. Full qualifying runs the single sessions; the race grid comes from Saturday qualifying. Every 2026 track runs the default format, and Grand Prix only offers the standard weekend.
- Tyres: every allocation preset hands out `soft_sets` softer-compound sets in P1, P2, Friday qualifying, Saturday qualifying and the race, and no medium or hard sets. On every 2026 track the softer, medium and harder compounds are all C4 (`sup`), and the three 2026 dry visuals show as Supersoft (`SuperSoft` sidewall context, red icon), so the track card lists one Supersoft C4. The mandatory-tyre presets only ask for a softer set in the shootout sessions (Friday qualifying), every track requires 0 pit stops, and there are no tyre warmers.
- Race control: safety car off and parc fermé off, as defaults and as the only values the `gamesetup_f1` tag allows. Red flags stay selectable, also in short races and with the safety car off. The grid cutscene and the grid rundown, which show the safety car, are off unless `grid_cutscene` or `grid_rundown` is on. DRS uses the `f1_1990` lap group, which opens on lap 200.

## Cars (db/cars.json, lang/cars.json, cars/, tools/build_cars.py, tools/cars.py)

The 1990 cars live in their own vehicle package, `f1_1990` (stat_id 59). Its `resource_path` is `f1_1990_vehicle_package/teams/[FILENAME]/wep/[FILENAME]` and its `ai_package_folder` stays `f1_2026_vehicle_package`, so the per-track AI data of the 2026 car folders serves the 1990 cars. No 2026 archive, row or file changes. The models, the package's `teams/common.erp` (13-inch tyres) and its tyre compounds come from the private car pack `swofty.f1-1990-cars`, which registers one asset group per car at runtime. Without the car pack, a session with a 1990 vehicle hangs on load.

| Vehicle | Car folder (AI data) | Chassis | Engine | Power | Mass |
|---|---|---|---|---|---|
| mclaren_1990 | mclaren | McLaren MP4/5B | Honda V10, 13,300 rpm | 669 hp | 558 kg |
| ferrari_1990 | ferrari | Ferrari 641 | V12, 13,500 rpm | 702 hp | 529 kg |
| williams_1990 | williams | Williams FW13B | Renault V10 | 655 hp | 571 kg |
| benetton_1990 | lotus | Benetton B190 | Ford V8, 12,500 rpm | 657 hp | 569 kg |
| tyrrell_1990 | mercedes | Tyrrell 019 | Ford V8 | 635 hp | 588 kg |
| lotus_1990 | redbull | Lotus 102 | Lamborghini V12 | 653 hp | 572 kg |
| march_1990 | force_india | Leyton House CG901 | Judd V8 | 627 hp | 595 kg |
| larrousse_1990 | myteam_ferrari | Lola LC90 | Lamborghini V12 | 661 hp | 565 kg |
| ligier_prost_1990 | sauber | Ligier JS33B | Ford V8 | 627 hp | 595 kg |
| minardi_1990 | toro_rosso | Minardi M190 | Ford V8 | 629 hp | 593 kg |
| brabham_1990 | gm_cadillac | Brabham BT59 | Judd V8 | 619 hp | 603 kg |
| dallara_1990 | myteam_honda | Dallara F190 | Ford V8 | 615 hp | 606 kg |
| arrows_1990 | haas | Arrows A11B | Ford V8 | 621 hp | 601 kg |

- Vehicle rows: full copies (246 keys) of the donor folder's 2026 row, stat_id 513–525 (Custom Grid Size uses 509–512), `vehicle_package f1_1990`, `hud_dial` from the option (default `f2`), `drs_capable False`, `equal_performance_vehicle` empty, chassis and engine names as `lng_f1990_chassis_<team>` / `lng_f1990_engine_<team>` (`lang/cars.json`). The engine id stays the donor's.
- Package row: a copy of `f1_2026` with the fields above, `has_cockpit_halo False`, the 2026 animation, crew, garage, pit-stop and pit-wall groups, and `is_26_aero_regulations False` when `classic_aero` is on.
- Handling: each car's handling file is built from its donor's stock 2026 handling file and shipped as `cars/handling/<ers>_<tyre_size>/<folder>_handling.erp`; `cars/handling.json` puts the one matching the options at `f1_1990_vehicle_package/teams/<folder>/wep/<folder>_handling.erp`, which the car's asset group lists. No EnergyRecoverySystem section (or zeroed, option `ers`), naturally aspirated engine, power = 650 hp × the team's power scalar, mass = 575 kg × its weight scalar, drag × its drag scalar, no moving front wing, no DRS hardware, 13-inch tyres (TyreRadius 0.323/0.330, AlloyRadius 0.182, damage graphs remapped, final drive scaled); `13in_raised` adds the radius drop to the ride heights.
- Engine sounds stay stock. The engine sound comes from a team's audio row (`f1_m_<team>_26.bundle_name`) in a table whose path is unknown (hashed), so a 1990-only sound needs new audio rows the build can't add yet (see `/mnt/work/.f1-1990/v2/cars-package.md`). The V12/V10/V8 bundles stay in `files/audio/` for that.

```sh
python3 addons/f1-1990/tools/build_cars.py --game "<pristine F1 25 copy>"          # compare with the shipped files
python3 addons/f1-1990/tools/build_cars.py --game "<pristine F1 25 copy>" --write  # replace them
```

`--game` must hold stock 2026 team archives: a game folder with a test build installed carries modified handling files. `/mnt/work/.f1-1990/data-physics/fakegame-full` is a pristine copy.

## Driver Career (db/career.json, db/career-lang.json, tools/career.py)

Driver Career 2026 is the 1990 career: one season of the 16 1990 rounds in order (Hockenheim on Shanghai until a
Hockenheim track is added), no sprints, no transfer window, 9-6-4-3-2-1 points, and the agent is Flavio Briatore.
The mode row's save container is `driver_career_1990`, but that container sits inside the one profile save
(`savegame@profile#...`) with every other career, so the 1990 profile lists and can load the real 2026 careers. A
separate save file needs the native save hook (N2: the profile save named `f1990`); until it lands, back up saves before
playing a career in this profile.
The **Career year** setting starts the career in 1990 (default) or 2026.
The Career menu's 2026 tile reads "1990 Career" (`lng_career_dlc_content` and its description). The 2025 tile and the
26 badge stay; hiding them needs the native request N6.
The season has no New Season Grid Reveal event; the reveal crashes the game with 13 teams.
The season-length screen lists three "1990 Season" entries, the `f1_26` Long, Medium and Short rows, all with the same
16 rounds. The game needs all three rows: the career activity report (0x143867ff0) reads the Short and Medium lengths
and crashes on a missing row.

The agent's English speech banks are replaced with silent copies of the same length (`files/speech/`), so her lines
show subtitles with no voice; `tools/mute_agent.py <F1 25> <out> [language ...]` writes them for any language. The
contract scene at the start of a career still shows until the native contract hook lands.

`tools/career.py <dir>` writes `db/career.json` and `db/career-lang.json`; `<dir>` holds the stock language tables
as `language_<code>.tsv` (key, tab, value, with `\n` escaped).
