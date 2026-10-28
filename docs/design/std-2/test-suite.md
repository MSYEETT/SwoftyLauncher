---
title: "Test suite: design and plan"
description: How swofty test reaches each game state without clicking, what already works in the game and the launcher, what needs building or reverse engineering, and the order to build it in.
---

# Test suite: design and plan

The goal: any test, written by an addon author or an agent, starts F1 25 in the state it needs and checks it, with no menu clicking, no playing through races and no risk to the player's own saves. [Testing your addon](/std/testing) is the author's side. This page is how it's built.

It has two halves:

- **The CLI** (`swofty test`) plans the run, takes the game lock, builds a throwaway profile, prepares the save sandbox and the environment, launches the game, collects the results and puts everything back.
- **The test agent** is a core component that runs only in test mode. It takes commands from the CLI over a local mailbox and carries them out inside the game: open a route, start or skip a session, hand the player's car to the AI, read and write values, wait for events, capture frames, save, quit.

## What already works

- **Launch and roll back.** `swofty run --apply --direct` builds a profile into the game folder and puts it back on exit. Session logs, crash reports with symbolised stacks, the data dir's session lock and exact-PID game detection all exist.
- **A race straight from launch.** The game's own `-benchmark <xml>` mode boots into an all-AI race in about 25 seconds with no menus. The XML sets the track, laps, weather, number of cars, the player's grid slot and, through an undocumented `setup_mode`, the box. The AI drives the player's car.
- **Opening menus.** The core already fires the game's UI routes on the home view model (an existing core menu driver does it), and the career hook sees every route the UI fires, so routes can be recorded and replayed.
- **Changing careers.** The Career Editor's live API sets money, resource points, R&D, contracts, points and calendar data in a running career.
- **Reading state.** The car and career structs in the KB, the core's events and log lines, the game's own log channels, `registry/effective.json` and frame capture.
- **Tracing.** The trace plugin installs hooks from KB signatures and logs registers and fields. It ran in the real game for the first time on 2026-10-28, with all 11 hooks installed.

## Environments

- **`boot`**: launch to the main menu. One press gets past the title screen: the virtual pad does it once the agent exists.
- **`benchmark`**: write `benchmark/swofty_test.xml` into the game folder for the run and launch with `-benchmark`. The CLI needs to pass game arguments in a direct launch. Race sessions only.
- **`route`**: `boot`, then the agent fires each route. Routes from the home screen work now. Picking an item inside a screen (a box, a track, a team) needs the screens' selection handlers.
- **`career`**: the save sandbox, the fixture's saves copied into it, then the agent continues the career and waits for the hub (`at`: `team_hq`, the default, `weekend_hub` or `session`). The CLI sends the agent `career load <slot|newest> at <at>` and waits for `[test] career ready at <at>` in loader.log. Continuing, from static RE (L1 unless noted):
  1. The title screen (`FeFlow_StateIntro_OnUpdate` 0x1436da4d0) waits for a button: the virtual pad's A press.
  2. Fire route `career` on the home view model, as the core's menu driver fires routes (L2).
  3. On the `fe_career` tile list, call the Driver Career or My Team tile's GetAction (`FeTile_Route_CareerItem`) and fire the route it returns, `solo_career_continue_driver` or `solo_career_continue_my_team`; firing the name alone skips the tile's data-group write.
  4. On the resume screen (vtables 0x1457264a0, 0x145726670; `Game_CareerContinue_HandleCommand` 0x141a1db90), find the card whose slot id (`vm+0x330+i*0x540`) is the fixture's, raise `slot_focused` then `slot_pressed` with data `i`. `Game_SaveSlotManager_SelectSlot` loads it with no confirm popup. Proof: `[career] career.loaded`, then the Team HQ flow.
  5. `weekend_hub` and `session`: Team HQ's advance command 0xdb427f42 (`Career_TeamHqAdvance_Start`) leads to the session's garage; the garage-to-track step is not found yet.
  - There is no deep link to a given slot: the deep-link manager's career activity resumes the mode's current slot only.
