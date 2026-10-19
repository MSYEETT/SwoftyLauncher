# career R&D UI render surfaces (the invisible 5th department)

Owner: career lane (distinct file to avoid colliding with `career.md`). Build `d873a1c8…`,
image base `0x140000000`, `.xtls` code. Companion to the `swofty.rnd-tyres-thermal` addon
(`/mnt/work/VSC/rnd-tyres-pitstops`). The data layer fully supports a 5th department
(code 4 = `Fake`/Tyres spare); this file records WHY each of the three R&D > Performance
render surfaces still shows nothing, and the exact lever per surface.

The whole R&D UI is keyed by the per-upgrade **ENUM A** (`upgrade_department`): `0=aero 1=chassis
2=durability 3=engine 4=Fake` (ci-fnv1a switch @0x14353a6dd). Value 4 (`Fake`) is the unused
spare the colour table `[4]`, the dept-code table entry 4, and `k_Career_UpgradeDeptNameTable[4]
="fake"` all line up on. The team `department`=Tyres / ENUM B=4 is a separate axis.

## Surface 1 — the department box (overview selector) {#box}

- Builder: `Career_Build_RnDDeptBoxGrid` @0x141c85f70 (no E8 callers; VM-dispatched). Builds
  **4 dept cards** (`Career_Construct_RnDDeptCard` 0x141c7aac0; enums 0/1/3/2) + **2 panel cards**
  (`tab_rnd_upgrade_purchase_panel`, `tab_rnd_perks`) into a pointer vector at builder+0x280..+0x290,
  then an attach loop calls `FUN_14431d720(builder+0x2d8, card+0x268, 0, 0)` per card — it passes
  **NO position (0,0)**. Native assigns no rect; it only parents the card node into the container.
- The container (builder+0x2d8) is a **UITH/UITC tab container**. The authored card TEMPLATES
  (`laptop_r_and_d_tab_header` UITH `title_root`, `tab_rnd_*` panels) live in
  `screens/laptop_r_and_d_departments.ui`; a department card needs NO per-dept authored node
  (name = `lng_rnd_dept_*` literal, icon = a frame in the `rnd_dept_icons_small` atlas).
- **Live ground truth (addon box-layout probe @`site_Career_RnDDeptBoxLayoutProbe` 0x141c86d86,
  v0.3.0 5th-box inject on):** the container slot vector at builder+0x548..+0x550 (= container
  +0x270) and the contiguous 0x120-stride slot objects BOTH **grow to 7** and `slot[6]` binds the
  injected Tyres card node + a fresh inner node. So the attach DOES create a slot (this corrects
  the earlier "attach never writes +0x270" note). **The only missing piece is the on-screen cell
  RECT:** the grid lays out a FIXED authored set of 6 cells (4 dept + 2 panel); slot index 6 maps
  to no authored cell rect, so the card is parented+bound but has no place to draw → invisible.
- **Lever: authored-asset (scene), not native.** Add a 5th dept cell rect to the R&D > Performance
  screen scene's grid layout (the cell-position list that the UITC binds slot→rect by index/id;
  authored in the screen `.idf`/`.ui` scene, not in the per-dept card). The declarative UIDF/ERP
  patch pipeline is not built, so this is deferred; a runtime clone of an existing cell's layout
  node/rect into slot[6] using the captured pointers is possible but blind/crash-prone.

## Surface 2 — the upgrade tiles (the functional win) {#tiles}

The tiles are **fully data-bound**. In `screens/laptop_r_and_d_departments.ui`:
`tab_rnd_department_panel` has `row_spawner [F1TS] dataContext=@rows selection=@selected_entry`
(+ `magnet`/`magnet_right`/`magnet_top` for 2D placement); each spawned row is `rnd_category_row`
with `tile_spawner [F1TS] dataContext=@tiles` → `rnd_card_with_info` cards. So tiles = `@tiles`
within `@rows`, positioned by magnets + `grid_coordinates`.

**Data layer — PROVEN to include code 4 (both loaders):**
- `Career_Load_UpgradeRows` @0x143539bc0 — flat 0x80-byte `UpgradeRecord` vector.
- `Career_Load_UpgradeTreeNodes` @0x14353b030 — the TILE DATA SOURCE: reads `grid_coordinates.x/y`
  + every column, appends into the combined tree model at UpgradesComponent+0x68+0x28.

