# vm: UI view-model registry

Owner: see `subsystems.tsv`. Write-up: research/exe.md §1 (conventions), §3.4; research/ui-native.md (binding, dispatch, screens, routes).

## Map
- `g_Vm_Registry` (0x146ca54f8) is the registry singleton pointer; T000 saw it non-null after the main window appeared.
- `Vm_Set_String(reg, parent, name, NeStr**, overwrite)` → `Vm_NodeSet_String(node, v, 0, 1)`; `Vm_Set_Bool(...)` → `Vm_NodeSet_Bool`. `Vm_Set_Int` (0x140d38fd0) and `Vm_Set_UInt` (0x140d38210) are the int/index siblings. overwrite=1 replaces an existing value.
- `parent` for an item's properties is `HomeItem.button_node.node + 8`; with no node use `k_Vm_EmptyNodeHandle` (0x146ca0e90).
- Node names and commands are djb2 hashes (`h_*` anchors); components use md5-64 (ui.md).

## UI node-class (widget) registry {#classes}
Every `.ui` node has a 4-char class FourCC. `Vm_Register_NodeClasses` (0x14419b1d0, L2) constructs ~97 class-factory objects (each 0xc0 bytes: vtable at +0x00, FourCC tag at +0x10) and installs them through the callback `Vm_Install_NodeFactories` (0x1441cac40, L2), which does `Vm_Find_NodeFactory(mgr, fourcc)` (0x14416bc70) per class and registers the factory at `mgr+0x8210`'s vtable+0x38. `Vm_Build_UidfNode`/`Vm_Read_UidfNode` look a node's class up here when a screen loads; a FourCC with no factory makes the node build fail with code 5 (ui-native.md §10).

The 97 classes, in registration order (FourCC → best-known role; `UIxx` = NeUI element, `DBxx` = data-binding node, others engine):
```
UIGL group-list   UIGA  UIGR  UIGV  UIFL flow   UISE screen-entry/route-group   UISC screen
UISA  UIEX  UIAC action/route-pin   UINT  UICM  UICT  UIGP group   UIKL  UIDD  UILB label
UILS  UISU  UIBT button   UIHB  UIHC  UITG toggle   UIVP  F1FV fullscreen-video   UISR  UILT list
UIL2 list2   UITB toggle-bar   UIEL element-list   UILA  UILE  UISL slider   UITE text-entry
UITV  PNSQ  UIVC  UITI  UICO  UIRU  UITL  UITT  UISN  UIXA  UISB  SCRL scroll   UICF  UIPL
TIME  LOCA localise   UISM  UIMO  UIRE  UIVS  SFMV  DBCD  UIDS data-source   SVCD single-value-decision
MVCD multi-value-decision   WUDN  DBRV DBVV DBAM DBIT DBTC DBTV DBRN  UITR  UITO  UIDO  UIPX
SYPT  UIMC  UIDM  DDMC  UICN  UIVF  UIUC  UITF  UIDF  UITP  UIIN input   UIEA  UIVO  UIAT  UIAM
USCR  SWIP swipe   UITC  UISG  UITH  UITX text-element   UIRN  UILX  UIXI  UIBI  UISP
```
Unannotated tags are known classes whose role is not yet read (L1, `imm:` the FourCC). The two-letter suffix is a stable hint (…L=list, …B=button/bar, …P=panel, …N=input/node).

## Element class inits (per-class `vt[0x48]` from-data init) {#element-init}
Shared base chain (ui-native.md §11): `Vm_ElementBase_Init` (0x1441ffb90, ~64 classes; returns 3 without componentId/nodeId) → `Vm_ListBase_Init` (0x144208c80; shared by UITG/UITB/UILT/UIL2/UIEL; its result is `Vm_Read_PropBinding(props,"data")`). Concrete inits:
- `Vm_UiToggle_Init` (0x144272810, UITG), `Vm_UiSlider_Init` (0x1442593b0, UISL; reads flags with `Vm_Read_PropBool` at +0xa4..+0xa7/+0x18, then PropBinding data/value/min/max/step — all int, no float).
- Property readers: `Vm_Read_PropBinding` (0x14417e200; dbentry: no key→3, missing/empty→5), `Vm_Read_PropBool` (0x144206120).

