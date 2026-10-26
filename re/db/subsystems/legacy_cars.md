# Car archives of older F1 games

Lane: `assets`. Tool: `tools/legacy_car.py` (`convert`, `check`).

Evidence: file-format analysis only; no exe addresses. Corpora:

- F1 2016: the loose sidecars of `Pascal Wehrlein helmet 2017.rar` (`character_package/drivers/male/2016-sauber_felipe_nasr/textures`); no ERP.
- F1 2017 `c_ferrari_95`; F1 2019 `sauber`, `c_mclaren_76`, `c_ferrari_90`, F2 2018 and F2 2019 `artgp`; F1 2020 `c_mclaren_91`.
- F1 22: `helmet_vettel.erp` of `Vettel Mexico GP helmet 2022 (1).zip`, a character archive (surfaces, SRVs, samplers, one material).
- F1 23: `f2_2023_vehicle_package/teams/artgp`, `f1_2023_vehicle_package/teams/alpine24`, and the helmet archive of `Felipe Massa Ferrari Career Mode Helmet.rar`.
- F1 24: no game file. `F1 24 Cars in F1 25.zip` is a third-party port whose archives keep F1 24 resources (named `f1_2024_vehicle_package/...`) beside the slot's; the F1 24 column comes from those resources.
- F1 25: `f1_2025_vehicle_package/teams/{ferrari,mclaren,sauber}` with their livery archives, and `f2_2025_vehicle_package/teams/artgp`.

No sample of F1 2018 or F1 2021. The `F1 20xx Cars for F1 25` mod archives of 2016, 2017, 2018, 2020, 2021 and 22 hold F1 25 slot resources only and sample no older format.

Not tested in game yet.

## Resource versions

The ERP container is version 3 in every sampled game. The ERP index version of each resource:

| Resource | F1 2016 | F1 2017, 2019, 2020 | F1 2018, F1 2021 | F1 22 | F1 23 | F1 24 | F1 25 |
|---|---|---|---|---|---|---|---|
| `GfxMeshRes` | no sample | 0x10000 | no sample | no sample | 0x60000 | 0x60000 | 0x60000 |
| `GfxBufferRes` | no sample | 3 | no sample | no sample | 4 | 4 | 4 |
| `GfxSurfaceRes` | no sample | 8 | no sample | 8 | 9 | 9 | 9 |
| `GfxMaterialRes` | no sample | 1 | no sample | 1 | 1 | 2 | 2 |
| `GfxSampleState` | no sample | 1 | no sample | 1 | 1 | none | none in car archives |
| `GfxSRVResource` | no sample | 2 | no sample | 2 | 2 | 2 | 2 |
| `GfxModelRes`, `ModelProxyRes`, `GfxMatBindRes` | no sample | 0x20001, 2, 1 | no sample | no sample | 0x20001, 2, 1 | 0x20001, 2, 1 | 0x20001, 2, 1 |
| `VTF` | no sample | 1 or 2 | no sample | no sample | 6 | 7 | 8 |
| `GfxMshSkin`, `GfxMshVertex` | no sample | none | no sample | no sample | none | 0x10000 (`shared/`) | none in car archives |

Fragment compression: F1 2017 to F1 2020 write zlib as 1 and stored fragments as 0x81. F1 22 writes zstd as 0x10 and stored fragments as 0x90. F1 23 and F1 25 write zstd as 0x11 and stored fragments as 0x91. Bit 0x80 marks a stored fragment.

F1 22's material parses as the version 1 layout of F1 23, and its surface header is the 40-byte header of versions 8 and 9.

A team archive of these games holds every resource its car uses except shaders: the `shared/` generics, rain beads and core textures it draws are copies inside the archive.

## GfxBufferRes

Fragment 0 is 16 bytes, `u32 kind, u32 stride, u32 count, u32 format`; fragment 1 holds the data. Kind is 0x109 (vertex) or 0x10a (index) in version 3, 0x1109 and 0x110a in version 4. Stride, count, format and data are the same in both.

| stride | format | data |
|---|---|---|
| 12 | 0x27 | float3 |
| 8 | 0x23 | half4 |
| 4 | 0x1d | 4 bytes |
| 4 | 0xd | 4 bytes, blend index |
| 8 | 0x21 | 8 bytes |
| 2 | 0x13 | u16 indices |

## GfxMeshRes

Version 0x10000: a 36-byte header (9 × u32), then `n` records of 88 bytes, a string pool, names and a trail.

```
0x00 n          submeshes
0x04 nvb        vertex buffer references
0x08 nib        index buffer references
0x0c n
0x10 n
0x14 pool       offsets 0x14..0x20 are relative to the end of the header
0x18 ib         the first index buffer string
0x1c names
0x20 trail      8 bytes per record
```

Version 0x60000: a 76-byte header (19 × u32), the same records, pool, names and trail, then a vertex declaration table and a tail.

```
0x00..0x20      as above, offsets relative to the end of the 76-byte header
0x24 ndecl      equals nvb
0x28 decl       ndecl entries of 5 × u32
0x2c tail       ndecl entries of (u32 1, u32 k)
0x30 0
0x34 pool
0x38 0
0x3c pool
0x40 0
0x44 0
0x48 lod        the LOD number on a root mesh (`?lod=N&mesh`), 0 on a part mesh
```

A record: `[0]` stream count, `[1]` 4, `[2]` 2, `[3]` first index, `[4]` index count, `[5]` base vertex, `[6..9]` bounding sphere, `[10]` trail offset, `[11]` 1, `[12]` name offset, `[13]` 0, `[14..]` streams. In version 0x10000 a stream is a byte offset into the string pool; in version 0x60000 it is a declaration index, and declaration `k` describes vertex buffer reference `k`.

