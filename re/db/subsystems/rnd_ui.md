# career R&D UI and the department-4 engine paths

Owner: career lane (kept apart from `career.md`). Build `d873a1c8…`, image base `0x140000000`,
`.xtls` code. Companion to the `swofty.rnd-tyres-thermal` addon (`addons/rnd-tyres-thermal/`,
whose `native/HOOKS.md` keeps the hook-by-hook detail and log lines). This file keeps the engine
facts: what each R&D surface is built from, where the department count is fixed at 4, and which
code paths refuse department 4.

Levels: every item added from the addon work is **L1**. The instruction patterns and E8 callers
were re-checked by disassembly on this build. The addon's live gdb and `/proc/<pid>/mem` reads are
not filed as `T###` traces, so nothing here is L3.

The whole R&D UI is keyed by the per-upgrade **ENUM A** (`upgrade_department`): `0=aero 1=chassis
2=durability 3=engine 4=Fake` (ci-fnv1a switch @0x14353a6dd, ~27 inlined copies; `Invalid`=5,
unknown token = 0). Value 4 (`Fake`) is the unused spare that the colour tables' `[4]`, the
dept-code table entry 4 and `k_Career_UpgradeDeptNameTable[4]="fake"` line up on. The team
`department`=Tyres / ENUM B=4 (`site_Career_TeamDeptEnumStore`) is a separate axis and is not read
by any R&D screen. A "5th department" is the code-4 slot made real.

## Where 4 is fixed (summary) {#fixed-four}

| layer | where | how |
|---|---|---|
| departments screen cards | `Career_Build_RnDDeptBoxGrid` 0x141c85f70 | 4 literal card ctor calls, card vector sized to 6 |
| name lookups | `k_Career_RnDDeptNameTable` 0x145755e58 (4 entries) | index 4 reads `colour[0]` as a `char*` |
| upgrade instances | `Career_Reset_UpgradeInstance` path @0x143517e0c | dept 4 gets a disabled sentinel |
| state mutators | 5 functions, `cmp byte [x+0x72],3 ; ja ret` | dept > 3 never changes state |
| availability graph | `Career_Search_UpgradeGraph` @0x143528a05 | dept 4 = the tree hub |
| team modifiers | `Career_Init_TeamModifiers` / `Career_Init_SharedTeamModifiers` | entries created for depts 0..3 only |
| Back / open-by-name | `Career_Dispatch_RnDContainerCommand` open case | names other than the 4 stock + 2 panels are ignored |
| pie object | `Career_Construct_RnDPieObject` 0x14180c490 | exactly 4 slots constructed |
| pie populate, tile counts | 0x141f48218, 0x141f49c23/0x141f49c28 | `cmp …,3 ; ja` guards |
| colour tables | 6 tables, bound `< 6` | `[4]` is white |

The data layer has no such limit: both upgrade loaders read code 4, and
`Career_Load_DepartmentEntries` loops the DB instance count.

## Surface 1 — the departments screen cards {#box}

- Builder: `Career_Build_RnDDeptBoxGrid` @0x141c85f70 (no E8 callers; VM-dispatched) fills
  `view_model_data.laptop_r_and_d_departments_container` (`flow_r_and_d.erp`). It builds **4 dept
  cards** (`Career_Construct_RnDDeptCard` 0x141c7aac0; enums 0/1/3/2) + **2 panel cards**
  (`tab_rnd_upgrade_purchase_panel`, `tab_rnd_perks`) into a pointer vector at builder+0x280..+0x290,
  then an attach loop calls `FUN_14431d720(builder+0x2d8, card+0x268, 0, 0)` per card. Native
  assigns no rect; it parents the card node into the container.
- Card ctor args `(card, ctx, builder, allocator, name_lang_key, icon_atlas_path, enum)`; the icon
  path is `<pkg>.rnd_dept_icons_small.<frame>`. A card needs no per-department authored node.
  Card fields: icon +0x818, name +0x830, enum (dept) +0x6d8, team +0x6bc, Upgrades model +0x688,
  colour +0x948 = `k_Career_RnDDeptColourTable[enum]` (bound `< 6`). Card vtable
  `vt_Career_RnDDeptCard` 0x145740250, size 0x950.
