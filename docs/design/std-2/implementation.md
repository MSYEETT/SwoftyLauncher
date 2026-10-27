---
title: Implementation plan
description: How standard library 2.0.0 gets built by parallel agents in rounds, with frozen contracts, one integration branch, independent review and a single in-game tester.
---

# Implementation plan

2.0.0 is built by several agents working at the same time, in rounds, with one lead (the session that owns the release) integrating as each piece lands.

## The rules every round follows

- **Contracts before code.** The lead writes and freezes the headers and schemas first. Every agent codes against them. A change to a contract goes through the lead, never through one agent's branch.
- **One branch, one PR.** All work lands on `feat/std-2` and its one draft PR. Each agent works in its own worktree on `std2/<stream>`, branched from `feat/std-2`, and the lead merges it in.
- **Each agent owns its files.** A brief lists the files an agent may change, and agents don't edit outside them, so merges don't conflict.
- **Short rounds.** An agent gets about 75 minutes and one job. When it reports, the lead merges, runs the full tests, and either sends it on or gives it its next job, continuing its context.
- **Someone else checks the work.** Every stream's diff is reviewed by a different agent, and reverse-engineering rows get an independent verify from a different agent, as `re/PROTOCOL.md` requires.
- **One in-game tester.** Only the lead launches F1 25: with the game lock, never while swofty plays, and with in-game checks batched into one session per round.

## The workstreams

```text
                         W0 contracts (lead)
       ┌───────────┬──────────┼───────────┬────────────┬───────────┐
       W1 json     W5 hooks   W6 launcher W9 compat    W10 schema
       │           + services │           gate         check + CI
       W2 validator           │
       │                      │
       W3 registry core ──────┼──────────────┐
       │                      │              │
       W4 snapshots           W12 build-time W8 what-if example
       │                      appliers
       W7 Progressive 2.0     │
       (needs W3, W5)         W13 Classic Seasons on records
                              │
                              W14 engine sound example
                              │
                       W11 public docs (needs W3-W14 stable)
```

**W0. Contracts** (lead, first):

- `swofty_api.h` with `SwoftyRegistry`, `SwoftyHooks`, `SwoftyServices`, the version 9 `SwoftyApi`, and the exports;
- the C++ headers `swofty/json.hpp`, `swofty/registry.hpp`, `swofty/hooks.hpp`, `swofty/services.hpp`, with full declarations and stubs that return `unavailable`;
- `swofty/career/season.hpp` and `swofty/career/ratings.hpp`;
- `packages/sdk/std/registries/career.season/1.0.json` and `career.driver_ratings/1.0.json`, with examples;
- `packages/sdk/std/tests/`: a shared corpus of base, layers and expected effective value, used by both the C++ and the Rust implementation.

**W1. `swofty::json`** (native SDK, header-only): a value type, a parser, `dump`, JSON Pointer (with the `slot=1` key form), and Merge Patch with `x-key` arrays and `$delete`. Tests: the RFC 7396 vectors, the JSON test suite, and the shared corpus.

**W2. Schema validator** (core): the JSON Schema subset plus `x-id`, `x-key` and `x-ref`, and the registry `check` hook. Tests: every example in `packages/sdk/std/registries/*/examples`, plus the must-fail ones.

**W3. Registry core** (`native/src/core/registry/`):

- records, static and dynamic providers, layers and precedence, origins, listing, watching, adapters;
- the C table and its exports;
- locking, and the flow-thread wait during a career load;
- reading `swofty/registry/index.json`.

Tests: the shared corpus, randomized layer stacks checked against a small Python reference implementation, and a read benchmark logged to `perf.log`.

**W4. Snapshots** (core): the `swofty_registry` save container, compression, budgets, migration through adapters, and keeping unknown majors. It's the riskiest stream because the save heap is small, so it goes last among the core streams and gets the most in-game time. Tests: fixture saves from each version in `packages/sdk/std/fixtures/`.

**W5. Hooks and services** (core):

- the hooks table: `define`, `fire`, `attach` with phases and priority;
- the game hook point machinery: install the mixin on the first subscriber, yield to a 1.x overwrite and report it (no game points ship in 2.0, but the machinery is tested with a test point on a harmless function);
- services: `publish` and `when`, with deferred callbacks.

**W6. Launcher side** (Rust: `packages/core`, `apps/cli`, `apps/launcher`):

- parsing and validating the new manifest fields;
- the build step that writes `swofty/registry/` and `index.json`;
- profile layers in `$DATA/registry/`;
- the five host calls;
- `requires` checks at install and Play;
- registry conflicts in `conflicts.json`;
- picking up `registry/effective.json` for Send Logs.

Validation must agree with W2 on the shared corpus.

**W7. Progressive Career 2.0** (`addons/progressive`):

- `tools/registry.py`, which writes the static provider files from `years.json`;
- the dynamic ratings provider;
- every DLL reads effective `career.season` and `career.driver_ratings` instead of `years.json`;
- the `progressive.history` service, the `progressive.season.apply` point, and the old exports kept.

**W8. Example addon** (`templates/std/whatif`): the launcher page from [The launcher side](./launcher), a static example layer, and its manifest. The docs include its code.

**W12. Build-time resolution and appliers** (Rust, `packages/core/src/std/`): resolving build-time registries (base, `extends` chains, layers, load order), and the core's appliers for `era.box`, `content.package`, `vehicle` and `engine_sound`. They turn records into `file.put`, database and ERP operations, which run before every addon's own operations. It shares the resolver with W6, and must agree with W3 on the shared corpus.

**W13. Classic Seasons on records** (`addons/classicseasons/`): `tools/registry.py` writes each season's `content.package`, `era.box`, `vehicle` and `engine_sound` records from its existing data. Each season addon provides them under its own keys, and its hand-written operations shrink to what the appliers don't cover. Test: every season builds to the same game files as 0.6.x.

