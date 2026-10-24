# assets: eaid:// resolution, asset groups, worlds

Owner: see `subsystems.tsv`. Mostly unexplored (research/exe.md §5).

## Map
- World selection is data (world-variant table rows `worldEri`/`worldAssetGroup`), see research/exe.md §4. Code only formats `asset_groups/environment_package/tracks/%s/wep/%s_%s.assetgroup` for dev-session extra worlds (`Asset_Load_DevSessionWorlds`).
- Candidates for the resolver: `Asset_Parse_EaidA/B` (0x143b130a0/0x143b13710), `Asset_Map_EaidToGroupA/B` (0x14024fe80/0x140250ee0), `Asset_Get_AssetGroupsRoot` (0x1413b7640). Decompiles exist, not analysed.
- UI component ids are `md5(eaid path without extension)[:8]` big-endian (ui.md).
- GfxMaterialRes (`.material`): F1 25 and F1 23 layouts and the F1 23 to F1 25 conversion rules are in [`gfx_material.md`](gfx_material.md). Tool: `tools/gfx_material.py`. Textures, SRVs, material bindings, meshes and models use the same byte format in both games. Materials differ: F1 25 has bindless `me_*HeapIndex` constants and no sampler records, while F1 23 has `<texture>.sampler` GfxSampleState references.

## Open
- Does the resolver support loose-file overrides? (Would remove the need to rewrite ERPs.)

## Inbox
