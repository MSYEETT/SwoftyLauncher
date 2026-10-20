# Drivers

Owner: drivers (`native/src/drivers.cpp`). Status per op is at the end of each entry.

Every rating read and write goes through the career's `DriverStatisticsComponent` (My Team) or its
subclass `DriverCareerPlayerDriverStatisticsComponent` (Driver Career; same driver table plus the
player's attribute sets). The plugin gets the component from the championship entity's own component
registry (`ce::comp::get`) on every use and checks its driver table before reading. Drivers are looked up by
`driverStatId`, never by position in the table, and every write is read back through the game's own
rating getter for that same `driverStatId`.

## Ids

- `driverStatId` (int): the id the save uses, `(kind << 14) | index`. `kind` is 0 for the normal
  driver pool; other kinds are custom or generated drivers.
- `infoStatId` (int): a statistic, from `driver_statistics_info` (`stat_id`). The five stock ones are
  named `experience`, `racecraft`, `awareness`, `pace`, `focus`. The snapshot lists every statistic the
  component holds, so mods that add statistics show up too.

## How a rating is made (what the editor writes)

The game's getter (`Career_Get_DriverStatValue`, component vtable +0x280) computes:

`value = round(clamp(base(x) + dynamicLevel + temporaryBoost + perks + specialist + (team attribute - 1)))`

- `base(x)` is the driver's evolution curve for that statistic, a Catmull-Rom curve over normalised
  points, evaluated at `x` = career progress (career clock / career length, both in minutes, 0..1) and scaled to
  `miny..maxy` (0..100). The curve is `patchedEvolutionGraph` when the career has an applied ratings
  patch (`last_patch_applied_checksum` > 1 and the gamedb flag `use_story_mode_data` is 0) and that
  statistic has one, otherwise `evolutionGraph`. With live updates off the game forces the applied
  checksum to 1, so the base curve is used. The op never writes these flags.
- Dynamic statistics (`isDynamicStat`) have no curve; their base is 0 and the value is `dynamicLevel`.
- The team attribute term (`MyTeamAttributes` value for the driver's team, minus 1) applies to
  non-dynamic statistics of drivers who hold a team seat.
- Clamp bounds come from the component (vtable +0x268 / +0x270).
- The op computes every non-curve term as the game getter's raw value minus the curve value, so the
  team attribute term is included, and it reads the result back through the getter.

`drivers.ratings.set` edits the curve the getter is using right now (or `dynamicLevel` for a dynamic
statistic): it rewrites the curve's point values in place, keeps the point count and x positions,
and rebuilds the curve's 65-entry lookup table and cached end value with the game's own curve
evaluator (`Career_Eval_Curve`), exactly as the game's curve loader (`Career_Parse_CurveString`) does. Nothing else changes: no checksum, no patch state. The save
already stores both curves and `dynamicLevel`, so the edit persists with the next save. A later EA
ratings patch replaces it only if the player accepts that patch.

## Snapshot section `drivers`

```json
{
  "available": true,
  "reason": "",
  "component": "DriverStatisticsComponent",
  "patch": { "appliedChecksum": 0, "declinedChecksum": 0, "usesPatchedCurves": false },
  "stats": [
    { "infoStatId": 4, "name": "pace", "weight": 0.571, "default": 25.0 }
  ],
  "drivers": [
    {
      "driverStatId": 9,
      "name": "max_verstappen",
      "initials": "VER",
      "number": 1,
      "team": { "teamStatId": 478, "name": "red_bull" },
      "seat": { "hasCar": true, "position": 0 },
      "onGrid": true,
      "player": false,
      "overall": 95,
      "ratings": {
        "pace": { "value": 96, "base": 95.8, "curve": "base" }
      }
    }
  ],
  "otherDrivers": [
    { "driverStatId": 120, "name": "some_f2_driver", "overall": 71, "values": [60, 70, 65, 72, 80] }
  ],
  "progress": 0.31,
  "contracts": null,
  "recordedMoves": [],
  "pendingSwaps": []
}
```

- `available` false means there is no career, or the component is missing: `reason` says which, and
  nothing else is in the section.
- `drivers` lists the drivers in the championship line-up (every driver with an entrant row: race
  seats and reserves). `otherDrivers` lists the rest of the career's statistics table (free agents,
  F2 and other drivers) compactly: `values` holds the shown value of each statistic in `stats` order
  (null when the driver has none). `drivers.ratings.set` accepts any driver from either list.
- `seat.hasCar` is false for a reserve; `seat.position` is the seat order in the team (null when
  unset). `team` is null for a row with no team. `onGrid` is `seat.hasCar`; `player` is the human
  entrant.
- `name` (row label, e.g. `max_verstappen`), `initials` and `number` come from the gamedb
  `entrants.drivers` row with that `stat_id`; team `name` from `entrants.teams`; statistic names from
  `driver_statistics_info` (`StatisticInfo_N`), read with the db library's `string_at` / `int_at`.
  Anything that is not 1..63 bytes of printable UTF-8 is `null` (statistic names fall back to
  `stat_<infoStatId>`). `number` is the gamedb default number.
- `ratings` is keyed by statistic name. `value` is what the game shows (the game's own getter,
  clamped and rounded). `curve` is `"patched"`, `"base"` or `"dynamic"` (no curve). `base` is that
  curve's value at `progress`, one decimal (null for dynamic statistics). Op results add
  `dynamicLevel` and `boost` per statistic.
- `overall` is the game's own overall rating (`Career_Get_DriverOverallRating`, the weighted sum the
  contract calculator and Team HQ cards use), rounded.
