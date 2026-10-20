# Engine hooks — scoping result

Two reverse-engineering passes (findings in the session scratchpad `rnd/` and `rnd-dept/`).

## Verdict: likely doable — mostly data + ONE small mixin. No confirmed hard fixed-4 blocker.

There is no single `array[4]` gating departments on the parse, apply, or economy paths. The one enum that defines the team R&D departments is hookable at a **single, uniquely-scannable site**.

## Key facts
- **Team R&D departments (enum, single resolver @~0x1427743xx):** Powertrain=1, Chassis=2, Aerodynamics=3, **Tyres=4**, default=0. Unknown name → 0 (no crash). So at the enum level F1 25 already has a **Tyres** department; "Durability" belongs to the engine-supplier set, not the team-dept enum.
- **Per-upgrade `upgrade_department` enum (switch @0x14353a700, ~27 inlined copies):** Chassis=1, Durability=2, Engine=3, Fake=4, Invalid=5. Unknown → 0.
- **Department-entry loader `fcn.141232fff`:** loops by DB instance count (`cmp r8d,[rsp+0xa4]`), not 4; stores into a pointer-array `[rsi+0xc60]` stride 16. Residual fixed-capacity candidate: embedded sub-arrays at `rsi+0x738 + N*0x48` (not proven a cap).
- **Apply `fitUpgradesUpToRating @0x143514670`:** dynamic robin_hood iteration; groups by `==` on enum bytes; lerps rating between `DepartmentData` bounds `[r14+0x10]/[r14+0x14]`; writes car-stat delta to the vehicle attr array (count `[veh+0x1960]`, base `[veh+0xd10]`, stride 0xc40, via 0x1449e2850). A new department's upgrades apply with NO code change **if they reuse existing effect types** (tyre_wear/brakes/general_maintenance).
- **Departments UI (`laptop_r_and_d_departments_container.erp`):** data-bound tab nav (`tab_controller`/`primary_toggle_navigation`, `@department_name/colour/icon/summary`). Strong evidence tabs render from the department collection; not 100% proven spawner vs fixed 4.
- **Economy + AI:** data-driven; cost/time/fail fields parsed generically; AI uses per-TYPE `AIUpgradeData`, so reusing existing effect types needs no new AI rows.

## The mixin (1 hook)
`SWOFTY_MIXIN_SITE` on the team-dept enum store. Scan signature (verified unique, count=1):
```
75 06 B1 03 EB 02 31 C9 88 8F 40 03 00 00
```
Site = the `88 8F 40 03 00 00` store (`mov [rdi+0x340], cl`). Handler: `rax` still holds the ci-FNV-1a-64 hash of the department token; for our two new DB tokens, set `rcx` low byte to 5/6 so the store persists the new enum value. Only needed for a genuinely new enum value — if "Tyres"(4) is an unused spare, department A may need zero enum code.

## Data (pure, no code)
- 1 `UpgradeDepartmentEntry` row (cost/time/fail scales + `department` = the token) per season family + bumped instance count.
- `upgrade_N` tree reusing existing `upgrade_type`/`affect_on_vehicle_unit_type`; bumped instancecount.
- `lang.set` strings for name/description/icon. Optional ERP group.

## First-cut findings (this build's deeper RE; scope: single "Tyres & Thermal Management" department)

Ground-truth reads of the shipping GVDB (742,857 records) + the live `[rnd-probe]` session refined the
scaffold verdict above:

- **`Tyres`(4) is a confirmed unused spare.** No row anywhere has `department = Tyres`; the four stock
  `UpgradeDepartmentEntry` rows carry `department` = `Engine`/`Durability`/`Chassis`/`Aerodynamics`. So the
  new department claims value 4 by writing `department = "Tyres"` — **no new enum value is minted**.
- **The enum store site `0x142774258` never fired during the career/MyTeam R&D session** (the probe hooked
  it; zero `[#0]` lines). The `UpgradeDepartmentEntry` loader reads `department` as a **demasked string**
  (generic getter `0x1445a2cf0` + a copy loop at `0x141232a91`), not through that hash-switch. So the
  mixin at `0x142774258` is a **safe confirm/activate + diagnostic**, not the load-bearing lever for the
  department tab.
- **The department rows live at `UpgradeDepartmentEntry_4..7`** under the two season containers
  (`{016caf2bb4d4f128}`, `{3121442cecc7bb08}`); the loop reads 4 of them. The new row is index **8**.
- **No `_UpgradeDepartmentEntry_instancecount_` record exists** in the DB; the count (4) is built at
  runtime (`0x1445a25c0` fills a descriptor, count at `+0x200`). The count key is set to 5 as the data
  hypothesis; whether the game honours it is the #1 in-game-test question.
- **The loader's embedded sub-arrays at `rsi+0x738 + N*0x48` have ~18 slots** (prologue builds 18
  pointers), so the residual fixed-capacity risk has headroom well past a 5th department.

## Make-or-break unknowns → the in-game test resolves these
1. Does the game read the 5th row? (count built at runtime; no DB instancecount record — a native
   count-bump is the fallback if the tab does not appear.)
2. UI tab list: spawner (+N) or fixed 4? Which lang key binds the department name?
3. Is the TEAM car-stat apply the same `fitUpgradesUpToRating` or a separate fn?
4. Tree attachment: which hashed `{...}` container the Tyres tab loads (the dept→tree lookup is unresolved).
5. Save-game tolerance of the new enum value/rows.
6. Enum overflow: a consumer that indexes a fixed array by `[obj+0x340]` and over-runs at value 4 (none
   found; `rsi+0x738+N*0x48` has ~18 slots).

## Count mechanism — RESOLVED (rnd-tyres iteration, RE + live rnd-probe trace)

The department row count is **not** a literal 4 and **not** "built from nothing".
`Career_Load_DepartmentEntries` (0x1412326e0) loops `r8d = 0 .. row_count-1`
(`cmp r8d,[rsp+0xa4]` @0x141232a28), calling `Gdb_Get_RowByIndex(cursor, out, i)`
which reads the row **`UpgradeDepartmentEntry_<i>`** (literal 0-based index).
`row_count` comes from `Gdb_Init_TableCursor` (0x1445a25c0): it builds the DB key
`<container>._UpgradeDepartmentEntry_instancecount_`, looks it up, and stores the
int at `cursor+0x200` (`mov [rsi+0x200],ecx` @0x1445a28df). Missing key → 0.

Ground truth (base GVDB + the `rnd-probe` loader.log, db_count=4, idx 0..3, two
`rsi` objects): the two containers the loader actually reads are
**`{5e21a18f9f59dbf6}`** and **`{1f304d36c3c2b947}`**, each with
`_UpgradeDepartmentEntry_instancecount_ = 4` and rows `_0.._3`
(Aero/Chassis/Durability/Powertrain). The static families `{016caf2bb4d4f128}`/
`{3121442cecc7bb08}` (rows `_4..7`, **no** instancecount key) are a different,
unread copy — the earlier "add at `_8`" plan targeted the wrong container.

Fix: data row at `UpgradeDepartmentEntry_4` in `{5e21…}`/`{1f30…}`; native Hook B
forces the count 4→5 at the `+0x200` store (sig `gamedb.dept_count_store`,
site `site_Gdb_DeptCountStore` @0x1445a28df), gated on `cursor+0x100 ==
"UpgradeDepartmentEntry"` so no other table is touched.

