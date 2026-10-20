# F1 1990 native DLL

`f1_1990.dll` gives the 1990 profile its own profile save, signs the first 1990 Driver Career contract without the negotiation scene, and sets the race grid from each driver's best lap in either 1990 qualifying session: Friday (SprintShootout group, 2) and Saturday (Qualifying group, 1). It loads at the early stage, before the game loads its profile.

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

## Contract skip

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

The career-lane bindings were raised to L2 with `sitereg.py --force` (lane owner career-re) with the developer's approval on 2026-10-21; they still need an `agree` verify row from another agent.

## Friday qualifying tyres (`f1_1990_tyres.dll`)

A separate DLL, so a binding it can't resolve drops only this hook and never the save separation. `Sess_TyreAllocation_OnSessionStart` (0x140bcc190, the tyre allocation component's session-start virtual) is wrapped: after it runs for a sprint-shootout session (ids 10..14) on a 1990 weekend, every unused, undamaged dry set (softer, medium, harder; no wear, not used) of every car gets its handed-back flag (set +0x09 bit 3) cleared and its session tag (+0x00) set to 0, and the allocation is marked dirty (+0xe8). A tag-0 set passes the game's "usable now" check (0x142e88630/0x142e88730) in every session, which otherwise has no case for shootout sessions and leaves only inters and wets. Friday and Saturday qualifying then share the unused dry sets. It logs `[f1-1990] FQ tyres: session <id>: <n> unused dry set(s) made usable on <cars> car allocation(s)`.

Option `fq_tyre_fix` (toggle, default on) turns it off. With `no_tyre_allocation` the allocation is off and the usable check passes anyway; the hook then changes nothing that matters.

## Replay state holders (`f1_1990_replay.dll`)

A separate DLL, so a binding it can't resolve drops only this hook and never the save separation. The replay/flashback recorder (`Ne_Replay_WriteFrames`, 0x1445d0e70) keeps a state holder per recordable: a heap block with a 0x80-byte header, a current and a previous state buffer. Its class (one of 20, capacity 0x20 to 0xffff) is fixed at registration. Each frame the recordable serializes into a static scratch buffer and the recorder copies the state into the holder with no capacity check. At 26 cars, at the race start, a per-car recordable outgrows its holder; the copy then overwrites the next heap block (seen: the tyre system's wheel map table, which aborts the race start with heap error 0x16 when the table is freed).

`site_Ne_ReplayAfterRecord` (0x1445d1712, after the recordable serialized, before both copies) is a site hook. When the state is larger than the holder's capacity, the holder is replaced by one of the smallest class that fits. The new holder comes from the registry's allocator; the header and both buffers are copied; the registry slot and the recorder's cached pointers move to it. The old holder stays allocated. The class table is checked against the exe's code once; a mismatch turns the hook off. It logs `[f1-1990] replay holders: <recordable> state <n> bytes > capacity <c>; holder <old> -> <new> (capacity <c>)`. A state that fits returns at the first compare, so a 22-car grid is untouched.

Option `replay_holder_fix` (toggle, default on) turns it off.
