# F1 1990 native DLL

`f1_1990.dll` signs the first 1990 Driver Career contract without the negotiation scene, sets the race grid from each driver's best lap in either 1990 qualifying session (Friday: SprintShootout group, 2; Saturday: Qualifying group, 1) and, with `dedicated_profile_saves` on, gives the profile its own profile save. It loads at the early stage, before the game loads its profile. Every hook acts on 1990 content only: the grid and tyre hooks on the 1990 weekend structure (shootout and qualifying groups, no `race_2`), which only the `f1_1990` setup mode has, and the contract skip on a career whose data group is `f1_career_1990`, so 2025 and 2026 careers and sessions in the same profile stay stock.

## Save separation (`dedicated_profile_saves`, off by default)

Only for a profile that should keep its whole game profile apart. Without it, 1990 careers live in the shared profile save in their own slots (data group `f1_career_1990`).


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

`src/contract_skip.cpp` signs the player's first 1990 Driver Career contract and passes the negotiation scene (native request N1). It runs only for a career whose data group is `f1_career_1990` (read like the save-slot predicates 0x1438fb390/0x1438fb5d0: each championship's CareerSetupComponent data-group index into its group list), with one human player.

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

## 1990 Career tile (`f1_1990_ui.dll`)

A separate DLL, loaded at the window stage after the others; option `career_menu_ui` (default on), and only when the 1990 career content is in the database (`src/profile.hpp`). The 2025 and 2026 tiles stay stock.

| binding | address | use |
|---|---|---|
| `site_FeTile_CareerContentEnd` | 0x141741831 | site at the end of `FeTile_Build_List` case 6 (r14 = the list): `FeHome_Add_CareerTile(list, 3)` appends `button_3`, then its home item is replaced by a content item |
| `FeTile_Construct_CareerItem` | 0x141725380 | call; builds the 0x1c8 content item with the 2026 tile's route id |
| `FeTile_CareerDlcItem_Populate` | 0x141726680 | called by our populate; then title `lng_f1990_career_content`, description `lng_f1990_career_content_description`, notification 0 (no badge) |
| `FeTile_Route_CareerItem` | 0x141725620 | called by our press after it writes the selected content |
| `g_FeTile_CustomisationSystem` / `g_FeTile_SelectedCareerContent` | 0x146064df0 / 0x146066c42 | the press selects the car group with stat id 2 (`1990`), else the first dlc group (2026) |

The item carries data group `f1_career_1990` (+0x180), suffix `90` (+0x188), is_dlc 0 (+0x190) and the 2026 entitlement string (+0x1c0); its vtable is a copy of the 2026 tile's with populate and press replaced. The router and the wizard's use of the data group are not traced yet (see `/mnt/work/.f1-1990/v2/third-tile.md`).

## Flavio in 1990 careers (`f1_1990_voice.dll`)

A separate DLL; option `agent_voice_mute` (default on). `Game_SpeechSystem_PostLine` (0x140a30b60) is wrapped: while the loaded career's data group is `f1_career_1990` (worked out on the core's `career.loaded` event, cleared on `career.unloaded`), a line whose speaker is `agent` or `agent_cutscene` gets its emitter's `vol_speech` RTPC set to 0 (emitter vtable +0x60, the setter the post uses for `random_number_generator`) before it is posted as usual. The line still plays, so its subtitles and the end-of-event callbacks that finish it (0x140ac1160/0x140ac11a0) run; a line that was not posted would never finish. 2025 and 2026 careers keep the agent's voice, and the addon no longer replaces `en_agent*.nfs`.

It logs `[f1-1990] voice: career loaded (data group f1_career_1990); agent voice muted`, `... agent line muted (emitter ...)` and, if a muted emitter also speaks for another speaker, `... that line is silent too`.

The same DLL switches the agent's strings. `data/flavio-strings.json` (the career lane's `tools/career.py`; shipped next to the DLL through the manifest entry's `files`) holds, per game language, every stock string that names Elena rewritten for Flavio. On the first 1990 career load the DLL reads it through the core's storage API, then sets each text for its own language with the core's runtime string overrides (`swofty/lang.hpp`, API v7); `career.unloaded`, or a non-1990 `career.loaded`, clears exactly those overrides, so other careers and modes read the stock tables. It does not call `refresh()`. Built against an SDK below v7 the code is compiled out, and with a core below v7 `swofty::lang::available()` is false; either way the agent keeps Elena's strings.

It logs `[f1-1990] strings: set 1858 agent string(s) in 11 language(s) (0 failed)` on a 1990 career load, `... cleared ...` on unload, and otherwise why the strings stay stock (`built against SDK API v6`, `the core has no runtime string overrides`, or a read or parse error).