A declaration: `u32 slot, u32 0, u32 semantic, u32 semantic index, u32 format`. The slot is the stream's position in the records that use it. Semantics: 1 position, 2 normal, 3 tangent, 5 texture coordinates, 7 blend. Declaration formats per buffer: float3 0x2f, half4 0x23, 0x1d 0x1d, blend index 0xd, 8-byte blend 0x1f.

Version 0x10000 has no declarations. A record's streams come in a fixed order:

| streams | order |
|---|---|
| 6 | position, normal, tangent, texcoord 0, texcoord 1 (0x1d), blend index |
| 5 | position, normal, tangent, texcoord 0, blend index |
| 4 | position, normal, tangent, texcoord 0 (classic tyre tread and wall) |
| 2 | position, blend index |
| 1 | a 40-byte interleaved stream: float3 position, half4 normal, half4 tangent, half4 texcoord, 4-byte blend index (mirror glass) |

F1 25 draws mirror glass as five separate streams.

## GfxSurfaceRes

Fragment 0 is 40 bytes, `u32 2, u32 0x108, u32 format, u32 width, u32 height, u32 depth, u32 mips, u32 1, u32 0, u32 0`, the same in versions 8 and 9. Fragment 1 holds the levels from `width × height` down. A third fragment names a `.mipmaps` sidecar that holds the larger levels; `width`, `height` and `mips` describe the archive's levels only.

Sidecar descriptor, version 8: `u8 n, path[n], u32 count`, then `count` × `{u8 compression, u64 offset, u64 packed, u64 size}`, then floats. Version 9: `u8 n, path[n], u8 0, u32 count`, then `count` × `{u8, u8 compression, u64 offset, u64 packed, u64 size}`. Compression 0 is raw and 3 an LZ4 block. The path is relative to the game folder (`f1_2019_vehicle_package/teams/sauber/textures/sauber_decal_da.tif.mipmaps`). The byte after the path is 0 in version 9 and the low byte of a non-zero count in version 8; `legacy_car.py` reads the layout from that byte. A version 9 descriptor can list no levels (count 0, F1 23 helmet archive); the archive then holds every level.

Sidecar names: `<texture>.tga.mipmaps` in F1 2016, `<texture>.tif.mipmaps` from F1 2017, with a year (`.tif.2019.mipmaps`, `.tif.2023.mipmaps`, `.tif.2025.mipmaps`) on character and core textures.

F1 2016 sidecars hold raw levels, largest first. The stock `felipe_nasr_helmet_n.tga.mipmaps` (5,591,040 bytes) is 16-byte blocks from 2048×2048 down to 64×64. In the mod's `_d` and `_s` sidecars the file is the payload of the DDS beside it: every level from the top down to 1×1.

The SRV (`GfxSRVResource`) of a surface holds its level count at byte 12.

## GfxMaterialRes

Version 1 is the F1 23 layout (`re/db/subsystems/gfx_material.md`) in F1 2017 to F1 23. Every vehicle shader the corpora name exists in F1 25 except `tyre_sidewall/tyre_sidewall_no_logo.nefx2`.

## Conversion onto an F1 25 slot (`legacy_car.py convert`)

- Buffers: version 4, kind | 0x1000.
- Meshes: version 0x60000, declarations from the stream order; interleaved 40-byte streams split into five buffers named after the archive's last `vb=`.
- Materials: `plan23to25` and version 2; `tyre_sidewall_no_logo` becomes `tyre_sidewall`.
- Surfaces: version 9 with two fragments. Sidecar levels found under `--sidecars` are put in front of the archive levels, and width, height, level count and the SRV follow; without them the archive levels stay.
- Fragments: read as zlib, zstd (0x10 or 0x11) or stored, written as zstd 0x11.
- Versions outside the table's F1 2017 to F1 25 values stop the conversion with an error naming them.
- Dropped: `GfxSampleState`, `VTF`, `World`, `WOInstances`, `EventGraph`.
- Models, proxies and material bindings named after the source car's `.idf`/`.emb` take the slot's names. Every other resource keeps its source name.
- From the slot's archive: `World`, `WOInstances`, `EventGraph` and the resources they reference, such as the handling file. The slot's layers name `<car>.idf?context=render`, `<car>.emb` and `<car>.emb?context=default`.
- Every material-binding context the slot has and the source lacks is a copy of the source's `default`.

`legacy_car.py check` checks a car archive the way F1 25's own cars read:

- fragments are zstd 0x11 or stored 0x91;
- buffers are version 4 with kind bit 0x1000, meshes 0x60000, materials 2 with a shader of `tools/gfx_material_layouts.json`, surfaces 9;
- a surface has two or three fragments, a third is a version 9 descriptor, and its SRV's level count equals its own;
- no `GfxSampleState`;
- declaration slots, every stream's semantic and format against those F1 25 uses, and every index and vertex inside its buffers.

Stock F1 25 car archives pass it (`ferrari`, `ferrari_monza_25`, `mclaren`, `mclaren_silverstone_25`, `sauber`, `sauber_barcelona_25`, `sauber_miami_25`, F2 `artgp`), as do the six conversions and the `ferrari.erp` of the 2016, 2018, 2021, 22 and 24 third-party ports.

`legacy_car.py selftest` runs the fragment, LZ4, descriptor, mesh and check code on synthetic resources.
