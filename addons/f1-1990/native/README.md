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
| `FeTile_Route_CareerItem` | 0x141725620 | called by our press after it writes the selected content; wrapped to record whether the last career tile pressed was the 1990 one (exported as `F1990_CareerWizard1990` for `f1_1990_tabs.dll`) |
| `g_FeTile_CustomisationSystem` / `g_FeTile_SelectedCareerContent` | 0x146064df0 / 0x146066c42 | the press selects the car group with stat id 2 (`1990`), else the first dlc group (2026) |

The item carries data group `f1_career_1990` (+0x180), suffix `90` (+0x188), is_dlc 0 (+0x190) and the 2026 entitlement string (+0x1c0); its vtable is a copy of the 2026 tile's with populate and press replaced. The router and the wizard's use of the data group are not traced yet (see `/mnt/work/.f1-1990/v2/third-tile.md`).

## Flavio in 1990 careers (`f1_1990_voice.dll`)

A separate DLL; option `agent_voice_mute` (default on). `Game_SpeechSystem_PostLine` (0x140a30b60) is wrapped: while the loaded career's data group is `f1_career_1990` (worked out on the core's `career.loaded` event, cleared on `career.unloaded`), a line whose speaker is `agent` or `agent_cutscene` gets its emitter's `vol_speech` RTPC set to 0 (emitter vtable +0x60, the setter the post uses for `random_number_generator`) before it is posted as usual. The line still plays, so its subtitles and the end-of-event callbacks that finish it (0x140ac1160/0x140ac11a0) run; a line that was not posted would never finish. 2025 and 2026 careers keep the agent's voice, and the addon no longer replaces `en_agent*.nfs`.

It logs `[f1-1990] voice: career loaded (data group f1_career_1990); agent voice muted`, `... agent line muted (emitter ...)` and, if a muted emitter also speaks for another speaker, `... that line is silent too`.

The same DLL switches the agent's strings. `data/flavio-strings.json` (the career lane's `tools/career.py`; shipped next to the DLL through the manifest entry's `files`) holds, per game language, every stock string that names Elena rewritten for Flavio. On the first 1990 career load the DLL reads it through the core's storage API, then sets each text for its own language with the core's runtime string overrides (`swofty/lang.hpp`, API v7); `career.unloaded`, or a non-1990 `career.loaded`, clears exactly those overrides, so other careers and modes read the stock tables. It does not call `refresh()`. Built against an SDK below v7 the code is compiled out, and with a core below v7 `swofty::lang::available()` is false; either way the agent keeps Elena's strings.

It logs `[f1-1990] strings: set 1858 agent string(s) in 11 language(s) (0 failed)` on a 1990 career load, `... cleared ...` on unload, and otherwise why the strings stay stock (`built against SDK API v6`, `the core has no runtime string overrides`, or a read or parse error).

## Flavio's character ref (`f1_1990_agent.dll`)

A separate DLL, loaded at the early stage. The UI keeps one VM node per `entrants.characters` row under `character_list`; `Game_EntrantsUi_BuildCharacterRef` (0x1424f1d70) fills it (`stat_id`, `filename`, the names, `job_role`, and the textures `message_avatar` / `message_avatar_race` from `entrants.ui.asset_templates.characters.textures`, `[NAME]` = the row name), and `Game_EntrantsUi_OnCharacterListLoaded` (0x1424bacc0), the `GameCharacterListLoaded` handler, clears the list and builds it from the game's character vector (message +0x10; 0x38-byte rows). The subtitle box, emails and contacts bind these nodes.

While the loaded career's data group is `f1_career_1990`, the builder gets a stand-in record for the `elena` row: the `flavio` row (stat_id 105, `lng_f1990_agent_*`, texture `message_flavio`) with Elena's stat_id. The handler wrap keeps the character vector; when `career.loaded` or `career.unloaded` changes the 1990 state, the DLL calls the handler with it again, so the `elena` node is rebuilt from the stand-in or from her own row. The `message_avatar_race` array has no `message_flavio` slice.

It logs `[f1-1990] agent: character list built from 0x... (N rows, 1990 career ...)` per build (first 8), `agent: 1990 career loaded; character list rebuilt, the elena ref shows flavio` and `agent: 1990 career left; ... shows elena`, and `no flavio row in the character list` when the data side is missing.

## Choose Driver tabs (`f1_1990_tabs.dll`)

