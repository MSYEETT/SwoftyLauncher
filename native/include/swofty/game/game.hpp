#pragma once
#include <swofty/game/base.hpp>

namespace swofty::game {

struct ChampionshipHolder {
    uint8_t _pad_0x0[0x260];
    CareerEntity *entity; // L2 experimental
};
static_assert(offsetof(ChampionshipHolder, entity) == 0x260);

struct DriverSecretMeetings {
    uint8_t _pad_0x0[0x10];
    void *current_allocator; // L2 experimental
    SecretMeeting *current; // L2 experimental
    uint8_t _pad_0x20[0x8];
    void *previous_begin; // L2 experimental
    void *previous_end; // L2 experimental
    uint8_t _pad_0x38[0x8];
    uint32_t driver; // L2 experimental
    uint8_t _pad_0x44[0x4];
};
static_assert(offsetof(DriverSecretMeetings, current_allocator) == 0x10);
static_assert(offsetof(DriverSecretMeetings, current) == 0x18);
static_assert(offsetof(DriverSecretMeetings, previous_begin) == 0x28);
static_assert(offsetof(DriverSecretMeetings, previous_end) == 0x30);
static_assert(offsetof(DriverSecretMeetings, driver) == 0x40);
static_assert(sizeof(DriverSecretMeetings) == 0x48);

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

struct SecretMeeting {
    uint32_t id; // L2 experimental
    uint8_t _pad_0x4[0x10];
    uint8_t reason; // L2 experimental
    uint8_t _pad_0x15[0xb];
    uint32_t team; // L2 experimental
};
static_assert(offsetof(SecretMeeting, id) == 0x0);
static_assert(offsetof(SecretMeeting, reason) == 0x14);
static_assert(offsetof(SecretMeeting, team) == 0x20);
static_assert(sizeof(SecretMeeting) == 0x24);

using Game_CareerSetup_ResetEntrants_t = void(void *setup);
inline constexpr fn<"Game_CareerSetup_ResetEntrants", Game_CareerSetup_ResetEntrants_t> Game_CareerSetup_ResetEntrants{}; // L2 experimental
using Game_CareerWizard_PushNextStep_t = void(void *wizard);
inline constexpr fn<"Game_CareerWizard_PushNextStep", Game_CareerWizard_PushNextStep_t> Game_CareerWizard_PushNextStep{}; // L2 experimental
using Game_CareerWizard_StartsInF1_t = bool(void *entrants, void *wizard_data, const char *entrant_list);
inline constexpr fn<"Game_CareerWizard_StartsInF1", Game_CareerWizard_StartsInF1_t> Game_CareerWizard_StartsInF1{}; // L2 experimental
using Game_Career_IsDriverCareerMode_t = bool(const void *championship);
inline constexpr fn<"Game_Career_IsDriverCareerMode", Game_Career_IsDriverCareerMode_t> Game_Career_IsDriverCareerMode{}; // L2 experimental
using Game_ContractTeamSelect_OnEnter_t = uint64_t(void *state);
inline constexpr fn<"Game_ContractTeamSelect_OnEnter", Game_ContractTeamSelect_OnEnter_t> Game_ContractTeamSelect_OnEnter{}; // L2 experimental
using Game_ContractTeamSelect_OnExit_t = uint64_t(void *state);
inline constexpr fn<"Game_ContractTeamSelect_OnExit", Game_ContractTeamSelect_OnExit_t> Game_ContractTeamSelect_OnExit{}; // L2 experimental
using Game_ContractTeamSelect_OnUpdate_t = uint32_t *(void *state, uint32_t *link);
inline constexpr fn<"Game_ContractTeamSelect_OnUpdate", Game_ContractTeamSelect_OnUpdate_t> Game_ContractTeamSelect_OnUpdate{}; // L2 experimental
using Game_DriverContracts_AdvanceSecretMeeting_t = void(void *contracts, uint32_t driver, uint8_t notify);
inline constexpr fn<"Game_DriverContracts_AdvanceSecretMeeting", Game_DriverContracts_AdvanceSecretMeeting_t> Game_DriverContracts_AdvanceSecretMeeting{}; // L2 experimental
using Game_DriverContracts_BuildOffers_t = void(void *contracts, uint32_t driver);
inline constexpr fn<"Game_DriverContracts_BuildOffers", Game_DriverContracts_BuildOffers_t> Game_DriverContracts_BuildOffers{}; // L2 experimental
using Game_DriverContracts_OfferSecretMeeting_t = void(void *contracts, const void *meeting, uint32_t team, uint32_t driver, uint8_t reason, uint8_t notify);
inline constexpr fn<"Game_DriverContracts_OfferSecretMeeting", Game_DriverContracts_OfferSecretMeeting_t> Game_DriverContracts_OfferSecretMeeting{}; // L2 experimental
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
using Game_F1WorldActivityContainer_SelectEntrantList_t = void(void *container, const char *entrant_list);
inline constexpr fn<"Game_F1WorldActivityContainer_SelectEntrantList", Game_F1WorldActivityContainer_SelectEntrantList_t> Game_F1WorldActivityContainer_SelectEntrantList{}; // L2 experimental
using Game_Refresh_PerformanceComparison_t = void(void *comparison);
inline constexpr fn<"Game_Refresh_PerformanceComparison", Game_Refresh_PerformanceComparison_t> Game_Refresh_PerformanceComparison{}; // L2 experimental
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
inline constexpr call<"site_Game_CareerContinue_DriverSlotTitle"> site_Game_CareerContinue_DriverSlotTitle{}; // L2 experimental
inline constexpr site<"site_Game_CareerWizardConfirm_NextRoute"> site_Game_CareerWizardConfirm_NextRoute{}; // L2 experimental
inline constexpr site<"site_Game_CareerWizard_BackRoute"> site_Game_CareerWizard_BackRoute{}; // L2 experimental
inline constexpr call<"site_Game_DriverSelectHeader_BuildTabs"> site_Game_DriverSelectHeader_BuildTabs{}; // L2 experimental
inline constexpr site<"site_Game_DriverSelect_BackRoute"> site_Game_DriverSelect_BackRoute{}; // L2 experimental
inline constexpr call<"site_Game_DriverSelect_BadgeFormat"> site_Game_DriverSelect_BadgeFormat{}; // L2 experimental
inline constexpr call<"site_Game_DriverSelect_TeamLogoFormat"> site_Game_DriverSelect_TeamLogoFormat{}; // L2 experimental
inline constexpr call<"site_Game_TeamSelect_TeamLogoFormat"> site_Game_TeamSelect_TeamLogoFormat{}; // L2 experimental
inline constexpr vtable<"vt_Game_CareerDLCSelectViewModel", 19> vt_Game_CareerDLCSelectViewModel{}; // L2 experimental
inline constexpr vtable<"vt_Game_CareerWizardSettingsSelectViewModel", 19> vt_Game_CareerWizardSettingsSelectViewModel{}; // L2 experimental, CareerWizardSettingsSelectViewModel.
inline constexpr vtable<"vt_Game_DriverCareerDriverSelectViewModel", 22> vt_Game_DriverCareerDriverSelectViewModel{}; // L2 experimental
inline constexpr vtable<"vt_Game_DriverCareerSlotPredicate", 6> vt_Game_DriverCareerSlotPredicate{}; // L2 experimental
inline constexpr vtable<"vt_Game_TrackListComponent", 79> vt_Game_TrackListComponent{}; // L2 experimental, TrackListComponent

}
