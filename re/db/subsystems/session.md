# session: entrants, participant selection, weekend start, garages

Owner: see `subsystems.tsv`. Write-up: research/grid24-cap.md §2–3. Goal: 24-car (later 26-car) grids.

## Map: how a Grand Prix gets its cars {#participants}
1. GP mode row → game setup mode `f1_26` → entrant setup mode row 8 (`entrants.entrant_setup_modes.entrant_setup_mode_8`), parsed by `Sess_Parse_EntrantSetupMode` into `EntrantSetupMode` (0x288; `max_entrants` +0x268 stored at `site_Sess_MaxEntrantsStore`).
2. `Sess_Setup_ChampionshipEntrants` (0x140ba3ad0): `EntrantsManager.setup_mode`, `.selectable_list` = first selectable list.
3. Entrant lists are parsed by `Sess_Parse_EntrantList` (0x142453c40) for `i < row_count` (`gamedb#instancecount`): **the 22-car cap is `entrant_list_34._entrant_instancecount_ = 22`, data, not code**.
4. `Sess_Build_CandidatePool` (0x14245fb70) filters by car class, then `Sess_Select_Entrants` (0x1424a2350): `limit = reuse_entrants ? max_entrants : min(max_entrants, #candidates)` (load at `site_Sess_SelectorLimitLoad`, 0x1424a30a8: rax → limit, [rsp+0x60] = EntrantSetupMode), MT draw, player insert, team prune (`max_teams`), entrant prune.
5. `Sess_Weekend_Start` (cxx `game::championship::Weekend::start`) copies entrants into the session vector; no fixed size.
6. Garages: `Sess_SortGarages_Thunk` → `.data1` body → `Sess_Assign_GarageSlotsByClass` → `Sess_Assign_GarageSlotsByPreference` (one garage per team, slots 0,1,2…).

## Caps found
| cap | kind | where |
|---|---|---|
| list `_entrant_instancecount_` | data | `Gdb_Init_TableCursor` / `Sess_Parse_EntrantList` |
| teams/vehicles `_instancecount_` | data | same reader |
| `max_entrants`, `max_teams`, `reuse_entrants` | data | `EntrantSetupMode` |
| benchmark `num_cars` [1,26] | code, dev only | `site_Sess_BenchmarkNumCarsClamp` |
| "Max ai participants" | code, debug slider | `Sess_Build_MaxAiParticipantsOption` |
| world garages / grid slots | data (WEP) | 24 slots / 12 garages in `_f1_26_myteam` |

## Open (towards 26 cars)
- Per-car pools beyond the session vector: physics, AI, replay, results, HUD timing tower. Start from `Sess_ParticipantVec_PushBack` (0x140af2ab0, ~12 callers) and `Sess_Unk_141489440` (`session.cars` VM binding).
- Front-end team/driver list reader (shows T12 although rows were outside the counts).

## Inbox
- 2026-10-17 12:09 re-infra: 0x141489440 binds session.cars; worth a look for the 26-car UI audit
