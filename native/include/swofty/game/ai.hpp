#pragma once
#include <swofty/game/base.hpp>

namespace swofty::game {

struct AiDriverConfig {
    uint8_t _pad_0x0[0x90];
    void **blocking_fraction_of_follow_speed; // L2 experimental
    void **blocking_fraction_of_follow_speed_at_race_start; // L2 experimental
    void **blocking_max_blocking_distance; // L2 experimental
    void **blocking_max_blocking_distance_at_race_start; // L2 experimental
};
static_assert(offsetof(AiDriverConfig, blocking_fraction_of_follow_speed) == 0x90);
static_assert(offsetof(AiDriverConfig, blocking_fraction_of_follow_speed_at_race_start) == 0x98);
static_assert(offsetof(AiDriverConfig, blocking_max_blocking_distance) == 0xa0);
static_assert(offsetof(AiDriverConfig, blocking_max_blocking_distance_at_race_start) == 0xa8);

struct AiCarRecord {
    uint8_t vehicle_index; // L2 experimental
    uint8_t _pad_0x1[0x22f];
    void *handling_config; // L2 experimental
    AiDriverConfig driver_config; // L2 experimental
};
static_assert(offsetof(AiCarRecord, vehicle_index) == 0x0);
static_assert(offsetof(AiCarRecord, handling_config) == 0x230);
static_assert(offsetof(AiCarRecord, driver_config) == 0x238);

struct AiSystem {
    uint8_t _pad_0x0[0xa8];
    AiCarRecord *records_begin; // L2 experimental
};
static_assert(offsetof(AiSystem, records_begin) == 0xa8);

inline constexpr address<"Ai_BehaviourTestRetire_Update"> Ai_BehaviourTestRetire_Update{}; // L2 experimental
inline constexpr address<"Ai_Calc_GripLimitScaleA"> Ai_Calc_GripLimitScaleA{}; // L2 experimental
inline constexpr address<"Ai_Calc_GripLimitScaleB"> Ai_Calc_GripLimitScaleB{}; // L2 experimental
inline constexpr address<"Ai_DriverVariationInRaceSystem_Setup"> Ai_DriverVariationInRaceSystem_Setup{}; // L2 experimental
inline constexpr address<"Ai_Job_UpdateCarsParallel"> Ai_Job_UpdateCarsParallel{}; // L2 experimental
inline constexpr address<"Ai_Select_ErsDeployMode"> Ai_Select_ErsDeployMode{}; // L2 experimental
inline constexpr address<"Ai_Update_CompoundPaceCurves"> Ai_Update_CompoundPaceCurves{}; // L2 experimental
inline constexpr site<"site_Ai_BuildSystem_RecordPushed"> site_Ai_BuildSystem_RecordPushed{}; // L2 experimental
inline constexpr site<"site_Ai_TableGripInput"> site_Ai_TableGripInput{}; // L2 experimental
inline constexpr site<"site_Ai_UpdateCars_CornerLineLoad"> site_Ai_UpdateCars_CornerLineLoad{}; // L2 experimental
inline constexpr site<"site_Ai_UpdateCars_NoCornerLine"> site_Ai_UpdateCars_NoCornerLine{}; // L2 experimental

}
