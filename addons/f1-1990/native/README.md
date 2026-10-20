# F1 1990 native DLL

`f1_1990.dll` gives the 1990 profile its own profile save and sets the race grid from each driver's best lap in either 1990 qualifying session: Friday (SprintShootout group, 2) and Saturday (Qualifying group, 1). It loads at the early stage, before the game loads its profile.

## Save separation

Every file of a save is named from the save operation's name: Steam remote storage names are `<title dir>@<save name>#<file>` (also `<save name>@...` and `<save name>_backup#...`), and the file names inside come from the save job (profile: index `QKRHMYXE`, data `NXDSMWW.BWW`). The game picks the profile save from the enumerated save list, not by name. So:

| binding | address | use |
|---|---|---|
| `Ne_Request_SaveOpKind1` / `Ne_Request_SaveOpKind2` | 0x144138aa0 / 0x1441391a0 | wrap; a save operation named `profile` is queued as `f1990`, so every file of the profile save (index, data, backups) is `savegame@f1990#*` |
| `Ne_SaveSystem2Win_Start` | 0x14413af70 | wrap; before the device starts, the ISteamRemoteStorage it caches (helper +0x108, used only to list remote files) is replaced by a proxy that hides `savegame@profile*` and `savegame@profile_backup*` from `GetFileCount`/`GetFileNameAndSize` and forwards every other slot unchanged |

The first launch in the profile finds no profile save and starts a new game profile; the stock files are never listed, opened or written. Each renamed operation logs `[f1-1990] save: profile -> f1990 (#n)`; the proxy logs `save list enumeration filtered` and, once, how many stock files it hid. A failed init logs `SAVE SEPARATION OFF`, and a device without the Steam helper (local saves) logs `SAVE SEPARATION INCOMPLETE`, three times each.

`cleanup_stray_save` (toggle, default off) deletes `savegame@profile#D1990.UEL`, left in Steam Cloud by the first build, through the real interface's `FileExists`/`FileDelete` at the next start, and logs the result. Turn it off again afterwards.

## Grid hooks

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

## Contract skip (not in the default build)

`src/contract_skip.cpp` signs the player's first 1990 Driver Career contract and passes the negotiation scene (native request N1). It runs only on a database where a `game_modes.ModeChampionship` row saves to `driver_career_1990` or runs `F1_career_1990`, with one human player.

| binding | address | use |
|---|---|---|
| `Career_Decide_ContractNegotiation` | 0x142d57d10 | wrap; on link 0x5e3c3c2d with `Career_Needs_DriverContract`, build the contract (current team, type 0, 1 season), rebuild it at the recommended target and sign it |
| `Career_Build_DriverContract` | 0x140d79dd0 | call |
| `Career_Sign_DriverContract` | 0x140d79ff0 | call (`replaced_key` 0x3fff0000, notify 1) |
| `Career_Needs_DriverContract` | 0x140d83090 | call |
| `Career_ContractIntro_OnEnter` / `_OnUpdate` | 0x1436c2620 / 0x1436c2dc0 | wrap; enter skipped, update returns the continue link |
| `Career_ContractUpdate_OnEnter` / `_OnUpdate` / `_OnExit` | 0x1436c1760 / 0x1436c1e00 / 0x1436c1fc0 | wrap; enter idles without the screen, update continues, exit returns 1 without closing a screen that never opened |
| `Career_ContractOutro_OnEnter` / `_OnUpdate` | 0x1436c3330 / 0x1436c3750 | wrap; enter skipped, update returns the continue link |
| `g_Career_FlowLinkContinue` | 0x146cb2284 | data (continue link, 0x7c9b1ec4 in logs) |
| `k_Career_DriverCareerContractsManagerComponentDesc` | 0x14605c0d8 | data (existing binding) |

The recommended target is the screen's: rating + the matching rating range's offset, inside the slider and at most 100 (`src/contract_target.hpp`, tested by `test/run.sh`).

None of these bindings except the component descriptor is in the KB yet: they belong to the `career` lane (owner career-re), whose owner has to register them before `CONTRACT_SKIP=1 ./build.sh` compiles.
