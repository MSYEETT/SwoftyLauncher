# fe_tiles: generic tile-menu builder and item classes

Owner: see `subsystems.tsv`. Write-up: research/exe.md §3.1–3.2.

## Map
- `FeTile_Build_List(TileList*, ctx, a3, a4, menu_kind, sub_kind, a7)` (0x14173df80) serves home, multiplayer, F1 World, showroom, customisation and career content. Callers 0x141bb2125 (FEHomeViewModel) and 0x141a48cc5.
  - clears `TileList.items` (0x10-byte `TileEntry {node, tile}`), binds VM `buttons` (+0x38) and `selected_button` (+0x58), switches on (menu_kind, sub_kind);
  - after the switch (`site_FeTile_PopulateLoop`, 0x141745016) it calls `item->vt.init` then `item->vt.populate` for every item in vector order, so an item can be re-typed between its adder and its populate.
- Tile = 0xd8 bytes (`structs/Tile.h`), HomeItem = 0x148 bytes (`structs/HomeItem.h`), vtable layout `structs/ItemVtbl.h` (19 slots).
- Base implementations: `FeTile_ItemBase_*` (slot names in functions.tsv). Generic getAction `FeTile_ItemGeneric_GetAction` returns `{1, item->id}` and has 21 byte-identical clones, so reach it through a vtable, never a signature.
- `.data1` bodies (black boxes, call only): `FeTile_ItemVec_PushBack_Body`, `FeTile_List_GetSelectedItem_Body`, `FeTile_ItemBase_IsSelectable_Body`.

## Open
- `FeTile_Unk_14174c710`, `FeTile_Unk_14174cb20`, `FeTile_Unk_141745280`: decompiled next to the builder, roles unknown.
- Which other menus (menu_kind values) map to which cases.

## Inbox