## Node helpers {#nodes}
- `Vm_Resolve_RegistryNode` (0x1442b1b60, L1, **2166 refs**): resolve a `g_Vm_Registry` node by a path-string token (`#ui_menu_data.…`, `view_model_data.…`, `network.…`). The core VM lookup — frontend and HUD VMs both go through it. `Vm_Link_RegistryNode` (0x144179360, 918 refs) links the resolved node into a holder/handle.
- `Vm_GetOrCreate_ChildNode` (0x1442b2bf0, L2): resolve/create a child node under a parent; `k_Vm_EmptyNodeHandle` on miss.
- `Vm_Resolve_NodePath_Thunk` (0x1442b2770) is the path-resolve used by `Vm_Apply_BindingEvent` and the core `set_*` API.

## Routing {#routes}
- `Vm_Fire_Route(router, vm, &name, flag)` (0x1442c0e20, L2, 266 callers): looks the VM's node up in the router robin-hood map (router+0x48), finds the `UISE` route-group element and calls the route runner.
- **`Vm_Run_Route` (0x144188370, L2)** is that runner (previously unmapped, ui-native.md §9): `void(uiSys, node, routeName, group, int, a6, a7)`, invoked as `Vm_Run_Route((*uiSys)->vt[0x1c0](), node, name, group, 0, flag, 0)`. It runs the route steps and is where a missing-screen step raises its error — the single hook point for the route guard, covering both native `Vm_Fire_Route` routes and data-fired UIAC pins.
- Route-command executors (slot +0x58 of each route-cmd vtable): `Vm_RouteCmd_ResolveTargetScreen` (0x144187620), `Vm_RouteCmd_OpenScreen` (0x143889310) → `Vm_Open_Screen` (0x144182660).
- `Vm_Update_FlowStateMachine` (0x144423ce0, sig `vm.flow_update`) ticks the flow state machine each frame; it logs `State [%s] onUpdate returned LinkRoute [name,hash]` (0x144424e27) and drives screen navigation. The frontend-side states live in the `fe_flow` lane.

## Open
- `Vm_Fire_Route`/label rendering not yet runtime-confirmed (trace test).
- Hook point for brand-new data commands (`raise_command {name}`): generic dispatch is `Vm_Dispatch_BindingMessage` (0x1442c00e0); per-screen handlers are `*VM::onCommandRaised` slot 10.
- Element class vtables beyond toggle/slider not yet named individually; the factory objects built by `Vm_Register_NodeClasses` are the enumeration point (one 0xc0 object per FourCC, in the order above).