- The container (builder+0x2d8) is a UITC tab container. Its slot vector is at
  builder+0x548..+0x550 (= container+0x270), 0x10 per entry: entry+8 = slotObj, slotObj+0x20 =
  bound card node (`card+0x268`), slotObj+8 = inner content node. The tab headers stack into an
  `eStacker` with no authored children (`components/smart/primary_toggle_navigation.idf`), so a
  card injected after the 4th stock card gets `slot[6]` and appears as a tab (addon live run).
- First tab: the builder reads `g_Career_RnDOverviewSelectedDept` clamped `< 4` (@0x141c86c87)
  for the name, then joins the selected card at 0x141c86d03 (`mov rax,[rax] ; mov [rsi+0x7b8],rax`).

## Surface 2 — the upgrade tiles {#tiles}

The tiles are data-bound in `screens/laptop_r_and_d_departments.ui`:
`tab_rnd_department_panel` has `row_spawner [F1TS] dataContext=@rows`; each row is
`rnd_category_row` with `tile_spawner [F1TS] dataContext=@tiles` → `rnd_card_with_info` cards,
placed by magnets + `grid_coordinates`.

**Data layer — includes code 4:**
- `Career_Load_UpgradeRows` @0x143539bc0 — flat 0x80-byte `UpgradeRecord` vector.
- `Career_Load_UpgradeTreeNodes` @0x14353b030 — the tile data source: reads `grid_coordinates.x/y`
  + every column into the combined tree model at UpgradesComponent+0x68+0x28.

Both loop `k_Career_UpgradeDeptCodeTable` (0x145735d60, codes 0..4, break at 5) with no code-4
exclusion, opening `<team_link>.fake.upgrade` (`team_link` = the team's engine-supplier tree token,
e.g. `upgrades.2026.trees.mercedes.fake`) by `_upgrade_instancecount_` (`Gdb_Init_TableCursor`).
Node links resolve via `Gdb_Find_RowByLabel` (0x1445a2bd0, scans `0..instancecount-1`), so rows
past the instance count are invisible to link resolution. The stock `fake` container holds the tree
hub `central_fake_upgrade` (label `default`, instancecount 1); every branch root hangs off it by an
outward `links` edge, so a new branch is reachable only when the hub's `links` names its root.

**Display layer:** the page is built per department card by `Career_Build_RnDDeptPage` and filtered
by the instance query (§page). The tiles of department 4 are missing because its upgrade
**instances** never become enabled (§instances), not because the page or the selection index
refuses 4. `g_Career_RnDOverviewSelectedDept` does not choose the page: a hardware watchpoint showed
it unchanged while the department pages build.

Probe: `site_Career_RnDTreeRowCountProbe` @0x14353b1af (`cmp dword [rsp+0x378],0`, the first
row_count test after `Gdb_Init_TableCursor`; `[rsp+0x378]` = cursor row_count, `[rsp+0x58]` =
&current dept code) reads how many nodes each department's source yields.

## The department page and the instance query {#page}

`Career_Build_RnDDeptPage` @0x141c7b880 (pdata 0x141c7b880..0x141c7f997, called from 0x141c7b3d0
@0x141c7b6f4) builds one department tab. `rsi` = the 0x950 department card: dept =
`byte [card+0x6d8]`, team = `dword [card+0x6bc]`, per-team model U = `[card+0x688]`.
At 0x141c7d8cf it calls `Career_Query_EnabledUpgrades(U, out=rsp+0xdf0, &team=rsp+0x8c, 0xffff)`
and keeps the results whose `byte [u+0x72]` equals the card department. At 0x141c7dcb5 it reads
the team modifier list (`Career_Get_TeamModifiers`) for the page's research-slot count.

`Career_Query_EnabledUpgrades` @0x1435188d0 (86 E8 callers) keeps an instance only if its last
state entry with `day <= mask` has **enabled (+0xc) != 0**. In mode 0 it first calls
`Career_Set_UpgradesOwner` (@0x14351892b) when the owner differs from the cache.

Live count on a stock career (Durability page): 95 results, by dept `0:25 1:23 2:24 3:23 4:0`.

## Upgrade instances and the department-4 sentinel {#instances}