Caveat: `department = Tyres` → per-upgrade `upgrade_department` **ENUM A = 0**
(Aero's slot) in this loader's inline switch; the economy entry may overwrite
Aero's slot rather than add a 5th. The team-dept **ENUM B** (Tyres=4, Hook A) is
separate and drives the dept identity. Whether a 5th tab renders needs the
in-game test.

## R&D department UI — RESOLVED (hardcoded 4, NOT data-driven) {rnd-tyres UI iteration}

The R&D > Performance page (the 2x2 department box grid + the DEVELOPMENT
POTENTIAL pie) does **not** enumerate the `UpgradeDepartmentEntry` data model.
The department set is **hardcoded to 4 in native code**, so the loader-count
bump to 5 (Hook B) loads a 5th economy row but produces **no** 5th UI box. The
box grid and the pie are two independent bespoke builders, and the box/pie
geometry is **pre-authored UIDF** (4 slots), so a 5th box+slice is a
code + ERP + asset rework, not a data/asset lever. (KB: career.md#dept-ui, all
L1, build b171e9793323.)

### Box grid — `Career_Build_RnDDeptBoxGrid` @0x141c85f70 (no E8 callers; vtable)
Populates `view_model_data.laptop_r_and_d_departments_container`. It makes
**exactly four literal** `Career_Construct_RnDDeptCard` (0x141c7aac0) calls with
compile-time `(lang_key, icon, enum)` triples:

| slot | lang key | icon (`rnd_dept_icons_small.*`) | enum | colour |
|---|---|---|---|---|
| 0 | `lng_rnd_dept_aero`        | `aero`        | 0 | 0xffeead00 blue   |
| 1 | `lng_rnd_dept_chassis`     | `chassis`     | 1 | 0xff3ec58d green  |
| 2 | `lng_rnd_dept_engine`      | `engine`      | 3 | 0xff7a2aed pink   |
| 3 | `lng_rnd_dept_maintenance` | `maintenance` | 2 | 0xff16deff yellow |

(`engine` = Powertrain, `maintenance` = Durability.) Plus two non-dept panel
cards (`tab_rnd_upgrade_purchase_panel`, `tab_rnd_perks`). The card vector is
sized to **exactly 6 pointers (0x30)** and truncated. Initial selection =
`DAT_145f93c08` clamped `<4` against the 4-entry `k_Career_RnDDeptNameTable`
(0x145755e58). The card constructor reads the colour from
`k_Career_RnDDeptColourTable` (0x145755e78) with bound `<6`; slots **[4] and [5]
are 0xffffffff white spares**.

### Pie + overview — `Career_Build_RnDPotentialPie` @0x141f41d60
Loops `k_Career_UpgradeDeptCodeTable` (0x145735d60 = `00 01 02 03 04 05 …`) but
is **gated `if (code < 4)` and `if (4 < i) break`**, with a per-case `switch`
mapping `code -> lng_rnd_dept_%s / _overview_description` and **inline hardcoded
colours** (same 4; default white + lang `error_department`). Pie sectors are
pre-authored VM nodes driven by `init_pie_sector` / `highlight_pie_sector`
(`Career_Build_RnDPieSector` @0x141f45d30) — **4 sector nodes only**.

### Why the 5th box is excluded
The UI never consults the department data model; it builds 4 cards from
compile-time constants (box grid = 4 unrolled calls; pie = table loop gated
`<4`). There is no code path that creates a 5th card, and no UIDF node for a
5th box or 5th pie sector.

### Smallest correct lever for a genuine 5th box+slice (deferred rework)
A multi-site native + UIDF + asset change:
1. `Career_Build_RnDDeptBoxGrid` (0x141c85f70): grow the card vector 6 -> 7
   (change the `0x30` truncation), inject a 5th `Career_Construct_RnDDeptCard`
   for Tyres with `(lng_rnd_dept_tyres, rnd_dept_icons_small.tyres, 4)`, and
   relax the `<4` selection clamp.
2. `Career_Build_RnDPotentialPie` (0x141f41d60): relax `code<4 -> <5` and
   `4<i -> 5<i`, add a `case 4:` colour.
3. Colour: write a distinct ABGR at `k_Career_RnDDeptColourTable[4]`
   (0x145755e78, currently white).
4. Assets: `lng_rnd_dept_tyres` + `lng_rnd_dept_tyres_overview_description`, a 5th
   `tyres` frame in `rnd_dept_icons_small`.
5. UIDF (`laptop_r_and_d_departments.ui`): a 5th box layout node and a 5th
   pie-sector node (the box/pie geometry is authored, not computed).

Several sites write into fixed-stride per-entry arrays (0x220 stride in the pie
builder); mis-sizing risks a crash, and the pie physically cannot show a 5th
slice without the extra UIDF sector node — so this is **not** safely
implementable blind and is scoped as a following iteration.

## Mixins in `rnd_departments.cpp` (KB bindings)
Since the mixin port every hook below is a KB binding resolved by signature at load; the scans, byte
windows, RVAs and per-site "NOT installed (SAFE)" paths of the sections that follow are gone. One
unresolved binding drops the addon. The addon has no settings: every piece below is always on, and the
former flags' defaults are the behaviour. A site that used to be a byte patch reproduces the patched
instruction and resumes at its own join binding.

Removed with the settings, together with their hooks: the box-layout probe (`render_fifth_box_layout`),
the upgrade-tree instrument (`attach_tyres_tree`), the tree-rows probe (`diag_tree_rowcount`), the R&D page
instance diagnostic (`diag_tyres_instances`), the box-grid selection clamp and its name guard
(`select_fifth_dept`, default off; with the stock clamp the name read never sees index 4), the test price
(`tyres_test_price`) and the three Develop Upgrade diagnostics (back_pressed, option rows, purchase request).

| piece | bindings |
|---|---|
| Hook A, Hook B | `site_Career_TeamDeptEnumStore`, `site_Gdb_DeptCountStore` |
| UI groundwork | `k_Career_RnDDeptColourTable` ([4] written once at load), `site_Career_RnDDeptBoxGridEntry`, `site_Career_RnDPotentialPieEntry`, `g_Career_RnDOverviewSelectedDept` |
| 5th box | `site_Career_RnDDeptBoxGridInject`, `Career_Construct_RnDDeptCard`, `Ne_Make_StrFromCStr`, `Ne_Format_String`, `g_Ne_DefaultAllocator` |
| pie 5th slice | `site_Career_RnDPieBufSize`, `site_Career_RnDPieCtorRet`, the four `site_Career_RnDPie*SlotDtor` / `site_Career_RnDPerfViewModelSlotDtor` redirects, `site_Career_RnDPiePopulateDeptGuard` → `site_Career_RnDPiePopulateSlotCap`, `site_Career_RnDPiePopulateGate` → `site_Career_RnDPiePopulateNextCode` |
| tile counts | `site_Career_RnDTileCountGuards` → `site_Career_RnDTileCountIndexGuard` → `site_Career_RnDTileCountBody` / `site_Career_RnDTileCountExit` |
| instances | `site_Career_UpgradeInitFakeDeptBranch` → `site_Career_UpgradeInitNormal`, the five `site_Career_Upgrade*DeptGate` → `*Body`, `site_Career_UpgradeGraphHubTest` → `site_Career_UpgradeGraphRegularNode`, `Car_Reset_LastFittedUpgrade`, a wrap of `Career_Refresh_UpgradeAvailability`, `site_Career_DeserialiseUpgradesEnd`, `site_Career_RnDDeptPagePreQuery` |
| Develop Upgrade screen | `site_Career_RnDContainerOpenMaint` → `site_Career_RnDContainerOpenTail`, `Career_Refresh_RnDDeptMode`, `Vm_Set_TabEnabled`, `vt_Career_RnDDeptCard`, `site_Career_RnDBoxGridFirstTab`, `site_Career_RnDCompareDept`, `site_Career_RnDCardArt{Image,Mask}0..2`, `site_Career_RnDCardDeptIcon0..2`, `site_Career_RnDCardColourPick0..4` |

Install lines name the binding instead of an address, for example
`[rnd-tyres] 5th-box inject hook installed (site_Career_RnDDeptBoxGridInject)`.

## v0.3.0 — the LIVE 5th BOX, shipped (box only; pie deferred)

A genuine 5th department box now renders on R&D > Performance behind its own
independent gate. Two earlier assumptions were **wrong** and are corrected here.

### Correction 1 — the dept card does NOT bind a per-dept UIDF node
`Career_Construct_RnDDeptCard` (0x141c7aac0) takes
`(card, ctx, builder, allocator, name_cstr, icon_cstr, enum_byte)` — confirmed by
the call-site disassembly at 0x141c861bc-0x141c861e3 (5th arg [rsp+0x20] = name,
6th arg [rsp+0x28] = icon, 7th arg [rsp+0x30] = enum). The exe string bytes are:
- 5th arg = the **name lang key**: `lng_rnd_dept_aero` (0x1457407d6),
  `lng_rnd_dept_chassis`, `lng_rnd_dept_engine`, `lng_rnd_dept_maintenance`.
- 6th arg = the **icon atlas path**, formatted `%s.%s.%s` = `<pkg>` (from
  `*(lVar8+0x660)`) `.` `rnd_dept_icons_small` (0x14570bcda) `.` `<frame>`
  (`aero`/`chassis`/`engine`/`maintenance`). So the `%s.%s.%s` suffix is the icon
  **frame name in the `rnd_dept_icons_small` atlas**, NOT a view-model/UIDF node
  path. ctor stores icon at card+0x818 (param_1[0x103]), name at card+0x830
  (param_1[0x106]), enum at card+0x6d8 (param_1[0xdb]), colour at card+0x948
  (param_1[0x129]) = `k_Career_RnDDeptColourTable[enum]` (bound `<6`).

Consequence: a dept card needs **no per-dept authored UIDF node** and **no
launcher UIDF/ERP pipeline**. It renders through the same card VM node
(card+0x268) the other four use, attached to the container `builder+0x2d8` by the
builder's own attach loop. This removes the blocker the v0.2.0 notes recorded.

### The box injection (implemented)
- **Gate (independent of Hooks A/B + the colour write):** one `.xtls` scan of
  `48 C7 40 18 00 00 00 00 48 8B 8E 98 02 00 00` (unique, count=1; match =
  0x141c8655f). Hook site = **match+8 = 0x141c86567**, the `mov rcx,[rsi+0x298]`
  that starts the purchase-panel alloc, right after the 4th dept card is stored
  and before the panels are built. safetyhook MidHook. Feature flag
  `render_fifth_box` (toggle, default on). On any scan miss / layout mismatch /
  >1 match it logs and injects nothing.
- **At the site:** `rsi` = builder (param_1), `rdi` = the icon-prefix object
  (lVar8, non-volatile, set at 0x141c8601e, clobbered at 0x141c8657f, so live
  here); begin[0..3] = the four dept cards; panels not yet built.
- **Grow:** the card-pointer vector (begin `[self+0x280]`, end `[+0x288]`, cap
  `[+0x290]` in pointer units, allocator `[+0x278]`) is sized to exactly 6
  (end-begin==0x30). If cap<7, reallocate to 8 via the allocator vtable (alloc =
  `vt+0x18(allocator,bytes,align)`, free = `vt+0x38(allocator,ptr)`), copy the 6
  pointers, free the old buffer, set cap=8. Set `end = begin + 0x38` (7 slots).
  The resumed stock code then fills begin[4]/begin[5] (panels) off a freshly-read
  begin, leaving **begin[6] = the Tyres card**.
- **Construct:** allocate a 0x950 card via the card allocator `[self+0x298]`;
  build the icon NeStr with `Ne_Make_StrFromCStr(&s,"",0,0)` (0x1441974e0) +
  `Ne_Format_String(&s,"%s.%s.%s", *(char**)(lVar8+0x660), "rnd_dept_icons_small",
  "aero")` (0x1441630f0); call `Career_Construct_RnDDeptCard(card, *(self+0x28),
  self, cardAllocator, "lng_rnd_dept_tyres", s->chars, 4)`; store begin[6]=card;
  release the temp NeStr through `g_Ne_DefaultAllocator` (0x146ca54a8, free =
  vt+0x38). NeStr chars at +9, refcount (u16) at +4.
- **Icon:** BORROWED — frame `aero` of `rnd_dept_icons_small` (no new texture).
  Logged as `icon-borrowed="<pkg>.rnd_dept_icons_small.aero"`.
- **Colour:** reuses `k_Career_RnDDeptColourTable[4]` (0x145755e88) = Tyres orange
  0xff0078ff, written by the v0.2.0 colour groundwork; the card ctor reads it via
  its `<6` bound at construction time (after load).
- **Name:** our own `"lng_rnd_dept_tyres"` literal (the loc system resolves it at
  display time; confirmed resolving live).
- **Selection clamp NOT relaxed.** `DAT_145f93c08` stays clamped `<4` against the
  4-entry `k_Career_RnDDeptNameTable` (0x145755e58). A saved index 4 would read
  `name[4]` which overlaps `colour[0]` (the colour table immediately follows the
  name table at 0x145755e78) → a colour read as a char* → crash. So the 5th box
  only RENDERS; it is never the selected card this round.

Deviation from the original spec note ("grow at the allocation/truncation site"):
growing after the 4th card (where lVar8 is live and all four cards exist) is
cleaner and leaves the stock grow/truncate and the panel stores untouched. The
builder's own attach loop (begin..end) then binds all seven slots identically.

### Confirmed-safe vs the in-game test
- **Safe (static-proven):** the scan is unique on this build; the card is built by
  the real constructor identically to a stock dept card, so the attach loop
  (`card+0x260` method + `FUN_14431d720(builder+0x2d8, card+0x268,0,0)`) cannot
  fault on it; selection stays `<4`; every read/write is region-guarded; the two
  panels still land in begin[4]/begin[5].
- **The test resolves:** whether the container `builder+0x2d8` lays the 5th card
  out visibly (it may be a fixed 4-slot 2x2 grid — 5th invisible/overlapping — or
  an auto-flow that shows a 3rd row). This is the one thing static analysis cannot
  settle; the injection is faithful either way.

### Pie path — RESOLVED (next iteration), the LIVE builder is NOT 0x141f41d60
The Performance-page pie did not fire 0x141f41d60 because there are **two parallel
R&D view-model classes**:
- Class A (research-overview/graph screen, vtable 0x14575e000): pie builder
  `0x141f41d60` (+0x98), sector handler `0x141f45d30` (+0x50). NOT this page.
- **Class B (LIVE, vtable 0x14575e300)**: pie builder
  **`Career_Build_RnDPieOverview` @0x141f46eb0** (+0x98), sector handler
  `0x141f4ac40` (+0x50). Built by `0x141c43290` — the **same 0x141c4xxxx screen
  family that owns the box grid `Career_Build_RnDDeptBoxGrid` @0x141c85f70**.
  `0x141f46eb0` has no E8 callers; it is reached via VM dispatch (0x14431d720)
  from vtable-B slot 0x14575e398, binds `onTabActivated`, and emits
  `init_pie_sector` / `rnd_overview_activated`.

Mechanism: `0x141f46eb0` allocs a **0x900** sector buffer (`0x80 + 4*0x220`) at
0x141f46f75, constructs 4 slots via `FUN_141f45650` at buf+0x80/+0x2a0/+0x4c0/
+0x6e0 (unrolled, stride 0x220), then populates via helper **`0x141f48030`**
(**corrected in v0.7.1:** `FUN_141f45650` is the slot DESTRUCTOR, and those four calls
destroy the moved-from temporary; the slots are constructed inside `FUN_14180c490`,
see the v0.7.1 section):
loops `k_Career_UpgradeDeptCodeTable`, guard `if(code<4)` (`cmp r14d,3/ja`
@0x141f48218), index cap `if(4<written)` (`cmp r13d,4/ja` @0x141f4821e, already
permits 5), a 4-entry jump table (bound `cmp r14d,3` @0x141f4822b, table
@0x141f48231); **colours are an INLINE switch** (aero 0xffeead00 @0x141f4858e,
chassis 0xff3ec58d, maintenance 0xff16deff, engine 0xff7a2aed, default white — the
pie does NOT read the colour table), names formatted `lng_rnd_dept_%s` /
`lng_rnd_dept_%s_overview_description` from the matched icon name; sector VM nodes
resolved by name-hash via `Vm_GetOrCreate_ChildNode` under `rnd_wheel_icon`
(empty handle on miss); highlight clamp `if(3<DAT_145f93c08)`.

5th-slice plan (LIVE path — patch 0x141f46eb0 + 0x141f48030, **NOT** 0x141f41d60):
1. Buffer grow @0x141f46f75: `mov edx,0x900` (`ba 00 09 00 00`) → `ba 20 0b 00 00`
   (**0xb20** = 0x80 + 5*0x220). Mandatory before relaxing the guard.
2. Construct a 5th slot: inject `lea rcx,[rbx+0x900]; call FUN_141f45650` (the four
   are unrolled at 0x141f46fdf..0x141f4700a; no inline room → code cave/detour).
3. 0x141f48030 code guard @0x141f48218 `cmp r14d,3`→`4`; jump-table bound
   @0x141f4822b `cmp r14d,3`→`4` and extend the rel32 table @0x141f48231 to 5
   entries with a `case 4`; index cap @0x141f4821e already permits 5.
4. Add a `case 4` colour (e.g. 0xff0078ff) + icon-name `tyres` so
   `lng_rnd_dept_tyres` / `_overview_description` resolve.
5. Highlight clamp (line ~254, `3 < DAT_145f93c08`) → `4`.
6. Assets: a 5th pie-sector node under `rnd_wheel_icon` (resolved by name-hash) +
   the pie-wheel **geometry reworked for 5 wedges** (authored, not computed), and
   the two lang keys. This is why the pie slice is deferred — unlike the box, the
   wedge is authored art.

Decompiles: `re/decomp/141f46eb0.c` (live builder), `re/decomp/141f48030.c`
(populate loop), plus class-A `141f41d60.c`/`141f45d30.c` for contrast.

## v0.4.0 — three independently-gated pieces (box visibility, tree, pie slice)

Three pieces, each behind its OWN exactly-one `.xtls` scan + feature flag,
independent of Hooks A/B, the colour write and the v0.3.0 box inject. On any scan
miss / layout mismatch / >1 match each logs `[rnd-tyres]` and does nothing.

### The data model that ties it together (RESOLVED)
The whole R&D UI (box grid, pie, upgrade trees) is keyed by the per-upgrade
**ENUM A** (`upgrade_department`), not the team-dept ENUM B:
`0=aero 1=chassis 2=durability 3=engine 4=Fake` (switch @0x14353a6dd). Value **4
("Fake") is the unused spare** that card enum 4 / `k_Career_RnDDeptColourTable[4]`
and `k_Career_UpgradeDeptNameTable[4]="fake"` all line up on. So the "5th
department" is the code-4/"Fake" slot made real. (The team `department`=Tyres /
Hook A / ENUM B=4 is a separate axis driving the team's setup, not the R&D grid.)

### Piece 1 — the 5th box is invisible: CAUSE RESOLVED, visible cell DEFERRED
`Career_Build_RnDDeptBoxGrid`'s container (builder+0x2d8) is an embedded **UITC
tab-container VM**. A department is visible/selectable only if it has an entry in
the authored **slot vector at container+0x270..+0x278 (= builder+0x548..+0x550)**,
0x10 per entry: entry+8 = slotObj, slotObj+0x20 = bound card node, slotObj+8 =
inner content node. Those slots are created at **screen init** from the authored
tab/VM nodes (`view_model_data.laptop_r_and_d_departments_container`) + the
authored `rnd_card_with_info` scene instances in `frontend_overlay.idf`, keyed by
node id — a **fixed four**. The per-card attach `FUN_14431d720` only registers a
child (container+0x28/+0x378); it **never writes +0x270**. So the v0.3.0 5th card
is parented but has no slot and no authored scene cell → invisible (matches the
live result).
A genuine 5th cell needs a 5th +0x270 slot **and** a 5th `rnd_card_with_info`
scene instance with a real cell rect. The slot-register routine (UITC `vt[0x48]` /
screen builder 0x141c43290) is not in the decompiled corpus and the 5th cell rect
lives in the un-dumped `frontend_overlay.idf` scene; cloning a slot + fabricating
a rect blind risks a crash, and the declarative UIDF/ERP pipeline is not built — so
**no visible-cell lever is safely shippable from the addon this round**.
**Shipped until the settings cleanup, which removed it (it was `render_fifth_box_layout`, scan
`career.rnd_box_layout_probe` @0x141c86d86, read-only):** a one-shot diagnostic
that dumps the live slot-vector vs card-vector + every authored slot's bound/inner
node, so the follow-up (an external f1-modding ERP scene patch adding a 5th
authored cell, or a runtime slot+scene-node clone using the captured pointers) has
exact ground truth.

### Piece 2 — the upgrade tree (flag `attach_tyres_tree`)
The read-only instrument described here was removed with its setting; the data placement stands.
`Career_Load_UpgradeRows` (0x143539bc0) loops codes 0..4 and opens
`<team_link>.<deptname>` (`"%s.%s"`, deptname from `k_Career_UpgradeDeptNameTable`;
[4]="fake"), reading `upgrade_<N>` rows into one vector. Each record stores only
its **ENUM-A byte**, so the department page shows records whose ENUM A == its code.
Therefore the Tyres (code-4) page shows records with `upgrade_department="Fake"`
(ENUM A=4). `db/tyres_tree.json` sets `upgrade_department="Fake"` — the flagged
**economy-collision fix** ("Tyres"→0 would collide with Aero; "Fake"→4 is the
distinct spare).
The "fake" container is NOT empty — it is the tree's **central hub**
(`fake.upgrade_0 = central_fake_upgrade`, instancecount 1, linking the branch
roots).

**Container RESOLVED (v0.4.1, live diagnostic).** `team_link` is the team's
**engine-supplier constructor** token, not the constructor and not a generic
`user_defined` hub: a McLaren (Mercedes PU) save opens
`upgrades.2026.trees.mercedes.fake` = `{deae0310dc31dce2}` (`_upgrade_instancecount_`
`{b2932bd32df83903}`, `upgrade_0._metadata_` `{cabec8fb14f94a7a}` — all three match
the diagnostic). The GVDB holds **46 real `.fake` hub containers**, each with the
`central_fake_upgrade` hub (`_upgrade_instancecount_ = 1`, identical 15-key
schema). 12 are `upgrades.2026.trees.<team>.fake` (`ferrari`, `force_india`,
`gm_cadillac`, `haas`, `mclaren`, `mercedes`, `redbull`, `renault`, `sauber`,
`tororosso`, `user_defined`, `williams`); the other 34 are additional
verified-identical fake hubs carried by hash. No 2025-season team trees exist, and
`engine_suppliers.trees.engine_supplier_upgrades_N.fake` (e.g. `{3c55d614fc76ce3e}`)
is empty (no hub) so it is not written. The four `user_defined.fake`-family hubs
the v0.4.0 note mislabelled (`{ae8319a01aa30478}` `{5927ed801bdb6765}`
`{75a045fd77ae52b9}` `{f71aa592926ce9d9}`) are four of these 46 and now carry the
corrected rows.

**Shipped data:** the 3 Tyres nodes are written as `upgrade_1/2/3` into all 46
containers (preserving `upgrade_0`), each `_upgrade_instancecount_` bumped 1→4,
every field a field-for-field clone of the stock `fake.upgrade_0` schema (15 keys,
correct type tags) with real `upgrade_type` (Tyre Wear / Brakes / Upgrade All
Durability) and `upgrade_department=Fake`. The DB-op `{hash}` selector continues FNV
through `.upgrade_N.field` (`fnv1a64_from(container_hash, ".upgrade_N.field")`;
verified against the GVDB keys and applied end-to-end through the engine — 0 stock
collisions, round-trip stable).

**Reachability RESOLVED (v0.4.1).** The whole R&D tree is one graph rooted at the
hub `default` (grid `0,0`), which links OUTWARD (child/unlock edges) to each
department's branch root by the target's `_metadata_` id. A node is reachable only
if a directed path from `default` reaches it, so the Tyres branch attaches the stock
way: `fake.upgrade_0.links` gains our root (`,tyres_thermal_1`, preserving the stock
branch targets) and `tyres_thermal_1` → `tyres_thermal_2` → `tyres_thermal_3` chain
outward. (The v0.4.0 rows linked child→hub and never attached the hub to the branch
→ unreachable.)

**Shipped native (scan `career.load_upgrade_rows_deptpath` @0x143539c84, default on,
read-only):** a hook (r8=team_link cstr, r9=deptname cstr, rdi=code) that logs each
code's resolved path + FNV container hash; the code-4 line should now equal
`{deae0310dc31dce2}` on the McLaren save (one of the 46 real hubs), confirming the
placement (and revealing the exact `{hash}` if a future build/save differs).
Remaining in-game-test items: the display-filter (ENUM A == card enum), confirm the
branch renders and is buyable, and the economy/rating integration.

### Piece 3 — the development-potential pie 5th slice (flag `render_fifth_pie`, default OFF)
Live builder `Career_Build_RnDPieOverview` (0x141f46eb0) + populate
`Career_Populate_RnDPieSlices` (0x141f48030). Scan `career.rnd_pie_buf_alloc`
@0x141f46f6b (`mov rbx,[rsi+0x270]; mov rax,[rbx]; mov edx,0x900; …`), all sites by
fixed RVA + region/value verified, patches ordered so the guard is relaxed LAST:
1. buffer `0x900→0xb20` immediate @0x141f46f76 (= 0x80 + 5*0x220).
2. MidHook @0x141f47012 (rbx=buffer, after the 4th `FUN_141f45650`): construct the
   5th slot `FUN_141f45650(buf+0x900)` (slot i @ buf+0x80+i*0x220). **Wrong, replaced in
   v0.7.1:** that block never runs with a live buffer and `FUN_141f45650` is a destructor.
3. relax the populate `code<4` guard `cmp r14d,3 → 4` @0x141f4821b. (The index
   guard `cmp r13d,4` @0x141f4821e already permits 5 writes; left intact.)
These are faithful extensions of the stock prologue, not blind writes. **DEFERRED
(authored art):** the pie **wedge geometry** (four pre-authored 90° sectors) and
the jump-table `case 4` name/colour. With the guard relaxed, code 4 takes the
populate DEFAULT jump-table case → name token `error_department`, colour white, and
its sector node resolves by name-hash under `rnd_wheel_icon` to an EMPTY handle
(benign, node-guarded). So the 5th slice is **populated but no wedge draws** until a
5th authored sector node + a `case 4` are added. Extending the 4-entry rel32 jump
table @0x141f48231 needs a code cave within ±2GB and would be a blind crash risk,
so it is not done. Default OFF because it has no visible payoff yet and is the
riskiest piece; toggling at runtime logs that the (install-time) code patches need
a relaunch.

### New KB signatures (build d873a1c8…, sigcheck green)
- `career.rnd_pie_buf_alloc` → `site_Career_RnDPieBufAlloc` @0x141f46f6b
- `career.rnd_box_layout_probe` → `site_Career_RnDDeptBoxLayoutProbe` @0x141c86d86
- `career.load_upgrade_rows_deptpath` → `site_Career_LoadUpgradeRows_DeptPath` @0x143539c84

## v0.5.0 — the 5th department SELECTION (flag `select_fifth_dept`, default on)

> **Removed.** The setting, the clamp relax and the name guard are gone; the box-grid clamp is stock.

> **Superseded in v0.6.0 (dead end).** `DAT_145f93c08` is the R&D > Performance overview
> box-grid selection; a hardware watchpoint proved it never changes while the department
> tab pages are built, so it does not choose the tile page. The second clamp (TeamHQ VM
> command 0x55702d31, table slot 4 = `open_driver_ratings_rtt_panel`) is unrelated to R&D.
> v0.6.0 removes the TeamHQ relax, defaults `select_fifth_dept` off, and keeps only the
> box-grid name guard (always installed, a no-op for idx 0..3). The real blocker is in the
> v0.6.0 section below.

Makes the code-4 (`Fake`/Tyres) department SELECTABLE so it can be the active R&D
page. The tile data for code 4 is already loaded (both `Career_Load_UpgradeRows`
@0x143539bc0 and `Career_Load_UpgradeTreeNodes` @0x14353b030 loop codes 0..4, and
the v0.4.2 `diag_tree_rowcount` probe reports the code-4 row count). The blocker is
purely DISPLAY/SELECTION: the active-department index is the global
`DAT_145f93c08` (u8), and every non-pie reader clamps it `< 4`
(`cmp al,4 ; cmovae eax,0`), so the active dept can never be 4 and the Fake(4)
`@rows`/`@tiles` are never spawned.

### The crash trap (RESOLVED, verified against the exe)
`k_Career_RnDDeptNameTable` @0x145755e58 has only FOUR entries
(`{aero,chassis,maintenance,engine}`) and `k_Career_RnDDeptColourTable`
@0x145755e78 immediately follows it (+0x20). So reading `nameTable[4]` returns the
qword at 0x145755e78 = `colour[0]|colour[1]` = `0xff3ec58dffeead00`, which the box
grid then dereferences as a `char*` → crash. This is THE crash that a naive clamp
relax would hit on the first name read of dept 4.

### The two active-dept read/clamp sites (RIP-xref of 0x145f93c08; the four
`0x141f4xxxx` pie reads are a separate authored-art follow-up and are LEFT ALONE)
- **Box grid** `Career_Build_RnDDeptBoxGrid` @0x141c85f70, the name read (decomp
  141c85f70.c:488-493). Disassembly @0x141c86c7e:
  ```
  141c86c7e  0f b6 05 83 cf 30 04   movzx eax,[DAT_145f93c08]
  141c86c85  31 c9                  xor ecx,ecx
  141c86c87  3c 04                  cmp al,4            <- clamp immediate @0x141c86c88
  141c86c89  0f 43 c1               cmovae eax,ecx      (idx>=4 -> 0)
  141c86c8c  48 0f be c0            movsx rax,al
  141c86c90  48 8d 0d c1 f1 ac 03   lea rcx,[k_Career_RnDDeptNameTable]
  141c86c97  48 8b 14 c1            mov rdx,[rcx+rax*8]  <- the crash-trap read
  141c86c9b  48 c7 44 24 50 00..00  mov [rsp+0x50],0     <- MidHook site (rdx=name, rax=idx)
  ...
  141c86caf  e8 ..                  call Ne_Make_StrFromCStr(&out, rdx, 0, 0)
  ```
  Scan (`.xtls`, unique, count=1, hit=0x141c86c7e):
  `0F B6 05 83 CF 30 04 31 C9 3C 04 0F 43 C1 48 0F BE C0 48 8D 0D C1 F1 AC 03 48 8B 14 C1`
  → clamp immediate = hit+10 (0x141c86c88), MidHook site = hit+29 (0x141c86c9b).
