# career: My Team / R&D (upgrades, departments, resource points, economy, season flow)

Owner: see `subsystems.tsv` (prefix `Career`). Game-DB model + launcher feasibility:
`/scratchpad/rnd/01_db_model_findings.md`, `02_native_findings.md`,
`/scratchpad/rnd-dept/01_mechanism_findings.md`, `02_feasibility_and_hook_plan.md`.

Scope: the career / My-Team subsystem a modder would hook — R&D upgrade trees (load / fit /
AI spend), the department enums + economy (resource points, cost/time/fail-rate scales,
facilities), the career component object model, and the telemetry/message surface for R&D events.
All addresses are VAs, image base `0x140000000`, build `d873a1c8…` (Steam). Code is `.xtls`;
GVDB string *values* are XOR-masked (unmasked inline with the 4-byte mask at `0x145890978`,
`g_Gdb_StringXorMask` in the gamedb lane — see `k_Career_*` notes).

## The R&D pipeline is almost entirely data-driven {#pipeline}

Every stage reads the GVDB by field NAME through `Gdb_Get_FieldInt` (gamedb `0x1445a2cf0`, L2),
so new `upgrade_*` / `UpgradeDepartmentEntry_*` rows are ingested with no code change. The only
hard-coded surfaces are (a) the two department string→int enums and (b) the ~21-entry
`upgrade_type` / unit-type vocabularies. Full feasibility in the scratchpad notes.