Per-team model `Upgrades` (0x1b10 bytes) is created by `Career_Add_TeamUpgrades` @0x14123ca40
(`mov edx,0x1b10` @0x14123cad4): loads the record model, then `Career_Instantiate_UpgradeTrees`
@0x14352cac0, then `Career_ResetAll_Upgrades` @0x14352e1c0.

| off | field |
|---|---|
| +0x08 | u8 mode (0 = one shared tree with an owner cache, 1 = per team, 2 = per driver) |
| +0x0c | u32 team id |
| +0xe0 + k*0xc40 | tree k (k < 2): +0x08 allocator, +0x10/+0x18 instance vector (0xe0 stride), +0x30 inline pointer array (0x180 slots), +0xc30 count; so array `U+0x110+k*0xc40`, count `U+0xd10+k*0xc40` |
| +0x1960 | u64 number of trees |
| +0x1970 + k*0x58 | tree graph (nodes 0x40 stride: [0] = instance, +0x10/+0x18 neighbours, +0x30 grid x/y) |
| +0x1a60.. | robin-hood map owner → tree slot (mode 2) |
| +0x1a9c | u16[6] per-department counters (index 4 in bounds) |
| +0x1aa8 | u32 owner cache (mode 0); 0x3fff0000 = none |

`Career_Instantiate_UpgradeTrees` copies **every** 0x80-byte `UpgradeRecord` into a 0xe0 instance
(no department filter), runs the per-instance reset on the stack temp (call @0x14352d071), moves it
into the vector and pushes the pointer. The stores are indexed off `param_1` with the slot folded
into the index, so a scan for `[..+0xd10]` writes misses them.

Upgrade instance (0xe0):

| off | field |
|---|---|
| +0x08..+0x57 | SSO `upgrade_name` (+0x10 = chars) |
| +0x58 | u32 node hash = djb2 (seed 0x1505) of `"%s_%d" % (_metadata_ label, row flag)` |
| +0x70 / +0x71 / +0x72 | type / size / **department (ENUM A)** |
| +0x74 | `unlock_at_facility_level` |
| +0x7c | row flag: 0xff = team tree row, else engine-supplier stat id |
| +0xb8 | allocator; +0xc0/+0xc8/+0xd0 = state entries begin/end/cap (0x10 each) |

State entry (0x10): +0 u32 owner (0x3fff0000 = none), +4 u16 day, +8 u8 state, +9 fitted,
**+0xc enabled**, +0xd facility-locked. State names `k_Career_UpgradeStateNameTable` @0x14565cee0:
0 Unavailable, 1 Available, 2 InProgress, 3 Completed, 4 Failed, 5 Invalid, 6 PendingDevelopment,
7 InDevelopment, 8 PendingFitting.

Three layers keep a department-4 instance out, all on `byte [u+0x72]`:
1. **Init sentinel.** The per-instance reset 0x143517d80 (KB name `Car_Reset_LastFittedUpgrade`,
   car lane; callers: instantiate @0x14352d071, reset-all @0x14352e223, supplier refit
   @0x143515290) branches at `site_Career_UpgradeInitFakeDeptBranch` 0x143517e0c
   (`cmp byte [rcx+0x72],4 ; jne 0x143517e4c`). Department 4 gets one entry
   `{owner 0x3fff0000, day 0xffff, Invalid, enabled 0}` and returns, skipping the normal path
   (push Unavailable at day 0, set fitted, set enabled 1, set facility-locked). This is how the
   stock hub stays off every page.
2. **Mutators refuse department > 3** (`cmp byte [rcx+0x72],3 ; ja <ret>`):
   `Career_Push_UpgradeState` 0x1435230e0 (gate 0x1435230f3, log "Changing state from %s to %s"),
   `Career_Set_UpgradeEnabled` 0x143523510 (0x143523520), `Career_Set_UpgradeFacilityLocked`
   0x143523730 (0x143523740), `Career_Set_UpgradeFitted` 0x1435157d0 (0x1435157e0),
   `Career_Run_UpgradeAutoFit` 0x1412484e0 (0x1412486b8). A dept-4 instance never leaves the sentinel.
