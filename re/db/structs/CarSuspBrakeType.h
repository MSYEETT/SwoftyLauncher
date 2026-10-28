// struct: CarSuspBrakeType
// subsystem: car
// size: ?  L1 decomp:re/decomp/1446aa090.c
// notes: VTF SuspensionSystem/Brake[name] record (CarSuspension.brake_types[i]); plain floats. Density, Conductivity, rotor
// radii/thickness, SurfaceLayerThickness and CoolingHoleRadius go through locals into derived thermal values (not mapped).
struct CarSuspBrakeType {
    /* 0x0000 */ char name[16];                               // L2 decomp:re/decomp/1446aa090.c;kb:subsystems/car.md#suspension Brake name attribute
    /* 0x0010 */ uint8_t handbrake_support;                   // L2 decomp:re/decomp/1446aa090.c;kb:subsystems/car.md#suspension VTF SuspensionSystem/Brake/HandbrakeSupport
    /* 0x0014 */ float handbrake_brake_multiple;              // L2 decomp:re/decomp/1446aa090.c;kb:subsystems/car.md#suspension VTF SuspensionSystem/Brake/HandbrakeBrakeMultiple
    /* 0x0018 */ float handbrake_grip_multiple;               // L2 decomp:re/decomp/1446aa090.c;kb:subsystems/car.md#suspension VTF SuspensionSystem/Brake/HandbrakeGripMultiple
    /* 0x001c */ float specific_heat_capacity;                // L2 decomp:re/decomp/1446aa090.c;kb:subsystems/car.md#suspension VTF SuspensionSystem/Brake/SpecificHeatCapacity
    /* 0x0020 */ float coef_heat_transfer_constant;           // L2 decomp:re/decomp/1446aa090.c;kb:subsystems/car.md#suspension VTF SuspensionSystem/Brake/CoefHeatTransferConstant; addon radUpgradeBrakeCooling * 1.25
    /* 0x0024 */ float coef_heat_transfer_linear;             // L2 decomp:re/decomp/1446aa090.c;kb:subsystems/car.md#suspension VTF SuspensionSystem/Brake/CoefHeatTransferLinear; addon radUpgradeBrakeCooling * 1.25
    /* 0x0028 */ float coef_heat_transfer_quadratic;          // L2 decomp:re/decomp/1446aa090.c;kb:subsystems/car.md#suspension VTF SuspensionSystem/Brake/CoefHeatTransferQuadratic
    /* 0x00a0 */ CarSpline temp_efficiency_spline;            // L2 decomp:re/decomp/1446aa090.c;kb:subsystems/car.md#suspension VTF SuspensionSystem/Brake/TempEfficiencySpline
    /* 0x0348 */ float min_damage_brake_loss;                 // L2 decomp:re/decomp/1446aa090.c;kb:subsystems/car.md#suspension VTF SuspensionSystem/Brake/MinDamageBrakeLoss
    /* 0x034c */ float max_brake_loss_percent;                // L2 decomp:re/decomp/1446aa090.c;kb:subsystems/car.md#suspension VTF SuspensionSystem/Brake/MaxBrakeLossPercent
    /* 0x0350 */ float damage_accretion_rate;                 // L2 decomp:re/decomp/1446aa090.c;kb:subsystems/car.md#suspension VTF SuspensionSystem/Brake/DamageAccretionRate
};
