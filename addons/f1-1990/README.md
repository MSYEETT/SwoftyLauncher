# F1 1990

Turns the F1 2026 season into the 1990 season in a profile of its own.

## Grid size

The **Cars** setting (`cars`) is 22 (default) or 26. At 22 the grid is the 11 teams on the stock 2026 team rows and the addon needs no other addon. At 26 Larrousse and Dallara join on Custom Grid Size's two extra teams: enable Custom Grid Size with Cars at 26. The grid ops are in `db/grid.json`, written by `tools/grid.py` (see `tools/README.md`).

## Front end (erp/frontend.json, erp/badges.json, tools/frontend.py, tools/badges.py)

The Driver Select and Team Select backdrops show the 1990 driver and team names, from textures `tools/frontend.py` renders over the stock ones in `main_front_end.erp`. The team logo arrays (team select tiles, driver picker, timing tower, broadcast graphics, F1 World) carry placeholder 1990 badges from `tools/badges.py` (`erp/badges.json`, `erp/badges/`). **Hide the F2 and Icons tabs** (`hide_f2_icon_tabs`, default on) removes those two tabs from the Choose Driver screen by replacing `screen_groups.uicontentgroup` in `persistent.erp`, which the game loads at boot; turn it off if the game stops at boot.

## Rules (db/rules.json, db/rules_text.json, tools/rules.py)

`tools/rules.py` writes both op files. In the `f1_26` setup mode:

- Weekend (default structure): P1, P2, Friday qualifying (a single sprint-shootout session), Saturday qualifying (a single qualifying session), race. Full qualifying runs the single sessions; the race grid comes from Saturday qualifying. Every 2026 track runs the default format, and Grand Prix only offers the standard weekend.
- Tyres: every allocation preset hands out `soft_sets` softer-compound sets in P1, P2, Friday qualifying, Saturday qualifying and the race, and no medium or hard sets. On every 2026 track the softer, medium and harder compounds are all C4 (`sup`), and the three 2026 dry visuals show as Supersoft (`SuperSoft` sidewall context, red icon), so the track card lists one Supersoft C4. The mandatory-tyre presets only ask for a softer set in the shootout sessions (Friday qualifying), every track requires 0 pit stops, and there are no tyre warmers.
- Race control: safety car off and parc fermé off, as defaults and as the only values the `gamesetup_f1` tag allows. Red flags stay selectable, also in short races and with the safety car off. The grid cutscene and the grid rundown, which show the safety car, are off unless `grid_cutscene` or `grid_rundown` is on. DRS uses the `f1_1990` lap group, which opens on lap 200.

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