- **Tile page** — a TeamHQ MyTeam career VM command dispatcher, func @0x142337ff0,
  the `cmp eax,0x55702d31`/`je` case @0x1423380e9 (not in the decompiled corpus).
  IDENTICAL clamp, but indexing a DIFFERENT 4-entry dept code-name table
  @0x145784d80 (`{aero,chassis,maintenance,engine}`) that is followed by unrelated
  panel-event strings, so `table[4]` = `"open_driver_ratings_rtt_panel"` — a VALID
  C-string. Relaxing this clamp therefore does NOT crash; it only lets the
  active-dept content resolver run for index 4. Disassembly @0x1423380e9:
  ```
  1423380e9  0f b6 05 18 bb c5 03   movzx eax,[DAT_145f93c08]
  1423380f0  31 c9                  xor ecx,ecx
  1423380f2  3c 04                  cmp al,4            <- clamp immediate @0x1423380f3
  1423380f4  0f 43 c1               cmovae eax,ecx
  1423380f7  48 0f be c0            movsx rax,al
  1423380fb  48 8d 0d 7e cc 44 03   lea rcx,[0x145784d80]
  142338102  48 8b 14 c1            mov rdx,[rcx+rax*8]
  ```
  Scan (`.xtls`, unique, count=1, hit=0x1423380e9):
  `0F B6 05 18 BB C5 03 31 C9 3C 04 0F 43 C1 48 0F BE C0 48 8D 0D 7E CC 44 03 48 8B 14 C1`
  → clamp immediate = hit+10 (0x1423380f3).

### What ships (own gate, flag `select_fifth_dept`, default on)
1. **Name-guard FIRST.** safetyhook MidHook at the box-grid name site (hit+29): for
   `rax == 4` it sets `rdx` to a static `"Tyres & Thermal Management"` C-string, so
   `nameTable[4]` = colour[0] is never dereferenced. Installed BEFORE any clamp is
   relaxed, and it protects index 4 for the life of the process (not runtime-gated
   off while a clamp is relaxed). No-op for idx 0..3.
2. **Box-grid clamp** `cmp al,4` → `cmp al,5` (one byte @ hit+10), byte-verified
   (must be 0x04) and VirtualProtect-guarded, applied ONLY after the name hook is
   live.
3. **Tile-page clamp** `cmp al,4` → `cmp al,5` (one byte @ hit+10), byte-verified +
   guarded. No name hook needed (table[4] is a valid string).

Both clamp relaxations are inert until the active dept is actually 4, so they
cannot misbehave unless the 5th department is genuinely selected. Independent of
Hooks A/B, the colour write, the box inject and the probes; on any scan miss /
mismatch / >1 match the feature installs nothing and the rest keep working. The
`select_fifth_dept` ON_CHANGE only logs (the clamp byte-patches are install-time).

### Confirmed-safe vs test-resolves
- **Safe (static-proven against the exe):** both scans are unique on this build; the
  name read returns a valid string for idx 4 (crash trap closed); the tile table[4]
  is a valid string; both clamp bytes are verified 0x04 before patching; every
  read/write is region- and value-guarded; relaxations are inert unless dept 4 is
  active. The box grid's name-hash dispatch (0xc239ed13/0xfb78dbfb/0x7c93fd0c) has no
  case for a 5th card, so with the Tyres name it falls to the default branch — no
  crash either way.
- **Test resolves:** whether, with dept 4 active, the non-decompiled `@rows`/`@tiles`
  spawner (`0x141808d90` populate / `Career_Bind_UpgradeCard` @0x1417f8090) actually
  renders the Fake(4) tiles, or the page is still empty (the content resolver may
  key off the wrong/short name), or the tile-page handler keys the content
  differently. The `diag_tree_rowcount` line + the `dept-name[4] overrun intercepted`
  line disambiguate the outcome.

### New sites (record into the KB later; NOT written to re/db here)
- box-grid active-dept name read + clamp: func @0x141c85f70, clamp imm @0x141c86c88,
  name MidHook @0x141c86c9b
- tile-page (TeamHQ VM cmd 0x55702d31) active-dept clamp: func @0x142337ff0, clamp
  imm @0x1423380f3
- crash trap: `k_Career_RnDDeptNameTable[4]` @0x145755e78 = `colour[0]` as char*

## v0.6.0 — why the Tyres page is empty: the dept-4 upgrade INSTANCES never get a real state

### The page path (live gdb trace, this build)
`Career_Build_RnDDeptPage` @0x141c7b880 (16 KB, from 0x141c7b3d0) builds one R&D department tab.
`rsi` = the 0x950 department card (`Career_Construct_RnDDeptCard`), dept = `byte [card+0x6d8]`
(Tyres card = 4), team = `dword [card+0x6bc]`, per-team model = `[card+0x688]`. At 0x141c7d8cf it
calls `Upgrades_QueryEnabled` @0x1435188d0`(U=[card+0x688], out=rsp+0xdf0, &team=rsp+0x8c, 0xffff)`
and keeps results whose `byte [u+0x72] == card dept`. Durability page: 95 results, by dept
`0:25 1:23 2:24 3:23 4:0`. Tyres page: the same 95, `4:0` → nothing to show. The page logic is
fine; our upgrades are not in the query result.

### The per-team model (`Upgrades`, 0x1b10 bytes, one per team)
Created by `Career_Add_TeamUpgrades` @0x14123ca40 (alloc 0x1b10): loads the record model
(@0x143539260 → `Career_Load_UpgradeRows`, tree nodes), then `Career_Instantiate_UpgradeTrees`
@0x14352cac0, then `Career_ResetAll_Upgrades` @0x14352e1c0.

| off | field |
|---|---|
| +0x08 | u8 mode (0 = single shared tree with owner cache, 1 = per team, 2 = per driver; this career: 0) |
| +0x0c | u32 team id |
| +0xe0 + k*0xc40 | tree k (k < 2): +0x08 allocator, +0x10/+0x18 instance vector (0xe0 stride), +0x30 inline pointer array (0x180 slots), +0xc30 count. So the array is `U+0x110+k*0xc40`, count `U+0xd10+k*0xc40` |
| +0x1960 | u64 number of trees |
| +0x1970 + k*0x58 | tree graph (nodes 0x40 stride: [0]=instance, +0x10/+0x18 neighbours, +0x30 grid x/y) |
| +0x1a60.. | robin-hood map owner → tree slot (mode 2) |
| +0x1a9c | u16[6] per-department counters (dept 0..5, so index 4 is in bounds) |
| +0x1aa8 | u32 owner cache (mode 0); 0x3fff0000 = none |

`Career_Instantiate_UpgradeTrees` copies EVERY 0x80-byte UpgradeRecord (no department filter)
into a 0xe0 instance, calls the instance init on the stack temp, moves it into the vector and
pushes the pointer (`param_1[(k+1)*0x188 + cnt - 0x166]`, count `param_1[(k+1)*0x188 + 0x1a]`).
That is the "no direct `[..+0xd10]` write" the static scan missed: the stores are indexed off
`param_1` with the slot folded into the index.

### The upgrade instance (0xe0) and its state history
| off | field |
|---|---|
| +0x08..+0x57 | SSO `upgrade_name` (+0x10 = chars) |
| +0x58 | u32 node hash = djb2(seed 0x1505) of `"%s_%d" % (_metadata_ label, row flag)` |
| +0x70/+0x71/+0x72 | type / size / **department (ENUM A)** |
| +0x74 | `unlock_at_facility_level` |
| +0x7c | row flag: 0xff = team tree row, else engine-supplier stat id |
| +0xb8 | allocator; +0xc0/+0xc8/+0xd0 = state entries begin/end/cap (0x10 each) |

State entry: +0 u32 owner (0x3fff0000 = none), +4 u16 day, +6 u16, +8 u8 state, +9 fitted,
+0xa/+0xb, **+0xc enabled**, +0xd facility-locked, +0xe. State names @0x14565cee0:
0 Unavailable, 1 Available, 2 InProgress, 3 Completed, 4 Failed, 5 Invalid, 6 PendingDevelopment,
7 InDevelopment, 8 PendingFitting. `Upgrades_QueryEnabled` keeps an instance only if the last
entry with `day <= mask` has **enabled != 0**.

### The exact skip mechanism (three layers, all on `byte [u+0x72]`)
1. **Instance init** `Career_Reset_UpgradeInstance` @0x143517d80 (KB name
   `Car_Reset_LastFittedUpgrade`; it is the per-instance reset called by instantiation, reset-all
   and the supplier refit): at **0x143517e0c `cmp byte [rcx+0x72],4 ; jne 0x143517e4c`**. A Fake (4)
   instance gets ONE sentinel entry `{owner 0x3fff0000, day 0xffff, state Invalid, enabled 0}` and
   returns, skipping the normal path (`PushState(owner, Unavailable, day 0)`, `SetFitted`,
   `SetEnabled(1)`, tail-call `SetFacilityLocked(unlock_level != 0)`). Enabled stays 0 → the query
   drops it. This is how the stock hub stays off every page; our three rows are
   `upgrade_department = Fake` too, so they get the same sentinel.
2. **Every state mutator bails on dept > 3** (`cmp byte [rcx+0x72],3 ; ja ret`):
   `Upgrade_PushState` @0x1435230e0 (guard 0x1435230f3, "Changing state from %s to %s"),
   `Upgrade_SetEnabled` @0x143523510 (0x143523520), `Upgrade_SetFacilityLocked` @0x143523730
   (0x143523740), `Upgrade_SetFitted` @0x1435157d0 (0x1435157e0), and the auto-fit pass
   @0x1412484e0 (0x1412486b8). So a dept-4 instance can never leave the sentinel.
3. **The availability graph search** `Upgrade_GraphSearch` @0x1435288f0 treats every dept-4 node
   as the tree hub (`cmp byte [rax+0x72],4 ; je` @0x143528a05): used by
   `Career_Refresh_UpgradeAvailability` @0x14352f300 (modes 1/2) and `Upgrade_EvalAvailability`
   @0x14352e720 (mode 0 owner switch `Upgrades_SetOwner` @0x14352e620, which writes the state byte
   of the last entry directly). Left alone, our nodes would act as hubs and unlock out of order.

No `>= 4` loop, 4-entry tree list or `.fake` container skip exists on the instantiation path; the
instances are all there (live check below). A data-only fix is impossible: the Tyres card filters
on `+0x72 == 4`, and the same byte must be <= 3 for layers 1-3.

Save games: `Upgrades_Deserialise` @0x143536000 → per-tree `@0x143524c20` finds each instance by
`+0x58` and REPLACES its state history (no dept guard). `@0x143523fd0` serialises every instance,
so saves written with v0.4/v0.5 carry our sentinel and would overwrite a fixed init on load.

### Live confirmation (read-only `/proc/<pid>/mem` scan, game not stopped, 2026-10-19)
21 `Upgrades` objects, all mode 0. Every team's tree contains the hub and our three:
`central_fake_upgrade` hash 0xbd057d45 (= djb2("default_255"), label `default`),
`tyres_wear_reduction` 0x27e41c13, `tyres_brake_cooling` 0x27f63494, `tyres_thermal_reliability`
0x28084d15 (= djb2("tyres_thermal_N_255")), row flag 0xff, ONE state entry each, enabled 0
(fresh trees: state 5 Invalid; trees restored from the save: state 0 from the mode-0 direct write).
155-instance trees: by dept `[25,23,52,51,4]`, enabled `[25,23,24,23,0]` = the 95 the page query
returned.