A separate DLL. In a 1990 wizard the Choose Driver tab strip shows only the F1 tab, with no F5/F6 hints. The `driver_career_driver_select_header` view model builds its tabs with the shared tab builder 0x144251f40 from the content group `driver_career_driver_select_header` (`header_f1`, `header_selected_team_icon`, `header_f2`, `header_icon`; 0x30-byte entries, tab name at +0x08), tab i from entry i. The screen group widget (`UISG`, 0x144253930) spawns one tab header per entry of the same stock group and binds it to `headers.<name>`, so all four tabs stay in the header: a header without `header_f2` and `header_icon` leaves two unbound tab headers, drawn as empty boxes with their notification diamond. `site_Game_DriverSelectHeader_BuildTabs` (the call at 0x141a591f2) is redirected to record which tabs are F2 and Icons in a 1990 wizard, and `Game_DriverSelectHeader_SetTabState` (0x141a59fe0, `content_visible` and `enabled` of tab N) is wrapped to keep those tabs at 0 and 0, the state the view model gives a tab it doesn't offer, such as the selected team tab. The widget refreshes the hints (0x144252a40) when a tab's `system_data.enabled` changes and hides them while fewer than two tabs are enabled. The wrap also skips an index past the header's tabs. A wizard is a 1990 one when `F1990_CareerWizard1990` (from `f1_1990_ui.dll`) is 1 and no other career is loaded. Other careers keep all four tabs as the game sets them.

It logs `[f1-1990] driver select: 1990 wizard; header tabs mask 0xc off (F2 and Icons)` per build (first 4) and `driver select: tab 2 kept off (asked content 1, enabled 1)` (first 8).

## Power-unit rules (`f1_1990_pu.dll`)

A separate DLL; option `pu_rules_1990` (default on). The career's `EngineManagementComponent` keeps a 0xa60-byte record per driver with seven component slots (record +0x18, 0x10 apart; each holder: fitted components at +0x08, 0x10 each with wear at +0x04, fitted index +0x30, session wear range +0x3c/+0x40). Component configs are 0x250-byte sets per season (`engine_management.components`, read by 0x1411cb150): threshold +0x04, `first_new_grid_drop` +0x18, `subsequent_grid_drop` +0x1c, `parc_ferme_grid_drop` +0x20, `allocation` +0x24, wear ranges +0x28..+0x3c.

| binding | address | use |
|---|---|---|
| `Game_EngineMgmt_ApplySessionWear` | 0x1411d1560 | wrapped: amount 0 for a 1990 car (pending components are still fitted, the wear copied to the car unchanged) |
| `Game_EngineMgmt_AddGridPenalty` | 0x1411d0740 | wrapped: nothing filed for a 1990 car; its only callers are the new-component fit 0x1411d00e0 and the component switch 0x1411cf930 (parc fermé) |

A car is a 1990 car when the driver's career entrant row (driver key low byte = `id_lo` +0x50, high word = `id_hi` +0x52) has a vehicle (+0x5c, stat id in the high word) whose `entrants.vehicles` row has `vehicle_package` `f1_1990` or `f1_1990b`; that list is read on `career.loaded`. Other packages, and every mode without a career, keep the stock rules.

It logs `[f1-1990] power unit: N vehicle(s) of the f1_1990 and f1_1990b packages run without power-unit wear or grid penalties`, then per driver `session wear X skipped` and `N-place grid penalty (reason R) dropped` (first 8 and 16).

## Qualifying engine lines (`f1_1990_quali.dll`)

A separate DLL; option `quali_engine_1990` (default on). The rnd lane's `f1990_quali_rev_limit` and `f1990_quali_engine` are Engine Power (type 0) upgrades that stay in the Engine Power denominator.

| binding | address | use |
|---|---|---|
| `Game_Rnd_FillVehicleUpgrades` | 0x141238cb0 | wrapped: in a 1990 career, reads the current session's group (provider +0x270 vt +0x98 = the weekend, sessions +0x220, 0xa0 each, group +0x98, current index +0x238) and marks the fill when it is not 1 (Qualifying) or 2 (SprintShootout) |
| `Game_Rnd_CalcUpgradeTypeProgress` | 0x143526260 | wrapped: inside a marked fill, type 0 is recomputed (`src/rnd_progress.hpp`) with the two upgrades (node hash 0x1305fa1e / 0x4e03ba09 or name `f1990_quali*`) left out of the completed weighting only |

