# R&D

Owner: `native/src/rnd.cpp` (ce-rnd). Status: implemented, compiled; needs in-game proof. Both modes.
Every op runs on the flow thread while `ce::safe()` and goes through the game's own upgrade paths (no
raw state writes; the only direct store is the mode-0 owner-cache invalidation the game itself does
after every mutation), so the day-stamped state history, availability, the fitted counters and the save
stay valid.

**Identifiers.**
- `team`: the game's packed team key (u32, team stat id in the high 16 bits), as listed in
  `rnd.teams[].team`. The string `"player"` (the default when `team` is omitted) means the player's team
  (`rnd.player_team`).
- An upgrade is picked by `upgrade` (u32 node hash, `rnd.teams[].upgrades[].hash`, stable across saves)
  or by `name` (the gamedb `upgrade_name`). Several ops also take `dept` + `"all": true` to act on every
  upgrade of one department (or `"dept": -1` for every department).
- Departments (upgrade ENUM A, the game's names from `k_Career_UpgradeDeptNameTable`): `0` aerodynamics,
  `1` chassis, `2` durability, `3` engine, `4` fake (only with `swofty.rnd-tyres-thermal` installed: its
  Tyres rows; the disabled dept-4 hub is never listed).

**States** (`state`): `0` Unavailable, `1` Available, `2` InProgress (researching), `3` Completed
(fitted), `4` Failed, `5` Invalid, `6` PendingDevelopment (researched), `7` InDevelopment,
`8` PendingFitting (developed, not fitted). In the research → develop → fit lifecycle a finished
upgrade is `3`; with the research-only lifecycle research ends in `3` directly.

Errors (`ok: false`, `error`): `not_ready` (no career / R&D component not seen yet), `bad_args`,
`no_team`, `no_upgrade`, `wrong_state` (with the current state), `dept_unsupported` (dept 4 without the
Tyres addon), `derived_lock` (`rnd.unlock` in mode 0: the game re-derived the facility lock from the
team's facility tier), `game_refused` (the game's path ran but the state did not move; `after` says
where it is). Per-upgrade `reason` values in `after`: `prerequisites`, `facility_locked`,
`adaptation_pending`, `derived_lock`, `lifecycle` (a development step in the research-only lifecycle),
`dept_unsupported`, `game_refused`.

Every op logs one line per upgrade it moved: `[career-editor] rnd.<op> team=0x… "<name>" 0x<hash> dept=<name> <state> -> <state> [note]`, and `rnd.complete` adds a summary `… N of M upgrades moved`.

### `rnd.complete`
Finish research and/or development instantly, free (no RP is charged), for any team.

Args: `{ "team"?: u32|"player", "upgrade"?: u32, "name"?: string, "dept"?: -1..4, "all"?: bool,
"fit"?: bool = true, "stage"?: "research"|"develop"|"full" = "full" }`

Per upgrade, it steps through the game's own transitions until `stage` is reached. Which transitions
exist depends on the career's R&D lifecycle (`rnd.lifecycle`, the game's `g_Career_UpgradeLifecycle`):

- `research_develop_fit` (lifecycle 2): research → PendingDevelopment → development → PendingFitting →
  fit → Completed.
- `research_only` (lifecycle 0/1): research ends in Completed directly; there is no development phase, so
  `stage` and `fit` make no difference and `develop` / `full` both stop at Completed.

The steps:
- Available / Failed research → the game's U-level research start with a zero-length duration (its own
  instant branch: the fail flag is cleared, so it never fails), no RP debit → Completed or
  PendingDevelopment. In Upgrades mode 2 (per-driver trees) the game mirrors the start onto the other
  driver's instance of the same upgrade, so both trees' copies move; `before`/`after` list every
  instance with that hash.
- InProgress → the game's U-level research cancel (no refund; the RP paid at the start stays spent),
  then the zero-length start above.
- PendingDevelopment / Failed development (lifecycle 2 only) → the game's U-level development start with
  a zero-length duration → PendingFitting. If the game leaves it InDevelopment, the op cancels that
  development again (U-level cancel, no RP) and reports `game_refused` instead of leaving it stuck.
- InDevelopment (lifecycle 2 only) → the U-level development cancel, then the zero-length development
  start.
- PendingFitting and `fit` → the game's R&D purchase request handler fits it (as the Fit button does)
  → Completed.

Availability after each pass: in Upgrades modes 1/2 the game's start functions refresh availability
themselves. In mode 0 (one shared tree with an owner cache) they only invalidate the cache, so the op runs
the pass the game's next R&D query runs, `Career_Set_UpgradesOwner` for the team's driver, which
recomputes Available/Unavailable and the facility locks. That way children unlocked by a completion are
Available for the next pass of `all`.

Research and development slot limits are not applied (it is instant). With `all`, it repeats passes
(at most 32) so upgrades unlocked by a completion are completed too (a whole department or tree in one
op). Unavailable upgrades are skipped (`reason: "prerequisites"` or `"facility_locked"`; `rnd.unlock`
first).