### The fix (flag `instantiate_tyres_upgrades`, default on, install-time)
Mixin sites on the department compare of each gate (the KB rows the earlier sweep named), always on.
Each acts only on OUR instances: dept 4, row flag 0xff, and `+0x58` in our node hashes or
`upgrade_name` in our rows, and resumes on the branch the stock code takes for a real department.
Identification is by content, never address (the init runs on a stack temp).

| binding (instruction) | resumes at (ours) | effect for ours |
|---|---|---|
| `site_Career_UpgradeInitFakeDeptBranch` 0x143517e0c (`cmp [rcx+0x72],4 ; jne`) | `site_Career_UpgradeInitNormal` 0x143517e4c | normal init: Unavailable, enabled 1, locked 0 (`unlock_at_facility_level` = 0) |
| `site_Career_UpgradePushStateDeptGate` 0x1435230f3 (`cmp [rcx+0x72],3 ; ja ret`) | `site_Career_UpgradePushStateBody` 0x1435230fd | state changes apply |
| `site_Career_UpgradeSetEnabledDeptGate` 0x143523520 | `site_Career_UpgradeSetEnabledBody` 0x14352352a | as above |
| `site_Career_UpgradeSetLockedDeptGate` 0x143523740 | `site_Career_UpgradeSetLockedBody` 0x14352374a | as above |
| `site_Career_UpgradeSetFittedDeptGate` 0x1435157e0 | `site_Career_UpgradeSetFittedBody` 0x1435157ea | as above |
| `site_Career_UpgradeAutoFitDeptGate` 0x1412486b8 | `site_Career_UpgradeAutoFitBody` 0x1412486be | as above |
| `site_Career_UpgradeGraphHubTest` 0x143528a05 (`cmp [rax+0x72],4 ; je hub`) | `site_Career_UpgradeGraphRegularNode` 0x143528a0b | regular node, not the hub |

The hub (`default`/`central_fake_upgrade`) never matches, so it keeps the stock sentinel, stays
disabled, stays out of every query, and still terminates the graph search as the root.

Sanitizer (heals saves that persisted the sentinel): for OUR instances whose states are empty or
whose last entry is still disabled, call the game's own init (now on the normal path), then mirror
`Career_ResetAll_Upgrades`: mode 0 → owner cache `+0x1aa8 = 0x3fff0000` so the next query runs
`Upgrades_SetOwner` and recomputes availability; modes 1/2 → `Career_Refresh_UpgradeAvailability
(U, 0)` (only at load). Runs at: load (`site_Career_DeserialiseUpgradesEnd` 0x14353610f, after the trees, `rdi` = U), a wrap of
`Career_Refresh_UpgradeAvailability` (entry, `rcx` = U), and the R&D page just before the query
(`site_Career_RnDDeptPagePreQuery` 0x141c7d8ba, `rcx` = U). The init it calls is `Car_Reset_LastFittedUpgrade`.

Checked consumers of `+0x72` for index-by-department overruns once dept-4 upgrades become live:
department name/colour lookups in the upgrade panels are bounded (`cmp 3/5 ; ja default`), the
`+0x1a9c` counters are 6 wide, and cost/facility code compares the department against lists
(no match for 4 → no facility lock, stock row costs). Known cosmetic gap: the upgrade detail panel
shows the bounded fallback department label (`error_department`) for Tyres upgrades.

### Diagnostic (flag `diag_tyres_instances`, default on, read-only)
Removed with its setting. It was site `site_Career_RnDDeptPageQueryDone` 0x141c7d8d4 (after the query; `rsi` = card, count at
`rsp+0x19f0`). Logs per page build (Tyres page up to 24 times, other pages once):
`instances: R&D page dept=N team=… query=… by dept 0:… 1:… 2:… 3:… 4:…`, the Tyres-listed names,
and every dept-4 instance's last state.

### New sites (record into the KB later; NOT written to re/db here)
- `Career_Reset_UpgradeInstance` 0x143517d80 (dept==4 branch 0x143517e0c)
- `Career_Instantiate_UpgradeTrees` 0x14352cac0, `Career_Add_TeamUpgrades` 0x14123ca40,
  `Career_ResetAll_Upgrades` 0x14352e1c0
- `Upgrade_PushState` 0x1435230e0, `Upgrade_SetEnabled` 0x143523510,
  `Upgrade_SetFacilityLocked` 0x143523730, `Upgrade_SetFitted` 0x1435157d0, auto-fit 0x1412484e0
- `Upgrade_GraphSearch` 0x1435288f0, `Career_Refresh_UpgradeAvailability` 0x14352f300,
  `Upgrades_SetOwner` 0x14352e620, `Upgrade_EvalAvailability` 0x14352e720,
  `Upgrade_EvalFacilityLock` 0x14352ea40
- `Upgrades_QueryEnabled` 0x1435188d0, `Career_Build_RnDDeptPage` 0x141c7b880 (query 0x141c7d8cf)
- `Upgrades_Deserialise` 0x143536000 (+ per-tree 0x143524c20), tree serialise 0x143523fd0

## v0.6.1 — the DEVELOP UPGRADE screen for the Tyres upgrades

### The screen
The detail screen is the departments container's purchase tab, `LaptopRnDUpgradePurchaseViewModel`
(card name `tab_rnd_upgrade_purchase_panel`, `begin[4]` of the box-grid card vector, vtable 0x145740ac0):
build `0x141c8a080` (vt+0x30; finds the instance by `upgrade_hash_name` via `Upgrades_QueryEnabled` and
copies `byte [u+0x72]` to panel+0x6e8), update `0x141c8d540` (cost, slots, timeline, comparison, card),
messages `0x141c8bcc0` (vt+0x50), option rows `0x141c8cbb0` / `0x141c8f0a0`, unlocks `0x141c8c430`, upgrade
card binder `0x141814320` (also `0x14180fc60`, `0x1418169f0`). Panel fields: +0x620 container, +0x680
career, +0x6a0 Upgrades, +0x6a8 instance, +0x6cc team, +0x6d0 cannot-develop, +0x6d1 insufficient points,
+0x6d2 research limit reached, +0x6e0 view, +0x6e8 department.

### Root cause of the Back trap
`back_pressed` (djb2 0x493fe92b) in `0x141c8bcc0` @0x141c8c150:
`name = dept < 4 ? k_Career_RnDDeptNameTable[dept] : "error_department"`, then container
`vt+0x80("open", name)`. The container's dispatch (`0x141c86fc0`, vt+0x50; open case @0x141c87032) maps the
name's djb2 to a card: aero 0x7c93fd0c, chassis 0xc239ed13, engine 0xfb78dbfb, maintenance 0x5c31ec68,
0x396ca87 (purchase), 0x10230e0a (perks); anything else takes `jne 0x141c875f5` and returns. The event is
consumed and no tab changes, so the player is stuck. Same dead end: the perks tab's back (`0x141c89730`,
reads `return_department_name`, which the department page sets with the same fallback @0x141c7fdd1) and the
department page's cross-department jump to a linked upgrade (@0x141c8041e).

