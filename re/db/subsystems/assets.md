# assets: eaid:// resolution, asset groups, worlds

Owner: see `subsystems.tsv`. Mostly unexplored (research/exe.md §5).

## Map
- World selection is data (world-variant table rows `worldEri`/`worldAssetGroup`), see research/exe.md §4. Code only formats `asset_groups/environment_package/tracks/%s/wep/%s_%s.assetgroup` for dev-session extra worlds (`Asset_Load_DevSessionWorlds`).
- Candidates for the resolver: `Asset_Parse_EaidA/B` (0x143b130a0/0x143b13710), `Asset_Map_EaidToGroupA/B` (0x14024fe80/0x140250ee0), `Asset_Get_AssetGroupsRoot` (0x1413b7640). Decompiles exist, not analysed.
- UI component ids are `md5(eaid path without extension)[:8]` big-endian (ui.md).
- Stock constant values of the vehicle and track shaders, and which ones were checked in game: [`gfx_material_constants.md`](gfx_material_constants.md).
- GfxMaterialRes (`.material`): F1 25 and F1 23 layouts and the F1 23 to F1 25 conversion rules are in [`gfx_material.md`](gfx_material.md). Tool: `tools/gfx_material.py`. Textures, SRVs, material bindings, meshes and models use the same byte format in both games. Materials differ: F1 25 has bindless `me_*HeapIndex` constants and no sampler records, while F1 23 has `<texture>.sampler` GfxSampleState references.

## Open
- Does the resolver support loose-file overrides? (Would remove the need to rewrite ERPs.)

## Inbox
- 2026-10-25 14:24 cs-heads: ERP fragment compression byte (decomp:re/decomp/143a6c700.c, decomp:re/decomp/143a6d4b0.c): the loader reads it as a signed byte and stores a negative one (0x90, 0x91) as 0, a plain copy; the decompressor thread treats codes 3 to 0x3f as zstd (0x20 to 0x2f with dictionary code - 0x20). Its zstd loop (0x143a6d4b0) calls ZSTD_decompressStream (0x1447b0a60) in 0x20000-byte chunks until the input position reaches the packed size and never checks the result. A fragment under a zstd code that holds no zstd frame returns 'Unknown frame descriptor' without consuming input (libzstd 1.5.7 run with the same loop), so the job spins on its NeErpRsDecompressor thread, the resource is never created and its ERP never finishes loading: the first rebuilt Classic Seasons heads of PR #323 stored 20 materials uncompressed under More Driver Models' code 0x10 and a Grand Prix load stayed on PLEASE WAIT. tools/erpw.py and packages/formats compress only code 0x11 on write.