## Inbox
- 2026-10-17 17:11 ui-native: 2026-10-17 ui-native: VM binding registry, Vm_Dispatch_BindingMessage (0x1442c00e0, commands 0x4c / events 0x4d), VM vtable slot map, and the missing-screen route failure are written up in research/ui-native.md sections 1, 2 and 9. All new rows are L2 at most and unverified. T006 traces the dispatcher.
- 2026-10-17 17:46 ui-native: 2026-10-17 ui-native: screen fill path Vm_Load_UidfResource -> Vm_Load_UidfScreens (job on mgr+0xb900); prototype native/src/core/ui.cpp registers mod screens through it and guards missing route targets (research/ui-native.md section 10).
- 2026-10-18 22:13 telemetry: telemetry lane: per-frame publish hook point is Vm_Update_FlowStateMachine 0x144423ce0. Verified unique .xtls signature (matches exactly once, resolves to base+0x4423ce0): 41 57 41 56 41 55 41 54 56 57 55 53 48 81 EC 58 02 00 00 C5 F8 29 B4 24 40 02 00 00 C5 F9 28 F2 48 89 . native/src/core/telemetry.cpp installs a safetyhook mid hook here (gated by the experimental telemetry feature) to advance the feed frame counter and session clock each frame.
- 2026-10-19 13:22 ea-port: EA port: ui_vm.cpp view-model Targets (Vm_Register_ViewModel, Ne_Set_FixedString, Vm_Resolve_NodePath_Thunk, Vm_Set_Int call, Vm_Register_BindingNode) and vtable rvas are Steam-specific and scan .xtls; on EA they must scan .rdata and need patterns from a decrypted-memory dump. g_Vm_Registry/Vm_Set_String/Vm_Set_Bool (bindings.json) same situation.
- 2026-10-23 14:59 era-picker: era-picker: added Vm_GetOrCreate_ListChild (0x1442b30e0, sig vm.get_or_create_list_child, func_start) and Vm_Set_NodeRef (0x14083c270, sig vm.set_node_ref, bound through its call 0x141a6eeb9 in FeTile_Build_EraTiles because the setter template clones share its prologue) at L2 for the era picker's addon row. The lane is owned by ui-re, so the rows were added with the lead override; the owner may want to fold them into the vm map. The vt+0x10 / vt+0x28 child tests read as list node / object node: Vm_GetOrCreate_ListChild keeps a child only when vt+0x10 is true and creates with 0x1442b2f70, Vm_GetOrCreate_ChildNode keeps vt+0x28 children and creates with 0x1442b2a50, and Vm_Set_NodeRef links a new child by list append (0x1442b32b0) or keyed insert (0x1442b3770) on the same split.
- 2026-10-23 18:39 ui-engine-re: ui-engine-re: template path mapped (docs/reference/ui/engine.md, re/db/ui/loader_rules.json). Templates are indexed at load only (Vm_Load_UidfTemplates: name -> {NeStr, resid, pkg, u32 node offset} in the global store at *(uisys->vt[0x48]()+0x8168), duplicate name -> 0x58 and the UIDF handler drops the rest of the resource) and built per instantiation through Vm_Instantiate_Template_Thunk 0x144177fe0 -> body 0x153215c10 (.data1) -> Vm_Build_TemplateNode 0x1441815c0. A child failure aborts the instance and leaves the partial root attached to its parent (attach at 0x14418169e precedes the child loop; no cleanup on the rc path). UITC tabs: Vm_UiTabs_OnMessage 0x144264d50 -> Vm_UiTabs_AddTabTemplate 0x144266e30 (miss 0x144267110, failure 0x14426708d, both silent). Signals: Vm_Read_SignalList 0x1442c17e0 (to_pin/to_object required), Vm_Remap_InstanceSignals 0x144349300, Vm_Link_NodeReference 0x144179da0 (pending links), Vm_Dispatch_EventSignals 0x144251800 (null link skipped). All rows added at L1 with decomp+xref evidence (L2-ready); please raise and fold into the map. Struct candidates not written (owner files): UiTemplateRecord {NeStr* name @0, resid* @8, pkg base @0x10, u32 node_off @0x18, ctx @0x20}; attach step 0x30 {u8 kind @0, u32 nodeId @4, u64 componentId @8}; signal 0x18 {link handle* @0, u32 to_pin @8, u32 to_object @0xc, attrs* @0x10}.
- 2026-10-23 19:02 ui-engine-re: ui-engine-re: runtime-confirmed 2026-10-23 (gdb trace, breaking era build): the dynamic_era_select template ROOT (UIGP) init returns 3 in every content build because its slot 6 lists NBVN while its props lack visualNav; Vm_VisualNavBehaviour_Init 0x144289830 reads visualNav (Vm_Read_PropObject_Thunk 0x14417f1b0 -> Vm_Find_PkgObjectKey_Thunk 0x143927f90, string compare) and returns 3 on a miss. The later ok instances named tab_N( dynamic_era_select ) are the UITC header loop (Vm_UiTabs_AddTabTemplate names every instance after content_template). NBVM/NBDC/NBXA/NBTP inits are 0x1438d50b0 (return 0), so no VM-path lookup is involved. A root whose own init fails is never attached (correction to the earlier partial-tree note).