### Second root cause: the 5th department row clobbered Aero (fixed in data)
`Career_Load_DepartmentEntries` parses each row's `department` with ENUM A (switch @0x14123412a: Chassis 1,
Durability 2, Engine 3, Fake 4, Invalid 5, anything else 0) and inserts per-department attribute pointers
into 19 maps keyed by that byte (`component+0x738 + N*0x48`, insert `0x1412ba3d0`; status 1 = key exists,
the value is still overwritten). Our row said `department = Tyres` → 0, so since v0.2.0 it overwrote AERO's
attributes (cost/time scales, slots, rush, dual development) with Durability's. The per-team modifier lists
(`{kind, dept, ?, active, value}`, built by listeners registered in `0x141238520` from those maps, read by
`0x1434fb4b0`) therefore had no department-4 entries at all: research slots 0 (→ research-limit flag), rush
locked, no cost/time scales. `db/departments.json` now says `department = Fake` (→ 4): Aero gets its own
attributes back and department 4 becomes a real economy department sharing Durability's attributes, so
every modifier consumer below works for Tyres with no code. Residual risk: code that iterates these maps and
indexes a 4-wide array by the key (none found on the R&D paths; watch the stock departments' slots/costs).

### Sweep: code that branches on the department byte (508 `byte [x+0x72]` accesses scanned)
Classified with `capstone` over a full `objdump` of `.xtls`; plus the panel/card copies (+0x6e8, +0x6d8).

| site | function | dept 4 behaviour | screen | status |
|---|---|---|---|---|
| 0x143517e0c | instance init 0x143517d80 | `==4` → disabled sentinel | page | v0.6.0 hook |
| 0x1435230f3 0x143523520 0x143523740 0x1435157e0 0x1412486b8 | push state / enabled / facility lock / fitted / auto-fit | `>3` → return | purchase | v0.6.0 hooks |
| 0x143528a05 | availability BFS 0x1435288f0 | `==4` → treated as hub | unlock chain | v0.6.0 hook |
| 0x141c8c150 | purchase messages 0x141c8bcc0 (back_pressed) | `>3` → "error_department" → open no-op | **yes** | **fixed: Back route** |
| 0x141c7fdd1 | dept page 0x141c7fc50 (perks) | return name "error_department" | Tyres page | fixed: Back route |
| 0x141c8041e | dept page 0x141c7fc50 (linked upgrade) | open "error_department" | Tyres page | fixed: Back route |
| 0x141c8703d..0x141c873c7 | container open dispatch 0x141c86fc0 | unknown name → ret | **yes** | **hooked @0x141c873c2** |
| 0x14181459c 0x14180fe7a 0x141816f48 | card binders | `lng_rnd_dept_error_department` (missing key) | **yes** (dept tag) | lang key added |
| 0x141815611 0x14181041d 0x1418185b5 | card binders | `rnd_dept_icons_small.error_department` (no frame) | **yes** | icon → `chassis` |
| 0x141814b95 0x14180ff10 0x1418174c6 | card binders | colour 0x14571bce0[4] = white | **yes** | [4] → Tyres orange |
| 0x141c8e1ed | panel update | colour 0x145755e78[4] | **yes** | Tyres orange since v0.2.0 |
| 0x141c8e28c | panel update | kind 0 (slots) for dept → 0 → +0x6d2 research limit | **yes** (blocks buy) | fixed: Fake row |
| 0x141c8de49 | panel update → dept summary 0x1416d53f0 | kinds 3/2/1/0xd → zeros | **yes** | fixed: Fake row |
| 0x143536f04 | rush available 0x143536e90 | kind 5 for dept → none → Rush locked | **yes** (Rush `-`) | fixed: Fake row |
| 0x14352f975 | failure chance 0x14352f8d0 | kind 0xe → base | **yes** | fixed: Fake row |
| 0x1412434ea | start research/develop 0x1412431a0 | kind 0x10 → none | purchase | fixed: Fake row |
| 0x143516b50 0x14351c4ff 0x14351c5d1 0x14351f1b8 0x14351f2a0 0x143521e90 0x143521f70 0x1435220f1 0x14352e95b 0x14352ea6b 0x14352fad3 0x1435301f3 0x1435302dc 0x143534796 0x1435348b6 0x1435349c8 0x143534a97 0x141c7dca8 | R&D simulation / availability / facility lock / dept page slots | modifier kinds 0x0-0x19 for dept → none | progression | fixed: Fake row |
| 0x141c8ebfc → 0x141890e60 | vehicle performance comparison | team base rating list (+0x1a38) has no dept 4 → empty bars | **yes** | alias hook @0x141c8ec4c |
| 0x141893f59 | per-team performance data 0x1418914f0 | `lng_rnd_dept_%s` / tables | pie | pie agent (`0x1418914f0`) |
| 0x1416b6d51 | timeline 0x1416b6cd0 | `rnd_goto_error_department_department` VM string | yes | harmless |
| 0x141c737ff 0x141c837a2 0x141c7db76 0x141c495e9 0x141c55188 0x141c70d8a 0x141c74da9 0x141c77eb2 | other R&D views | name `error_department` | no | lang key covers `lng_rnd_dept_%s` |
| 0x1418b0f95 0x1419bf58f 0x141c78850 0x141c84170 0x141f3dab5 | other views | colour tables `<6` → white | no | bounded, cosmetic |
| 0x14071e711 0x14071e99d 0x140777f12 | telemetry | ENUM A name table [4] = "Fake" | no | valid string |
| 0x140f5e300 0x140f5e820 | per-dept counters | `==0..5` chain | no | handles 4 |
| 0x14372f44d 0x143730424 | 0x14372ef80 | `>3` → skip | no | bounded |
| ENUM A switch (~27 inlined copies, e.g. 0x14353a700) | parse | "Fake" → 4 | load | data |
| card binders `rnd_part_image/_mask.<upgrade_name>` | 6 Ne_Format_String sites | no frame for our names → blank card/unlock art | **yes** | borrowed parts |
| 0x143525b60 | upgrade cost | formula (below) | **yes** | test price hook |

### What v0.6.1 installs (flag `tyres_upgrade_screen`, default on; the Back route is latched at launch)
All of it is always on.
- **Back route** — `site_Career_RnDContainerOpenMaint` 0x141c873c2 (`cmp eax,0x5c31ec68`, the last name compare
  of the open case). For name hash `tyres` (0x107666dc) or `error_department` (0x7e5c1342, only produced for
  department >= 4): find the department card (`vt_Career_RnDDeptCard`, `+0x6d8 == 4`) that owns a container slot
  (`+0x548..+0x550`, `slotObj+0x20 == card+0x268`, `slotObj+8 != 0`), call `Career_Refresh_RnDDeptMode(&container,
  0)` (the stock department-mode refresh: dept cards on, panels off, `show_shoulders`), enable the Tyres card with
  `Vm_Set_TabEnabled(container+0x2d8, card+0x268, 1)`, set `rax = &cards[i]` and resume at
  `site_Career_RnDContainerOpenTail` 0x141c875a5 (`mov [rsi+0x7b8],card` → slot lookup → select). If no such
  card/slot exists, rewrite the hash to `maintenance` so the stock code opens Durability. Stock names never match.
- **Comparison alias** — `site_Career_RnDCompareDept` 0x141c8ec4c (before `call 0x1418130b0`): the comparison's
  department byte at `[rsp+0x108]` is 4 only for our upgrades; it becomes the effect type's stock department
  (Chassis for Tyre Thermal, Brake Cooling, Tyre Life; Powertrain for Power Unit Cooling).
- **Card art** — sites on the `lea r9` before 9 `Ne_Format_String` calls (`[rsp+0x20]` = frame name):
  `site_Career_RnDCardArtImage0..2` (`rnd_part_image`, 0x141810118/0x1418154c6/0x141818452, art sites 0/2/4),
  `site_Career_RnDCardArtMask0..2` (`rnd_part_mask`, 0x141810398/0x141815585/0x141818524, art sites 1/3/5) and
  `site_Career_RnDCardDeptIcon0..2` (`rnd_dept_icons_small`, 0x141810478/0x141815666/0x141818611, art sites 6..8,
  `error_department` → the Tyres frame).
- **Card colour** — `site_Career_RnDCardColourPick0..4`, on the instruction after each card binder's
  `k_Career_RnDCardColourTable[dept]` load (0x14180eb3d, 0x14180ff2b, 0x141814baf, 0x1418174e0, 0x1418b0faf): department 4
  reads Tyres orange 0xff0078ff instead of white. The table itself is not written.
- **Diagnostics** — the state pushes in the v0.6.0 push guard. The back_pressed, option-row and
  purchase-request diagnostics were removed with the settings.

### Test price (flag `tyres_test_price`, default on) — TEMPORARY
Removed: the Tyres upgrades always use the database cost. The cost function below is unchanged.
Cost = `0x143525b60(list, instance, team modifiers, option)`: base `cost` (+0x5c) or `cost_to_develop`
(+0x60, option & 6), team rows only; × modifier kind 3 (research) / 0x13 (development) cost scale; + kind 10 ×
upgrades owned; + `(base × instance+0xd8) / 100` (event-driven percent; −60 on the test save, which is why the
1000-point row showed 400). It is called by the affordability list `0x1435329b0`, the research and
development cost wrappers `0x14351dde0` / `0x14351df60` (`0x14351df60` is mislabelled
`Car_Read_VehicleAttribute` in the KB), the card and the panel.

### Names
Per-upgrade name key = `lng_rnd_<upgrade_name>`, description = `lng_rnd_<upgrade_name>_description` (card
binder `0x141814320`, desc `0x14385ad00`). The detail title is `lng_rnd_<size>` + `lng_rnd_<type>`
("Minor - Tyre Wear") and does not use the name. The department tag over the title is the Tyres card's own
name key (`lng_rnd_dept_tyres`, the same key the Tyres tab shows), now "Tyres".

## v0.7.0 — the five-sector development-potential pie (folded in)

The pie art and native half were prepared separately (`f1-modding/work/rnd-fifth-pie`) and merged here.

**Assets** (`erp/rnd_fifth_pie.json`, 60 ops, sources under `ui/`): per archive (`flow_career_hub.erp`,
`flow_team_hq.erp`) 3 `replace-resource` (`rnd_overview.idf` with 3+2 tiles, `laptop_r_and_d.ui` with
tile 5 bound to `@tyres` and a `dev_potential.tyres` group, `rnd_dev_potential_full.idf` pointing
`chart_1` at the new pie) and 27 `add-resource` (`rnd_dev_potential5.idf` five 72° sectors,
`rnd_dev_potential_sect72.idf` + its 3 proxies/models/meshes and 12 buffers, the five-spoke backdrop
material/texture). The replacements change size, so both archives take the rewritten-archive route
(launcher PR #138; the v1.0.4 CLI drops them). The research overview (`team_hq/research`) keeps the
stock four-sector pie.

**Native** (`src/rnd_fifth_pie.cpp`, `rnd_fifth_pie::install` right after `install_fifth_pie`, whose populate
changes slice and focus rely on). The pieces have no options and are always on. Every hook is a mixin on a
KB binding (`PIE_SITES.md`, "Bindings"):

| piece | bindings | for department code 4 |
|---|---|---|
| slice | `site_Career_RnDPieSliceNameJoin`, `site_Career_RnDPieSliceDescFormat` (redirect), `site_Career_RnDPieSliceDefaultColour`, `site_Career_RnDPieSliceIconArgs` | populate name `tyres`, description key, colour 0xff0078ff, wheel icon `maintenance` |
| focus | `site_Career_RnDPieFocusNameCmp` (resume `site_Career_RnDPieFocusJoin`) | `department_focused "tyres"` highlights the sector; `g_Career_RnDOverviewSelectedDept` untouched |
| performance | `site_Career_RnDPerfDeptLoopEnd`, `site_Career_RnDPerfDeptName`, `site_Career_RnDPerfChildName`, `site_Career_RnDPerfDeptColour`, `site_Career_RnDPerfGotoName` | publisher `0x1418914f0` emits `department_performance.tyres` (derived current/potential, orange `dept_colour`) |

All sites were checked for in-function branch targets inside the patched bytes (none). The publisher's
`dept_colour` is set at its read (`site_Career_RnDPerfDeptColour`), so `k_Career_RnDCardColourTable`
0x14571bce0 is not written by this piece; a white read becomes orange and an already-orange one is kept.
Expected log lines: `pie5 slice/perf/focus ... INSTALLED`, then on R&D > Performance
`pie5 slice: code 4 ...`, `pie5 perf: dept 4 published as department_performance.tyres`, and on the
Tyres tile `pie5 focus: department_focused "tyres" -> ...`.

## v0.7.1 — the pie crash at career load (slot 4 was never constructed)

### Symptom
With `render_fifth_pie` on, F1 crashed about 80 s into loading the career save, before R&D was opened.
The fault was an access violation at `0x1442bc19d` (`mov [rdx+8],rcx`, a handle unlink) with
`rcx`/`rdx` = ASCII `asset_gr`/`oups/f1_`. The stack was `0x1442bc19d` ← `0x141f45519` ← populate
`0x141f484cd` ← builder `0x141f4782a` ← `Vm_Dispatch_BindingMessage+0x20b`, with `r14=4`,
`r13=0x880`, `r12=obj+0x80`, and `rbx=obj+0x900` (slot 4). The last addon line before it was
`pie5 slice: code 4 name ...`.

### Who builds the pie, and when
`0x141c3f130` is the career-hub laptop builder; it creates every laptop tab when the hub loads.
In the R&D workstation block (`overrides.career.laptop.page_icons[3]`, `lng_workstation_r_and_d`),
`0x141c41ca7` calls `0x141c43290`, which creates the class-B view model (vtable 0x14575e300) for
`tab_content_rnd_upgrades_alt` (`lng_r_and_d_upgrades`, under `laptop_container`). When its layout
binds, `Vm_Dispatch_BindingMessage` (0x1442c00e0) delivers a 0x4d binding event to `vt[11]` =
0x1414bf420. That function's jump table (10 entries) maps ev 0→13, 1→14, 2→20+15 (when
`+0x628&1`), 3→6, 4→7, 7→**19**, 8→20 and 9→21. Slot 19 is `Career_Build_RnDPieOverview`
(0x141f46eb0, at vtable +0x98). The pie object is therefore built and populated at **career load**.
It is rebuilt on later ev 7s and torn down by slot 20 (0x141f49f80 → 0x141f44fa0).

### The pie object (class B, `this+0x668` = {allocator, object})
| site | what |
|---|---|
| 0x141f46f75 | `alloc(0x900, 0x10)` via `[this+0x270]->vt[3]` |
| 0x141f46f94 → `FUN_14180c490(obj, this+8)` | **object ctor**: 4 header handles +0x00..+0x80 bound to VM nodes, then an inlined loop 0x14180c7c0..0x14180c98e (`add rax,0x220 ; cmp rax,0xb08`) constructs **exactly four** 0x220 slots at +0x80/+0x2a0/+0x4c0/+0x6e0 |
| slot layout | 17 handles × 0x20 `{vtable, node, next, prev}`; vtables in order e588 e588 0ca8 0ca8 f6e0 f6e0 f1f0 f6e0 f6e0 f6e0 e588 8310 f670 f1f0 f1f0 f1f0 f670 (0x1456xxxxx; loop registers rcx/rdx/r8/r9 from the leas @0x14180c7a0..b5, r14 @0x14180c54c, r13 @0x14180c627) |
| 0x141f46fb8 → `FUN_142116c80(this+0x668, &pair)` | `unique_ptr` move-assign: `new = src.obj; src.obj = 0; old = dst.obj; dst.obj = new`; destroys and frees `old` (slot dtors + header + `alloc->vt[7]`) |
| 0x141f46fbd..0x141f4718a | destroys the moved-from temporary: `src.obj` is always null after the move, so this block is dead |
| `FUN_141f45650(slot)` | **slot destructor** (base vtable 0x145612e48, unlink + release each handle). Callers are all destroy paths: 0x142116c80, 0x141f44fa0 (slot-20 teardown), 0x142170f20 (class-B dtor), the dead block above, populate's temp at 0x141f484d0, and the class-A copies (0x141f41d60, 0x142170b10). Each destroy path calls it on +0x6e0, +0x4c0, +0x2a0, +0x80. No pointer to it exists in data |
| `FUN_141f45500(dst, src)` | slot move-assign: `FUN_1442bc170(dst+8+k*0x20, src+8+k*0x20)` for 17 handles |
| `FUN_1442bc170(dst, src)` | handle move-assign: if `dst.node` ≠ `src.node` and `dst.node` ≠ 0, unlink `dst` (`[next+8] = prev`) and release, then take `src` |
| populate 0x141f48030 | `r12 = [this+0x670]` @0x141f482a5; `r13d` = running slot index (guard `cmp r13d,4` @0x141f4821e); slot = `obj + 0x80 + idx*0x220` (@0x141f48335..0x141f48347, then `r12 += 0x80`); temp built by 0x14180b660, moved in at 0x141f484c8, temp destroyed at 0x141f484d0. Also called from the 0x141f4a760 callback (registered by the builder @0x141f47cf0) |

### Root cause
v0.7.0 grew the alloc to 0xb20, but its "slot-construct" hook at 0x141f47012 sat in the dead
moved-from block and called the slot **destructor**. Slot 4 (`obj+0x900..+0xb20`) was never
constructed on any path. It held whatever the heap block contained before (an `asset_groups/f1_…`
path here). The first populate at career load moved the temp into slot 4, and `FUN_1442bc170` read
a non-zero garbage `node`, then unlinked through garbage links and faulted. The registers match
exactly: `rsi = obj+0x908`, `[rsi] = rax` garbage, `[rsi+8]/[rsi+0x10]` ASCII. If the populate had
survived, every rebuild or teardown would then have freed slot 4's live handles without unlinking
them from their VM nodes, because only slots 0..3 are destroyed. That leaves dangling list links in
freed memory. Hypotheses (a) "another alloc site" and (c) "raw `char*`" are ruled out. The only ctor
callers are 0x141f46f94 (class B) and 0x141f41e56 (class A), and the name pointer is a static C
string, exactly like the stock `error_department` literal.

### The v0.7.1 fix (flag `render_fifth_pie`, default on, latched at launch)
Mixins in `rnd_departments.cpp`, all resolved from the KB at load (one unresolved binding drops the
addon). Every handler is always on.

| binding | kind | does |
|---|---|---|
| `site_Career_RnDPieBufSize` (0x141f46f7a, `mov r8d,0x10` after `mov edx,0x900`) | site | `edx` = **0xb40** = 0x80 + 5×0x220 + 0x20 footer; marks the allocation as grown for this thread |
| `site_Career_RnDPieCtorRet` (0x141f46f99, after the ctor, `r14` = new object or 0) | site | Proceeds only if this thread's allocation was grown, the object is writable to +0xb40, and slots 0..3 carry the same 17 handle vtables (the first object's are latched as the expected set). It then constructs slot 4 exactly like the ctor loop (17 × `{vt, 0, 0, 0}`, vtables copied from the stock slots), writes the footer at +0xb20 `{"RNDTYRE5", obj ^ 0x9e3779b97f4a7c15, 0xb40, gen}`, and registers the object (16-entry SRW-locked registry) |
| `site_Career_RnDPieMoveAssignSlotDtor`, `site_Career_RnDPieTeardownSlotDtor`, `site_Career_RnDPerfViewModelSlotDtor`, `site_Career_RnDPieMovedFromSlotDtor` (the first `call Career_Destroy_RnDPieSlot` on `+0x6e0` in the move-assign 0x142116c80, the slot-20 teardown 0x141f44fa0, the class-B dtor 0x142170f20 and the dead moved-from block) | redirect | For `rcx - (0x80 + k×0x220)`, k = 3..0: if that object is registered, take it out of the registry and check its footer and slot-4 vtables. If they are intact, clear the footer and call the slot dtor on `obj+0x900`, then run the stock slot destroy. A corrupt footer or vtable logs and leaves slot 4 alone. A no-op when nothing is registered |
| `site_Career_RnDPiePopulateDeptGuard` (0x141f48218, `cmp r14d,3 ; ja`) | site | Code 4 resumes at `site_Career_RnDPiePopulateSlotCap` (the guard reads as `cmp r14d,4`) |
| `site_Career_RnDPiePopulateGate` (0x141f48228, after both guards, before any side effect of the iteration) | site | For slot index `r13d ≥ 4`, read `obj = [[rsp+0x40]+0x670]` and require: registered, readable to +0xb40, footer intact (gen matches), all 17 slot-4 vtables as constructed. On any miss, log `pie5 slice SKIPPED: <reason>` and resume at `site_Career_RnDPiePopulateNextCode` (0x141f48200, `inc rbp`, codes ≥ 5 fail the guard, then exit) |

The slot dtor `Career_Destroy_RnDPieSlot` has no unique signature of its own (its body is a generic
handle-release run shared with other functions) and no data pointer, so its 26 E8 callers are the
only handle on it. The four redirected calls are the first slot destroy of every path that frees a
class-B pie object; the other groups destroy class-A objects or populate temporaries, which are never
registered.
`rnd_fifth_pie.cpp` relies on these populate changes for its slice and focus pieces.

### Expected log
Install:
```
[rnd-tyres] pie 5th slice INSTALLED: buffer 0x900->0xb40 (5 slots + footer), slot 4 constructed after the ctor and destroyed with the stock four, populate gate site_Career_RnDPiePopulateGate, guard cmp r14d,3->4 at site_Career_RnDPiePopulateDeptGuard
```
Career load (and each rebuild):
```
[rnd-tyres] pie: constructed slot 4 @0x<obj+0x900> in pie object 0x<obj> (alloc 0xb40, 17 handles, footer gen 1, 1 live)
[rnd-tyres] pie5 slice slot 4 verified: object 0x<obj> gen 1, capacity 0xb40 (footer), 17 constructed handles; populating code 4
[rnd-tyres] pie5 slice: code 4 name error_department -> tyres (VM child tyres, lng_rnd_dept_tyres)
```
Rebuild or teardown: `[rnd-tyres] pie: destroyed slot 4 of pie object 0x<obj> (gen N) before the stock slot destructors (M live)`.
Refusal (never a crash): `[rnd-tyres] pie5 slice SKIPPED: <reason>; slot 4 left untouched, the stock four populate normally`.
The reason is one of: `pie object 0x… was not built with the 0xb40-byte buffer by this addon (no slot 4)`,
`… footer at +0xb20 missing or stale (gen N); capacity not proven`,
`… slot 4 handle K is not the constructed vtable`, `view model … has no pie object`, or
`slot index … is past slot 4`.

### Live check (optional, gdb under Wine)
Use `trace_pie5.gdb`/`trace_pie5.sh` from the session scratchpad. Attach at the **main menu**,
before loading the save. Software breakpoints go only on unhooked bytes: 0x141f46f83 (alloc call:
`edx` must be 0xb40), 0x141f46fa1 (after the ctor hook: slot-4 h0 = `14565e588 0 0 0`, footer
magic), 0x141f484c8 if `r13 == 0x880` (slot-4 move-assign: dst h0 sane), and 0x141f48200 if
`r13 == 4 && r14 == 4` (gate skip). There is one hardware breakpoint, on 0x141f45650 if
`rcx == obj+0x900` (slot-4 destroy).

## v0.7.1 — Tyres research capacity (flag `tyres_research_capacity`, default on, install-time)

Symptom: develop-upgrade options `research-limit=1` for every Tyres upgrade.
Panel update 0x141c8d540: `list = TeamModifiers_Get 0x1434fb4b0(career+0xc90, team)` @0x141c8e299;
slots = sum of `{kind 0, dept == [panel+0x6e8], active}` entry values (0x141c8e2c6); in-use = enabled
upgrades of the dept with last state InProgress (0x141c8e5df..0x141c8e764); flag `[panel+0x6d2] =
[rsp+0x57] && slots <= in-use` @0x141c8e766. Team modifier list = vector of 8-byte entries
`{u8 kind, u8 dept (per-upgrade enum, Durability 2), u8 source, u8 active, f32 value}` at
`{alloc, begin, end, cap}`.
Cause: entries are only ever CREATED by the per-team inits, unrolled over depts 0..3:
`0x1434f9770` (modes 1/2, 232 calls to TeamModifiers_Add 0x1434f8a00, base loop `{0,1,2,3}` @0x1434fadd1)
and `0x1434f58b0` (mode 0, 80 calls). TeamModifiers_Add = add-if-absent, then two stable passes by kind and by dept (the
vector ends ordered by dept, then kind), and it adds kind/dept/source to the small sets at
list+0x20/+0x50/+0x80/+0x90. Listeners
(0x141238520 → callback 0x1412bc4c0) only update existing `kind|dept<<8|0x1070000` entries; the save
loader 0x1434fc330 (+0x1434f85e0) only restores saved keys. The Fake row gives the kind maps key 4, but no
list holds a dept-4 entry → slots 0 → limit. The v0.6.1 "fixed: Fake row" modifier consumers
(rush, scales, failure, start-research kind 0x10) had the same gap.
Fix (`src/rnd_capacity.cpp`): mirror every dept-2 entry as dept 4 in the list just read (missing via
TeamModifiers_Add, present ones get Durability's value); stock entries are never written. Both sync hooks
sit after the null check and BEFORE the game loads `[list+8]/[list+0x10]`, so a reallocation inside
TeamModifiers_Add is invisible to it.

| binding | hook | effect |
|---|---|---|
| `site_Career_RnDPanelModifierList` 0x141c8e2a7 | panel, rax = list, bl = dept | mirror for dept 4 |
| `site_Career_RnDPanelResearchLimit` 0x141c8e766 | panel check, ebp slots / eax in-use / r12b gate | diagnostic `capacity check:` (+ Durability slots/in-use from `[rsp+0x1ba0]`, count `[rsp+0x27a0]`) |
| `site_Career_RnDPageModifierList` 0x141c7dcca | dept page `Career_Build_RnDDeptPage`, rax = list, bl = card dept | mirror for dept 4 (Tyres tab slot count) |
| `Career_Add_TeamModifier` 0x1434f8a00 | called (`fn` binding) | TeamModifiers_Add(list, key, float value in xmm2) |

The three sites are one all-or-nothing set with the rest of the addon. The capacity piece has no option
and is always on; every handler is a no-op until `rnd_capacity::install` runs. No in-function branch targets inside the hook
windows. The whole-`.xtls` scan flags only non-code bytes (a
rel8 inside `mov [rsp+0x78],r14` at 0x141c7dcc3, and junk at 0x140f5cac9 outside any `.pdata` function).
Limits: AI teams' lists get no dept-4 entries (stock); a list re-init (0x1434f9770 from 0x14123b3f0, or
0x1434f95f0 rebuild) drops them until the next Tyres page/panel open. Saves persist the mirrored entries.

