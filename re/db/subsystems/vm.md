# vm: UI view-model registry

Owner: see `subsystems.tsv`. Write-up: research/exe.md §1 (conventions), §3.4.

## Map
- `g_Vm_Registry` (0x146ca54f8) is the registry singleton pointer; T000 saw it non-null after the main window appeared.
- `Vm_Set_String(reg, parent, name, NeStr**, overwrite)` → `Vm_NodeSet_String(node, v, 0, 1)`; `Vm_Set_Bool(...)` → `Vm_NodeSet_Bool`. overwrite=1 replaces an existing value.
- `parent` for an item's properties is `HomeItem.button_node.node + 8`; with no node use `k_Vm_EmptyNodeHandle` (0x146ca0e90).
- Routes: `Vm_Fire_Route(router, vm, &name, 0)` fires a route in the screen's route grouping (data side: `.uir` files, ui.md).
- Node names and commands are djb2 hashes (`h_*` anchors); components use md5-64 (ui.md).

## Open
- `Vm_GetOrCreate_ChildNode` (0x1442b2bf0) and `Vm_Fire_Route` (0x1442c0e20) have no saved decompile yet.
- Hook point for brand-new data commands (`raise_command {name}`): find the generic dispatch before the per-VM `onCommandRaised`.

## Inbox
