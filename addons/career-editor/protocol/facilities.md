# Facilities

Owner: `native/src/facilities.cpp` (ce-rnd). My Team only. Status: implemented, compiled; needs in-game
proof.

Both ops are `"unsafe": true`: besides the game's own completion call they store one or two node fields
directly (`days_remaining = 0`, and for `set_level` also `purchased = 1` and `seen_completed = 1`), the
same stores the game's daily pass and its unlock-at-start pass make. The serialiser saves those fields
as they are. If the game refuses a completion, both fields are put back.

Errors: `mode_unsupported` (not a My Team career), `not_ready` (no career, or the facility data did not
match the expected layout; `facilities.reason` says which), `no_team`, `bad_args`, `no_facility`,
`no_node`, `wrong_state` (the named node is not being built), `nothing_building`,
`nothing_to_complete`, `game_refused`.

Facilities are named by `facility`: `0` / `"engineering"`, `1` / `"personnel"`, `2` / `"corporate"`
(the gamedb `facility_type`). `team` defaults to the player's team (`"player"`); any key from
`facilities.teams[].team` also works.

Log: one line per node,
`[career-editor] facilities.<op> team=0x… <facility> node <stat id> "<name>" phase <n>: <from> (<d> days left) -> <to>`,
plus a summary line per op.

### `facilities.complete`
Args: `{ "team"?, "facility"?: 0..2|name (default: all), "node"?: u32 stat id }`.

Finishes builds now. For each node being built (unlocked and purchased) in the selection, it sets
`days_remaining` to 0 and calls the game's node completion, `Career_Complete_PerkTreeNode`, exactly as
the daily pass does when a build runs out of days. The game then:
- marks the node complete;
- applies its reward set (department tier attributes, workforce capacity, perks) and stamps
  `day_completed`;
- posts `MessageFacilityUpgradeComplete`, so the facility ceremony and the reward listeners run;
- unlocks the child nodes.

With `node`, that node may also be an available one that has not been bought: it is then bought for
free and completed the way the game's unlock-at-start pass does it (`purchased = 1`, the completion
call, `seen_completed = 1`). `Career_Complete_FacilityStartNode` itself is not called: it only acts on
nodes flagged unlock-at-start. A locked or already built node returns `wrong_state`. Without `node`, only
builds in progress are finished, and none in progress returns `nothing_building`.

Result `before`/`after`: `{ "team": u32, "f": [<facility as in the section, without "nodes">] }`.
Undo: backup-only.

### `facilities.set_level`
Args: `{ "team"?, "facility": 0..2|name, "level": 1..4 }`.

Completes, in up to 16 passes, every unlocked node of that facility whose `phase` is at most `level`,
including nodes that are not bought yet (they become bought for free), until no more unlock. It uses
the same completion call, after `purchased = 1` and `days_remaining = 0`, the pattern the game's
unlock-at-start pass uses. Nodes it bought itself get `seen_completed = 1`, as that pass sets, so a
large jump doesn't queue one ceremony per node. Builds already running keep their ceremony. Lowering a
level is not offered.

`after` shows the facility's new `level` (the highest phase among complete nodes).

Alternative branches: every unlocked node within the phase is completed, so a branch the player would
normally choose between is completed on both sides where the tree allows it. Nodes that revoke a lower
tier are handled by the game's own reward code. Undo: backup-only.

