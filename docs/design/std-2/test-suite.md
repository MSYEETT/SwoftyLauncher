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
- **`career`**: the save sandbox, the fixture's saves copied into it, then the agent continues the career and waits for the hub. Continuing needs the title-screen continue, the career tile and the resume card. The game's deep-link manager for save slots may be a shortcut.
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

The sandbox is a core feature at the save system's start (`Ne_SaveSystem2Win_Start`, 0x14413af70). In test mode it points the game's user-data folder at the run's own folder and replaces the Steam storage interface with one that never forwards to Steam. If it can't install, the agent quits the game before the title screen loads a save. The engine also has a local-save mode, used by the EA build, as a second route.

## Fixtures

- **What's in one:** the two profile save files, the addon storage that lives outside the save (Progressive's career index, the Career Editor's points sidecar), the launcher profile, and `fixture.json` with the exe build and the addons it needs. About 1 MB.
- **Where they live:** fixtures for public addons can ship in the addon's `tests/fixtures/`. The project's own fixtures live in a fixtures store, not the public repo.
- **The first ones:** a Progressive 2018 career from an existing backup, then captures from test runs: a My Team career at the contract window, a Driver Career at the season break, a Grand Prix weekend.
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