- `progress` is the curve position the game uses (career clock / career length in minutes, 0..1).
- `contracts` is null outside My Team. It lists `DriverContractsMyTeamComponent`'s records as the
  save stores them. A contract record:

```json
{
  "driverStatId": 12, "oldTeamStatId": 3, "newTeamStatId": 1,
  "salary": 1200000, "signingBonus": 0, "teamPerfMoney": 0, "teamPerfRaceValue": 0,
  "successChance": 0.75, "negotiationBonus": 0, "isCounteroffer": false,
  "reason": 0, "weekendIndex": 5, "careerDayIndex": 140,
  "dayContractSent": 138, "dayContractDecided": 140, "accepted": true, "acknowledged": true,
  "clause": { "stat": 1, "target": 3 }
}
```

  `reason` is the game's `driverMoveReason` enum value (names not mapped yet), `clause` is null when
  the record has no performance clause. The contract length is not part of these records; it is not
  read yet.
- `recordedMoves` is the last 20 entries of the game's own move log (`driverMoveData` in
  `CareerDriverContractRenewalComponent`, saved with the career). It is `null` in Driver Career, where
  the game keeps no move log. `reason` 0 is a signing, 3 a driver
  benched in his team (the values the game's own signing code writes). `drivers.swap` adds two.
- `pendingSwaps` is always empty: a swap applies immediately.

## Op `drivers.ratings.set`

Args:

```json
{
  "driverStatId": 12,
  "ratings": { "pace": 97, "focus": 80 },
  "mode": "flat",
  "expect": { "pace": 96 }
}
```

- `driverStatId` (int, required): must be in the snapshot's `drivers`.
- `ratings` (object, required, at least one key): statistic name or `"<infoStatId>"` → target value
  (number, 0..100, then clamped to the component's bounds, vtable +0x268 / +0x270). The target is the value the game
  shows (`value`), not the curve value: the plugin subtracts the other terms (dynamic level, boosts,
  modifiers) first.
- `mode` (string, optional, default `"flat"`):
  - `"flat"`: every curve point gets the target, so the rating stays at the target for the rest of
    the career (no further evolution of that statistic).
  - `"shift"`: every curve point moves by the same amount (clamped to 0..1 normalised), so the value
    at the current progress hits the target and the driver's growth or decline keeps its shape.
  - `"restore"`: undo (see Undo).
  Dynamic statistics ignore `mode` (their `dynamicLevel` moves by the difference).
- `expect` (object, optional): statistic → the `value` the launcher showed. If the live value differs
  the op fails with `stale` and changes nothing.
- The player's own driver: allowed. In Driver Career the player's statistics go through the subclass
  getter (vtable +0x280 override), which may read the player's attribute sets (`playerAttributeSets`,
  XP perks) instead of the curve. If the read-back does not show the target the op rolls back and
  fails with `write_failed`, so the player's driver is never left half-edited.
- Safety: before writing, the op records every other driver's value for the touched statistics; after
  writing, any change on another driver rolls the edit back and fails with `write_failed`. Every
  statistic set of the driver is written (the default set and each context-aware set), so the value
  is the same in every race context.

Result `before` / `after`:

```json
{ "driverStatId": 12, "ratings": { "pace": { "value": 96, "base": 96.0, "dynamicLevel": 0.0, "curve": "base" } }, "overall": 92 }
```

Errors (`error` is `"<code>: <detail>"`): `no_component`, `bad_args`, `unknown_driver`,
`unknown_stat`, `bad_value`, `stale`, `write_failed` (the curve is empty or not writable, the
read-back did not show the target, or another driver's value moved; every write of the op is rolled
back first).

Undo: yes. Undo is `drivers.ratings.set` with the `before` values and `"mode": "restore"`, which writes
back the exact point values from before the edit (the plugin keeps the old points of the last 64 curve
edits in memory; without a match, for example after the game restarted, `restore` writes a flat curve
at the old value instead). For dynamic statistics undo moves `dynamicLevel` back by the difference.

Modes: My Team and Driver Career. Runs only while `ce::safe()`.

Status: implemented; needs in-game proof (log line `[career-editor] drivers.ratings.set ...`).

## Op `drivers.contract.set`

**unsafe**: a raw write of the money fields of one contract record, which the record serialiser
writes straight back to the save. No game code was found that caches a value derived from them, but
the payout readers were not traced. My Team only.

Args:

```json
{ "driverStatId": 12, "list": "decided", "newTeamStatId": 1, "salary": 2500000, "signingBonus": 0, "expect": { "salary": 1200000 } }
```

- `driverStatId` (int, required); `list` (`"decided"` default, or `"pending"`); `newTeamStatId`
  (int, optional) picks the record when the driver has several in that list.
- At least one of `salary`, `signingBonus`, `teamPerfMoney`, `teamPerfRaceValue` (int,
  0..1,000,000,000). Teams, days, clauses and the contract length are not editable.
- `expect` (optional): field → the value the launcher showed.

Result `before` / `after`: the contract record (shape above). Errors: `wrong_mode`, `bad_args`,
`no_contract`, `ambiguous`, `bad_value`, `stale`, `write_failed`. Undo: `drivers.contract.set` with
the `before` values. Runs only while `ce::safe()`.

Status: implemented; needs in-game proof (log line `[career-editor] drivers.contract.set ...`).

## Op `drivers.swap`

Swaps seats **now**, mid-season, with the game's own driver-move primitives in the game's own order.

- **My Team:** for each driver `Career_Record_DriverMove(renewal, driver, oldTeam, newTeam, reason)`
  (move log `driverMoveData` + `MessageDriverMoveConfirmed`), then `Career_Set_EntrantData_Thunk`.
  Both drivers must have a team (race seat or reserve); a teamless driver is refused (`no_team`).
- **Driver Career** has no move log. The op follows the Driver Career signing
  (`DriverCareerContractsManagerComponent`, 0x140d766b0):
  - **Race driver ↔ race driver:** for each driver post `MessageDriverMoveConfirmed`
    (`DriverMoveConfirmedMessage`, reason 0) then `Career_Set_EntrantData_Thunk`; then finalise.
  - **Free agent (teamless entrant row) → race seat**, the game's "replace old driver" sequence:
    1. the free agent: `MessageDriverMoveConfirmed` {driver, old team none, new team = the seat's
       team, reason 0}, then `Career_Set_EntrantData_Thunk` with the seat's team, car, seat value and
       position;
    2. the seat's driver: `Career_Set_DriverContractState(contract, day, 3, registry value)` on his
       contract (found in `DriverCareerContractOwner` by `DriverContract.driver`), then
       `MessageDriverMoveConfirmed` {driver, old team, new team none (0x3fff), reason 1}, then
       `Career_Set_EntrantData_Thunk` with team, car, driver registry value, livery and badge all
       0x3fff0000, seat value 0xffff0012, position 0. He is now a free agent exactly as the game makes
       one.
  - **Finalise** (what every Driver Career signing batch ends with): `Career_Build_LineupChanges`
    (current line-up vs the weekend's), then post `MessageDriverMovesFinalised_upgrades`,
    `MessageDriverMovesFinalised_recognition` and `MessageDriverMovesFinalised`
    (`DriverMovesFinalisedMessage`). The `_upgrades` handler (UpgradesComponent 0x14123b3f0) creates
    the team modifiers a driver new to a race seat lacks (`Career_Init_TeamModifiers`) and re-assigns
    per-driver R&D. Without it the free agent's R&D is costed with no modifier list: the
    `Career_Calc_UpgradeCost` access violation of 2026-10-20 18:32 (`mods` = 0, read at +0x62).
  - **Refused in Driver Career:** reserves (a team but no car), `reserve_dc`. Driver Career R&D is
    per race driver (0x14351ccb0 costs only drivers in race positions; the modifier list is created
    only through the finalise path for race-seat changes), and no game path was found that moves a
    reserve this way.

What follows the seat: team, car (vehicle keys, so livery and car), seat value, seat position. What
stays with the driver: the row identity (driver appearance, helmet, suit), driver data, ratings,
driver flag. The car number stays with the driver (EntrantRecord.car_number is written from the incoming driver's own value, as the game's own signing does); the seat value (+0x78) comes from the replaced seat.

Args:

```json
{ "a": 12, "b": 40, "expect": { "aTeamStatId": 1, "bTeamStatId": 3 } }
```

- `a`, `b` (int, required): `driverStatId`s. Both need an entrant row (`seat` not null in the
  snapshot). My Team: race seat ↔ race seat on two teams, reserve ↔ race seat (including promoting
  a reserve inside his own team). Driver Career: race seat ↔ race seat, and free agent (no team) ↔
  race seat.
- `expect` (optional): the teams the launcher showed; a mismatch fails with `stale`.

Refused: `no_team` (My Team: a driver has no team), `reserve_dc` (Driver Career: a reserve),
`player_locked` (either row is the human entrant), `same_team` (two race drivers of one
team, or two reserves of one team), `no_seat` (neither has a car), `no_entrant` (a driver who is not
in the line-up: listed under `otherDrivers`, with no entrant row and so no seat; the error text says
so and points to the `drivers` list. The game's entrant writer only
rewrites existing rows and never adds one; no row-creating path was found in the game's signing
code), `refused` (the game has `disableDriverMoves` set), `busy` (the game is mid entrant
update), `no_component`, `bad_args`, `stale`.

Checks: before writing, each row must be the only row the game's entrant compare would match (same
human bit and, for AI rows, the same identity word at +0x48); otherwise the op is refused with
`refused` and writes nothing. The game's writer posts its begin message before it searches and only
ends the update when a row matched, so a miss would leave the game stuck mid entrant update. Each
write is checked again right before it runs. After the four calls the op re-reads the entrant table. Both rows must have kept their
identity and address and carry the other's team and car, and every other row must be byte-identical.
Otherwise it writes both rows back to their old data (recording the reverse moves) and fails with
`write_failed`.

Result `before` / `after`:

```json
{ "a": { "driverStatId": 12, "teamStatId": 1, "seat": { "vehicle": 65538, "hasCar": true, "position": 0 } },
  "b": { "driverStatId": 40, "teamStatId": 3, "seat": { "vehicle": 196610, "hasCar": true, "position": 1 } },
  "appliedNow": true }
```

Applies immediately (`appliedNow: true`); nothing is queued, so `pendingSwaps` stays empty.

Not changed by the op: contract records (`decided`/`pending` keep the team they were signed with), the
driver's contract held by the renewal component (the game's own signing writes a new contract there;
this op does not), and the driver market's own lists. The game's listeners get the driver-move messages; whether the market
re-derives its seat cache from them is what the in-game test has to show (see the test steps).