Expected log:
```
[rnd-tyres] capacity panel: hooked panel modifier list (rax) @0x141c8e2a7 (base+0x1c8e2a7)
[rnd-tyres] capacity panel: hooked panel research-limit check (ebp/eax) @0x141c8e766 (base+0x1c8e766)
[rnd-tyres] capacity INSTALLED: dept 4 mirrors Durability's team modifiers (research slots, scales, rush) when the develop-upgrade panel reads them; TeamModifiers_Add @0x1434f8a00
[rnd-tyres] capacity page: hooked R&D page modifier list (rax) @0x141c7dcca (base+0x1c7dcca)
-- Tyres tab / upgrade --
[rnd-tyres] capacity: Tyres page team 0x… list 0x…: dept 4 mirrored from Durability (src 58, inserted 58, updated 0) -> research slots dept4=N durability=N
[rnd-tyres] capacity check: team 0x… dept 4 slots=N in-progress=0 gate=1 -> research-limit=0 | Durability slots=N in-progress=M
```
(src is 58 in list modes 1/2, 20 in mode 0.)

## v0.7.1 — overview layout
`ui/rnd_overview.idf`: the bottom two tiles (`tile_4`, `tile_5`) are scaled 0.65 → 0.999096 and moved so the
bottom row spans the top row exactly (x 0.0036..8.4961, 0.2 gaps, y −1.708..−4.026; the next-upgrade
panel starts at about −5.04). Only those two transforms changed; ids, lookup and animations are byte-identical
(generator `f1-modding/work/rnd-fifth-box/make_rnd_overview5.py --bottom fill`). `erp/rnd_fifth_pie.json` is
unchanged.

## v0.7.2 — the pie that actually draws, the Tyres tile count, Tyres tags in lists, the Tyres tile route

### 1. What draws the Performance page pie (and why v0.7.1 still showed the stock 4×90°)
Chain (`laptop_r_and_d.ui`, `tab_content_rnd_upgrades_alt`): the tab's layout attach is
`componentId` 0xbe419b0b8324e33c = `art/screens/career/rnd_overview`; the `dev_potential` UIGP attaches to
node `dev_potential` (an eXRef to `rnd_dev_potential_full.idf`), and each department group attaches by the
chain `[chart_1, sector_N]`. `rnd_dev_potential_full.idf` is referenced only by `rnd_overview.idf`.

An eXRef element carries two references: the `fileRef` string param and the `xref` field, a u64 that is the
md5-64 (big-endian, `.idf` stripped) of the referenced scene's eaid, i.e. that scene's `componentId`. Every
stock eXRef has `xref == md5-64(fileRef)`. v0.7.0/v0.7.1 re-pointed `chart_1.fileRef` at
`rnd_dev_potential5.idf` but left `chart_1.xref = 10514293220787342744` = `rnd_dev_potential.idf` (the stock
four-sector pie), and the five sectors of `rnd_dev_potential5.idf` kept `xref = 14655847601348484236` =
`rnd_dev_potential_sector.idf` (the stock 90° wedge). Evidence:
- the game's `flow_career_hub.erp` and `flow_team_hq.erp` hold the v0.7.1 `rnd_dev_potential_full.idf`
  byte-identical to the addon file (sha1 0a37c125…), so the archive rewrite worked, yet the stock pie with
  its cross renders: the only reference to the stock pie left in the live data is the `xref` id;
- the viewer resolves eXRefs by `fileRef`, which is why the v0.7.0 previews showed five sectors. A viewer
  copy that resolves by the `xref` id instead (scratchpad `v072/viewer`, `engine_ref()` in `scene.py`)
  reproduces the in-game screenshot exactly from the v0.7.1 archives (`[engine-xref] chart_1: xref id
  10514293220787342744 -> …/rnd_dev_potential.idf (fileRef says …/rnd_dev_potential5.idf)`, four
  `rnd_dev_potential_sector.idf` wedges, the stock cross backdrop) and five 72° wedges from the v0.7.2 ones.

Fix (assets only, `f1-modding/work/rnd-fifth-pie/tools/build_pie.py` `set_xref`): `chart_1.xref =
6468277874628786377` (md5-64 of `…/rnd_dev_potential5`), `sector_1..5.xref = 11428019250876246940`
(md5-64 of `…/rnd_dev_potential_sect72`). Nothing else in the two scenes changes (typed parse compare);
`tools/validate.py` now fails any eXRef whose `xref` is not md5-64 of its `fileRef` and reports 0 problems on
the pristine archives.

Team HQ research overview (`tab_content_research_overview`, layout `team_hq/research/overview` →
`team_hq/research/dev_potential_full.idf` → stock `rnd_dev_potential.idf`) stays four sectors. It is a
different scene chain and a different view-model class (class A, vtable 0x14575e000: focus handler
0x141f45d30 compares only aero/chassis/maintenance/engine, pie populate gated `< 4`), with four authored
tiles and no Tyres focus. Its pie binds the same `focused_team_perf_root.department_performance.*` root, so
a fifth sector there would get data from the publisher, but without a Tyres tile it would be a wedge nobody
can focus, and without the publisher's Tyres entry an empty 72° gap. The Performance page has its own chain, so
it gets the five sectors and Team HQ keeps the stock pie with no gap.

### 2. The Tyres tile count (flag `pie_tyres_counts`, default on, install-time)
The tile shows `lng_progress_num_total` with `NUM = upgrades_fitted`, `TOTAL = total_upgrades` and a ring on
`upgrades_fitted_percent`. A pie slot holds these as handles 0..3 (slot builder 0x14180b660: `total_upgrades`
0x14180b6ac, `upgrades_fitted` 0x14180b746, `upgrades_fitted_percent` 0x14180b7d0). They are written by
`FUN_141f49bc0(this = rdi, query = rsi)`, called by `Career_Build_RnDPieOverview` (0x141f46eb0) and its
0x141f4a760 callback right after the populate and `Upgrades_QueryEnabled`: per code, total = query results
with `+0x72 == code`, fitted = those whose last state is Completed (3); the four handles are written at
`[this+0x670] + 0x88/0xa8/0xc8/0xe8 + idx*0x220`. Two guards kept slot 4 out:

| binding | instruction | v0.7.2 |
|---|---|---|
| `site_Career_RnDTileCountGuards` (0x141f49c23) | `cmp cl,3 ; ja` (code guard) | site: code 4 resumes at `site_Career_RnDTileCountIndexGuard` (reads as `cmp cl,4`) |
| `site_Career_RnDTileCountIndexGuard` (0x141f49c28) | `cmp ebx,3 ; ja exit` (slot index guard) | site: index 4 needs a registered pie object with an intact footer and 17 slot-4 vtables; then it resumes at `site_Career_RnDTileCountBody` (0x141f49c31, reads as `cmp ebx,4`), else at `site_Career_RnDTileCountExit` (0x141f49f5b, the function's exit = stock behaviour) |

Both sites are always on.
The `.ui` also missed the tile's refresh: `focusable.trigger` (raised by
`onTabActivated`) sent `Trigger` to the four stock tiles only, and the tile's `Trigger` is what runs
`progress_value.refreshTextInst` and the ring's `slowRefresh`; it now also targets `focusable.tyres`.

### 3. Tyres icon and colour in the R&D lists (always on)
`src/rnd_dept_tags.cpp`. Every tag falls back to the frame `"error_department"` (0x14587e10d, 51 references)
when its lookup misses: the lists test the department byte (`dept < 4 ? table[dept]`), the upgrade card the part
type and the perks title the `return_department_name` hash (both below). Colours read a `< 6` table on the
department byte whose [4] is white. The name was already right through the
`lng_rnd_dept_error_department` lang key. Sweep of every `%s.%s.%s` icon path on the R&D atlases
(`rnd_dept_icons_small` ×12, `rnd_category_icons` ×6, `rnd_department_strips` ×1, `rnd_wheel_icon` ×2):

