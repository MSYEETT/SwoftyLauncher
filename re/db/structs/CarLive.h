// struct: CarLive
// subsystem: car
// size: 0x9f60  L2 decomp:re/decomp/144628190.c;xref:0x144628508 (alloc(0x9f60, 0x10) through the allocator at [phys+0x48] vtable +0x18)
// notes: the live simulation car, cxx class Car (Game_Place_Vehicle 0x14473d810 takes it as its first argument,
// locks car+0x98 and logs 'Car::teleportRequest ... -> %p' with it). Allocated and initialised in Car_Chassis_Build (0x144628190, slot 7 of
// the chassis-runtime component vtable 0x145bf73a0; that component is CarPhysicsRecord, it keeps the car at
// +0x58). Same object as the old CarWheelState, now removed (CarPhysicsRecord+0x58; its +0xe8 byte is this vehicle_id, not a
// tyre-set index). Registered in g_Sess_VehicleTable[vehicle_id] (0x146ce86e0, id = g_Sess_VehicleCount++
// under the critical section). No vtable at +0 (qword 0). The embedded component container at +0x380
// (vptr 0x145c040c8, owner = this) holds up to 32 component (system) pointers; Car_Build_VtfComponents
// (0x14473c2e0) caches 23 of them by type name into +0x530..+0x5e0 after every component create.
// Each system keeps the owning CarLive at system+0x8 (CarComponentBase.car); CarDynamics (+0x900) keeps it
// at +0x18. The upgrade loader at +0x4a0 {allocator, vehicle_asset, systems, allocator, applied upgrades}
// is the argument of Car_Upgrades_ApplyToVehicle (R&D/setup upgrade blocks) and Car_Systems_Reset_Thunk.
struct CarLive {
    /* 0x0010 */ uint8_t teleport_request[0x58];     // L1 decomp:re/decomp/14473d810.c pending teleport/placement request written by Game_Place_Vehicle under the lock (+0x10..+0x48 transform rows, +0x60 from mtx+0x10); layout not mapped
    /* 0x0098 */ void *teleport_lock;               // L2 decomp:re/decomp/144628190.c;decomp:re/decomp/14473d810.c;xref:0x14462854d,0x14473d850 points at the CRITICAL_SECTION at +0xa0 (InitializeCriticalSection in Car_Chassis_Build); Game_Place_Vehicle enters it
    /* 0x00d0 */ void *allocator;                    // L2 decomp:re/decomp/1447484f0.c;xref:0x1447484fd,0x144748521 allocator every Car_Create_<X>System allocates its system from (vtable +0x18)
    /* 0x00e0 */ void *asset_state;                  // L2 decomp:re/decomp/144742790.c;xref:0x1447427cd,0x144742aba [+0x1040] byte = 1 when the sections came from the equal-performance base asset ('Car is using %s.')
    /* 0x00e8 */ uint8_t vehicle_id;                 // L2 decomp:re/decomp/144628190.c;xref:0x144629637,0x14462963e index into g_Sess_VehicleTable (= g_Sess_VehicleCount at create); key of the mgr+0x148 wheel map
    /* 0x0108 */ CarObfFloat dyn_scalar;             // L1 decomp:re/decomp/1446dc480.c;xref:0x1446dcc86,0x1446dcc8c written every tick by Car_Dynamics_Update ((f12*f14*[car+0xf0] + term) ^ [car+0x110]) unless +0x138 is set; meaning open
    /* 0x0120 */ uint32_t sim_mode_a;               // L2 decomp:re/decomp/144641610.c;xref:0x1446416e3 Car_Physics_PreAdvance tests == 1 (with +0x9f10 == 3) when sorting cars; meaning open
    /* 0x0124 */ uint32_t is_ai;                    // L2 decomp:re/decomp/1446dc480.c;decomp:re/decomp/14463a800.c;xref:0x1446dc4f3,0x1446dc551 dword, written only when the value < 2; == 1 selects the AI settings g_Car_PhysicsRateHzAi (Vehicle.FineUpdateRateAI) and g_Car_TyreContactSubStepCycleAi (3) in Car_Dynamics_Update
    /* 0x012c */ uint32_t equal_performance;         // L2 decomp:re/decomp/144742790.c;xref:0x144742870 only the zero-init write was found; non-zero = equal-performance mode: Car_Assets_CreateSystemForPerformanceMode takes each section from the base VTF asset instead of the team VTF
    /* 0x0130 */ uint32_t flags;                     // L2 decomp:re/decomp/144628190.c;decomp:re/decomp/14465fdc0.c;decomp:re/decomp/14473fd80.c;xref:0x144628580,0x1446296f6,0x14465fdee general flags dword, not only damage: init 0x0ffffff0 then masked at create (0x1446296f6..0x14462970b: &0x7f, +0x131 &0x30, +0x132 = 0, +0x133 &0xf1); bits 0/1 are recomputed in Car_Physics_AdvanceCar from +0x9ea8^+0x9eb0. Bit 0 = front wing collision damage (Car_FrontWing_OnCollision / Car_SuspensionDamage_OnCollision gate), bit 1 = front wing damage written to the aero loss F1Aero+0x199c, bit 3 = suspension damage wheel flags, bit 6 = gate of Car_Engine_StepInput, Car_Ers_StepWear and Car_FuelTank_StepBurn, bit 7 = PU (ES/MGU-K) wear in Car_Ers_StepWear; +0x132 bit 2 = fuel burn and fuel run-out throttle clamp, +0x132 bit 7 = tyre wear (Car_Tyre_UpdateWear), +0x133 bit 1 = suspension damage wheel flags (renamed from damage_flags, car verification round 2026-10-28)
    /* 0x0380 */ void *systems_vtable;               // L2 decomp:re/decomp/144628190.c;xref:0x14462877a component container vptr 0x145c040c8 (container = this+0x380)
    /* 0x0388 */ void *systems_allocator;            // L2 decomp:re/decomp/144628190.c;xref:0x144628781,0x1446297c9 allocator ([phys+0x48]); per-phase callback arrays are allocated from it (vtable +0x18)
    /* 0x0390 */ CarLive *systems_owner;             // L2 decomp:re/decomp/144628190.c;xref:0x144628788,0x1446296bc container owner = this car (container+0x10)
    /* 0x0398 */ uint32_t system_count;              // L2 decomp:re/decomp/14473c2e0.c;xref:0x1446287a5,0x14473c2ec number of live components in systems[]
    /* 0x03a0 */ void *systems[32];                  // L2 decomp:re/decomp/14473c2e0.c;xref:0x144629940 component pointers (container+0x20; zero-filled to +0x4a0 at create, so capacity 32); appended by Car_Systems_FindOrCreate 0x144759eb0; vtable +0x38 -> type object whose vtable +8 returns the VTF section name
    /* 0x04a0 */ void *upgrades_allocator;            // L2 decomp:re/decomp/144628190.c;xref:0x14462878f start of the upgrade loader sub-object (CarLive+0x4a0): Car_Upgrades_ApplyToVehicle and Car_Systems_Reset_Thunk take this address
    /* 0x04a8 */ CarVtfAsset *vehicle_asset;          // L2 decomp:re/decomp/144628190.c;xref:0x1446296ae [[phys+0x9e8]+0x28]: the vehicle asset passed to Car_Assets_CreateSystemForPerformanceMode (sections list at +0x30..+0x38 stride 0x40)
    /* 0x04b0 */ void *systems_ptr;                  // L2 decomp:re/decomp/144628190.c;xref:0x1446296b5 = this+0x380
    /* 0x04b8 */ void *applied_upgrades_allocator;    // L2 decomp:re/decomp/144628190.c;xref:0x14462879e allocator of the applied-upgrade vector
    /* 0x04c0 */ void *applied_upgrades_begin;        // L2 decomp:re/decomp/1555bdf80.c;xref:0x144628837,0x1555bdf9b applied VTF upgrade blocks (stride 0x28, name at +0x10) checked by Car_Upgrades_ApplyToVehicle; Car_Systems_Reset_Body destroys them (vt[0](e,0)) and empties the vector before re-reading the VTF
    /* 0x04c8 */ void *applied_upgrades_end;          // L2 decomp:re/decomp/1555bdf80.c;xref:0x1555bdf9f,0x1555bdfc7 end
    /* 0x04d8 */ uint32_t phase_callback_total;      // L2 decomp:re/decomp/144628190.c;xref:0x1446297b0,0x1446297ba total per-phase update callbacks collected from systems[] vtable +0x28 over 7 phases
    /* 0x04dc */ uint32_t phase_callback_count[7];   // L2 decomp:re/decomp/144628190.c;xref:0x1446297a9,0x1446297f8 callbacks per phase 0..6
    /* 0x04f8 */ void *phase_callbacks[7];           // L2 decomp:re/decomp/144628190.c;xref:0x144629803,0x14462981c per phase: array of 16-byte {fn, ctx} records
    /* 0x0530 */ void *engine;                       // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473c36e system "Engine"
    /* 0x0538 */ void *gearbox;                      // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473c3ee system "GearBox"
    /* 0x0540 */ void *fuel_tank;                    // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473c46e system "FuelTank"
    /* 0x0548 */ void *fuel_ecu;                     // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473c4ee system "FuelECU"
    /* 0x0550 */ void *brake_ecu;                    // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473c56e system "BrakeECU"
    /* 0x0558 */ void *chassis;                      // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473c5ee system "Chassis"
    /* 0x0560 */ void *steering;                     // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473c66e system "Steering"
    /* 0x0568 */ void *core_aero;                    // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473c6ee system "CoreAero"
    /* 0x0570 */ void *f1_aero;                      // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473c77e system "F1Aero"
    /* 0x0578 */ void *differential;                 // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473c8f2 system "Differential"
    /* 0x0580 */ void *suspension;                   // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473c96d system "SuspensionSystem" (wheels at +0x70 RL, +0x78 RR, +0x80 FL, +0x88 FR)
    /* 0x0588 */ void *debug_system;                 // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473c9e8 system "DebugSystem"
    /* 0x0590 */ void *f1_tuning;                    // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473ca63 system "F1Tuning"
    /* 0x0598 */ void *f1_assists;                   // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473cade system "F1Assists"
    /* 0x05a0 */ void *turbo;                        // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473cb59 system "Turbo"
    /* 0x05a8 */ void *ers;                          // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473cbd4 system "EnergyRecoverySystem"
    /* 0x05b0 */ void *convex_hull;                  // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473cc4f system "ConvexHull"
    /* 0x05b8 */ void *wheel_proxies;                // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473ccca system "WheelProxies"
    /* 0x05c0 */ void *f1_plank;                     // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473cd45 system "F1Plank"
    /* 0x05c8 */ void *f1_wing_damage;               // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473cdc0 system "F1WingDamage"
    /* 0x05d0 */ void *f1_suspension_damage;         // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473ce3b system "F1SuspensionDamage"
    /* 0x05d8 */ void *f1_tyre_damage;               // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473ceb6 system "F1TyreDamage"
    /* 0x05e0 */ void *f1_body_damage;               // L2 decomp:re/decomp/14473c2e0.c;xref:0x14473c883 system "F1BodyDamage"
    /* 0x08f8 */ void *dynamics_allocator;          // L2 decomp:re/decomp/144628190.c;xref:0x144629619 allocator of the CarDynamics object
    /* 0x0900 */ void *dynamics;                    // L2 decomp:re/decomp/144628190.c;decomp:re/decomp/14473fd80.c;xref:0x144629600,0x144740a85 CarDynamics (0xa80, vt_Car_Dynamics 0x145bff4d0, +0x18 = this): Car_Physics_AdvanceCar calls vt+8 once and vt+0x10/+0x18/+0x20 per substep (Vehicle.FineUpdateRate, default 360 Hz)
    /* 0x9f40 */ uint32_t handling_version;          // L2 decomp:re/decomp/144628190.c;xref:0x1446296cc,0x1446dcf20 copied from vehicle_asset+0x18; Car_Physics_StepVehicle runs the wheel/suspension pre-steps in a different order when < 7
    /* 0x9f48 */ void *vehicle_asset_data;           // L2 decomp:re/decomp/144628190.c;xref:0x1446296de copied from vehicle_asset+0x20
};
