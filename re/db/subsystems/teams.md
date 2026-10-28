# teams: team identity, side data, logos and badges, team count

Owner: see `subsystems.tsv`. Prefix `Team`. Scope: what the game needs to load an entirely new team from records alone
(the `entrants.teams` row with all its keys, every side table the game finds by team label), the loaded team record
behind `Sess_Load_Teams`, team colours, team logos and badges per context (the `overrides.uidb` logo arrays, their baked
`.texarray` resources, the registry-path format sites that pick a slice), and what limits the team count. The entrant
roster and identity objects stay with `entrants`; the participant selection pipeline stays with `session`.
Design for the core's team and logo appliers: work/std16/RE-TEAMS.md (std16 worktrees, gitignored).

## The loaded team record {#team-record}
`Sess_Load_Teams` (0x142469440, L2, session lane) is called with (entrants system, set): 0x142483002 passes set 0,
0x142461443 passes set 1. It replaces the set's slot vector at `system+0x2c8+set*0x10` (pair `{allocator +0x2c0,
slots* +0x2c8}`) and walks the `entrants.teams` cursor (`_team_instancecount_`, 358 stock). Every row whose `available`
is true becomes one `TeamRecord` (0x768 bytes, `structs/TeamRecord.h`), stored by `Team_Insert_RecordByStatId`
(0x140cdbd40, L2) at `slots[stat_id]`: a heap vector of 0x10-byte `{allocator, TeamRecord*}` entries that grows to the
stat id. A second row with the same stat id is dropped (the insert returns 0). There is no fixed team array: the team
count is bounded only by the u16 stat id and the row count key.

What the loader needs from a row, and how it fails:
- Strings (`display_name`, `_code`, `_short`, `_super_short`, `timing_tower_texture_path`, `livery_name`,
  `team_base`): absent = empty string. Copied into 0x80-byte holders (longer values are cut at 127 characters).
- Colours (`primary/secondary/tertiary_colour`, `team_wear_*_colour`, `ui_team_colour`): read as
  `*(int32*)(*slot + 8)` **without a null check** (0x14246adc6..0x14246ae86): a row missing any of the 7 colour keys
  crashes the loader. Values are the database's signed ARGB int32 (type 11), stored unchanged.
- Labels resolved to stat ids by `Gdb_Find_RowByLabel`: `nationality` (entrants.nationality, u8), `team_boss`
  (entrants.characters, u8, 0xff when empty), `expression_label` (entrants.expression_label, u16), `animation_set`
  (entrants.animation.driver_animation_sets, u8). Unresolved labels give 0 (the value slot is null-checked).
- `prestige` (optional int, kept when >= 0), `audio_id` (u8), `show_team_badge`, `show_custom_badge_in_UI`,
  `team_colour_valid` (bools, default 1 for the last).
- `story_info`: the nested table `<row>.story_info_N` (`description`, `hashtag`); a missing
  `_story_info_instancecount_` means no story (the record's `has_story` stays 0), not a crash.
- Optional `<row path>.paddockMarker` (string, `Gdb_Lookup_Key`); no stock row has it.
- `available` is false in the file for every team; the startup availability pass (`Sess_Apply_ContentAvailability`
  0x1413e4cf0, L1) sets it at runtime, count-bounded, for teams whose stat id is on a content list (B26f and Custom
  Grid Size teams became available through the entrant list that names them; not traced, L1).

Consumers reach a team as `[slots + stat_id*0x10 + 8]` → `TeamRecord*`, e.g. the logo key format sites
(`mov rdi,[rcx+rax+8]; mov r8,[rdi+0x18]` at 0x1416a3ad9 and 0x141879892): every logo and badge key is built from
`TeamRecord.label`, the row's `_metadata_`.

Runtime team sets: `Ent_EntrantsSystem_AddTeam` (0x142474a60, entrants lane, L1) copies a record body and inserts it
into set 1 (`system+0x2d8`) through `Team_Insert_RecordByStatId` (0x142475392); `Ent_EntrantsSystem_RemoveTeam` and
`_ReplaceTeam` refuse set 0 ("Not allowed to remove preloaded team"). The team key is `{u8 set, u8, u16 stat_id}`.

## When a team is loaded: availability {#team-available}
Every `entrants.teams` row has `available=False` in the file. `Sess_Apply_ContentAvailability` (0x1413e4cf0, L2,
single caller 0x1413ed3d5) collects team keys `{u8 set, u8, u16 stat_id}` from every career setup component's team
(component+0x2e0, 0x1413e9361) and from every `EntrantRecord.team` (+0x58) of the loaded entrant lists (0x1413e945f),
then walks the `entrants.teams` cursor (rows `0.._team_instancecount_-1`) and sets `available=True` on each row whose
`stat_id` matches a collected key with set 0 (0x1413e9fe9). `Sess_Load_Teams` then loads only those rows. A new team
is therefore loaded when an available entrant list has an entrant whose `team` label is the new row's `_metadata_`
(and the row lies below `_team_instancecount_`, which bounds both this pass and `Gdb_Find_RowByLabel`).

## Inbox
- 2026-10-28 19:15 re-teams: re-teams fork C 2026-10-28, team count: GP/TT/career team counts are bounded by data and the world, not by fixed code arrays. Data: the entrant list's _entrant_instancecount_, max_entrants, max_teams (-1 = none), entrants.teams/vehicles instance counts, <=32 entrant lists per setup mode. World: the track layer (track_data.track_layers, chosen by game setup mode + season; f1_26 row 8 serves f1_26/f1a_mp, myteam_26 row 9 serves My Team) decides the garages, grid slots and pit boxes; garage index = position in the team order list with no bound (Sess_Assign_GarageSlotsByTeamOrder 0x1424a8830 -> Sess_Assign_GarageSlotsByPreference reads *(garages+8)[i]), unlisted teams past the garage count get no garage, and a garage holds 2 cars (Sess_Set_GarageSlotEntrant_Thunk ignores slot >= 2). Code limits already known: 30 AI vehicle names (grid_capacity redirect), vehicle table 32 with the 27 clamp on session enter. Session grouping (Sess_Group_EntrantsByTeam_Thunk) and the standings are growing vectors. Details: scratch C-count.md, folded into work/std16/RE-TEAMS.md.