Every other caller of the progress function (UI, the power-unit wear range) and every other career get the game's value. It logs `[f1-1990] quali engine: car N set up for a session of group G; qualifying engine lines counted|left out` (first 8) and `Engine Power progress X -> Y outside qualifying` (first 8 changes).

## R&D card art and department names (`f1_1990_rnd.dll`)

A separate DLL; ships `data/rnd-art.json` and `data/rnd-strings.json` through its entry's `files`.

- **Card art:** the three card binders (0x14180fc60, 0x141814320, 0x1418169f0) name a frame `<upgrade_name>`, `_in_progress` or `_complete` and format it right after the six `site_Career_RnDCardArt{Image,Mask}N` sites (the rnd-tyres addon's bindings; handlers chain). At those sites a frame of an `f1990_*` upgrade becomes its donor's frame for the same state (`rnd-art.json` `frames`), named the way the list in pkg names it: `om_xm_<part>`, `_progress`, `_complete`, mask `om_xm_<part>_m` in `rnd_om_xm`; `<part>`, `_in_progress`, `_complete`, mask `<part>` in the plain lists (`src/rnd_art.hpp`). Only 1990 trees carry `f1990_` names, so there is no career check.
- **Department names:** `rnd-strings.json` `strings.<language>.<key>` are set through the core's runtime string overrides (API v7) on a 1990 `career.loaded` and cleared on `career.unloaded`; compiled out below SDK v7.

It logs `[f1-1990] rnd: N card art donor(s)`, `card art site S image "f1990_x_in_progress" -> "om_xm_y_progress"` (first 24), `no donor for "..."` and `set|cleared N department string(s)`.

## 1990 career setup (`f1_1990_wizard.dll`)

A separate DLL; option `career_wizard_1990` (default on). A new 1990 Driver Career runs Recommended / Custom settings,
the confirm step, then Choose Driver with the official 1990 drivers. The career wizard keeps its selections (options,
wizard state +0x100) and steps (+0xe0) in a state object whose +0x128 is the data group of the Career tile that opened
it (`f1_career_2025`, `f1_career_2026`, `f1_career_1990`); every hook acts only when that is `f1_career_1990`, so the
2025 and 2026 wizards keep every step.

| binding | address | use |
|---|---|---|
| `Game_CareerWizard_StartsInF1` | 0x14078ddd0 | wrap, yes: the router skips the Skip to F1 / Start in F2 step, the step machine drops the team-icons step (My Team / Konnersport) after Recommended settings, the driver select confirm takes the F1 path (as option 6) |
| `FeTile_Route_CareerItem` | 0x141725620 | wrap: the 1990 tile's `career_wizard_driver_career_to_f1` (season length step) becomes `career_wizard_driver_career_1990`, which `erp/wizard.json` adds to the content select's route grouping (`tools/wizard.py`) and which opens the settings step: no Full Season / Custom Season choice, the career keeps its full 1990 calendar |
| `Game_CareerWizard_PushNextStep` | 0x14386d890 | wrap: a classic drivers step (14, Add Driver Icons To Market) pushed after the settings becomes the confirm step (6) |
| `site_Game_CareerWizardConfirm_NextRoute` | 0x141a23c91 | site in the confirm step's continue (rdx = route name, rsi = view model, +0x270 the wizard): `driver_career_driver_type_select` (Official Driver / Custom Driver) becomes `driver_career_driver_select`, the route the game takes for a team-icon career |

It logs `[f1-1990] wizard: f1_career_1990 starts in F1, no F2 choice and no team-icons step (game said yes|no)`,
`wizard: 1990 tile opens the settings step (career_wizard_driver_career_to_f1 -> career_wizard_driver_career_1990)`,
`wizard: classic drivers step (14) replaced by the confirm step (6)` and
`wizard: confirm goes to driver_career_driver_select, no driver type step` (first few of each).

`src/driver_select_probe.cpp` is a temporary read-only probe of the Choose Driver populate
(`vt_Game_DriverCareerDriverSelectViewModel` slot 13): for the first 6 populates of any wizard it logs the wizard suffix,
options, the official entrant list map (name, stat id, entrant count), the slot's game mode type chain and components,
and how many driver cards the populate made (`[f1-1990] probe: ...`).