**Correction (2026-10-19):** that holds for loading, not for a new *department*. Upgrade instances of
department 4 get a disabled sentinel, five state mutators refuse department > 3, the availability
search treats department 4 as the hub, team modifier entries exist only for departments 0..3, and
every R&D screen fixes the count at 4. Map: [`rnd_ui.md`](rnd_ui.md#fixed-four).

| stage | function | addr | level |
|---|---|---|---|
| load the per-dept upgrade rows | `Career_Load_UpgradeRows` | 0x143539bc0 | L2 |
| load the team→supplier link path | `Career_Read_TeamLinks` | 0x1435396e0 | L2 |
| load AI "tweakables" (pick %, refunds, delays) | `Career_Load_RnDTweakables` | 0x14353cea0 | L2 |
| load department economy entries | `Career_Load_DepartmentEntries` | 0x1412326e0 | L2 |
| load resource-point rate config | `Career_Load_ResourceRateConfig` | 0x14122ada0 | L2 |
| load upgrade effect/attribute defs (CAR lane) | `Car_Aggregate_UpgradeStats` | 0x1409e3fb0 | L1 |
| fit upgrades to a rating / apply to car (CAR lane) | `Car_Fit_UpgradesToRating` | 0x143514670 | L2 |
| register career/MyTeam components | `Career_Register_Components` | 0x1412cc2f0 | L1 |
| parse team setup row (holds dept-enum store) | `Career_Parse_TeamSetupRow` | 0x14276fda0 | L1 |
| upgrade-card VM binder | `Career_Bind_UpgradeCard` | 0x1417f8090 | L1 |
| R&D telemetry/message subscriber | `Career_Report_RnDTelemetry` | 0x1406d8470 | L1 |

## Load: per-dept upgrade rows {#load-upgrade-rows}

`Career_Load_UpgradeRows` (0x143539bc0, cxx likely `game::upgrades::…`): outer loop over a 6-byte
department-code table `k_Career_UpgradeDeptCodeTable` (0x145735d60, codes 0..5, breaks at 5);
per code it picks a dept-name string from `k_Career_UpgradeDeptNameTable` (0x14584f880, index*8),
formats `<team>.…<dept>` and opens the `"upgrade"` table (`GStack._256 = "upgrade"`). For each row
it reads by name: `upgrade_name`, `upgrade_type`, `upgrade_size`, `upgrade_department`, `cost`,
`cost_to_develop`, `weighting`(float), `fitting_times.num_days_to_research`,
`fitting_times.num_days_to_develop`, `unlock_at_facility_level` (default 1),
`supplier_delivery_date_index` (default 1). It then builds a 0x80-byte `UpgradeRecord` into the
output vector (begin `[out+0x08]`, end `[out+0x10]`, cap `[out+0x18]`, grows ×1.5 via the
allocator vtable slot +0x18, frees via +0x38).

Three enums are resolved inline here:
- `upgrade_type` → 0..18 via 19 string compares against `k_Career_UpgradeTypeNameTable`
  (0x14584f6b8, indexed by `UNK_14566fd16`); unknown → 0.
- `upgrade_size` → 0 Minor / 1 Major / 2 Ultimate (3 compares `UNK_14565cf95/cf9b/cfa1`).
- `upgrade_department` **enum A** → a ci-fnv1a-64 switch at 0x14353a6dd (inlined ~27× across the
  exe, so NOT uniquely hookable): default 0; Chassis 1 (0x184f58717a1a6a3f); Durability 2
  (0x1e339d9dc3c05766); Engine 3 (0x3dacd07fc0cad3db); Fake 4 (0xef08697916801cf4); Invalid 5
  (0x2be5705c42a81bd0). Hash = ci-fnv1a of the lowercased field value; unknown → 0, no crash.
  This is a DIFFERENT enum from the team `department` enum B below.

`UpgradeRecord` field map is in `structs/UpgradeRecord.h`. Note the enum bytes land at +0x70/+0x71/
+0x72 (type/size/deptA), cost at +0x5c, weighting (AI pick weight) at +0x64.

## Load: team→supplier link path {#team-links}

`Career_Read_TeamLinks` (0x1435396e0): builds `teams_linked_to`, reads the `links` list field and
copies a path/handle block into the output object (`out[0] = vtable 0x145613538`). Eight callers
(0x14122ff36, 0x1434f6620, 0x1434fae20, 0x143539291, 0x14353abde, 0x14353bb96, 0x14353c54a,
0x14353cec2) — it is the shared prefix helper the other R&D loaders call to resolve a team's
engine-supplier tree container before reading `upgrade`/`tweakables`.

## Load: AI tweakables {#tweakables}

`Career_Load_RnDTweakables` (0x14353cea0): calls `Career_Read_TeamLinks`, opens the `…tweakables`
table and fills the config object `[arg0+0x68]` (`RnDDepartmentConfig`): the AI recommended-upgrade
chances (`recommended_upgrade.percentage_chance_of_picking_{vehicle,durability}_upgrade`,
`.{min,max}_percentage_of_upgrades_that_can_be_adapted`), the refund percentages
(`cancelling_{research,development,adaptation}.refund_percentage`), the development-delay thresholds/
days arrays and `development_delay.worst_case_cash_penalty`, and `target_mass_adjustment_at_max_upgrade`
+ `difficulty_mass_adjustment_scalar(_enabled)`. Vtable slot at 0x1458477f8; E8 callers 0x140603984,
0x141246056. A new row's weighting + these chances drive the AI pick automatically (prior pass §3).

## Load: department economy entries {#dept-entries}

`Career_Load_DepartmentEntries` (0x1412326e0): opens the `…DepartmentEntry` container, loops
`row_count` (DB instance count — NOT a hard 4) and per row reads the full economy vocabulary:
`department`, `attribute`, `upgradeTierAttribute`, `morale_effect_message`, `research_delay_chance`,
`research_delay_scale`, `research_cost_scale`, `research_time_scale`, `development_cost_scale`,
`development_time_scale`, `development_fail_rate`, `development_speed_penalty_for_repair`,
`development_failure_time_percentage_name`, `adapting_cost_scale`, `max_simultaneous_research`,
`max_simultaneous_development`, `dual_development_cost_scale`, `dual_development_available`,
`rush_available`, `rush_fail_rate`. Each entry is stored into the pointer array at `[arg0+0xc60]`
(stride 0x10, pointer at +8). Reached via vtable slot 0x1456cc930. Adding an `UpgradeDepartmentEntry_N`
row + bumped instancecount mints a new department's economy with no code.
**Correction (2026-10-19):** the row's `department` is parsed with ENUM A (switch @0x14123412a)
into 19 maps keyed by that byte (`component+0x738 + N*0x48`), so a token that is not an ENUM A name
(e.g. `Tyres`) resolves to 0 and overwrites Aero's attributes; `Fake` gives key 4. Even then the
per-team modifier lists (research slots, rush, cost/time scales) hold entries for departments 0..3
only, so department 4 has 0 research slots without native help
([`rnd_ui.md`](rnd_ui.md#modifiers)).

## Load: upgrade effect definitions {#effect-defs}

The attribute + effect-definition loader is `Car_Aggregate_UpgradeStats` (0x1409e3fb0, **car lane**,
owner car-re). For reference it reads `available`, `attributes`, `default_effects`, per-attribute
`default_value`/`min_value`/`max_value`/`use_clamp_min`/`use_clamp_max`/`restriction`/`statId`/
`network_serialisable`, `upgrade_fitter_adjustment`/`upgrade_fitter_ratio`/`replay_serialise_mods`,
and per effect-type `upgrade_type` + `bhp_adjustment_fully_upgraded` + `mass_adjustment_fully_upgraded`.
These are the effect-definition objects (`[effdef+0x7c]`, effect float `[+0x64]`) that
`Car_Fit_UpgradesToRating` groups and sums (full field list left in the car inbox).

## Load: resource-point rate config {#resource-rate}

`Career_Load_ResourceRateConfig` (0x14122ada0): reads `resource_points.session_types` + `session`,
walks the static table 0x1456cc3e0..0x1456cc4d0 (ends at the `AiResourceRateModifiers` string) and
pushes 0x30-byte rate-modifier records (begin/end/cap at `[arg1+0x08/0x10/0x18]`; default rate
`1.0` at +0x28; per-type template from `UNK_1456d0ab0`). E8 callers 0x1412282d9, 0x14122835f,
0x1412283dc. This is the per-session-type resource-point gain-rate config (practice programmes etc.);
`AiResourceRateModifiers` vs `PlayerResourceRateModifiers` split the AI/player rates (prior pass).

## Fit / apply {#fit}

The apply/fit function is `Car_Fit_UpgradesToRating` (0x143514670, **car lane**, owner car-re, L2),
cxx `game::upgrades::EngineSuppliers::fitUpgradesUpToRating`, pretty-fn `0x145846f3a`:
`(const Upgrades&, const DepartmentData&, DriverStatId)`. It consumes the R&D data this lane loads, so
the mechanism is recorded here for cross-reference. FULLY DYNAMIC:
- robin_hood/absl flat-hash iteration of the Upgrades container (control bytes, tzcnt, 0x40 groups)
  at `[arg0+0x38/0x40]`, then `[obj+0x180/0x188]`, then the `DepartmentData` effect-def array `arg1`.
- groups by BYTE equality — `byte[effdef+0x7c] == byte[group+0]` and `byte[group+0x72] == byte[upg+0]`
  — never by array index; candidate count = `[arg1+0xc00]` (dynamic).
- rating lerp: sums the effect float `[obj+0x64]` over two groups → ratio; bounds `[b+0x10]` (min),
  `[b+0x14]` (max), `[b+0x18]` (target); `rating = min + (max-min)*ratio`; converges ≤0x200 iters.
- write-back per vehicle: vector `[[arg0+8]+0x298 .. +0x2a0]`; per vehicle, attribute count
  `[veh+0x1960]`, base `[veh+0xd10]`, stride 0xc40, delta written via the thunk 0x1449e2850 (→ .data1
  body 0x1562c20a0). The attribute array + write thunk are the `car` lane (see car inbox).
- log channel "UPGRADES/V": "No upgrades to fit for %s", "Done in %s for %s, achieved/exceeded rating",
  "Calling %s for %s".
=> a new upgrade reusing an existing effect type/dept byte aggregates onto the car with no mixin.

## Department enum B (the TEAM R&D departments) — the key hook {#team-dept-enum}

The top-level team R&D departments are a SEPARATE enum resolved at a SINGLE inlined site inside
`Career_Parse_TeamSetupRow` (0x14276fda0, which also parses helmet/avatar/mods/team). ci-fnv1a-64 of
the `department` value → signed binary search → stored `mov byte [rdi+0x340], cl`:
default 0; Powertrain 1 (0xeacc53611edf399c); Chassis 2 (0x184f58717a1a6a3f); Aero 3
(0xd0fae783d5a9fb7c, token "Aero" not "Aerodynamics"); Tyres 4 (0xac9965efa31b61d0). Pivot const
0xeacc53611edf399b. The Aero/Tyres/Powertrain hash immediates each occur EXACTLY ONCE in the exe →
this resolver is the only place the team-dept enum is minted.

`site_Career_TeamDeptEnumStore` (0x142774258, the `mov [rdi+0x340], cl`) is the hook for new team
departments: at the store `rax` still holds the ci-fnv1a hash of the value and `cl` the base-game
resolution (0 for unknown tokens), so a SITE mixin can set `cl` to 5/6 for new tokens. Signature
`career.team_dept_enum_store` (unique, count-verified = 1). This is the single required code hook to
add R&D departments; the rest (UpgradeDepartmentEntry rows, upgrade rows, effect/dept wiring,
`DepartmentData` bounds, lng strings) is pure data — see `02_feasibility_and_hook_plan.md`.

**Correction (2026-10-19):** the store did not fire during a career R&D session, and no R&D screen
reads ENUM B. The R&D UI, upgrade instances and modifiers are keyed by the per-upgrade ENUM A, and
department 4 (`Fake`) is refused in several native paths, so adding an R&D department is not pure
data. See [`rnd_ui.md`](rnd_ui.md#fixed-four).

## Career / My-Team component object model {#components}

`Career_Register_Components` (0x1412cc2f0) registers the ECS-style career components by name+hash,
including the R&D ones: `ResourcePointsComponent`, `UpgradesComponent`, `RnDChallengesComponent`,
`UpgradesRegulationChangesComponent`, `EngineSuppliersComponent`, `EngineManagementComponent`,
`MyTeamAttributesComponent`, plus `GridSetupComponent`, `LocationComponent`, `RivalManagerComponent`,
`QuickPracticeComponent`, `RandomGarageFaultsComponent`, `OnlineServicesCoordinatorComponent`, etc.
This is where the career object graph is wired; a modder reaching R&D state goes through
`ResourcePointsComponent` (RP economy) and `UpgradesComponent` (owned/fitted upgrades).

## R&D event / telemetry surface {#telemetry}

`Career_Report_RnDTelemetry` (0x1406d8470, ElasticSearchReporter) subscribes to the R&D messages a
mod would hook for purchase/spend events: `upgrade_research_purchased_message`,
`upgrade_development_purchased_message`, `upgrade_{research,development,adaptation}_cancelled_message`,
`upgrades_update_failed_rnd_message`, `upgrade_adapted_message`, `UpgradeFittingChangedMessage`,
`MessageESMyTeamFacilityUpgradePurchased`, `MessageFacilityUpgradeCompleteESMessage`,
`MessageRnDEndofWeekendResourcePointTotal`, `DriverPerkPurchased`. The actual RP `-= cost`
instruction still needs a runtime trace to pin (see "needs trace" below).

## Needs runtime verification (no L3 here yet) {#needs-trace}

- `Car_Fit_UpgradesToRating` (0x143514670, car lane): no static caller (vtable/ptr dispatched) —
  a trace or vtable-slot id would confirm the dispatch and the car-attribute write offsets.
- The exact resource-point spend/award instruction (the `-= cost` on `ResourcePointsComponent`):
  not pinned statically; the purchase path is data-driven but the write site needs a trace of
  `upgrade_research_purchased_message`.
- Whether the team-dept enum value 5/6 overflows any fixed-capacity array keyed by `[obj+0x340]`
  (feasibility Unknown #1) — a short in-game spike.
- `game.session_enter` already exists (session lane, `Game_GameModule_OnSessionEnter`); the
  career/season-start flow hooks (new season, race-weekend enter) should be built around it +
  `Career_Parse_TeamSetupRow`.

## Inbox
- 2026-10-19 13:56 ai-re: ai lane created. Your Career_Load_RnDTweakables (recommended-upgrade AI pick %) + Career_Load_ResourceRateConfig (AiResourceRateModifiers) + RnDDepartmentConfig.difficulty_mass_adjustment_scalar are cross-referenced in subsystems/ai.md#xrefs as the AI R&D/economy surface (kept in your lane). The on-track race AI (overtaking/defending/ERS strategy/mistakes via ai_vehicle_config.*) is now in the ai lane.
- 2026-10-19 14:01 entrants-re: re-homed Career_Parse_TeamSetupRow 0x14276fda0 -> entrants lane as Ent_Parse_TeamSetupRow (L2) per your handoff. The dept-enum store site_Career_TeamDeptEnumStore 0x142774258 and sig career.team_dept_enum_store STAY in the career lane (unchanged); career.md#team-dept-enum still valid. Only the function name/subsystem changed.
- 2026-10-19 15:13 rnd-tyres: swofty.rnd-tyres-thermal 5th-department count lever: the UpgradeDepartmentEntry row count is NOT a literal 4 and NOT built from nothing - it is the DB key <container>._UpgradeDepartmentEntry_instancecount_ read by Gdb_Init_TableCursor (gamedb), cached at cursor+0x200, used as the loop bound [rsp+0xa4] in Career_Load_DepartmentEntries. Real containers {5e21a18f9f59dbf6}/{1f304d36c3c2b947} (rows _0..3). Native count-bump site: site_Gdb_DeptCountStore / sig gamedb.dept_count_store, gated on cursor row word == UpgradeDepartmentEntry and count==4 -> 5. The addon's rows must live at UpgradeDepartmentEntry_4 in those two containers (the loop reads 0..count-1). NB: department value Tyres maps to upgrade_department ENUM A = 0 (default, = Aero's economy slot) in this loader's inline switch - economy collision risk, needs runtime test.
- 2026-10-19 15:42 rnd-tyres: rnd-tyres: R&D department UI is HARDCODED to 4, NOT data-driven. The R&D>Performance dept box grid is built by Career_Build_RnDDeptBoxGrid (0x141c85f70): 4 literal Career_Construct_RnDDeptCard (0x141c7aac0) calls with compile-time (lang_key,icon,enum) = aero/chassis/engine/maintenance; card vector sized to exactly 6 ptrs then truncated; selection clamp DAT_145f93c08<4 vs k_Career_RnDDeptNameTable (4 entries). The DEVELOPMENT POTENTIAL pie + overview is Career_Build_RnDPotentialPie (0x141f41d60): loops k_Career_UpgradeDeptCodeTable but gated if(code<4)/if(4<i)break with inline per-case colours; pie sectors are pre-authored VM nodes (Career_Build_RnDPieSector, init_pie_sector/highlight_pie_sector). Card colour = k_Career_RnDDeptColourTable (0x145755e78, 6 entries, [4][5]=white spare). => The UpgradeDepartmentEntry loader-count bump to 5 feeds only the economy data model ([self+0xc60]); it CANNOT add a 5th UI box. A genuine 5th box+pie-slice needs a multi-site native UI rework (patch both builders: grow the 6-card vector + inject a 5th card in 0x141c85f70; relax code<4/i<=4 guards + add a colour case in 0x141f41d60; supply lng_rnd_dept_<code>/_overview_description, a 5th rnd_dept_icons_small frame, a colour at table slot [4], and a 5th pie-sector UIDF node). NOT a small data/asset lever.
- 2026-10-19 16:10 rnd-tyres: UI 5th-dept rework scoped from the 4 decompiles (box grid 0x141c85f70, card ctor 0x141c7aac0, pie 0x141f41d60, sector handler 0x141f45d30). CONFIRMED both builders are hardcoded-4 with exactly-sized, per-slot-init buffers: (box) the card vector resizes to EXACTLY 6 ptrs (0x30) and all 6 are used - 4 dept cards at begin[0..3] (enums 0/1/3/2) + 2 non-dept panels at begin[4] (0x6f0 obj) and begin[5] (0xad0 obj); each dept card binds a per-dept UIDF node via the `%s.%s.%s` path suffix (UNK_14570ce2d/DAT_14570ce32/DAT_1456708a3/UNK_1457556f1), so a 5th card needs the vector grown to 7 + a 5th 0x950 card + a 5th authored UIDF box node. (pie) the per-sector array is alloc'd at EXACTLY 0x900 (= 0x80 + 4*0x220) at 0x141f41d60 prologue and 4 slots are init'd via FUN_141f45650 at buf+0x80/+0x2a0/+0x4c0/+0x6e0; the loop guard is `if(code<4)` + `if(4<i)break`, stride 0x220, and each slot holds several refcounted VM-node handles (buf+0x80/+0x128/+0x148/+0x168). Relaxing `<4` blind overruns the 0x900 buffer / writes an uninit slot => crash. The name table k_Career_RnDDeptNameTable (0x145755e58, 4 entries) is immediately followed by the colour table k_Career_RnDDeptColourTable (0x145755e78), so name[4] overlaps colour[0] - relaxing the box selection clamp `<4` for a saved index 4 reads colour[0] as a char* => crash unless a real 5th name source is provided. Addon v0.2.0 ships only the SAFE groundwork: a native RVA write of colour table slot [4] (0x145755e88, stock white spare, read via the card ctor's `<6` bound) -> Tyres orange 0xff0078ff, gated on a 6-entry colour-table layout probe (ffeead00/ff3ec58d/ff16deff/ff7a2aed/ffffffff/ffffffff), plus one-shot entry probes on both builders. Game base confirmed 0x140000000 (no ASLR) so these RVAs are exact. Blockers for the live 5th box/slice: the launcher's declarative UIDF/ERP patch pipeline (operations.ui + patch-pkg) and core lang.set are NOT built yet (build.rs skips them), so the 5th UIDF box/sector node + the lng_rnd_dept_tyres name cannot be authored through the addon - deferred to a follow-up (full patch spec in the addon's native/HOOKS.md).
- 2026-10-19 16:58 rnd-tyres: rnd-tyres v0.3.0: shipped the LIVE 5th R&D department box (Tyres & Thermal) by native injection into Career_Build_RnDDeptBoxGrid (0x141c85f70) at site_Career_RnDDeptBoxGridInject (0x141c86567, sig career.rnd_box_grid_inject). CORRECTION to the earlier claim that a dept card binds a per-dept UIDF node via the %s.%s.%s path: it does NOT. Career_Construct_RnDDeptCard (0x141c7aac0) args are (card,ctx,builder,allocator,NAME_cstr,ICON_cstr,enum): the 5th arg is the name lang key (lng_rnd_dept_aero etc, 0x1457407d6), the 6th arg is the ICON ATLAS path formatted %s.%s.%s = <pkg>.rnd_dept_icons_small.<frame> (frame=aero/chassis/engine/maintenance = the atlas sprite, NOT a view-model node). So a dept card needs NO authored UIDF node and NO launcher UIDF/ERP pipeline - it renders through the same card VM node (card+0x268) the stock four use. The hook grows the 6-ptr card vector to 7 and builds a real Tyres card (name lng_rnd_dept_tyres, borrowed aero icon frame, enum 4 -> colour slot [4]) into begin[6]; the builder's own attach loop binds it; selection clamp DAT_145f93c08<4 left intact. Whether the box container lays the 5th card out visibly is the one open in-game-test question. ALSO CORRECTION to career.md L185 + the 0x141f41d60 note: the LIVE Performance-page pie is Career_Build_RnDPieOverview (0x141f46eb0) + helper Career_Populate_RnDPieSlices (0x141f48030) on class-B vtable 0x14575e300 (built by 0x141c43290, same 0x141c4xxxx screen as the box grid); 0x141f41d60 is class A (research-overview screen) and does NOT fire on this page. Full 5th-slice patch plan against the live builder is in the two new function notes + the addon native/HOOKS.md.
- 2026-10-19 19:22 ui-format: UI format reference now at subsystems/ui_system.md (new 'ui' lane). It cross-references the three R&D surfaces as the canonical 'native-built, not data-driven' case: box grid 0x141c85f70 (card vector vs fixed .idf layout rects, section 5/7), pie 0x141f46eb0/0x141f41d60, upgrade-tile list-spawner. The 5th-box 'no on-screen rect' limit is explained as the unparsed .idf scene-layout gap (ui_system.md#idf). Your lane still owns the R&D fix specs; ui_system.md is format-only.
- 2026-10-19 19:55 ui-idf: ui-format: .idf scene format SOLVED (ui_system.md#idf) + decoder tools/idf.py. R&D 5th-box lever: the on-screen box rects live in the screen's .idf eScene graph as per-element position(vec3)+scale+extents; a widget/card binds to an element by nodeId=djb2(dotted-path)=elementId; componentId=md5-64-BE(path sans .idf)=scene.name. Box grid 0x141c85f70 populates VM node view_model_data.laptop_r_and_d_departments_container and its attach loop appends the injected 5th card into that screen's layout container. Whether the 5th shows depends on that container's class: if eStacker (stock F1 card rows ARE eStackers, e.g. overview_panel.idf::card_stacker gap0.2 stackerType l with pre-authored card_0..4) it auto-lays-out N children -> 5th appears free; if fixed-anchor, add a 5th sibling element via idf.py clone-slot (new name, fresh guid, elementId=djb2(new path), append to children + nodeidlookup). idf.py round-trips byte-identical (4243/4243 stock scenes) so edits are write-back-safe. Decode this screen's container with: python3 tools/idf.py tree <scene.idf> to read which class it is.
- 2026-10-19 19:59 ui-idf: ui-format: PIE 5th-slice answer (coordinator Q). The R&D development-potential pie IS an authored .idf scene: rnd_dev_potential.idf has 4 eXRef sector nodes sector_1..4 under null_1, each -> shared rnd_dev_potential_sector.idf, ROTATED 0/90/180/270deg via the rotation quat (4x 90deg arcs tile the circle). BUT the wedge arc SHAPE is baked mesh geometry: GfxMeshRes rnd_dev_potential_sector.idf/comp_potential?context=render&lod=0&mesh (+ vb/ib buffers), referenced by the layout node's geom resourceid; the .idf stores position/scale/rotation/extents, NOT the arc angle. => Adding a 5th slice = (a) scene edit idf.py CAN do: add sector_5 eXRef + re-set the 5 rotations to 72deg steps (0/72/144/216/288); (b) SEPARATE ART TASK .idf does not cover: a new 72deg wedge mesh (stock 90deg mesh at 72deg spacing overlaps 18deg/wedge); (c) native Career_Populate_RnDPieSlices guard emit 5th slice (your lane). So pie node = same .idf lever as the box, but with an extra baked-mesh dependency the box (generic reusable eXRef card template) does not have. Full writeup ui_system.md#idf 5.6.
- 2026-10-19 23:41 rnd-kb: rnd-kb: recorded the rnd-tyres-thermal engine findings (36 functions, 31 globals/sites, all L1, disassembly + E8 callers re-checked on d873a1c8) in rnd_ui.md: department page Career_Build_RnDDeptPage 0x141c7b880 + instance query Career_Query_EnabledUpgrades 0x1435188d0 (#page); the Upgrades model/instance layout, the dept-4 init sentinel @0x143517e0c and the five cmp byte [x+0x72],3 mutator gates (#instances); team modifier entries created only for depts 0..3 (#modifiers); the Back route dead end (open-by-name ignores unknown names, #back); the pie object ctor 0x14180c490 (exactly 4 slots), slot dtor 0x141f45650 (was mislabelled a ctor in the 0x141f46eb0/0x141f48030 notes, corrected) and the dead moved-from block after 0x141f46fb8 (#pie-object); tile counts 0x141f49bc0 (#tile-counts); department icon/colour tables (#tags). Corrections added under #pipeline, #dept-entries and #team-dept-enum: adding an R&D department is not pure data.
- 2026-10-21 17:11 std-ui: core feat/std-career-ui (native/src/core/career_ui.hpp) reads the loaded career's data group like the save-slot predicate 0x1438fb390: CareerEntity +0x1040..+0x1048 championships (16-byte entries, championship +8), Championship +0x208 component list get (vtable +0x10) with k_Career_CareerSetupComponentDesc, CareerSetupComponent vtable +0x268 data-group index, *(component +0x290) +0x560..+0x568 group list (16-byte entries, group +8), name *(group +0x28) +0x10. Checked against a fresh decomp of 0x1438fb390 by std-ui-verify (matches; the predicate also has a gate 0x1484fad70 and a type check before the walk). Not struct rows yet: the career lane owner should raise Championship.components (+0x208), a CareerSetupComponent struct (+0x290, slot +0x268) and the group list layout.