### Snapshot section `facilities`
About 12 KB for a My Team career (3 facilities, about 109 nodes; every node of each tree, top-level
nodes and their descendants): the player's facilities in full with
short keys, every team as a level summary. Optional keys appear only when non-zero.
```json
{
  "ready": true,                  // false: "reason" = "no career loaded" | "not a My Team career" |
                                  //   "no facility data" | "team map layout not as expected" | "team map count mismatch"
  "available": true,              // a My Team career (the component exists)
  "team": 1234567,                // the player's packed team key, or null
  "f": [{                         // the player's facilities
    "ty": 0,                      // 0 engineering, 1 personnel, 2 corporate (the `facility` arg)
    "lv": 2,                      // level: highest phase among built nodes
    "hp": 2,                      // highest_phase_seen
    "wc": 40,                     // workforce_cap, only when the game has set one (workforce_cap_set)
    "nt": 36, "nb": 9, "na": 3,   // nodes total / built / available to buy
    "bld": [{ "i": 5, "dl": 3, "dd": 14 }],   // builds in progress: node id, days left, days total
    "nodes": [{ "i": 5, "n": "lng_facility_…", "s": 2, "p": 1, "lv": 2, "mx": 5, "c": 1500000, "dd": 14, "dl": 3 }]
  }],
  "teams": [{ "team": 1234567, "player": true, "lv": [[0, 2], [1, 1], [2, 1]] }]   // [type, level] per facility
}
```
Node keys: `i` stat id (the `node` arg), `n` name_loc_tag (a language key), `s` state (0 locked,
1 available, 2 building, 3 built), `p` phase, `lv`/`mx` UI level and max level, `c` gamedb base cost
(the game applies its cost modifiers when the player buys), `dd` days to complete, `dl` days left (only
while building).

### Engine paths
KB structs (re/db/structs, all L2): `MyTeamFacilitiesComponent` (team map slots, control bytes, count),
`FacilityTeamEntry`, `FacilityTeamData` (facilities, team key), `MyTeamFacility` (type, tree, workforce
cap and its set flag, highest phase seen), `FacilityPerkTree` (top-level nodes), `FacilityNode` (every
node field the ops read or write, and the child vector). The node list is the tree's top-level nodes
plus every descendant through each node's child vector (depth-first, at most 32 levels and 512 nodes,
each node once). facilities.cpp reads and writes through them; the only constants left are the
shared/weak-pointer element size and use-count offset (library layout) and the sanity bounds.

| binding | level | role |
|---|---|---|
| `ce::comp::get(MyTeamFacilities)` | — | the component. Its team map: slots `+0x2b8`, one control byte per slot at `+0x2c0` (non-zero = used, slots end where the control bytes begin), count `+0x2c8`. A slot points at an entry whose `+0x08` is the team data (0x80 bytes, built by the deserialiser 0x140f84f20). Team data: facility vector (16-byte shared pointers) at `+0x10..+0x18`, packed team key at `+0x28` |
| facility object (0x1f8, built by 0x140f7cfe0) | — | `+0x00` facility_type (0 Engineering, 1 Personnel, 2 Corporate: ci-FNV of the gamedb value), `+0x08` perk tree, `+0x18` team key, `+0x50` workforce_cap, `+0x54` workforce_cap_set, `+0xd4` prevCapacity, `+0x1d8` highest_phase_seen |
| perk tree | — | node list as 16-byte weak pointers at `+0x08..+0x10` (copied by 0x140b537a0 for the serialiser and the daily pass) |
| `vt_Career_FacilityNode` 0x1456af450 | L2 | node class (0xe8). `+0x08` state 0/1/2, `+0x0c` stat id, `+0x20..+0x28` children, `+0x40` phase, `+0x80` cost, `+0x88` name_loc_tag, `+0xa8` days_to_complete, `+0xac` days_remaining, `+0xb0` purchased, `+0xb3` seen_completed, `+0xb5` unlock_at_start, `+0xd8` day_completed, `+0xdc`/`+0xe0` ui_level/ui_max_level (parser 0x140f8a800, serialiser 0x140f8e8d0). The ops only touch nodes with this vtable |
| `Career_Complete_PerkTreeNode` 0x142ea5fe0 | L2 | node completion: state 1 → 2 when `isComplete` (vtable `+0x20` 0x140f8e5d0: `days_remaining == 0 && purchased`), claim rewards (vtable `+0x38`), children unlocked |
| `Career_Tick_FacilityNodes` 0x140f80f60 | L1 | daily pass: on purchased nodes not complete, decrements `days_remaining` and at 0 calls the completion |
| `Career_Claim_FacilityPerkRewards` 0x140f8d8d0 | L1 | facility node `+0x38`: applies the reward set, stamps `day_completed`, posts `MessageFacilityUpgradeComplete` |
| `Career_Complete_FacilityStartNode` 0x140f8a1a0 | L1 | unlock-at-start pass: `purchased = 1`, completion, `seen_completed = 1` (the pattern `set_level` follows) |
