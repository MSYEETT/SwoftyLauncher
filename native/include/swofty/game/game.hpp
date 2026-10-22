#pragma once
#include <swofty/game/base.hpp>

namespace swofty::game {

struct ChampionshipHolder {
    uint8_t _pad_0x0[0x260];
    CareerEntity *entity; // L2 experimental
};
static_assert(offsetof(ChampionshipHolder, entity) == 0x260);

struct GameManager {
    uint8_t _pad_0x0[0xc0];
    GameModeContext *mode_context; // L2 experimental
};
static_assert(offsetof(GameManager, mode_context) == 0xc0);

struct GameMessageBusHolder {
    uint8_t _pad_0x0[0x8];
    NeMessageBus *bus; // L2 experimental
};
static_assert(offsetof(GameMessageBusHolder, bus) == 0x8);

struct GameModeContext {
    uint8_t _pad_0x0[0x88];
    ChampionshipHolder *championship_holder; // L2 experimental
};
static_assert(offsetof(GameModeContext, championship_holder) == 0x88);

using Game_CareerWizard_PushNextStep_t = void(void *wizard);
inline constexpr fn<"Game_CareerWizard_PushNextStep", Game_CareerWizard_PushNextStep_t> Game_CareerWizard_PushNextStep{}; // L2 experimental
using Game_CareerWizard_StartsInF1_t = bool(void *entrants, void *wizard_data, const char *entrant_list);
inline constexpr fn<"Game_CareerWizard_StartsInF1", Game_CareerWizard_StartsInF1_t> Game_CareerWizard_StartsInF1{}; // L2 experimental
using Game_Career_IsDriverCareerMode_t = bool(const void *championship);
inline constexpr fn<"Game_Career_IsDriverCareerMode", Game_Career_IsDriverCareerMode_t> Game_Career_IsDriverCareerMode{}; // L2 experimental
using Game_DriverSelectHeader_SetTabState_t = void(void *header, uint64_t tab, uint8_t content_visible, uint8_t enabled);
inline constexpr fn<"Game_DriverSelectHeader_SetTabState", Game_DriverSelectHeader_SetTabState_t> Game_DriverSelectHeader_SetTabState{}; // L2 experimental
using Game_EngineMgmt_AddGridPenalty_t = void(void *engine_management, const uint32_t *driver, uint32_t places, uint32_t reason);
inline constexpr fn<"Game_EngineMgmt_AddGridPenalty", Game_EngineMgmt_AddGridPenalty_t> Game_EngineMgmt_AddGridPenalty{}; // L2 experimental
using Game_EngineMgmt_ApplySessionWear_t = void(void *engine_management, uint32_t driver, float amount, uint8_t flag);
inline constexpr fn<"Game_EngineMgmt_ApplySessionWear", Game_EngineMgmt_ApplySessionWear_t> Game_EngineMgmt_ApplySessionWear{}; // L2 experimental
using Game_EntrantsUi_BuildCharacterRef_t = void *(void *out, const uint8_t *character, void *node);
inline constexpr fn<"Game_EntrantsUi_BuildCharacterRef", Game_EntrantsUi_BuildCharacterRef_t> Game_EntrantsUi_BuildCharacterRef{}; // L2 experimental
using Game_EntrantsUi_OnCharacterListLoaded_t = int64_t(void *ctx, const void *message);
inline constexpr fn<"Game_EntrantsUi_OnCharacterListLoaded", Game_EntrantsUi_OnCharacterListLoaded_t> Game_EntrantsUi_OnCharacterListLoaded{}; // L2 experimental
using Game_Rnd_CalcUpgradeTypeProgress_t = float(void *const *instances, uint8_t type, uint8_t state, const void *meta);
inline constexpr fn<"Game_Rnd_CalcUpgradeTypeProgress", Game_Rnd_CalcUpgradeTypeProgress_t> Game_Rnd_CalcUpgradeTypeProgress{}; // L2 experimental
using Game_Rnd_FillVehicleUpgrades_t = void(void *component, uint8_t car, uint8_t a3, uint8_t a4);
inline constexpr fn<"Game_Rnd_FillVehicleUpgrades", Game_Rnd_FillVehicleUpgrades_t> Game_Rnd_FillVehicleUpgrades{}; // L2 experimental
using Game_SeasonLength_ApplyPreset_t = void(void *component, uint8_t preset);
inline constexpr fn<"Game_SeasonLength_ApplyPreset", Game_SeasonLength_ApplyPreset_t> Game_SeasonLength_ApplyPreset{}; // L2 experimental
using Game_SeasonLength_PushCalendarEntry_t = uint64_t(void *calendar, const void *entry);
inline constexpr fn<"Game_SeasonLength_PushCalendarEntry", Game_SeasonLength_PushCalendarEntry_t> Game_SeasonLength_PushCalendarEntry{}; // L2 experimental
using Game_SpeechSystem_PostLine_t = void(void *speech, void *line, uint8_t retrigger);
inline constexpr fn<"Game_SpeechSystem_PostLine", Game_SpeechSystem_PostLine_t> Game_SpeechSystem_PostLine{}; // L2 experimental
inline constexpr data<"g_Game_FlowLinkStay", uint32_t> g_Game_FlowLinkStay{}; // L2 experimental
inline constexpr data<"g_Game_Manager", void *> g_Game_Manager{}; // L2 experimental
inline constexpr data<"g_Game_MessageBus", void *> g_Game_MessageBus{}; // L2 experimental
inline constexpr data<"g_Game_Root", void *> g_Game_Root{}; // L2 experimental
inline constexpr data<"k_Game_TrackListComponentDesc", void *> k_Game_TrackListComponentDesc{}; // L2 experimental, TrackListComponent
inline constexpr site<"site_Game_CareerWizardConfirm_NextRoute"> site_Game_CareerWizardConfirm_NextRoute{}; // L2 experimental
inline constexpr site<"site_Game_CareerWizard_BackRoute"> site_Game_CareerWizard_BackRoute{}; // L2 experimental
inline constexpr call<"site_Game_DriverSelectHeader_BuildTabs"> site_Game_DriverSelectHeader_BuildTabs{}; // L2 experimental
inline constexpr site<"site_Game_DriverSelect_BackRoute"> site_Game_DriverSelect_BackRoute{}; // L2 experimental
inline constexpr vtable<"vt_Game_CareerWizardSettingsSelectViewModel", 19> vt_Game_CareerWizardSettingsSelectViewModel{}; // L2 experimental, CareerWizardSettingsSelectViewModel.
inline constexpr vtable<"vt_Game_DriverCareerSlotPredicate", 6> vt_Game_DriverCareerSlotPredicate{}; // L2 experimental
inline constexpr vtable<"vt_Game_TrackListComponent", 79> vt_Game_TrackListComponent{}; // L2 experimental, TrackListComponent

}