- **`grand_prix` / `time_trial`**: either the game's settings-driven dev session (`DevSessionDataContract`: track, session, setup mode, entrants; reader 0x141125e80), if what switches it on can be found, or navigation with in-screen selection.

## The agent's commands

- **Skip a session:** post `MessageSetAdvanceDestination` (vtable 0x1457548b8) on the message bus, the same message the game's Simulate Session popup sends. Destination 2 is next session, 3 simulate, 4 skip to a session, 6 quick practice skip; the session type is the race (15) and so on.
- **Advance the career calendar:** `Career_TeamHqAdvance_Start(vm+0x730, day)`, what Team HQ's Advance button calls; the game then advances one day at a time.
- **The AI drives the player's car:** `Game_Set_VehicleController_Thunk(system, vehicle id, 1, reason)` with the dev reason (6), as the benchmark mode does at session start. Pass 0 to hand it back.
- **Read and write car and career values:** the typed accessors `cars::live` uses, so tests and addons share one implementation of the scrambled values and the derived-value rules.
- **Input:** a virtual pad inside the game process. The game reads XInput, so the agent can press buttons without focus, unlike ydotool. Whether a parked window still reads it is the first thing to check.
- **Save now:** a new core operation, so a test can capture a fixture at the state it reached.

## The save sandbox

The game keeps every career, Grand Prix and setting in one profile save: two encrypted files in Steam's `userdata/<account>/3059520/remote/`, which Steam Cloud syncs. It reads and writes them through Steam's storage API, so turning Steam Cloud off isn't enough: the files would still be written to the player's real folder.

The sandbox is a core feature at the save device's start (`Ne_SaveSystem2Win_Start`, 0x14413af70; the RE is in `re/db/subsystems/core.md#save-sandbox`). It runs only when `swofty test` starts the game with a run token:

- **The run folder.** The CLI makes `<data>/test-runs/<run>-<batch>/` with `run.json` (`{"schema":1,"run","token","saves":"saves","exe_sha256"}`) and starts the game with `SWOFTY_TEST_RUN=<folder>`, `SWOFTY_TEST_TOKEN=<token>` and `SWOFTY_EARLY_STAGE=1`, so the core loads at the game's first mutex, before the save device exists. After the batch the token is removed from `run.json`.
- **The proxy.** The device caches Steam's `ISteamRemoteStorage` at its Steam helper +0x108, written once before Start. The core's wrap of Start checks the device (its vtable slot +0x08 is Start, the helper exists, `m_storeInCloud` +0x3e8 is on, the interface's vtable lies outside the game) and swaps the pointer for a proxy that serves every slot from `saves/remote/`. The device calls FileWrite, FileRead, FileDelete, FileExists, GetFileSize, GetFileTimestamp, GetFileCount, GetFileNameAndSize and the write batch; the proxy serves those and a few more, answers 0 to the rest and logs them, and never calls Steam. The save files are opaque bytes to it. Loads and saves mount into `%TEMP%` and go through the interface, so nothing else is written.
- **The user data folder stays.** The helper's +0x00 (`GetUserDataFolder`) is only the prefix the device strips from its title folder to build remote names, never a path it opens; rewriting it would break the names and redirect nothing.
- **The corrupt-save upload is off.** Device slot +0x58 (`Ne_SaveSystem2Win_WriteInfoXml` 0x144141c00) zips a save and uploads it through Steam's global interface, outside the helper, so in a test run it returns without doing anything.
- **Fail closed.** With a token in the environment, any doubt ends the game, logging `[saves] TEST SANDBOX FAILED: <why>` and writing `sandbox.json` `{"state":"failed"}`: a token that isn't the run's, a binding that doesn't resolve, a wrap that doesn't install, a device or interface that isn't the expected one, or a mount (`Ne_SaveSystem2Win_Mount`, which every load and save passes before any file read) before the sandbox armed. Armed, it logs `[saves] test sandbox armed (run <id>)` and writes `{"state":"armed"}`.
- **Preflight and backstop.** The CLI refuses a career batch unless the core's binding set for this exe's sha256 carries `Ne_SaveSystem2Win_Start`, `_Mount` and `_WriteInfoXml`; the export gate needs an independent `agree` for each, so that is the static proof. After the batch it fails the tests unless `sandbox.json` says armed, and compares the player's Steam save folder listing (names, sizes, times, never contents) before and after.
- **Local-save mode** (`Steam.cloudEnabled` false, the EA layout) is not a sandbox: the device then reads and writes the player's own `userdata/<id>/3059520/local/savegame`. The sandbox refuses it.

## Fixtures

- **What's in one:** the two profile save files, the addon storage that lives outside the save (Progressive's career index, the Career Editor's points sidecar), the launcher profile, and `fixture.json` with the exe build and the addons it needs. About 1 MB.
- **Layout:** `fixture.json` (schema 1: `name`, `description`, `created`, `exe` `{sha256, version}`, `addons` `{id: version}`, `holds` `{careers: [{mode, slot, name, season, round, team, driver}], notes}`, `files` `{path: {size, sha256}}`, `source`), `save/` with the Steam remote names (`savegame@profile#NXDSMWW.BWW` and `#QKRHMYXE`, plus mid-session saves; never highlights or corrupt-save bundles) and `storage/<addon id>/`. A run checks every file against `fixture.json`, copies `save/` into the sandbox and `storage/` into the batch profile's addon storage copy, which the session loads. Each career batch gets its own launcher profile and run folder.
- **Where they live:** a test's `fixture` is a folder next to its test file (`fixtures/my-team-r3`), else a fixture in the store (`$SWOFTY_TEST_FIXTURES`, else `<data>/test-fixtures/`) by the same path or its last part. Fixtures for public addons can ship in the addon's `tests/fixtures/`. The project's own fixtures carry swofty's careers and names, so they stay out of the repo: the store on this machine is `/mnt/gaming/swofty-std16/fixtures/` (set `SWOFTY_TEST_FIXTURES` to it).
- **Capture:** `swofty test fixture capture <name>` copies the newest run's sandbox (or `--run <id>`'s) and the addon storage the run left, with the run's exe and addons; `--from <dir>` takes a copy of a save folder instead and refuses the live one. `--career mode:slot`, `--description` and `--notes` fill `holds`. A fixture is never overwritten. `swofty test fixture list` and `check <name>` verify the files.
- **The first ones:** `swofty-profile-20261028`, swofty's whole profile from the 2026-10-28 backup copy (exe d873a1c8, game 1.0.141.7865; its careers are not listed yet), then a Progressive 2018 career from an existing backup and captures from test runs: a My Team career at the contract window, a Driver Career at the season break, a Grand Prix weekend.
- **Building states instead of capturing them:** the Career Editor's API, plus new operations to advance the calendar, set race results and save.

## Rules

- The test agent and test DLLs load only when `swofty test` builds the profile and passes a token for that run. Normal Play never loads them.
- One game per machine. Tests queue on the existing game lock (`/mnt/work/.f1-game.lock.d` here), and `--take-over` closes a running game first.
- The CLI never accepts the offline play terms for the player: a run without them fails with exit code 2.
- Artefacts have size caps. Frames are optional.

## Build order

1. **CLI only, now.** `swofty test plan` and `run` with the `boot` and `benchmark` environments: pass game arguments in a direct launch, stage the benchmark XML, and check logs, crashes, performance and that the game folder is clean afterwards. Most car-physics traces (T112 to T125) can run unattended this way.
2. **The test agent.** The mailbox, routes from the home screen, skip session, the AI driving the player's car, value reads and writes through `cars::live`, frame capture, the virtual pad and `swofty test shell`.
3. **Saves and careers.** The save sandbox, fixtures, continuing a career into its hub, calendar advance, race results and save now. Then `career` environments.
4. **Any screen and session.** Selection inside screens and the dev session start, for `grand_prix` and `time_trial` from any box, track and car.
5. **CI.** A machine that runs every addon's smoke tests before a launcher release, and publishes the reports.

The reverse engineering behind steps 3 and 4, in order of value: the save sandbox install point, continuing a career, what switches on the dev session, selection inside the Grand Prix and Time Trial screens, what ends a session after a skip, and the race results write.