3. **Hub test.** `Career_Search_UpgradeGraph` 0x1435288f0 treats every dept-4 node as the hub
   (`cmp byte [rax+0x72],4 ; je` @0x143528a05). Used by `Career_Refresh_UpgradeAvailability`
   0x14352f300 (modes 1/2) and `Career_Eval_UpgradeAvailability` 0x14352e720 (mode 0, via
   `Career_Set_UpgradesOwner` 0x14352e620, which writes the last entry's state byte directly).

A data-only fix is impossible: the page filters on `+0x72 == card dept`, and the same byte must be
`<= 3` for layers 1–3. Saves: `Career_Deserialise_Upgrades` 0x143536000 → per tree 0x143524c20
finds each instance by `+0x58` and **replaces** its state history (no department guard); 0x143523fd0
serialises every instance. A save written while an instance held the sentinel restores the
sentinel over a fixed init.

Live read (21 `Upgrades` objects, mode 0): every team's tree held the hub and the added rows, one
state entry each, enabled 0; 155-instance trees by dept `[25,23,52,51,4]`, enabled `[25,23,24,23,0]`
= the 95 the page query returned.

## Team modifier lists: departments 0–3 only {#modifiers}

`Career_Load_DepartmentEntries` 0x1412326e0 parses each row's `department` with ENUM A (switch
@0x14123412a) and inserts per-department attribute pointers into 19 maps keyed by that byte
(`component+0x738 + N*0x48`, insert 0x1412ba3d0; an existing key is overwritten). A row whose
`department` is not an ENUM A token (e.g. `Tyres`) resolves to 0 and **overwrites Aero's**
attributes; `Fake` gives key 4.

The per-team modifier list (`Career_Get_TeamModifiers` 0x1434fb4b0, 57 callers) is a vector of
8-byte entries `{u8 kind, u8 dept (ENUM A), u8 source, u8 active, f32 value}` at
`{alloc, begin, end, cap}`. Entries are **created** only by the per-team inits, unrolled over
departments 0..3: `Career_Init_TeamModifiers` 0x1434f9770 (modes 1/2, base dept loop
`mov dword [rsp+0x30],0x3020100` @0x1434fadd1) and `Career_Init_SharedTeamModifiers` 0x1434f58b0
(mode 0). `Career_Add_TeamModifier` 0x1434f8a00 is add-if-absent + stable sort by kind. Listeners
(registered by 0x141238520, callback 0x1412bc4c0) only update existing `kind|dept<<8|0x1070000`
entries, and the save loader 0x1434fc330 only restores saved keys. So a department-4 row in the maps
still yields no list entries: research slots 0 (purchase panel research-limit flag), rush locked,
no cost/time scales.

Consumers: purchase panel `Career_Update_RnDPurchasePanel` 0x141c8d540 (list @0x141c8e299; slots
= sum of `{kind 0, dept == [panel+0x6e8], active}` values; flag `[panel+0x6d2] = gate && slots <=
in-use` @0x141c8e766, `site_Career_RnDPanelResearchLimit`), the department page (@0x141c7dcb5),
rush availability 0x143536e90 (kind 5), failure chance 0x14352f8d0 (kind 0xe), start
research/develop 0x1412431a0 (kind 0x10), upgrade cost 0x143525b60 (kinds 3/0x13 scale, kind 10).

## Back route: the container opens tabs by name {#back}

The purchase tab is `LaptopRnDUpgradePurchaseViewModel` (vtable 0x145740ac0, card
`tab_rnd_upgrade_purchase_panel`): build `Career_Build_RnDPurchasePanel` 0x141c8a080 (vt+0x30;
finds the instance by `upgrade_hash_name` via the query, copies `byte [u+0x72]` to panel+0x6e8),
update `Career_Update_RnDPurchasePanel` 0x141c8d540, messages `Career_Handle_RnDPurchaseMessage`
0x141c8bcc0 (vt+0x50). Panel fields: +0x620 container, +0x680 career, +0x6a0 Upgrades, +0x6a8
instance, +0x6cc team, +0x6d0 cannot-develop, +0x6d1 insufficient points, +0x6d2 research limit
reached, +0x6e0 view, +0x6e8 department.

