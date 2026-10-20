# F1 1990 native DLL

`f1_1990.dll` sets the race grid from each driver's best lap in either 1990 qualifying session: Friday (SprintShootout group, 2) and Saturday (Qualifying group, 1).

## Hooks

| binding | address | use |
|---|---|---|
| `Sess_Build_GroupResult` | 0x140afa220 | wrap; after a group's rows are built (last session of the group, or a save load) |
| `Sess_Record_GroupTimes` | 0x140af2880 | wrap; after a group's per-driver best laps are recorded (every qualifying-type session, driven or simulated) |
| `Sess_Find_GroupRowPosition` | 0x140af8aa0 | call; driver key to the group-1 row position |

After either wrap the group-1 row positions (`row+0x50`) are rewritten: drivers with a time are ranked by min(Friday, Saturday); drivers without one follow in Saturday order. The race grid (`grid_type` qualifying) reads those positions. A weekend is treated as 1990 when it has both a SprintShootout and a Qualifying group session and no `race_2` session; other weekends are untouched.

Log lines (swofty log): `[f1-1990] GRID best-of (<why>)` once per pass, then one `[f1-1990] GRID driver <kind>/<stat id> fri <s> sat <s> best <s>: position <old> -> <new>` per driver.

## Build

`./build.sh` generates dev bindings (`re/tools/bindgen.py --dev`) into `build/dev` and builds `f1_1990.dll` with zig. The bindings are L2 without a verify row, so the core only resolves them with `build/dev/bindings.json` installed as the game's `swofty/bindings.json`.

`test/run.sh` runs the ranking unit tests on the host.