Both loop the SAME dept-code table `k_Career_UpgradeDeptCodeTable` (0x145735d60, codes 0..4, break
at 5) and **include code 4 (`fake`) with no exclusion**, opening `<team_link>.fake.upgrade` and
reading its rows by `_upgrade_instancecount_` (via `Gdb_Init_TableCursor`). Node links resolve via
`Gdb_Find_RowByLabel` (0x1445a2bd0) which scans rows `0..instancecount-1`, so rows at index >=
instancecount are invisible to link resolution — the addon's `_upgrade_instancecount_` bump 1->4
on the 46 `*.fake` hubs puts `upgrade_1/2/3` in range, and the rows carry `grid_coordinates` +
`links` + `upgrade_department=Fake` (ENUM A=4). So the Tyres nodes ARE loaded into the model.
Hypotheses (a) model-doesn't-load and (b) loader-filters-out-code-4 are **disproven statically**.

**Display layer — the exclusion:** `@rows`/`@tiles` are built per the ACTIVE department. The
selected-department index `DAT_145f93c08` is **clamped `<4`** everywhere it is read
(`Career_Build_RnDDeptBoxGrid` @0x141c85f70 `if(3 < DAT_145f93c08)`; the pie populate highlight
clamp; the 4-entry `k_Career_RnDDeptNameTable` 0x145755e58 that index 4 would overrun into
`colour[0]` → char\* → crash). The department tab set is the same hardcoded 4. So the active
department can never be code 4, the per-department `@rows`/`@tiles` filter never admits ENUM A==4,
and the loaded Tyres nodes are never spawned → no tiles. This is hypothesis (c), refined: the
source isn't missing, the DISPLAY cannot select/filter department 4.

- **Lever:** display/selection, NOT data. Extend `k_Career_RnDDeptNameTable` to a 5th entry (so
  index 4 is a real cstr, not `colour[0]`), relax the `<4`/`3<` selection clamps to `<5`/`4<`, and
  ensure the active-tab → `@rows`/`@tiles` filter keys on ENUM A and admits 4. Coupled to the
  Surface-1 box selection work. The tree data is already correct and reachable.
- **Diagnostic (v0.4.2, `diag_tree_rowcount`, default on, read-only):** hook
  `site_Career_RnDTreeRowCountProbe` @0x14353b1af in `Career_Load_UpgradeTreeNodes` logs per dept
  code the loader's `row_count` (= tiles the data source yields). For code 4: `row_count=4` (hub +
  our 3) confirms the data layer and pins the blocker to display; `row_count=1` would mean the
  instancecount bump is not honoured at runtime (a data problem). Read in `loader.log` as
  `[rnd-tyres] tree-rows code=4 FAKE/TYRES: ... row_count=N`.

## Surface 3 — the development-potential pie {#pie}

Live builder `Career_Build_RnDPieOverview` @0x141f46eb0 (+ populate `Career_Populate_RnDPieSlices`
@0x141f48030, class-B vtable 0x14575e300; NOT the dormant class-A 0x141f41d60). Allocs a 0x900
sector buffer (0x80 + 4*0x220), constructs **4** slots via FUN_141f45650, populates with `code<4`
guards. **The 4 wedges are pre-authored 90° sector VM nodes** resolved by name-hash under
`rnd_wheel_icon`; colours are an INLINE switch (the pie does NOT read the colour table); the jump
table @0x141f48231 has 4 entries, default case = `error_department`/white.

- **Lever (deferred, authored art):** the addon's `render_fifth_pie` already grows the buffer
  0x900->0xb20, constructs a 5th slot, and relaxes the `code<4` guard to `<5` so code 4 populates
  slot[4]. But a wedge draws only with (1) a 5th authored sector node under `rnd_wheel_icon` and a
  5-wedge pie geometry (authored, not computed), and (2) a jump-table `case 4`
  (name `lng_rnd_dept_tyres`, colour 0xff0078ff) — extending the 4-entry rel32 table @0x141f48231
  needs a ±2GB code cave. So the 5th slice is populated but no wedge draws yet.