`back_pressed` (djb2 0x493fe92b) at `site_Career_RnDPurchaseBackName` 0x141c8c158:
`name = dept <= 3 ? k_Career_RnDDeptNameTable[dept] : "error_department"`, then the container's
`vt+0x80("open", name)`. The container dispatch `Career_Dispatch_RnDContainerCommand` 0x141c86fc0
(vt+0x50; open case @0x141c87032) maps djb2(name) to a card: aero 0x7c93fd0c, chassis 0xc239ed13,
engine 0xfb78dbfb, maintenance 0x5c31ec68 (last compare @0x141c873c2), purchase 0x396ca87, perks
0x10230e0a. Any other name takes `jne 0x141c875f5` and returns: the event is consumed and no tab
changes, so Back is a dead end. The stock open tail at 0x141c875a5 stores the card at
`[rsi+0x7b8]`, looks up its slot and selects it; `Career_Refresh_RnDDeptMode` 0x141c87620 is the
department-mode refresh (dept cards on, panels off, `show_shoulders`). The same dead end: the perks
back (0x141c89730, reads `return_department_name`, set with the same fallback @0x141c7fdd1) and the
department page's jump to a linked upgrade (@0x141c8041e), both in 0x141c7fc50.

## Surface 3 — the development-potential pie and the overview tiles {#pie}

What draws it: `laptop_r_and_d.ui` → `tab_content_rnd_upgrades_alt`, layout attach `componentId`
0xbe419b0b8324e33c = `art/screens/career/rnd_overview` → the `dev_potential` UIGP attaches to node
`dev_potential` (an eXRef to `rnd_dev_potential_full.idf`) → each department group attaches by
the chain `[chart_1, sector_N]`; `chart_1` is an eXRef to `rnd_dev_potential.idf`, whose
`sector_1..4` eXRefs → `rnd_dev_potential_sector.idf` (a baked 90° wedge mesh), rotated
0/90/180/270°. The Team HQ research overview (`tab_content_research_overview` →
`team_hq/research/overview` → `dev_potential_full.idf`) is a separate chain driven by class A.
**The engine follows each eXRef by its `xref` id, not its `fileRef`** (§exref).

Two view-model classes:
- Class A (research overview, vtable `vt_Career_RnDResearchViewModel` 0x14575e000): pie builder
  `Career_Build_RnDPotentialPie` 0x141f41d60 (+0x98), sector handler 0x141f45d30 (+0x50).
- **Class B (R&D > Performance, vtable `vt_Career_RnDPerfViewModel` 0x14575e300)**: pie builder
  `Career_Build_RnDPieOverview` 0x141f46eb0 (+0x98), sector handler 0x141f4ac40 (+0x50), populate
  `Career_Populate_RnDPieSlices` 0x141f48030. Created by `Career_Create_RnDPerfViewModel`
  0x141c43290, called @0x141c41ca7 from the career-hub laptop builder `Career_Build_HubLaptop`
  0x141c3f130 (R&D workstation block). When its layout binds, `Vm_Dispatch_BindingMessage`
  delivers a 0x4d event to vt[11] = `Vm_Route_BindingEvent` 0x1414bf420 (10-entry jump table:
  ev 0→13, 1→14, 2→20+15, 3→6, 4→7, 7→**19**, 8→20, 9→21); slot 19 is the pie builder. The pie
  object is therefore built and populated **at career load**, rebuilt on later ev 7s and torn down
  by slot 20 (0x141f49f80 → `Career_Teardown_RnDPieOverview` 0x141f44fa0).

Populate (0x141f48030): loops the dept-code table; code guard `cmp r14d,3 ; ja` @0x141f48218
(`site_Career_RnDPiePopulateDeptGuard`), slot index cap `cmp r13d,4 ; ja` @0x141f4821e (already
permits a 5th write), 4-entry jump table (bound @0x141f4822b, table @0x141f48231); inline colours
(aero 0xffeead00, chassis 0xff3ec58d, maintenance 0xff16deff, engine 0xff7a2aed, default white);
names `lng_rnd_dept_%s` / `_overview_description` from the matched icon name; sector VM nodes
resolved by name-hash under `rnd_wheel_icon` (empty handle on a miss); highlight clamp
`3 < g_Career_RnDOverviewSelectedDept`. The overview tiles bind the pie slot's VM child of the
same name (`@aero|@chassis|@engine|@maintenance`).