| site (`lea r9,[atlas]`, frame in `[rsp+0x20]`) | screen | department 4 frame |
|---|---|---|
| `site_Career_RnDInProgressPartIcon` 0x141f3da29 (fn 0x141f3bec0) | R&D In Progress list ("Part Information") part icon | `error_department_<type>` → stock `<dept>_<type>` (`chassis_tyre_wear`, `chassis_brakes`, `maintenance_general_maintenance`) |
| `site_Career_RnDInProgressStrip` 0x141f3d71d (fn 0x141f3bec0) | same list, Driver Career colour strip | `rnd_error_department_strip` → `rnd_maintenance_strip` |
| `site_Career_RnDDeptRowCategoryIcon` 0x141c83920 (fn 0x141c82470) | department page category rows (`rnd_category_row`, keyed by the card's +0x6d8) | category mapping |
| `site_Career_RnDResearchRowCategoryIcon` 0x141c78018 (fn 0x141c76b60) | research category rows (`research_upgrades_category_row`) | category mapping |
| `site_Career_RnDPopupCategoryIcon` 0x1419bf600 (fn 0x1419be1e0) | Team HQ R&D popups (research/development completed, failed, delayed) | category mapping |
| `site_Career_RnDUpgradeListDeptIcon` 0x14174dc04 (fn 0x14174cb20), `site_Career_RnDListRowDeptIcon` 0x14174eea5 (fn 0x14174e170) | upgrade lists in the 0x1417 family | `error_department` → `maintenance` |
| `site_Career_UpgradeCardDeptIcon` 0x1417f87af (fn 0x1417f8090) | upgrade card (`Career_Bind_UpgradeCard`, car monitor objectives); the frame (`rbx`) comes from the part type, 0–2 engine, 3–6 chassis, 7–10 aero, 11–18 maintenance (table 0x14571bcc0), and type > 18, so the Tyres types 19–22, gives `error_department` | `error_department` → `maintenance` |
| `site_Career_RnDPerksTitleDeptIcon` 0x141c8876f (fn 0x141c87af0) | department perks title icon; the frame (`r12`) is a literal picked by the djb2 hash of `return_department_name` (aero 0x7c93fd0c, chassis 0xc239ed13, maintenance 0x5c31ec68, engine 0xfb78dbfb), anything else gives `error_department` | `error_department` → `maintenance` |

Each site is the 7-byte `lea r9`, one MidHook window. No window holds a branch target: none in its function's
linear disassembly or jump tables, and the four whole-`.xtls` rel8 candidates (from 0x1417f87ad, 0x141c8874b,
0x141c77ff7, 0x141c838ff) are bytes inside other instructions. The v0.6.1 card-binder department icons
(0x141810478/0x141815666/0x141818611) now borrow `maintenance` too (was `chassis`), so Tyres has one department
icon, the one its tile shows.

Colours: the tables stay stock. A site on the store after each `< 6` read sets department 4's white to the Tyres
orange, testing the movsx'd department still in the index register:

| site | store, table | colour, department | department 4 white → |
|---|---|---|---|
| `site_Career_RnDListRowColourStore` 0x14174e591 (fn 0x14174e170) | `mov [rsp+0x3c],r8d`, `k_Career_RnDListColourTable` | r8d, rdx | 0xff0078ff |
| `site_Career_RnDListRowLightColourStore` 0x14174e5af (fn 0x14174e170) | `mov [rsp+0x44],ecx`, `k_Career_RnDListLightColourTable` | ecx, rdx | 0xff99c9ff |
| `site_Career_RnDPopupColourStore` 0x1419bf5aa (fn 0x1419be1e0) | `mov [rsp+0x8c],ecx`, `k_Career_RnDPopupColourTable` | ecx, rax | 0xff0078ff |
| `site_Career_RnDInProgressColourStore` 0x141f3dacf (fn 0x141f3bec0) | `mov [rsp+0x6c],eax`, `k_Career_RnDInProgressColourTable` | eax, rcx | 0xff0078ff |

Each store is the `ja` target of its bound check and that `ja` is the only branch to it, so the department
register holds on both paths. Windows: 5, 8, 7 and 12 bytes, none with a branch target inside. The In Progress
table 0x14576f5d8 is also read by the Driver Career rewards (0x1420bcfb0, 0x1420c1bd0). Not touched: 0x145784c18
(Team HQ department tiles 0x1422fe430 and department events 0x1422e5af0 index it by an event department, where [4]
may not mean Tyres) and the perks title colour (the perks panel maps unknown names to index 5).

### 4. The Tyres tile opens Durability (flag `pie_tyres_open`, default on, install-time)
Tile press → `focusable.select_pressed` → `raiseEvent goto_rnd_departments` → `laptop_container.ui`
`raise_command goto_rnd` → the departments flow → `Career_Build_RnDDeptBoxGrid` picks the first tab from
`DAT_145f93c08` clamped `< 4` (0x141c86c7e) and maps the name to a card (aero/chassis/engine, else
`cards[3]` = Durability). The Tyres focus deliberately never writes `DAT_145f93c08` (every pie/box read
clamps it), so the screen opened the last stock department focused before Tyres. The v0.7.1 log has
`box-grid builder reached: … selection=2` right after `department_focused "tyres"`, and the box-layout probe
right after shows the Tyres card already owns slot[6] (`bound-node=0x32a091068 = card+0x268`), so the
Back-route fallback was not involved.

`DAT_145f93c08` (`g_Career_RnDOverviewSelectedDept`) has exactly two writers (RIP-xref): 0x141f4aa49
(class B, Performance page focus) and 0x141f460a9 (class A, Team HQ research overview focus). Each runs
exactly when its handler's strcmp chain (aero/chassis/maintenance/engine) matches. The two handlers are
byte clones around both stores, so the flag hooks sit on each chain's first strcmp, where the code still
differs, and compare the data string themselves. Fix:

| site | kind | does |
|---|---|---|
| `site_Career_RnDPieFocusNameCmp` 0x141f4a9e6 (class B, also the focus hook) | site | data `tyres` sets "Tyres is the focused tile"; a stock name clears it (a stock focus on the Performance page) |
| `site_Career_RnDResearchFocusNameCmp` 0x141f46046 (class A) | site | a stock name clears it (a stock focus on the Team HQ research overview) |
| `site_Career_RnDPieFocusRestore` 0x141f48476 `cmp r14b,al ; jne` (populate focus restore) | site | flag set: al = 4 for code 4, 0xff for codes 0..3, so the rebuilt page focuses the Tyres tile again |
| `site_Career_RnDBoxGridFirstTab` 0x141c86d03 `mov rax,[rax] ; mov [rsi+0x7b8],rax` (box-grid first-tab join) | site | flag set and the Tyres card (`vt_Career_RnDDeptCard`, `+0x6d8 == 4`) owns a container slot: `rax` = its card-vector element, and the stock tail stores, enables and selects it. No such slot: stock first tab |

All windows pass the in-function branch-target check; the whole-`.xtls` scan finds no branch into them. The
route site sits next to the Back route (it reuses `find_tyres_tab`) and acts only when the tracking hooks are
live. Other ways in already land on the Tyres tab: the departments tab bar, Back
from a Tyres upgrade, the perks back and cross-department jumps (v0.6.1 open dispatch).

### Expected log (v0.7.2 additions)
Install:
```
[rnd-tyres] pie5 open: hooked R&D > Performance stock focus @0x141f4a9e6 (base+0x1f4a9e6)
[rnd-tyres] pie5 open: hooked Team HQ research overview stock focus @0x141f46046 (base+0x1f46046)
[rnd-tyres] pie5 open: hooked populate focus restore compare @0x141f48476 (base+0x1f48476)
[rnd-tyres] pie5 open INSTALLED: the Tyres focus is tracked for the departments screen's first tab and restored on the Performance page
[rnd-tyres] pie5 counts INSTALLED: Career_Write_RnDPieTileCounts counts code 4 into slot 4 (cmp cl,3->4 at site_Career_RnDTileCountGuards, cmp ebx,3->4 and the slot-4 gate at site_Career_RnDTileCountIndexGuard)
[rnd-tyres] dept tag: hooked <site> @0x… (base+0x…)                       (x9)
[rnd-tyres] dept tag: hooked R&D list row colour store @0x14174e591 (base+0x174e591)
[rnd-tyres] dept tag: hooked R&D list row light colour store @0x14174e5af (base+0x174e5af)
[rnd-tyres] dept tag: hooked Team HQ R&D popup colour store @0x1419bf5aa (base+0x19bf5aa)
[rnd-tyres] dept tag: hooked in-progress list colour store @0x141f3dacf (base+0x1f3dacf)
[rnd-tyres] dept tags INSTALLED: 9/9 icon sites; …
[rnd-tyres] Tyres tile route LIVE: pressing the Tyres tile on R&D > Performance opens the Tyres department page
```
R&D > Performance:
```
[rnd-tyres] pie5 counts: slot 4 verified (object 0x… gen N); Tyres tile upgrades_fitted/total_upgrades = 0/3
```
Focus Tyres, press it:
```
[rnd-tyres] pie5 focus: department_focused "tyres" -> focused_department = tyres, …
[rnd-tyres] box-grid builder reached: … selection=2 …
[rnd-tyres] departments first tab: Tyres was the focused tile -> Tyres tab (card=0x… slot elem=0x…, stock element 0x…)
```
Focus a stock tile afterwards: `[rnd-tyres] pie5 open: R&D > Performance focused stock department index N;
Tyres focus cleared`. In Progress with a Tyres upgrade listed: `dept tag: in-progress list part icon
"error_department_tyre_wear" -> "chassis_tyre_wear"` and `dept tag: in-progress list colour for department 4
white -> 0xff0078ff`; an R&D list row: `dept tag: R&D list row colour for department 4 white -> 0xff0078ff`;
a Team HQ R&D popup: `dept tag: Team HQ R&D popup colour for department 4 white -> 0xff0078ff`. Refusals end
in `(SAFE)` (`pie5 counts SKIPPED: …`, `… stock first tab kept`).

### Live checks (gdb under Wine, `handle all nostop noprint pass` + `handle SIGTRAP stop noprint nopass`)
Hardware breakpoints on the instruction after each hook window:
- `hbreak *0x141f49c3d if $rbx==4` on R&D > Performance → `x/gx $rdi+0x670`, then `p *(char*)0x141f49c25`
  = 4; after the function returns, slot 4's `total_upgrades` node is the one the tile shows.
- `hbreak *0x141c86d0d` when the departments screen opens from the Tyres tile → `x/gx $rsi+0x7b8` is the
  Tyres card (`x/bx $rax+0x6d8` = 4).
- `hbreak *0x141f3da30` with a Tyres upgrade in development → `x/s *(char**)($rsp+0x20)` = `chassis_tyre_wear`.

## v0.8.0 — the Tyres department icon (setting `tyres_icon`, db, default on)

### How a department icon is stored
An icon path `<pkg>.<atlas>.<frame>` (`<pkg>` = `#overrides.career.rnd_om_xm` on the 2026 career, logged by the
5th box) names an entry of an `overrides.uidb` list (`databases.erp`, `eaid://ui_package/databases/overrides.uidb`).
A list such as `overrides.career.rnd_om_xm.rnd_wheel_icon` has `TextureArray: true`, a `TextureArrayName`, and
`Contents` = `[{Name, Value = source .tif path}]`. The frame is the slice at the entry's index in that texture
array: a `GfxSRVResource` `….texarray` (slice count at 0x14) and a `GfxSurfaceRes` `….texarray.image` (fragment 0
= 40-byte head: format at 8, width 12, height 16, mips 24, slices 28; fragment 1 = the slices back to back). There
are no UV rects and the `.tif` paths in `Value` are not resources in any UI archive. The launcher's map activation
(`packages/core/src/activation/ui.rs`) appends track flags the same way: entry index = slice index.

| list (plain and `rnd_om_xm.`) | format | slice | stock frames | archives | Tyres frame |
|---|---|---|---|---|---|
| `rnd_dept_icons_small` | BC7 (70) | 48×48, white, alpha 255 + 128 | aero chassis maintenance engine | `flow_career_hub` | `tyres` (slice 4) |
| `rnd_wheel_icon` | BC7 | 60×60, department colour, alpha 255 + 128 | aero chassis maintenance engine regulation shutdown | `flow_career_hub`, `flow_team_hq` | `tyres` (slice 6), RGB 255,120,0 |
| `rnd_department_strips` | BC7 | 1112×4 solid colour | rnd_aero/chassis/engine/maintenance_strip | `flow_career_hub`, `flow_team_hq` | `rnd_tyres_strip` (slice 4) |
| `rnd_category_icons` | BC7 | 32×32 | 21 (`rnd_om_xm`: 20) `<dept>_<type>` frames | both | none: the category rows keep the stock icon of the upgrade's type |

The stock frames are two-tone: the main shape at alpha 255, the secondary shape at alpha 128, one RGB per frame,
about 1 px padding. The UI archives hold no vector format (resource types: meshes, models, materials, BC7 surfaces,
scenes, `.ui` and databases); the pie's round sectors are BC7 quarter-disc masks on mesh quads. Icons are textures.

### The assets (`erp/rnd_tyres_icon.json`, built by `tools/build_icons.py`)
`icons/tyres.svg` is rasterised with `resvg` at 48 and 60 px, given one RGB (white, or the Tyres orange
0xff0078ff), and BC7-encoded with `tools/bc7enc.py` (mode 6, round trip max 9/255). The strip is solid orange. Each
array gets `replace-resource` on the head (slices + 1), the pixels (stock slices byte for byte + the new slice)
and the SRV view (slices + 1): 18 operations in `flow_career_hub.erp` and 12 in `flow_team_hq.erp`. One `patch-pkg`
on `overrides.uidb` appends the six entries with `clone-entry` (`src` = the list's last entry, `as` = the frame,
`Value` = `…/tyres.tif` or `…/rnd_tyres_strip.tif`).

Offline check (stock copies, every operation applied with `tools/erpw.py` and `tools/pkg.py`): 10332 untouched
resources of `flow_career_hub.erp`, 19716 of `flow_team_hq.erp` and 7 of `databases.erp` decompress byte-identical;
every stock slice of the ten grown arrays is byte-identical; the lists read `[…, tyres]` / `[…, rnd_tyres_strip]`.

### The native side (`src/rnd_tyres_icon.cpp`)
`rnd_tyres_icon::probe()` runs first in `SWOFTY_ON_LOAD`. It reads the ERP index of
`<exe folder>\2025_asset_groups\ui_package\flow_career_hub.erp` and needs fragment 1 of all six career-hub
`.texarray.image` resources at their grown size (11520, 25200, 22240 bytes). Then every Tyres frame switches:

| site | file | 0.7.2 frame | 0.8.0 frame |
|---|---|---|---|
| 5th box card ctor icon (`install_fifth_box`) | `rnd_departments.cpp` | `rnd_dept_icons_small.aero` | `.tyres` |
| Develop Upgrade card art, `error_department` (`on_part_art`) | `rnd_departments.cpp` | `maintenance` | `tyres` |
| R&D list department icons (`TagKind::DeptIcon`, 4 sites) | `rnd_dept_tags.cpp` | `maintenance` | `tyres` |
| In Progress colour strip (`TagKind::Strip`) | `rnd_dept_tags.cpp` | `rnd_maintenance_strip` | `rnd_tyres_strip` |
| Tyres tile wheel icon (`on_slice_icon`, populate H4) | `rnd_fifth_pie.cpp` | `rnd_wheel_icon.maintenance` | `rnd_wheel_icon.tyres` |

Any other probe result (archives not rewritten, a runtime delivery that left the files stock) keeps
the 0.7.2 frames, so no hook ever names a frame the game lacks. No new hook sites.

### Expected log (v0.8.0)
```
[rnd-tyres] tyres icon LIVE: 6/6 grown R&D icon arrays in flow_career_hub.erp; Tyres shows rnd_dept_icons_small.tyres, rnd_wheel_icon.tyres and rnd_department_strips.rnd_tyres_strip
[rnd-tyres] pie5 slice INSTALLED: code 4 populates as tyres / 0xff0078ff / icon tyres
[rnd-tyres] dept tags INSTALLED: 9/9 icon sites; department 4 shows the tyres department icon, the rnd_tyres_strip strip, …
[rnd-tyres] pie5 slice: code 4 wheel icon -> rnd_wheel_icon.tyres
[rnd-tyres] 5th box INJECTED: … icon="#overrides.career.rnd_om_xm.rnd_dept_icons_small.tyres" …
[rnd-tyres] card art: site N department icon "error_department" -> "tyres"
```
Fallback: `tyres icon off: 0/6 grown R&D icon arrays … (SAFE)` and the 0.7.2 lines with `maintenance` /
`aero` / `rnd_maintenance_strip`.

## v0.9.0 — the Tyres upgrade types (setting `tyres_upgrade_types`, default on, install-time)

`src/rnd_upgrade_types.cpp`. The `tyres_upgrade_types` setting is gone: the types are always on while the addon is
enabled. The Tyres department gets its own upgrade types instead of borrowing stock ones, so its upgrades never
dilute Chassis' Tyre Wear or Brakes:

| id | `upgrade_type` | camel-case (telemetry) | UI key suffix | VTF block | full-progress effect |
|---|---|---|---|---|---|
| 19 | `Tyre Thermal` | `TyreThermal` | `tyre_thermal` | `radUpgradeTyreThermal` | Tyre Front/Rear `UpgradeTempFactor` ×0.85 |
| 20 | `Brake Cooling` | `BrakeCooling` | `brake_cooling` | `radUpgradeBrakeCooling` | Brake Front/Rear `CoefHeatTransferConstant`, `CoefHeatTransferLinear` ×1.25 |
| 21 | `Tyre Life` | `TyreLife` | `tyre_life` | `radUpgradeTyreLife` | Tyre Front/Rear `UpgradeWearFactor` ×0.90 |
| 22 | `Power Unit Cooling` | `PowerUnitCooling` | `pu_cooling` | `radUpgradePUCooling` | Engine `RadiatorSurfaceArea` ×1.15 |

How a type reaches the car (from the R&D effect report): `upgrade_type` string → id (name table
0x14584f6b8, 19 entries, unknown → 0 = Engine Power) → per-type progress 0x143526260 → the vehicle
fill 0x141238cb0 puts `map[type][0] = {progress, mass, bhp}` into the vehicle upgrade component
(`comp+0x28`) → 0x14fa4deb0 calls 0x142f64860 → 0x14fa63050(comp, type) for keys < 11 and 11..17 →
0x14fa63050 names the block (`type < 0x12`, relative table 0x145803ce4) and replays its ops with
`perc` = clamp(Σ progress) as the lerp. Department is never read on this path.

### Every place sized for 19 types (sweep)

Found by RIP-xref of the order table 0x14566fd16 / its end 0x14566fd29 (9 functions), the name table
0x14584f6b8 (2), the block-name table 0x145803ce4 (1, raw rel32 scan: it is in `.data1`), the
type→department table 0x14584f848 (2), the camel-case table 0x14564a0d0 (3), the string "INVALID
UPGRADE" 0x14587df9f (16 inlined switches), and a byte sweep for `movzx r,byte [x+0x70]` followed by a
rip-relative table `lea` (26 tables, the two camel-case reads are the only type-indexed one).
The 0.9.0 in-game test crashed on session entry at a site none of these find: the `std::bitset` throw
thunks (13 of them, each loading "invalid bitset position" 0x14561370e) have 229 callers, and a sweep of the
bound compare before each branch to them finds `0x13` only twice, both in 0x14295c0a0.

| site | what it does with the type | v0.9.0 |
|---|---|---|
| 0x143539bc0 `Career_Load_UpgradeRows` loop 0x14353a3d0..0x14353a405, join **0x14353a408** `mov [rsp+0xc8],r15b` | 19-name compare, miss → 0 | `site_Career_UpgradeRowTypeJoin`: when `r14 == 0x13` (stock loop exhausted) and the decoded string `r13` is one of ours (case-insensitive), `r15` = 19..22 |
| 0x14353c4f0 `upgrade_type_metadata` loader loop 0x14353c890..0x14353c8be, join **0x14353c8c1** `lea rsi,[rsp+0x248]` | same compare for the metadata rows | `site_Career_UpgradeMetaTypeJoin`: when `rsi == 0x13` and `r12` is ours, `r14` = 19..22 (stored to `rec[0]` at 0x14353c6f0; no write of r14 between) |
| 0x14352cac0 copy into `U+0x1a38` | dynamic vector, grows | none needed |
| 0x143526260 progress | generic (`inst.type == T`, metadata `rec[0] == T`; UAD bonus only for 11..17) | none needed |
| 0x141238cb0 vehicle fill, 0x141238f0a `lea r15,[order]`, 0x141238f84 `lea rax,[order end]` | iterates the 19 order bytes (`k_Career_UpgradeTypeOrder`) | `site_Career_VehicleFillOrderStart` (**0x141238f11**, the `lea rbp` after the first lea) sets `r15` = the extended order table (0..18 from `k_Career_UpgradeTypeOrder`, then 19..22; a static array in the DLL); `site_Career_VehicleFillOrderEnd` (**0x141238f8b** `cmp r15,rax ; je`, after the per-iteration end lea) sets `rax` = its end while `r15` walks that table. The stock table is never written |
| 0x14fa4deb0 apply, second filter **0x14fa4e01c** `lea eax,[rdx-0xb] ; cmp al,6` | keys 11..17 only | `site_Career_ApplyUpgradeKeyFilter` (`.data1`, 5-byte window): key 19..22 → `rip` = `site_Career_ApplyUpgradeKeyCall` (0x14fa4e023 `mov rcx,rsi ; call 0x142f64860`) |
| 0x14fa57080 apply (flag-0 variant) | applies only keys 6 and 2, then erases them; mass/power only | nothing to extend: no type other than 2/6 is applied in this variant, stock or ours |
| 0x14fa63050 block name, `cmp dl,0x11 ; ja` → **0x14fa6315f** `lea rdx,["?"]`, join 0x14fa63166 | names 0..17 (`k_Career_UpgradeBlockNameTable`), else "?" | `site_Career_ApplyUpgradeBlockNameDefault` (`.data1`, body without `.pdata`, `func_start` 0x14fa63050): type 19..22 → `rdx` = our block name, `rip` = `site_Career_ApplyUpgradeBlockNameJoin`. A car VTF without the block matches nothing in the replay loop (no op applied) |
| 0x141c7b880 `Career_Build_RnDDeptPage` category loop, 0x141c7e4c8 `lea r14`, 0x141c7e4df `lea r15` | one category row per type (0x141c82470, returns on an empty list) | `site_Career_RnDDeptPageTypeLoop` (**0x141c7e4e6**, the loop-entry `jmp` after both leas; nothing between reads r14) sets `r14`/`r15` = the extended table; the list is already filtered by the page department (0x141c7d921), so the new rows appear only on the Tyres page |
| 0x141c73b80 research category loop, 0x141c75733 `lea r12`, 0x141c7574a `lea r13` | one research row per type (0x141c76b60, empty list → return) | `site_Career_RnDResearchTypeLoop` (**0x141c75751**, the loop-entry `jmp`) sets `r12`/`r13`; list filtered by department at 0x141c74cc1 |
| 16 inlined switches `cmp <type>,0x12 ; ja` → `lea r8\|r9,["INVALID UPGRADE"] ; jmp <join>` | `lng_rnd_%s` label (r8) and `<dept>_<type>` category frame (r9) | one site per switch, on the default `lea` (or on the `cmp` head where the default's bytes repeat in clone binders and have no unique signature); type 19..22 → dest register = suffix, `rip` = the switch's join binding (`<site>Join`). Sites (hook / type reg / join): `site_Career_UpgradeCardTypeLabel` 0x1417f831d r14b 0x1417f83d2 (upgrade card), `site_Career_Unk141814320TypeLabel` head 0x141814638 rax 0x14181470f, `site_Career_Unk1418169f0TypeLabel` head 0x141816def rax 0x141816ec6, `site_Career_RnDResultTypeLabel` 0x1418b0b11 rax 0x1418b0bc0, `site_Career_RnDResultTypeIcon` 0x1418b0d32 rcx 0x1418b0de0, `site_Career_TeamHqRnDPopupTypeIcon` 0x1419bf4ae rcx 0x1419bf563, `site_Career_RnDPurchaseScreenTypeLabel` head 0x141c514a4 rax 0x141c5157b, `site_Career_RnDUpgradeInfoTypeLabel` head 0x141c7005a rax 0x141c70131, `site_Career_RnDResearchRowTypeLabel` 0x141c76daf r9d 0x141c76e64, `site_Career_RnDResearchRowTypeIcon` 0x141c77df8 sil 0x141c77ea1, `site_Career_RnDTodoTypeLabel` head 0x141c80be4 rax 0x141c80cbb, `site_Career_RnDDeptRowTypeLabel` 0x141c826b6 esi 0x141c8276b, `site_Career_RnDDeptRowTypeIcon` 0x141c836f1 sil 0x141c8379a, `site_Career_RnDPurchasePanelTypeLabel` head 0x141c8e919 rax 0x141c8e9f0 (develop panel), `site_Career_RnDInProgressTypeLabel` 0x141f3d185 rax 0x141f3d1d0, `site_Career_RnDInProgressTypeIcon` 0x141f3d960 rcx 0x141f3d9b0. A head redirect leaves the registers as the default path does (only the jump-table dispatch writes rax/rcx) |
| 0x14071e470 telemetry "upgrade fitted" (`UpgradeFittingChangedMessage` handler, subscribed at 0x1406db634 and 0x1406fdba0: logs "Player fitted upgraded part to driver", queues `rnd_upgrade_finished` through 0x140777ff0; the binding names keep the older "research" wording), **0x14071e6fa** `lea rcx,[0x14564a0d0] ; mov rdx,[rcx+rax*8]` | camel-case name, **no bound** | `site_Career_TeleResearchTypeName`: type ≥ 19 → `rdx` = camel name (or `"unknown"`), `rcx = rbx`, `rip` = `site_Career_TeleResearchTypeNameCall` (0x14071e708) |
| 0x14071e810 telemetry "upgrade failed", **0x14071e98d** `mov rdx,[rbp+rax*8]` | same, no bound | `site_Career_TeleFailedTypeName`: type ≥ 19 → `rdx` = camel name, `rip` = `site_Career_TeleFailedTypeNameJoin` (0x14071e992) |
| 0x140777d50 telemetry upgrade record, **0x140777eff** `lea rdx,[0x14564a0d0] ; mov rdx,[rdx+rax*8]` | same, no bound | `site_Career_TeleRecordTypeName`: type ≥ 19 → `rdx` = camel name, `rip` = `site_Career_TeleRecordTypeNameCall` (0x140777f0a) |
| 0x14295c0a0 weekend objectives (Team HQ → session), **0x14295c6b3** `movzx edx,byte [rdx+0x70] ; cmp edx,0x13 ; jb 0x14295c690` | `std::bitset<19>` of the types with an Available upgrade (`bts r15d,edx`), the filter for the upgrade-discount rewards; type ≥ 19 → `jmp 0x14295cf89` → throw "invalid bitset position" | `site_Career_WeekendObjectiveType` (7-byte window, no relative operand): instance type 19..22 → `rip` = `site_Career_WeekendObjectiveNext` (0x14295c694, next instance, bit not set). The rewards name stock types only (camel-case resolver 0x1409ea830), so the second bitset at 0x14295cf42 never sees ours |
| 0x14353d4f0 regulation-change loader (type→department 0x14584f848, `< 0x13` else 5) | picks stock types of an affected department | none: regulation changes never pick a Tyres type (department 4 is never "affected") |
| 0x143527a70 regulation-change save reader (default `types_affected` = all 19) | same | none |
| 0x14275f620 entrants lane, 0x1409b9aa0 (attribute-set / car lane) | uniform values per stock type | none: not on the career path |
| 0x1409ea830 camel-case resolver (attribute sets) | `EnginePower` default | none: not on the career path |

Without the three telemetry guards a Tyres purchase, failure or fit would read the string bytes after
0x14564a0d0 ("EnginePo…") as a pointer. They are part of the all-or-nothing core.

### Gate and order

Every hook above is a KB binding (subsystem `career`); the mixin system resolves them by signature and installs
all of them or none, so no per-site byte windows, scans or rollback remain. The `.data1` sites
(`site_Career_ApplyUpgradeKeyFilter`/`KeyCall`, `site_Career_ApplyUpgradeBlockNameDefault`/`Join`) carry `.data1`
signatures under the PROTOCOL §2 exception, so on the EA build (packed `.data1`) the addon does not register at all.

There is no setting: the types are on whenever the addon is enabled. `install()` runs from the addon's
`SWOFTY_ON_LOAD`, after every site is live. It fills the extended order table (0..18 from `k_Career_UpgradeTypeOrder`,
then 19..22) and then turns the module live in one store, so every consumer and both resolvers switch together.
Until then every handler returns at once. `live()` is that flag.

Every hook window was checked against every direct branch target in its function (rel32 `E8/E9/0F 8x`, rel8
`EB/7x`): none lands inside a window except at its first byte (the resolver joins and the switch defaults are
branch targets at their start). Join and skip destinations are only jump targets, never patched.

### Upgrade apply log (setting `log_upgrade_apply`, default on, read-only)

The read-only apply log and its `log_upgrade_apply` setting are gone; `site_Career_UpgradeBlockReplayEntry`
(0x144742c70, the block replay the per-type apply body tail-jumps to) and `k_Career_UpgradeBlockNameTable` stay in the KB
but are no longer used. A Tyres type reaching the car shows as the block-name line below, and with the game's
`VEHICLE_UPGRADES` channel on as the game's own `Applying <block> upgrade to vehicle N. (perc0to1: x)`. That perc is
max(0, Σ progress) for Σ ≤ 1; above 1 the `.data1` clamp's `vblendvps` (0x14fa630c7) reads its 1.0 constant one byte
low (0x14538250f) and yields ~-0.0, a stock quirk.

### Other native changes

- `rnd_departments.cpp`: our rows are `tyres_thermal_1..15` (hash `djb2("tyres_thermal_<n>_255")`, built in a
  loop; rows 1-3 keep their v0.6 hashes), names `tt_*`/`bc_*`/`tl_*`/`pc_*` from the spec, plus the three
  legacy names as a fallback. The vehicle performance comparison department comes from the instance type
  (19-21 Chassis, 22 Powertrain), else from the row (13-15 Powertrain, others Chassis).
- Card art and category icons: see "v0.9.0 — the card art" below.
- The test price is gone; the tree carries real prices.

### Data (`tools/build_vtf.py`)

- `erp/rnd_upgrade_vtf.json` + `vtf/`: the four blocks, inserted after the stock `radUpgrade*` blocks in 44
  handling VTFs (94 copies, 65 archives): 2026 team cars, `fom_car`, `fom_car_<team>`, `myteam_*`; 2025 team
  cars, `fom_car`, `myteam_*`. Each op is emitted only where the car's base section has that parameter.
  Every stock byte of each VTF is unchanged around the inserted run; the TSMD stats are rebuilt and checked.
- `db/upgrade_types.json`: rows `upgrade_type_metadata_19..22` in the 46 `upgrades.2026.trees.<team>`
  tables (count 19 → 23), cloned from Tyre Wear (19, 21), Brakes (20) and ICE Durability (22), department
  `Fake`, mass/bhp 0, `percentage_worth` 1, `affect_on_vehicle` −15 / 25 / −10 / 15 percentage.
- `--no-pu` builds ids 19-21 only; set `kShipPuCooling = false` in `rnd_upgrade_types.cpp` to match.

### Power Unit Cooling (why id 22 ships)

The Engine reader 0x144685680 stores `RadiatorSurfaceArea` at params +0x1128 (runtime +0x1140, params at
engine+0x18). The engine temperature update 0x144689710 (runtime temperature +0x253c) cools by
`h(v) × RadiatorSurfaceArea × (T − ambient) × dirty-air factor` (area read at 0x14468999b, 0x1446899db);
temperature feeds the power factor 0x1446db280 through `TempEfficiencySpline` (1.0 at 388-398 K, 0.94 at 418 K,
0.85 at 448 K). The upgrade replay reaches the Engine through its component vt+8 (0x14475ab50 → reader with
the lerp global 0x145faa650), like the stock `radUpgradeWear*` blocks that already target `<Engine>`.
Open: whether the per-vehicle temperature flag `[veh+0x131] & 2` is set in career races, and how hot the
engine runs (the effect is zero on the 388-398 K plateau).

Brake `CoefHeatTransferConstant/Linear` (brake params +0x20/+0x24, reader 0x1446aa090) has no static
consumer found: brake temperature (wheel+0xe8) is read every tick (efficiency spline, overheat damage, the
tyre heat update at 0x1446b7f86) but its only static writer is the reset at 0x1446b2221. The stock
`very_high` setup block scales the same two values. Brake Cooling needs the in-game check.

### Expected log (v0.9.0)

Install:
```
[rnd-tyres] tyres art: part images OWN (2/2 arrays with the 45 Tyres slices in flow_team_hq.erp), part masks OWN (2/2 with 15), category icons OWN (2/2 with 4 in flow_career_hub.erp); a borrowed piece uses stock frames (SAFE)
[rnd-tyres] upgrade types: 4 new types (Power Unit Cooling shipped)
[rnd-tyres] upgrade types INSTALLED: 19 radUpgradeTyreThermal, 20 radUpgradeBrakeCooling, 21 radUpgradeTyreLife, 22 radUpgradePUCooling; 22 consumer hooks, 6 order-table leas -> extended order table 0x… (23 types, 4 sites), 2 resolver hooks
```
Career load:
```
[rnd-tyres] upgrade types: upgrade_type_metadata row "Tyre Thermal" -> type 19 (radUpgradeTyreThermal)
[rnd-tyres] upgrade types: upgrade row "Brake Cooling" -> type 20 (radUpgradeBrakeCooling)
```
Tyres page / In Progress / cards: `[rnd-tyres] upgrade types: department category row 0x141c82470 label type 19 -> "tyre_thermal"`
and `dept tag: department category row icon "error_department_tyre_thermal" -> "tyres_tyre_thermal"`
(`chassis_tyre_wear` with `tyres_card_art` off).
Team HQ → session (weekend objectives), once per Tyres upgrade that is Available:
```
[rnd-tyres] upgrade types: weekend objectives 0x14295c0a0 leaves Available type 19 (TyreThermal) out of the discount type bitset
```
Session load (vehicle build), for each Tyres type the car's upgrade map holds (first 8 lines):
```
[rnd-tyres] upgrade types: apply 0x14fa63050 type 19 names block radUpgradeTyreThermal
```
With the game's `VEHICLE_UPGRADES` channel on: `Applying radUpgradeTyreThermal upgrade to vehicle N. (perc0to1: x)`.
The per-site lines are core's: `swofty/loader.log` lists each `site_Career_*` binding with its chain. A binding that
does not resolve drops the whole addon (core names it); nothing is minted unless every consumer is live.

## v0.9.0 — the card art (setting `tyres_card_art`, db, default on)

### Where the game picks a card's frame
The three upgrade card binders (0x14180fc60, 0x141814320 and 0x1418169f0) read the instance's last state byte
(`[inst+0xc8] - 8`) at 0x141810065, 0x14181541a and 0x1418183a5: state ≤ 1 keeps `upgrade_name`,
state 2 formats `"%s_in_progress"` (0x14587e11e), anything else `"%s_complete"` (0x14587e12d). The
result goes to `Ne_Format_String("%s.%s.%s", pkg, "rnd_part_image", name)`; the mask call
formats `pkg.rnd_part_mask.<upgrade_name>` with no suffix. `pkg` is `#overrides.career.rnd_om_xm`
on the 2026 career. Those are the only references to `rnd_part_image`, `rnd_part_mask` and the
two suffix strings in `.xtls`.

The 2026 lists do not use those suffixes: `rnd_om_xm.rnd_part_image` names its frames
`om_xm_<part>`, `om_xm_<part>_progress` and `om_xm_<part>_complete`, and `rnd_om_xm.rnd_part_mask`
names its masks `om_xm_<part>_m`. The plain lists use `<part>`, `<part>_in_progress`,
`<part>_complete` and the mask `<part>`.

### The hook (`on_part_art`, `rnd_departments.cpp`, the six v0.6.1 art sites)
For one of our upgrades the hook rewrites the frame name `[rsp+0x20]` to the list's own name for
the state. `om_xm_` is added when the pkg contains it. A mask is a `site_Career_RnDCardArtMaskN` site: the
`lea r9` each site sits on loads `rnd_part_mask` there, and `r9` itself is not loaded yet when the site runs.

| card state | binder name | `rnd_om_xm` frame | plain frame |
|---|---|---|---|
| locked / available | `<name>` | `om_xm_<name>` | `<name>` |
| in progress (research or development) | `<name>_in_progress` | `om_xm_<name>_progress` | `<name>_in_progress` |
| fitted / complete | `<name>_complete` | `om_xm_<name>_complete` | `<name>_complete` |
| mask | `<name>` | `om_xm_<name>_m` | `<name>` |

`<name>` is the upgrade's own name when `rnd_tyres_icon::probe_art()` finds the grown arrays, else
the borrowed stock part (`tyre_blankets`, `calipers`, `turbo_charger_wear_1`), which has the same
four frames in both lists. Images and masks switch separately.

### The probe (`rnd_tyres_icon::probe_art`)
It reads the ERP index of the installed archives and needs fragment 1 of each array at exactly the
stock size plus our slices (240×240 BC7 = 57600 bytes a slice, 32×32 = 1024):

| array (`.texarray.image`) | archive | stock slices | ours | size |
|---|---|---|---|---|
| `rnd_part_image` | `flow_team_hq.erp` | 362 | 45 | 23443200 |
| `rnd_om_xm.rnd_part_image` | `flow_team_hq.erp` | 337 | 45 | 22003200 |
| `rnd_part_mask` | `flow_team_hq.erp` | 120 | 15 | 7776000 |
| `rnd_om_xm.rnd_part_mask` | `flow_team_hq.erp` | 110 | 15 | 7200000 |
| `rnd_category_icons` | `flow_career_hub.erp` | 21 | 4 | 25600 |
| `rnd_om_xm.rnd_category_icons` | `flow_career_hub.erp` | 20 | 4 | 24576 |

### Category icons (`rnd_dept_tags.cpp`)
The category rows build `<department>_<type>` (`"%s_%s"`, 0x1456708d0), and department 4 gives
`error_department`. With the arrays grown the four category sites (`site_Career_RnDDeptRowCategoryIcon`,
`site_Career_RnDResearchRowCategoryIcon`, `site_Career_RnDPopupCategoryIcon`,
`site_Career_RnDInProgressPartIcon`) ask for `tyres_tyre_thermal`,
`tyres_brake_cooling`, `tyres_tyre_life` or `tyres_pu_cooling`; otherwise `chassis_tyre_wear`,
`chassis_brakes`, `chassis_tyre_wear` and `maintenance_internal_combustion`. The icon site in
0x1418b02b0 (`"%s_%s"` into a string, no `[rsp+0x20]` frame) is not hooked and shows no icon for
department 4, as before.

### Expected log (v0.9.0 card art)
Install:
```
[rnd-tyres] tyres art: part images OWN (2/2 arrays with the 45 Tyres slices in flow_team_hq.erp), part masks OWN (2/2 with 15), category icons OWN (2/2 with 4 in flow_career_hub.erp); a borrowed piece uses stock frames (SAFE)
```
Tyres page and Develop Upgrade screen (first 32 rewrites):
```
[rnd-tyres] card art: site 2 image "tt_thermal_model" -> own "om_xm_tt_thermal_model"
[rnd-tyres] card art: site 3 mask "tt_thermal_model" -> own "om_xm_tt_thermal_model_m"
[rnd-tyres] card art: site 2 image "tt_thermal_model_in_progress" -> own "om_xm_tt_thermal_model_progress"
[rnd-tyres] card art: site 2 image "tt_thermal_model_complete" -> own "om_xm_tt_thermal_model_complete"
[rnd-tyres] dept tag: department category row icon "error_department_tyre_thermal" -> "tyres_tyre_thermal"
```
Without the grown arrays: `part images borrowed (0/2 …)` and `-> borrowed "om_xm_tyre_blankets_progress"`, and
the category rows fall back to the stock icons.