Result: `before`/`after` = `{ "team": u32, "upgrades": [ { "hash", "name", "state" } ] }`.
Undo: backup-only. The journal's inverse is `rnd.reset` on the same upgrades (fresh state, not the
exact prior in-progress state).

### `rnd.unlock`
Args: `{ "team"?, "upgrade"?|"name"?|("dept"+"all") }`. Clears the facility lock through the game's
facility-lock setter (`Career_Set_UpgradeFacilityLocked`: no-op when the flag is already clear, updated
in place when the latest entry has today's day, appended as a new day-stamped entry otherwise), then
refreshes availability (modes 1/2: availability refresh; mode 0: the owner pass). The upgrade becomes
Available when its prerequisites are complete; otherwise it stays Unavailable with
`reason: "prerequisites"`.

The game derives facility locks from the team's facility tier (the upgrade's `unlock_at_facility_level`
against the team's facility modifier for that department). In mode 0 the owner pass recomputes the lock
at once, so an unlock the tier does not allow is undone straight away: the op returns `derived_lock`
(per upgrade `reason: "derived_lock"`). In modes 1/2 the game recomputes locks when facility levels
change. Undo: backup-only.

### `rnd.reset`
Args: `{ "team"?, "upgrade"?|"name"?|("dept"+"all") }`. Returns upgrades to their initial state with
the game's per-instance reset (`Car_Reset_LastFittedUpgrade`, the one its reset-all and supplier refit
use: history cleared, Unavailable at day 0, enabled, facility lock from `unlock_at_facility_level`),
then refreshes availability as above (mode 0: the owner pass) and, when a fitted upgrade was reset, re-posts the team's fitted
ratio (`Career_Post_UpgradeFittedRatio`, the message the game sends after every fit/unfit). No RP is
returned (use `rnd.refund` first for an upgrade in progress). Department 4 → `dept_unsupported`.
Undo: backup-only.

### `rnd.refund`
Args: `{ "team"?, "upgrade"?|"name"? }`. Only for an upgrade that is InProgress or InDevelopment:
cancels it through the game's own cancel path (the purchase request handler with cancel set), which
returns RP at the game's refund rate and restores the previous state. Result `after` adds
`"rp_refunded": true`. Other states → `wrong_state`. Undo: none (backup-only).

