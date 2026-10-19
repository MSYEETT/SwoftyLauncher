# car: per-car performance / setup / physics model

Owner: see `subsystems.tsv` (car-re). Scope: the vehicle object + its performance attribute array, the
R&D apply path that writes performance into a car, car setup, tyre/ERS/fuel/engine/damage surfaces, and
how performance feeds the simulation. Steam build base 0x140000000, sha256 d873a1c8….

## Map: R&D performance apply -> the car attribute array {#apply}
`Car_Fit_UpgradesToRating` (0x143514670, cxx `game::upgrades::EngineSuppliers::fitUpgradesUpToRating(const Upgrades&, const DepartmentData&, DriverStatId)`) is the anchor. Fully data-driven, no fixed-4:
1. Walks the Upgrades robin_hood map (`[upgrades+0x38]` ctrl / `[+0x40]` slots, gate `[upgrades+0x48]`). Each slot is an upgrade GROUP (gate `[group+0x190]`); inner robin_hood map at `[group+0x180]/[+0x188]` yields upgrade ITEMs.
2. For each item it collects, from `DepartmentData.effect_defs` (inline `void*[0x180]` at dept+0, count at `dept+0xc00`), the `UpgradeEffectDef`s whose `dept_group_enum` (+0x7c) == the group byte `[group+0]` and `item_group_enum` (+0x72) == the item byte `[item+0]`. Grouping is `==`, never index-into-array[4].
3. Rating lerp: sums `UpgradeEffectDef.effect_value` (+0x64) over the matched set; ratio drives `min=[item+0x10]`, `max=[item+0x14]`, cap `[item+0x18]` (the upgrade/bound object's rating bounds). A sub-pass splits by the byte at `[effdef.variant_tag_ptr-8]` (0/3/6). Logs on channel "UPGRADES/V" (hash 0x6a15fc5f).
4. Write-back: owner = `*(upgrades+8)` (team/engine-supplier). For each car pointer in owner `[+0x298 begin .. +0x2a0 end]` (stride 8): if `*(veh+0x1960)!=0`, iterate `count=*(veh+0x1960)` attribute elements at base `veh+0xd10`, stride **0xc40**. Each element holds a fitted-effectdef pointer list (list at element+0x000 cap 0x180, count at element+0xc00); `Car_Find_Qword`(thunk 0x1449e2850, a SIMD std::find-over-qwords into .data1 body 0x1562c20a0) searches it for the current effect-def. `Car_Apply_UpgradeToVehicle` (0x1435173e0) then applies: reads the attribute via `Car_Read_VehicleAttribute` (0x14351df60) and stores results into `UpgradeEffectDef+0xa4/+0xa8`; `Car_Reset_LastFittedUpgrade` (0x143517d80, `resetLastFittedUpgrade`) runs per fitted attribute.
   Hook sites: `site_Car_AttrArrayIter` (0x143514c50, r12=CarPerfVehicle, r14=effect-def) and `site_Car_AttrArrayBase` (0x143514c64).
`Car_Aggregate_UpgradeStats` (0x1409e3fb0; decompiled at entry 0x1409e6000) is a second consumer that re-reads `upgrade_type` + `bhp_adjustment`/`mass_adjustment` by name — string-driven, not a dept[4] index.
No static callers of the apply path (indirect/encrypted dispatch); role proven by the in-binary pretty-function string (`s_Upgrades_FitUpgradesUpToRating`) + decompiled data flow.

## Attribute unit-type vocabulary
`k_Car_AttrUnitTypeNames` (0x14564a168) is the ordered unit-type name blob: EnginePower, FuelConsumption,
ChassisWeightRedistribution, ChassisWeightReduction, FrontDownforce, RearDownforce,
ControlElectronicsDurability, EnergyStoreDurability, GearboxDurability, InternalCombustionEngineDurability,
MotorGeneratorUnitHeatDurability, MotorGeneratorUnitKineticDurability, TurboChargerDurability (Drag, Brakes,
TyreWear, DRS, GeneralMaintenance strings nearby). This is the `affect_on_vehicle_unit_type` vocabulary. The
R&D DB model (gamedb) exposes these via `upgrade_type_metadata` with `bhp_adjustment_fully_upgraded`,
`mass_adjustment_fully_upgraded`, `performance_rating_base/fully_upgraded`. `g_Car_AttrDef_EnginePower`
(0x145650168) is EnginePower's AttributeDefinition dispatch table.
NEEDS RUNTIME TRACE (L3): the exact attribute-NAME -> attribute-array-ELEMENT-index mapping and attr_count.
The name->small-int resolution is the FNV unit-type switch near 0x14353a700 / 0x1417f8311 (per scratchpad);
reading `attr_elem`'s header fields and which element each unit type lands in needs a live session trace.

## Live physics / telemetry vehicle (DIFFERENT object) {#live}
From `Sess_Veh_UpdateWheelMap` (0x142e90560, session lane): the vehicle SYSTEM/manager walks its live
vehicle list `[mgr+0x110 begin .. +0x118 end]` (stride 8). Per live vehicle `veh`:
- `[veh+0x20]` = physics/sim-record child; `[[veh+0x20]+0x58]` = wheel/tyre-state object; `[that+0xe8]` = u8 tyre-set/wheel index (0xff = none).
- wheel container `[[[veh+0x20]+0x58]+0x580]` holds 4 wheel pointers at +0x70/+0x78/+0x80/+0x88; each wheel's scalar is `[wheel+0x88]` (float). UpdateWheelMap averages the four (×0.25) into a robin_hood map<u8,float> at `mgr+0x148` (keyvals `mgr+0x160`, key u8 +0, value float +4) — the 26th entry triggers the 32->64 rehash B26h heap abort.
- big live-vehicle telemetry region at `veh+0x9c70..+0x9ca8` with a 0x78-stride record vector `[veh+0x9c90 .. +0x9c98]`.
This is the entry point for live per-car telemetry (speed/rpm/gear/tyre temp+wear/fuel). The SwoftyCarState
publish in native/src/core/telemetry.cpp needs these offsets; the per-wheel float semantics (temperature vs
wear vs load) need a runtime trace to confirm. Session lane owns this function; inbox note left there.

## Vehicle package row {#vehicle-package}
`Game_Read_VehiclePackageRow` (0x142504a90, game lane) parses `entrants.vehicles.vehicle_N` into
`VehiclePackageRow`: stat_id (+0, init 0xff), strings (+0x08 resource_path, +0x18 specialisation, +0x28
blob_shadow_decal, +0x38/+0x50 asset groups / ai_package_folder / driver_animation_set), and performance-
relevant flags +0x60 has_cockpit_halo, +0x61 is_story_vehicle_package, +0x62 is_equal_performance_vehicle_package,
+0x63 is_26_aero_regulations. Caller is plaintext .data1 code at 0x14cca1269.

## Setup / suspension / physics rig (existing game-lane L1, performance-relevant)
- `Game_F1CarSetupDataManagerBase_DeserialiseBindings` 0x140c0c0c0 — car setup (aero/suspension/gears/brakes) bindings deserialise.
- `Game_Read_VtfSuspensionSystem` 0x1446aa090, `Game_Read_VtfFloat` 0x14464e760 — VTF physics parameter readers (Chassis COMY/COMZ, Tyre/TyreRadius, Suspension Camber/RideHeight/LocalRestPosition).
- `Game_Build_SuspensionRig` 0x14462d090, `Game_Find_SuspensionNodes` 0x14475c9a0, `Game_Find_WheelNodes` 0x14462bc90, `Game_Create_CarPartProxies` 0x144635360 / `Game_Create_CarPartProxy` 0x144636150.
- `Game_Setup_FrontendCarFromVtf` 0x1428fe140 — cosmetic (visual height/camber on a render node), not physics.
DB keys: car_setup, SetupData, GearRatio, differential, brake_bias strings exist (gamedb) — setup is data-driven.

## Tyre / ERS / fuel / engine / damage surfaces (catalog; mostly existing game-lane L1)
- Tyre: `Game_TyrePerformanceRuleManager_*` (0x1415291c0 OnSpectatorChanged, 0x14152a9f0 OnSectorCrossed, 0x14152ad40 OnAWSPanelOpened, 0x14152adf0 OnPitted, 0x14152aec0 OnTerminalDamage); `Game_TyreAllocationComponent_Ctor` 0x140d055b0 / `_SendNetMessage` 0x140bd1ac0; `Game_HudTyrePerformanceViewModel_SetToOpen` 0x14152b5f0. Strings TyreModel/TyreTemperature/tyre_temperature/tyre_wear present.
- ERS/fuel/engine: strings FuelMix/fuel_mix, ers_mode, EngineMode, PowerUnit, EnergyStore*, EngineWear present (VM/config keys); no dedicated function pinned yet — candidates for a decomp+trace pass.
- Damage/reliability: strings DamageComponent, vehicle_damage, reliability present; `Game_VehicleResetToGarageComponent_Trigger` 0x142f33d80. Not yet decompiled.

## Open / needs runtime verification
- attr-name -> attr-array-element index map and attr_count (L3 trace at site_Car_AttrArrayIter).
- live-vehicle per-wheel float semantics (temp/wear/load) and the +0x9c90 telemetry record layout.
- tyre wear/temperature, ERS deployment/energy-store, fuel-mode, engine-mode, damage/reliability writer functions (decomp from the component ctors + the live physics child at [veh+0x20]+0x58).

## Inbox
- 2026-10-19 13:33 career-re: career-re: 0x1409e3fb0 (Car_Aggregate_UpgradeStats) full field reads for the effect-def/attribute loader: available, attributes, default_effects; per-attribute default_value/min_value/max_value/use_clamp_min/use_clamp_max/restriction/statId/network_serialisable; upgrade_fitter_adjustment/upgrade_fitter_ratio/replay_serialise_mods; per effect-type upgrade_type + bhp_adjustment_fully_upgraded + mass_adjustment_fully_upgraded. Reached via vtable slot 0x145665c20. These build the effect-def objects (byte +0x7c, effect float +0x64) your Car_Fit_UpgradesToRating consumes.
- 2026-10-19 13:33 career-re: career-re: R&D load/economy side is in the new career lane. Car_Fit_UpgradesToRating consumes RnDDepartmentConfig (structs/RnDDepartmentConfig.h, filled by Career_Load_RnDTweakables 0x14353cea0 at [EngineSuppliers+0x68]) for the rating bounds/targets, and UpgradeRecord (structs/UpgradeRecord.h, built by Career_Load_UpgradeRows 0x143539bc0). The DepartmentData effect-def count [dept+0xc00] and per-vehicle attr array (+0xd10 stride 0xc40 count +0x1960, writer thunk 0x1449e2850) are yours.
- 2026-10-19 14:01 entrants-re: the per-vehicle AI-config object built by Sess_Load_VehicleAiConfig (0x1425027e0) carries an identity byte block at +0xcd8 package_stat_id, +0xcd9 engine_supplier_stat_id, +0xcda performance_class, +0xcdb performance_tier (structs/VehicleIdentity.h, entrants lane). engine-supplier = vehicle 'engine' label -> entrants.engines row -> stat_id; vehicle_package = 'vehicle_package' label -> entrants.vehicles.vehicle_packages row -> stat_id. This object likely overlaps your vehicle/CarPerfVehicle object - worth cross-checking the base.
- 2026-10-19 23:41 rnd-kb: rnd-kb: Car_Reset_LastFittedUpgrade 0x143517d80 is also the per-INSTANCE upgrade reset on the career side: E8 callers instantiate @0x14352d071 (Career_Instantiate_UpgradeTrees), reset-all @0x14352e223 (Career_ResetAll_Upgrades), supplier refit @0x143515290. It branches on the department byte at site_Career_UpgradeInitFakeDeptBranch 0x143517e0c (cmp byte [rcx+0x72],4 ; jne): dept 4 gets one disabled sentinel state entry. Consider a rename once the cxx string attribution is re-checked. Separately the rnd-tyres work found Car_Read_VehicleAttribute 0x14351df60 called as the R&D development-cost wrapper (research twin 0x14351dde0, both around cost 0x143525b60); its current label may be wrong. Details: rnd_ui.md#instances, #modifiers.