## The pie object: constructor, slots, destructor, dead moved-from block {#pie-object}

Class B holds the pie at `this+0x668` = `{allocator, object}`.

| site | what |
|---|---|
| 0x141f46f75 | `alloc(0x900, 0x10)` via `[this+0x270]->vt[3]` (`site_Career_RnDPieBufAlloc` scan @0x141f46f6b) |
| 0x141f46f94 → `Career_Construct_RnDPieObject` 0x14180c490 | **the only constructor**: 4 header handles +0x00..+0x80, then an inlined loop 0x14180c7c0..0x14180c98e (`add rax,0x220 ; cmp rax,0xb08` @0x14180c988) builds **exactly four** 0x220 slots at +0x80/+0x2a0/+0x4c0/+0x6e0. Callers: class A 0x141f41e56, class B 0x141f46f94 |
| slot layout | 17 handles × 0x20 `{vtable, node, next, prev}`; handles 0..3 = `total_upgrades`, `upgrades_fitted`, `upgrades_fitted_percent` … (slot builder `Career_Build_RnDPieSlot` 0x14180b660) |
| 0x141f46fb8 → `Career_MoveAssign_RnDPieObject` 0x142116c80 | `unique_ptr` move-assign: `new = src.obj; src.obj = 0; old = dst.obj; dst.obj = new`; destroys and frees `old` |
| 0x141f46fbd..0x141f4718a | destroys the moved-from temporary: `src.obj` is always null after the move, so this block **never runs** (`site_Career_RnDPieMovedFromDestroy`) |
| `Career_Destroy_RnDPieSlot` 0x141f45650 | the slot **destructor** (base vtable 0x145612e48, unlink + release each handle). All 26 E8 callers are destroy paths (move-assign, teardown 0x141f44fa0, class-B dtor `Career_Destroy_RnDPerfViewModel` 0x142170f20, the dead block, populate's temp @0x141f484d0, class-A copies), each on +0x6e0/+0x4c0/+0x2a0/+0x80 only. No data pointer to it |
| `Career_MoveAssign_RnDPieSlot` 0x141f45500 | `Vm_MoveAssign_NodeHandle(dst+8+k*0x20, src+8+k*0x20)` for the 17 handles; populate moves its temp in @0x141f484c8 |
| `Vm_MoveAssign_NodeHandle` 0x1442bc170 | if `dst.node != src.node` and `dst.node != 0`: unlink dst (`[next+8] = prev`) and release, then take src |

Consequences for a 5th slot:
- Growing the 0x900 allocation does not construct anything; slot 4 must be built after
  `Career_Construct_RnDPieObject` returns (0x141f46f99), the same way the ctor loop builds the four.
- Code placed in 0x141f46fbd..0x141f4718a is dead, and calling 0x141f45650 there **destroys**.
- An unconstructed slot holds the heap block's old bytes; the first populate move-assign reads a
  garbage `node` and faults in the unlink (`mov [rdx+8],rcx` @0x1442bc19d, observed at career load
  with ASCII path bytes in `rcx`/`rdx`).
- Every destroy path only destroys four slots, so a 5th slot needs its own destroy on each of them
  (hooking the slot-dtor entry covers rebuild, teardown and dtor), or its handles stay linked to
  their VM nodes from freed memory.

## Tile counts {#tile-counts}

`Career_Write_RnDPieTileCounts` 0x141f49bc0 (`this` = rdi, query result = rsi) writes each overview
tile's `total_upgrades` (query results with `+0x72 == code`), `upgrades_fitted` (those whose last
state is Completed) and `upgrades_fitted_percent` into the slot handles at
`[this+0x670] + 0x88/0xa8/0xc8/0xe8 + idx*0x220`. Guards `cmp cl,3 ; ja` (code) @0x141f49c23 and
`cmp ebx,3 ; ja 0x141f49f5b` (slot index) @0x141f49c28 (`site_Career_RnDTileCountGuards`). Callers:
the pie builder @0x141f4785c, 0x141f4a2e0 @0x141f4a4f1, and the 0x141f4a760 callback @0x141f4a94d,
each right after the populate and the instance query. On screen the tile shows
`lng_progress_num_total` (NUM = `upgrades_fitted`, TOTAL = `total_upgrades`) and a ring on
`upgrades_fitted_percent`. The values only redraw on the tile's `Trigger`, which runs
`progress_value.refreshTextInst` and the ring's `slowRefresh`; `focusable.trigger` (raised by
`onTabActivated`) sends `Trigger` to a fixed list of tiles in `laptop_r_and_d.ui`, so a new tile
must join that list.