### `rnd.fit`
Args: `{ "team"?, "upgrade"?|"name"?, "fitted"?: bool = true }`. `fitted: true` fits a PendingFitting
upgrade (→ Completed); `fitted: false` takes a fitted upgrade off the car (Completed → PendingFitting),
both through the game's purchase request handler, as the R&D screen does. `fitted: false` needs the
`research_develop_fit` lifecycle (the handler's own condition), otherwise `wrong_state`. In mode 0 the
owner pass runs afterwards. Undo: `rnd.fit` with the opposite `fitted`.

### `rnd.team_detail` (read only, anytime)
Args: `{ "team"?: u32|"player" }`. Registered with `CE_OP_ANYTIME`; reads memory only. `ok: true`,
`before` empty, `after` = one team in the same shape as `rnd.player` below (full upgrade list). Use it
to show or edit an AI team's tree; the section only carries the player's.

### Snapshot section `rnd`
About 22 KB on a live Driver Career (166 upgrades on the player's team, 11 teams): the player's team in
full with short keys, every team as a summary.
```json
{
  "ready": true,                 // false: "reason" says why (no career, component not found)
  "day": 36,                     // career day (CareerTime minutes / 1440), the day ops stamp
  "player_team": 31719424,       // packed team key of the human entrant, or null
  "lifecycle": "research_develop_fit" | "research_only",   // g_Career_UpgradeLifecycle 2 / 0-1
  "player": {                    // null when no single human entrant matches a team
    "team": 31719424, "team_stat_id": 484, "player": true,
    "mode": 0,                   // Upgrades model mode: 0 shared tree, 1 per team, 2 per driver
    "owner": 3538944,            // driver key the ops pass for this team
    "counts": { "total": 166, "unavailable": 120, "available": 5, "researching": 3, "researched": 0,
                "developing": 0, "pending_fitting": 0, "fitted": 38, "failed": 0, "locked": 20 },
    "depts": [{ "d": 0, "t": 25, "f": 0, "p": 1, "a": 1, "l": 5 }],
    "u": [{ "h": 3704016914, "n": "om_xm_redesigned_brake_ducts", "d": 0, "s": 0, "c": 400, "t": 20,
            "r": "prerequisites" }]
  },
  "teams": [{                    // every team, the player's included
    "team": 31719424, "team_stat_id": 484, "player": true, "fitted_ratio": 0.229,
    "counts": { … as above … },
    "depts": [{ "d": 0, "t": 25, "f": 0, "p": 1, "a": 1, "l": 5 }]
  }]
}
```
`depts[]`: `d` department (0 aerodynamics, 1 chassis, 2 durability, 3 engine, 4 fake = Tyres addon), `t` total,
`f` fitted (state 3), `p` in progress (researching + developing), `a` available, `l` facility-locked.
Departments with no upgrades are left out. `fitted_ratio` = fitted / total over the team.

`u[]` (one per upgrade; optional keys only when they apply):

| key | meaning |
|---|---|
| `h` | node hash (the `upgrade` arg of every op) |
| `n` | gamedb `upgrade_name` (also usable as `name`) |
| `d` | department |
| `s` | state (0 Unavailable, 1 Available, 2 InProgress, 3 Completed = fitted, 4 Failed, 6 PendingDevelopment, 7 InDevelopment, 8 PendingFitting) |
| `c`, `t` | gamedb research cost (RP) and research days |
| `dc`, `dt` | gamedb development cost and days (only when non-zero) |
| `b`, `e` | start and end day while InProgress (research) or InDevelopment (development); progress = (`day` − `b`) / (`e` − `b`) |
| `l` | 1: facility-locked |
| `ad` | adaptation byte (1 needs adaptation, 2 adapted, 3 adapting); ops refuse fit/complete on 1 and 3 |
| `r` | why it can't be completed now: `prerequisites`, `facility_locked`, `adaptation_pending` |
| `ul` | `unlock_at_facility_level` (only when non-zero) |
| `sp` | 1: an engine-supplier row |
| `tr` | tree index (only for the second tree in per-driver mode 2) |

Fitted means `s == 3`. Costs and days are the gamedb base values; the game applies team modifiers
(cost/time scales, facility tiers) when the player buys. Only listed: departments 0–3, plus department 4
upgrades that are enabled (the Tyres addon's rows; the stock dept-4 hub stays hidden).

### Engine paths (for the KB rows)
| binding | role here |
|---|---|
| KB structs `UpgradesComponent`, `Upgrades`, `UpgradeTree`, `UpgradeInstance`, `UpgradeStateEntry` (re/db/structs; `CareerTimeComponent.current_minutes` from ce-econ) | every field rnd.cpp reads or writes (component vector and CareerTime sibling; model mode, team key, trees, tree count, owner cache; instance gamedb copy, progress days and state history; state entry owner, day, state, adaptation, enabled, facility lock) |
| `ce::comp::get(Upgrades)` | UpgradesComponent: Upgrades models at +0x298..+0x2a0, CareerTime at +0x2c0 (minutes +0x2b0), lifecycle byte +0x3e0 (copied into `g_Career_UpgradeLifecycle` at init), the layout the component's own day update (vtable slot +0x128, 0x14123a860) and purchase handler read |
| `g_Career_UpgradeLifecycle` 0x1460fc310 | 2 = research → develop → fit, 0/1 = research only; picks the steps and the snapshot's `lifecycle` |
| `Career_Start_UpgradeResearch` 0x1435168d0 | research start; end == start clears the fail flag and runs the instant branch 0x143523a40 (Completed when lifecycle < 2, PendingDevelopment when 2); mode 2 mirrors onto the other owners' same-id instances; refresh only in modes 1/2 |
| `Career_Start_UpgradeDevelopment` 0x1435173e0 | (renamed from `Car_Apply_UpgradeToVehicle`) development start, 10 args: U, owner, instance, start, end, option, dual, roll, rng, store; end == start pushes PendingFitting only when lifecycle == 2 |
| `Career_Cancel_UpgradeResearch` 0x1435320f0 / `Career_Cancel_UpgradeDevelopment` 0x143532390 | U-level cancels (no RP), called by the component cancels 0x141241770 / 0x1412419d0 after they refund; the development cancel mirrors in mode 2 only when the entry's dual flag (+0xb) is 1 |
| `site_Career_RnDPurchaseRequestEntry` 0x1412414c0 | the purchase request handler: state 8 → fit (0x141242bd0 → 0x143517c50), state 3 with lifecycle 2 → unfit (0x141242f30 → 0x143530980), cancel flag + state 2/7 → refund and cancel (0x141241770 / 0x1412419d0); adaptation byte 1 with state 3/8 → adaptation start 0x141241c10 (never reached by our ops) |
| `Career_Set_UpgradesOwner` 0x14352e620 | the mode-0 availability pass a query runs when the owner cache differs: state bytes and facility locks recomputed for that owner |
| `Car_Reset_LastFittedUpgrade` 0x143517d80 | per-instance reset |
| `Career_Set_UpgradeFacilityLocked` 0x143523730 | facility-lock setter (history entry, see `rnd.unlock`) |
| `Career_Refresh_UpgradeAvailability` 0x14352f300 | availability refresh (modes 1/2) |
| `Career_Post_UpgradeFittedRatio` 0x14352fe00 | fitted-ratio message after a fit change |
