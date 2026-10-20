#pragma once
#include <swofty/game/base.hpp>

namespace swofty::game {

using Career_Add_TeamModifier_t = void(void *list, uint32_t key, float value);
inline constexpr fn<"Career_Add_TeamModifier", Career_Add_TeamModifier_t> Career_Add_TeamModifier{}; // L2 experimental
using Career_Calc_UpgradeCost_t = int32_t(void *list, void *inst, void *mods, uint32_t option);
inline constexpr fn<"Career_Calc_UpgradeCost", Career_Calc_UpgradeCost_t> Career_Calc_UpgradeCost{}; // L2 experimental
using Career_Construct_RnDDeptCard_t = void *(void *card, void *ctx, void *builder, void *allocator, const char *name, const char *icon, uint8_t dept);
inline constexpr fn<"Career_Construct_RnDDeptCard", Career_Construct_RnDDeptCard_t> Career_Construct_RnDDeptCard{}; // L2 experimental
using Career_Refresh_RnDDeptMode_t = void(uintptr_t *holder, uint32_t mode);
inline constexpr fn<"Career_Refresh_RnDDeptMode", Career_Refresh_RnDDeptMode_t> Career_Refresh_RnDDeptMode{}; // L2 experimental
using Career_Refresh_UpgradeAvailability_t = void(void *upgrades, uint16_t day);
inline constexpr fn<"Career_Refresh_UpgradeAvailability", Career_Refresh_UpgradeAvailability_t> Career_Refresh_UpgradeAvailability{}; // L2 experimental
inline constexpr data<"g_Career_RnDOverviewSelectedDept", uint8_t> g_Career_RnDOverviewSelectedDept{}; // L2 experimental
inline constexpr data<"k_Career_RnDCardColourTable", uint32_t[6]> k_Career_RnDCardColourTable{}; // L2 experimental
inline constexpr data<"k_Career_RnDDeptColourTable", uint32_t[6]> k_Career_RnDDeptColourTable{}; // L2 experimental
inline constexpr data<"k_Career_RnDErrorDeptName", char[17]> k_Career_RnDErrorDeptName{}; // L2 experimental
inline constexpr data<"k_Career_UpgradeBlockNameTable", int32_t[18]> k_Career_UpgradeBlockNameTable{}; // L2 experimental
inline constexpr data<"k_Career_UpgradeTypeOrder", uint8_t[19]> k_Career_UpgradeTypeOrder{}; // L2 experimental
inline constexpr site<"site_Career_ApplyUpgradeBlockNameDefault"> site_Career_ApplyUpgradeBlockNameDefault{}; // L2 experimental
inline constexpr site<"site_Career_ApplyUpgradeBlockNameJoin"> site_Career_ApplyUpgradeBlockNameJoin{}; // L2 experimental
inline constexpr site<"site_Career_ApplyUpgradeKeyCall"> site_Career_ApplyUpgradeKeyCall{}; // L2 experimental
inline constexpr site<"site_Career_ApplyUpgradeKeyFilter"> site_Career_ApplyUpgradeKeyFilter{}; // L2 experimental
inline constexpr site<"site_Career_DeserialiseUpgradesEnd"> site_Career_DeserialiseUpgradesEnd{}; // L2 experimental
inline constexpr site<"site_Career_LoadUpgradeRows_DeptPath"> site_Career_LoadUpgradeRows_DeptPath{}; // L2 experimental
inline constexpr site<"site_Career_RnDBoxGridFirstTab"> site_Career_RnDBoxGridFirstTab{}; // L2 experimental
inline constexpr site<"site_Career_RnDBoxGridNameClamp"> site_Career_RnDBoxGridNameClamp{}; // L2 experimental
inline constexpr site<"site_Career_RnDBoxGridNameIndex"> site_Career_RnDBoxGridNameIndex{}; // L2 experimental
inline constexpr site<"site_Career_RnDBoxGridNameRead"> site_Career_RnDBoxGridNameRead{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardArtImage0"> site_Career_RnDCardArtImage0{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardArtImage1"> site_Career_RnDCardArtImage1{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardArtImage2"> site_Career_RnDCardArtImage2{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardArtMask0"> site_Career_RnDCardArtMask0{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardArtMask1"> site_Career_RnDCardArtMask1{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardArtMask2"> site_Career_RnDCardArtMask2{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardColourPick0"> site_Career_RnDCardColourPick0{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardColourPick1"> site_Career_RnDCardColourPick1{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardColourPick2"> site_Career_RnDCardColourPick2{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardColourPick3"> site_Career_RnDCardColourPick3{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardColourPick4"> site_Career_RnDCardColourPick4{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardDeptIcon0"> site_Career_RnDCardDeptIcon0{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardDeptIcon1"> site_Career_RnDCardDeptIcon1{}; // L2 experimental
inline constexpr site<"site_Career_RnDCardDeptIcon2"> site_Career_RnDCardDeptIcon2{}; // L2 experimental
inline constexpr site<"site_Career_RnDCompareDept"> site_Career_RnDCompareDept{}; // L2 experimental
inline constexpr site<"site_Career_RnDContainerOpenMaint"> site_Career_RnDContainerOpenMaint{}; // L2 experimental
inline constexpr site<"site_Career_RnDContainerOpenTail"> site_Career_RnDContainerOpenTail{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptBoxGridEntry"> site_Career_RnDDeptBoxGridEntry{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptBoxGridInject"> site_Career_RnDDeptBoxGridInject{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptBoxLayoutProbe"> site_Career_RnDDeptBoxLayoutProbe{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptPagePreQuery"> site_Career_RnDDeptPagePreQuery{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptPageQueryDone"> site_Career_RnDDeptPageQueryDone{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptPageTypeLoop"> site_Career_RnDDeptPageTypeLoop{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptRowCategoryIcon"> site_Career_RnDDeptRowCategoryIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptRowTypeIcon"> site_Career_RnDDeptRowTypeIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptRowTypeIconJoin"> site_Career_RnDDeptRowTypeIconJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptRowTypeLabel"> site_Career_RnDDeptRowTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_RnDDeptRowTypeLabelJoin"> site_Career_RnDDeptRowTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDInProgressColourStore"> site_Career_RnDInProgressColourStore{}; // L2 experimental
inline constexpr site<"site_Career_RnDInProgressPartIcon"> site_Career_RnDInProgressPartIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDInProgressStrip"> site_Career_RnDInProgressStrip{}; // L2 experimental
inline constexpr site<"site_Career_RnDInProgressTypeIcon"> site_Career_RnDInProgressTypeIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDInProgressTypeIconJoin"> site_Career_RnDInProgressTypeIconJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDInProgressTypeLabel"> site_Career_RnDInProgressTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_RnDInProgressTypeLabelJoin"> site_Career_RnDInProgressTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDListRowColourStore"> site_Career_RnDListRowColourStore{}; // L2 experimental
inline constexpr site<"site_Career_RnDListRowDeptIcon"> site_Career_RnDListRowDeptIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDListRowLightColourStore"> site_Career_RnDListRowLightColourStore{}; // L2 experimental
inline constexpr site<"site_Career_RnDPageModifierList"> site_Career_RnDPageModifierList{}; // L2 experimental
inline constexpr site<"site_Career_RnDPanelModifierList"> site_Career_RnDPanelModifierList{}; // L2 experimental
inline constexpr site<"site_Career_RnDPanelResearchLimit"> site_Career_RnDPanelResearchLimit{}; // L2 experimental
inline constexpr site<"site_Career_RnDPerfChildName"> site_Career_RnDPerfChildName{}; // L2 experimental
inline constexpr site<"site_Career_RnDPerfDeptColour"> site_Career_RnDPerfDeptColour{}; // L2 experimental
inline constexpr site<"site_Career_RnDPerfDeptLoopEnd"> site_Career_RnDPerfDeptLoopEnd{}; // L2 experimental
inline constexpr site<"site_Career_RnDPerfDeptName"> site_Career_RnDPerfDeptName{}; // L2 experimental
inline constexpr site<"site_Career_RnDPerfGotoName"> site_Career_RnDPerfGotoName{}; // L2 experimental
inline constexpr call<"site_Career_RnDPerfViewModelSlotDtor"> site_Career_RnDPerfViewModelSlotDtor{}; // L2 experimental
inline constexpr site<"site_Career_RnDPerksTitleDeptIcon"> site_Career_RnDPerksTitleDeptIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDPieBufSize"> site_Career_RnDPieBufSize{}; // L2 experimental
inline constexpr site<"site_Career_RnDPieCtorRet"> site_Career_RnDPieCtorRet{}; // L2 experimental
inline constexpr site<"site_Career_RnDPieFocusJoin"> site_Career_RnDPieFocusJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDPieFocusNameCmp"> site_Career_RnDPieFocusNameCmp{}; // L2 experimental
inline constexpr site<"site_Career_RnDPieFocusRestore"> site_Career_RnDPieFocusRestore{}; // L2 experimental
inline constexpr call<"site_Career_RnDPieMoveAssignSlotDtor"> site_Career_RnDPieMoveAssignSlotDtor{}; // L2 experimental
inline constexpr call<"site_Career_RnDPieMovedFromSlotDtor"> site_Career_RnDPieMovedFromSlotDtor{}; // L2 experimental
inline constexpr site<"site_Career_RnDPiePopulateDeptGuard"> site_Career_RnDPiePopulateDeptGuard{}; // L2 experimental
inline constexpr site<"site_Career_RnDPiePopulateGate"> site_Career_RnDPiePopulateGate{}; // L2 experimental
inline constexpr site<"site_Career_RnDPiePopulateNextCode"> site_Career_RnDPiePopulateNextCode{}; // L2 experimental
inline constexpr site<"site_Career_RnDPiePopulateSlotCap"> site_Career_RnDPiePopulateSlotCap{}; // L2 experimental
inline constexpr site<"site_Career_RnDPieSliceDefaultColour"> site_Career_RnDPieSliceDefaultColour{}; // L2 experimental
inline constexpr call<"site_Career_RnDPieSliceDescFormat"> site_Career_RnDPieSliceDescFormat{}; // L2 experimental
inline constexpr site<"site_Career_RnDPieSliceIconArgs"> site_Career_RnDPieSliceIconArgs{}; // L2 experimental
inline constexpr site<"site_Career_RnDPieSliceNameJoin"> site_Career_RnDPieSliceNameJoin{}; // L2 experimental
inline constexpr call<"site_Career_RnDPieTeardownSlotDtor"> site_Career_RnDPieTeardownSlotDtor{}; // L2 experimental
inline constexpr site<"site_Career_RnDPopupCategoryIcon"> site_Career_RnDPopupCategoryIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDPopupColourStore"> site_Career_RnDPopupColourStore{}; // L2 experimental
inline constexpr site<"site_Career_RnDPotentialPieEntry"> site_Career_RnDPotentialPieEntry{}; // L2 experimental
inline constexpr site<"site_Career_RnDPurchaseBackOpen"> site_Career_RnDPurchaseBackOpen{}; // L2 experimental
inline constexpr site<"site_Career_RnDPurchaseOptionsEntry"> site_Career_RnDPurchaseOptionsEntry{}; // L2 experimental
inline constexpr site<"site_Career_RnDPurchasePanelTypeLabel"> site_Career_RnDPurchasePanelTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_RnDPurchasePanelTypeLabelJoin"> site_Career_RnDPurchasePanelTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDPurchaseRequestEntry"> site_Career_RnDPurchaseRequestEntry{}; // L2 experimental
inline constexpr site<"site_Career_RnDPurchaseScreenTypeLabel"> site_Career_RnDPurchaseScreenTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_RnDPurchaseScreenTypeLabelJoin"> site_Career_RnDPurchaseScreenTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDResearchFocusNameCmp"> site_Career_RnDResearchFocusNameCmp{}; // L2 experimental
inline constexpr site<"site_Career_RnDResearchRowCategoryIcon"> site_Career_RnDResearchRowCategoryIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDResearchRowTypeIcon"> site_Career_RnDResearchRowTypeIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDResearchRowTypeIconJoin"> site_Career_RnDResearchRowTypeIconJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDResearchRowTypeLabel"> site_Career_RnDResearchRowTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_RnDResearchRowTypeLabelJoin"> site_Career_RnDResearchRowTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDResearchTypeLoop"> site_Career_RnDResearchTypeLoop{}; // L2 experimental
inline constexpr site<"site_Career_RnDResultTypeIcon"> site_Career_RnDResultTypeIcon{}; // L2 experimental
inline constexpr site<"site_Career_RnDResultTypeIconJoin"> site_Career_RnDResultTypeIconJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDResultTypeLabel"> site_Career_RnDResultTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_RnDResultTypeLabelJoin"> site_Career_RnDResultTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDTileCountBody"> site_Career_RnDTileCountBody{}; // L2 experimental
inline constexpr site<"site_Career_RnDTileCountExit"> site_Career_RnDTileCountExit{}; // L2 experimental
inline constexpr site<"site_Career_RnDTileCountGuards"> site_Career_RnDTileCountGuards{}; // L2 experimental
inline constexpr site<"site_Career_RnDTileCountIndexGuard"> site_Career_RnDTileCountIndexGuard{}; // L2 experimental
inline constexpr site<"site_Career_RnDTodoTypeLabel"> site_Career_RnDTodoTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_RnDTodoTypeLabelJoin"> site_Career_RnDTodoTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDTreeRowCountProbe"> site_Career_RnDTreeRowCountProbe{}; // L2 experimental
inline constexpr site<"site_Career_RnDUpgradeInfoTypeLabel"> site_Career_RnDUpgradeInfoTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_RnDUpgradeInfoTypeLabelJoin"> site_Career_RnDUpgradeInfoTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_RnDUpgradeListDeptIcon"> site_Career_RnDUpgradeListDeptIcon{}; // L2 experimental
inline constexpr site<"site_Career_TeamDeptEnumStore"> site_Career_TeamDeptEnumStore{}; // L2 experimental
inline constexpr site<"site_Career_TeamHqRnDPopupTypeIcon"> site_Career_TeamHqRnDPopupTypeIcon{}; // L2 experimental
inline constexpr site<"site_Career_TeamHqRnDPopupTypeIconJoin"> site_Career_TeamHqRnDPopupTypeIconJoin{}; // L2 experimental
inline constexpr site<"site_Career_TeleFailedTypeName"> site_Career_TeleFailedTypeName{}; // L2 experimental
inline constexpr site<"site_Career_TeleFailedTypeNameJoin"> site_Career_TeleFailedTypeNameJoin{}; // L2 experimental
inline constexpr site<"site_Career_TeleRecordTypeName"> site_Career_TeleRecordTypeName{}; // L2 experimental
inline constexpr call<"site_Career_TeleRecordTypeNameCall"> site_Career_TeleRecordTypeNameCall{}; // L2 experimental
inline constexpr site<"site_Career_TeleResearchTypeName"> site_Career_TeleResearchTypeName{}; // L2 experimental
inline constexpr call<"site_Career_TeleResearchTypeNameCall"> site_Career_TeleResearchTypeNameCall{}; // L2 experimental
inline constexpr site<"site_Career_Unk141814320TypeLabel"> site_Career_Unk141814320TypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_Unk141814320TypeLabelJoin"> site_Career_Unk141814320TypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_Unk1418169f0TypeLabel"> site_Career_Unk1418169f0TypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_Unk1418169f0TypeLabelJoin"> site_Career_Unk1418169f0TypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeAutoFitBody"> site_Career_UpgradeAutoFitBody{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeAutoFitDeptGate"> site_Career_UpgradeAutoFitDeptGate{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeBlockReplayEntry"> site_Career_UpgradeBlockReplayEntry{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeCardDeptIcon"> site_Career_UpgradeCardDeptIcon{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeCardTypeLabel"> site_Career_UpgradeCardTypeLabel{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeCardTypeLabelJoin"> site_Career_UpgradeCardTypeLabelJoin{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeGraphHubTest"> site_Career_UpgradeGraphHubTest{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeGraphRegularNode"> site_Career_UpgradeGraphRegularNode{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeInitFakeDeptBranch"> site_Career_UpgradeInitFakeDeptBranch{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeInitNormal"> site_Career_UpgradeInitNormal{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeMetaTypeJoin"> site_Career_UpgradeMetaTypeJoin{}; // L2 experimental
inline constexpr site<"site_Career_UpgradePushStateBody"> site_Career_UpgradePushStateBody{}; // L2 experimental
inline constexpr site<"site_Career_UpgradePushStateDeptGate"> site_Career_UpgradePushStateDeptGate{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeRowTypeJoin"> site_Career_UpgradeRowTypeJoin{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeSetEnabledBody"> site_Career_UpgradeSetEnabledBody{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeSetEnabledDeptGate"> site_Career_UpgradeSetEnabledDeptGate{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeSetFittedBody"> site_Career_UpgradeSetFittedBody{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeSetFittedDeptGate"> site_Career_UpgradeSetFittedDeptGate{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeSetLockedBody"> site_Career_UpgradeSetLockedBody{}; // L2 experimental
inline constexpr site<"site_Career_UpgradeSetLockedDeptGate"> site_Career_UpgradeSetLockedDeptGate{}; // L2 experimental
inline constexpr site<"site_Career_VehicleFillOrderEnd"> site_Career_VehicleFillOrderEnd{}; // L2 experimental
inline constexpr site<"site_Career_VehicleFillOrderStart"> site_Career_VehicleFillOrderStart{}; // L2 experimental
inline constexpr site<"site_Career_WeekendObjectiveNext"> site_Career_WeekendObjectiveNext{}; // L2 experimental
inline constexpr site<"site_Career_WeekendObjectiveType"> site_Career_WeekendObjectiveType{}; // L2 experimental
inline constexpr vtable<"vt_Career_RnDDeptCard", 0> vt_Career_RnDDeptCard{}; // L2 experimental

}