**W14. Engine sound example** (`templates/std/engine-sound`): a data-only addon with its own `engine_sound` record, a layer on the 2017 Ferrari, and an `extends` variant with its own era box.

**W9. Compatibility gate** (`tools/compat/`): downloads every Addon Browser addon version, builds each on two cores, compares the files and build hash, runs the Wine harness to the main menu, and compares the loaded DLLs, chains and start lines. It runs 1.5 against 1.5 first, to prove it reports no false differences.

**W10. Schema tooling and CI:** `tools/std/schemacheck.py` (minor or major, immutable released files), example validation, the fixture-save job, and wiring the compat gate into the release workflow.

**W11. Public docs:** the pages in [the docs plan](./rollout#the-docs), written once the APIs stop moving, by section, with code included from `templates/std/`.

## The rounds

**Round 0, the lead, about 2 hours.** Write W0, open `feat/std-2` and its draft PR, create the worktrees, and write the briefs. swofty reviews the contracts here, the cheapest moment to change them.

**Round 1, six agents at once:**

- W1 `swofty::json`;
- W2 the validator, against W1's header;
- W5 hooks and services;
- W6a the launcher's manifest fields and build files;
- W9 the compat gate, with a 1.5 against 1.5 run;
- W10 schemacheck and CI.

As each reports, the lead merges it and runs the native build, the host tests, `cargo test` and the Wine test. A reviewer agent goes over each merged stream.

**Round 2, six agents:**

- W3 the registry core;
- W6b the host calls, conflicts and `requires`;
- W7a Progressive's `tools/registry.py` and static files, compared against `years.json` record by record;
- W12 build-time resolution and the core's appliers;
- a reviewer on round 1's merged code;
- fixes from round 1's reviews, back to the agents that wrote it.

**Round 3, six agents (W14 starts when W12 merges):**

- W4 snapshots;
- W7b Progressive's DLLs on effective values, the service and the point;
- W8 the what-if example;
- W13 Classic Seasons on records, with the build compared against 0.6.x;
- a reviewer on round 2;
- W9 runs the full gate on the 2.0 core for the first time;
- W14 the engine sound example.

**Round 4, in game (the lead, one session, with the lock and a save backup):**

- Progressive 2.0 against 1.2 in a 2016 and a 2017 Follow-your-driver career: seats, swaps, ratings, logos, fuel, contracts;
- the example addon's 2017 change on Choose Driver, the grid and in follow mode;
- a career saved, the layer changed, the career reloaded: it keeps its snapshot;
- Classic Seasons 2016 and 2017 in Time Trial on records: the same cars, liveries and sounds as 0.6.x;
- W14's engine sound on the 2017 Ferrari, and its "F1 2017 (V10)" box as its own entry;
- a 1.x addon set from the gate's list, to the main menu and into a session.

Each failure becomes a fix brief, sent back to the agent that owns the code, and is checked again in the next in-game session.

**Round 5, docs and release:**

- three agents write the public docs by section: the registry and its subfeatures, hooks, services and operations, and the launcher, manifest and moving from 1.x;
- a reviewer reads them against the code;
- the lead writes the release notes, runs the gate one last time, and asks swofty for the go-ahead to tag 2.0.0.

## A brief

Every agent gets a brief in this shape:

```text
Goal         W3: the registry core, as specified in docs/design/std-2/{registry,providers,reading,layers}.md
Contracts    packages/sdk/include/swofty_api.h (SwoftyRegistry), packages/sdk/include/swofty/registry.hpp,
             packages/sdk/std/tests/ (the shared corpus). Do not change them; ask the lead.
You own      native/src/core/registry/**, native/test/registry/**
Don't touch  everything else; in particular native/src/core/stage.cpp (lead) and the save code (W4)
Worktree     /mnt/work/VSC/F1ModManager-wt/std2-registry on std2/registry, from feat/std-2
Done when    the shared corpus passes; the randomized precedence test matches tools/std/reference.py for
             10,000 stacks; the C table answers through SwoftyApi and SwoftyCoreRegistry; native/build.sh
             and native/test/run.sh pass; a read benchmark is in the report
Report       what you built, the test output, open questions, anything you'd change in the contracts
Rules        no game launches; no stash/reset in other worktrees; taskset -c 2-7,10,11,14-31;
             CARGO_TARGET_DIR under /mnt/work, deleted when you're done; Conventional Commits with no
             attribution (the pnpm PATH for the hook); push std2/registry; no PR of your own
Time         about 75 minutes, then report where you are
```

## Limits

- **Disk:** `/mnt/work` has about 40 GB free, and a Rust target directory takes about 11 GB. So at most two Rust streams build at once, each deletes its target directory when it reports, and native streams use zig, which needs little space.
- **CPU:** every build is pinned to the shared cores, with at most four heavy builds at a time.
- **Agents:** up to six at once per round.
- **The game:** one lock holder, and in-game checks only in the round 4 session and its follow-ups.

## Where it can go wrong

- **Snapshots and the save heap (W4):** the riskiest piece. It's built after the registry is stable, tested on fixture saves first, then in game with a save backup taken before each session.
- **Two validators (W2 and W6):** C++ and Rust can disagree. The shared corpus is the referee, and a disagreement fails both builds.
- **Contract churn:** if round 1 shows a contract is wrong, the lead changes it once, between rounds, and every brief is updated before round 2 starts.
- **Scope creep:** 2.0.0 ships the first slice only. Game hook points, identities and operations wait for 2.1 and 2.2, which get their own plan in this shape, including the reverse-engineering streams with independent verifies.