Undo: `drivers.swap` with the same `a` and `b` (the swap is symmetric; it records two more moves).

Modes: My Team (move log + message) and Driver Career (message only). Runs only while `ce::safe()`.

Log lines, Driver Career:
`[career-editor] drivers.swap driver <A> [team <X> car 0x<cx> pos <p>] <-> driver <B> [...] -> driver <A> [team <Y> ...], driver <B> [team <X> ...] (Driver Career: 2 moves announced, moves finalised; <N> other entrants unchanged, applied now)`
and for a free agent: `... (Driver Career replace: incoming announced, outgoing contract released and announced to no team, moves finalised; ...)` (the free agent is printed first; a teamless row shows `team 16383 car 0x3fff0000 pos -1`), each followed by
`[career-editor] drivers.swap finalised <n> line-up change(s) (MessageDriverMovesFinalised_upgrades, _recognition, MessageDriverMovesFinalised)`.
My Team says `2 moves recorded` and posts no finalise.

Status: implemented; needs in-game proof. Test: swap two AI drivers on different teams in Team HQ,
check the Team HQ grid / standings driver list, save, reload the career, advance to the next weekend
and load into a session.

## Bindings

| KB name | VA | level | use |
|---|---|---|---|
| `Career_Get_DriverOverallRating` | 0x14113d120 | L2, sig `career.get_overall` | `overall` in the snapshot and in op results |
| `Career_Eval_Curve` | 0x1445075d0 | L2, sig `career.eval_curve` | curve value at `progress`, lookup-table rebuild |
| `Career_Get_CareerLengthDays_Thunk` | 0x14111dd30 | L2, sig `career.get_career_length_days_thunk` | career length in minutes, divisor of the clock for `progress` |
| `Career_Get_DriverStatValue` | 0x141140ee0 | L2, no sig | called through the component vtable +0x280 (never by address) |
| `Career_Get_DriverStatBase_Thunk` | 0x1411471f0 | L2, no sig | evidence for the curve choice and the dynamic-stat rule |
| `Career_Parse_CurveString` | 0x141157100 | L2, no sig | evidence for the lookup-table rebuild |
| `Career_Serialise_DriverContractList` | 0x140da5610 | L2, no sig | evidence for the contract record layout |
| `Career_Set_EntrantData_Thunk` | 0x1407b72e0 | L2, sig `career.set_entrant_data_thunk` | `drivers.swap`: rewrites one entrant row |
| `Career_Record_DriverMove` | 0x140d8f1f0 | L2, sig `career.record_driver_move` | `drivers.swap` (My Team): move log + driver-move message |
| `vt_Career_MessageDriverMovesFinalised`, `…FinalisedUpgrades`, `…FinalisedRecognition` | 0x145696d10 / d20 / d30 | L2, sigs `career.message_driver_moves_finalised*` | Driver Career finalise |
| `Career_Build_LineupChanges` | 0x142457a90 | L2, sig `career.build_lineup_changes` | Driver Career finalise payload |
| `Career_Set_DriverContractState` | 0x140d9de30 | L2, sig `career.set_driver_contract_state` | Driver Career replace: release the outgoing contract |
| `vt_Career_MessageDriverMoveConfirmed` | 0x145696d40 | L2, sig `career.message_driver_move_confirmed` | `drivers.swap` (Driver Career): the move message, posted with `swofty::career::post` |
| `Career_Sign_DriverToTeam` | 0x140d8fcb0 | L1 (DISPUTE V0294), no sig | evidence for the call order only; not called. Its `offer` is a 0x2c-byte contract record it also writes into the driver's contract, and it first backfills the incoming driver's old seat with that team's reserve (no reserve: it signs nothing) |
| `Career_Process_PendingDriverMoves` | 0x140db18e0 | L1, no sig | evidence (season-change processor, not called) |
| `Career_Assign_EntrantTeamSlot` | 0x140db70a0 | L2, no sig | evidence (only valid inside the processor: it works on the processor's local seat map) |

KB structs (re/db/structs, all fields L2, raised by ce-drivers): `DriverStatisticsComponent`,
`DriverStatsEntry`, `ContextStatSet`, `DriverStatistic`, `EvolutionCurve`, `CurvePoint`, `StatInfo`,
`DriverContractsMyTeamComponent`, `ContractRecord`, `CareerDriverContractRenewalComponent`,
`DriverMoveRecord`, `EntrantWriteData`, `DriverCareerContractsManagerComponent`,
`DriverMoveConfirmedMessage`, `DriverMovesFinalisedMessage`, `LineupChange`,
`DriverCareerContractOwner`, `DriverContract`, and `EntrantRecord` (extended). Read from other lanes:
`CareerEntity` (economy), `CareerTimeComponent` (economy).

Snapshot status: implemented; needs in-game proof (no log line; check `snapshot.json`).
