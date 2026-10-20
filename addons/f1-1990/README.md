# F1 1990

Turns the F1 2026 season into the 1990 season in a profile of its own.

## Rules (db/rules.json, db/rules_text.json, tools/rules.py)

`tools/rules.py` writes both op files. In the `f1_26` setup mode:

- Weekend (default structure): P1, P2, Friday qualifying (a single sprint-shootout session), Saturday qualifying (a single qualifying session), race. Full qualifying runs the single sessions; the race grid comes from Saturday qualifying. Every 2026 track runs the default format, and Grand Prix only offers the standard weekend.
- Tyres: every allocation preset hands out `soft_sets` softer-compound sets in P1, P2, Friday qualifying, Saturday qualifying and the race, and no medium or hard sets. The softer compound is C4 (`sup`) on every 1990 calendar track and shows as Supersoft (`SuperSoft` sidewall context). Mandatory-tyre presets are empty, the calendar tracks require 0 pit stops, and there are no tyre warmers.
- Race control: safety car off and parc fermé off, as defaults and as the only values the `gamesetup_f1` tag allows. Red flags stay. DRS uses the `f1_1990` lap group, which opens on lap 200.

## Cars (db/physics.json, erp/cars/, files/audio/, tools/build_cars.py)

`tools/build_cars.py` writes the handling files and both op files from the stock 2026 team archives and the season pack.

| Car folder | 1990 team(s) | Engine | Power | Mass |
|---|---|---|---|---|
| ferrari | Ferrari | V12, 13,500 rpm | 702 hp | 529 kg |
| mclaren | McLaren | V10, 13,300 rpm | 669 hp | 558 kg |
| williams | Williams, Larrousse (T12) | V10, 13,300 rpm | 658 hp | 568 kg |
| lotus | Benetton | V8, 12,500 rpm | 657 hp | 569 kg |
| redbull | Lotus | V12, 13,500 rpm | 653 hp | 572 kg |
| mercedes | Tyrrell | V8, 12,500 rpm | 635 hp | 588 kg |
| toro_rosso | Minardi | V8, 12,500 rpm | 629 hp | 593 kg |
| force_india | Leyton House | V8, 12,500 rpm | 627 hp | 595 kg |
| sauber | Ligier | V8, 12,500 rpm | 627 hp | 595 kg |
| gm_cadillac | Brabham | V8, 12,500 rpm | 619 hp | 603 kg |
| haas | Arrows, Dallara (T13) | V8, 12,500 rpm | 618 hp | 604 kg |

- Handling: no `EnergyRecoverySystem` section and no ERS parts in the R&D upgrade blocks; naturally aspirated engine (`TurboMode Off`); power = 650 hp × the team's power scalar, mass = 575 kg × its weight scalar, drag = the stock car's drag × its drag scalar; teams sharing a car folder get the mean of their scalars. Downforce, tyres, suspension and brakes stay stock. The final drive keeps the stock top-gear speed at the rev limit; fuel use per second at full power stays stock. No moving front wing.
- Database: `hud_dial` (option, default `f2`) and `drs_capable False` on the 13 vehicle rows; `is_26_aero_regulations False` on the `f1_2026` package (option `classic_aero`); every power-unit component in both component sets gets allocation 99, no grid drops and no wear.
- Option `ers`: `removed` (default) or `zeroed`, which keeps the ERS section with no harvest, deploy, MGU-K braking or override.
- Engine sounds: the V12, V10 and V8 bundles of the F1 25 Engine Sound Pack replace `ferrari_2026.bdl`, `renault_2026.bdl` and `ford_2026.bdl` (V8 also `mercedes_2026.bdl`), and each 2026 team's `f1_m_<team>_26.bundle_name` points at its 1990 engine. T12 takes the Lotus row's `audio_id`, so Larrousse plays the V12.
- Grand Prix with equal performance on uses the `f1_2026_equal_performance` cars, which keep ERS.

```sh
python3 addons/f1-1990/tools/build_cars.py --game "<F1 25>"          # compare with the shipped files
python3 addons/f1-1990/tools/build_cars.py --game "<F1 25>" --write  # replace them
```

The stock 2026 team archives and game database must be the stock files.

## Driver Career (db/career.json, db/career-lang.json, tools/career.py)

Driver Career 2026 is the 1990 career: one season of the 16 1990 rounds in order (Hockenheim on Shanghai until a
Hockenheim track is added), no sprints, no transfer window, 9-6-4-3-2-1 points, and the agent is Flavio Briatore.
Its careers save in their own `driver_career_1990` container, apart from 2026 careers.
The **Career year** setting starts the career in 1990 (default) or 2026.
The season has no New Season Grid Reveal event; the reveal crashes the game with 13 teams.

The agent's English speech banks are replaced with silent copies of the same length (`files/speech/`), so her lines
show subtitles with no voice; `tools/mute_agent.py <F1 25> <out> [language ...]` writes them for any language. The
contract scene at the start of a career still shows until the native contract hook lands.

`tools/career.py <dir>` writes `db/career.json` and `db/career-lang.json`; `<dir>` holds the stock language tables
as `language_<code>.tsv` (key, tab, value, with `\n` escaped).