## Department tags: icon frames and colour tables {#tags}

Every department tag names its frame `dept < 4 ? table[dept] : "error_department"` (string
0x14587e10d, 51 references) and reads a `< 6` colour table whose `[4]` is white. Names resolve
through `lng_rnd_dept_<frame>`, so `error_department` needs the key `lng_rnd_dept_error_department`.
Icon paths are `Ne_Format_String("%s.%s.%s", pkg, atlas, frame)`; atlases
`rnd_dept_icons_small` (×12 sites), `rnd_category_icons` (×6), `rnd_department_strips` (×1),
`rnd_wheel_icon` (×2); the frame is `[rsp+0x20]` at the `lea r9,[atlas]` before the call.

| table | readers | note |
|---|---|---|
| `k_Career_RnDDeptColourTable` 0x145755e78 | card ctor 0x141c7aac0, panel update @0x141c8e1ed | follows the 4-entry name table |
| `k_Career_RnDCardColourTable` 0x14571bce0 | upgrade card binders @0x141814b95/0x14180ff10/0x1418174c6, performance publisher 0x1418914f0 | |
| `k_Career_RnDListColourTable` 0x14570ddd8 + `k_Career_RnDListLightColourTable` 0x14570ddf0 | upgrade lists 0x14174e170 | single reader |
| `k_Career_RnDPopupColourTable` 0x145736548 | Team HQ R&D popups 0x1419be1e0 | single reader |
| `k_Career_RnDInProgressColourTable` 0x14576f5d8 | In Progress list 0x141f3bec0, Driver Career rewards 0x1420bcfb0/0x1420c1bd0 | shared: patch the store (0x141f3dacf), not the table |
| `k_Career_TeamHqDeptEventColourTable` 0x145784c18 | Team HQ dept tiles 0x1422fe430, dept events 0x1422e5af0 | indexed by an event department; `[4]` need not mean department 4 |

Name tables: `k_Career_RnDDeptNameTable` 0x145755e58 `{aero,chassis,maintenance,engine}` is
followed directly by the colour table, so `[4]` = the qword `colour[0]|colour[1]`
(0xff3ec58dffeead00) read as a `char*` → crash; `k_Career_TeamHqDeptNameTable` 0x145784d80 has the
same four, but `[4]` is the next string `open_driver_ratings_rtt_panel` (valid, no crash).
`g_Career_RnDOverviewSelectedDept` 0x145f93c08 has exactly two writers (0x141f4aa49 class B focus,
0x141f460a9 class A focus); every reader clamps it `< 4` (`cmp al,4 ; cmovae eax,0`, e.g.
`site_Career_RnDBoxGridNameClamp` 0x141c86c87, `site_Career_TeamHqDeptNameClamp` 0x1423380f2).

## Scene references resolve by `xref` id, not `fileRef` {#exref}

An `eXRef` element carries two references to the scene it instances: the `fileRef` string param and
the `xref` field, a u64 that is the md5-64 (big-endian, path without `.idf`) of the referenced
scene's eaid — that scene's `componentId`. Every stock eXRef has `xref == md5-64(fileRef)`. **The
engine resolves by `xref`.** Re-pointing only `fileRef` leaves the game drawing the old scene while
a viewer that follows `fileRef` shows the new one. Observed: archives whose `chart_1.fileRef` named a
five-sector pie but whose `chart_1.xref` was still md5-64(`…/rnd_dev_potential`) rendered the stock
four 90° wedges in game; a viewer resolving by `xref` reproduced the screenshot exactly, and setting
`xref` = md5-64 of the new path fixed it. Rule for scene edits: set `xref` together with `fileRef`,
and fail any eXRef whose `xref` is not md5-64 of its `fileRef`. Format detail:
[`ui_system.md`](ui_system.md#idf).
