# GfxMaterialRes constants: vehicle and track shaders (F1 25 stock values)

Lane: `assets`. Source: every `GfxMaterialRes` in `2025_asset_groups/f1_2025_vehicle_package` (7,590 materials, 24 shaders) and in `environment_package/tracks/melbourne` plus `environment_package/common` (1,248 materials, 64 shaders) of the Steam build, read with `tools/gfx_material.py parse25`. Format and record layout: [`gfx_material.md`](gfx_material.md). Bindless `me_*HeapIndex` constants are left out; the engine fills them at load.

"Stock value" is the most common value among the shader's materials, with its share when not every material has it; "Range" spans every component of every material. An addon changes these values with the [`set-material`](../../../docs/reference/ops/erp.md#materials) ERP operation.

## Checked in game

Benchmark race at Melbourne, Steam build, launcher branch `feat/overlay-w4-postfx`, runtime resource overlay route (`swofty/assets/resources/0.erp`):

| Material | Constants set | Result on screen |
|---|---|---|
| `teams/redbull/materials/redbull_paint.material` (`vehicle_metallic_paint`) | `me_lacquerColour (0, 1, 0)`, `me_flakesStrength 1`, `me_flakesColourInfluence (1, 0, 1)` | Yellow paint turns green, dark blue turns purple. |
| same | `me_lacquerColour (0.1, 1, 0.1)` alone | Barely visible. |
| same | `me_uPaintOffset 0.37`, `me_vPaintOffset 0.21` | No visible change: they do not move the livery texture. |
| `shared/tyres/materials/sidewall*.material` (`tyre_sidewall`) in `teams/common.erp` and `teams/common_fom.erp` | `me_logoColour (0, 1, 0)` | The compound stripe on the sidewall turns green on slicks in a dry race; the stock dry colours are white, yellow, cyan, pink and red (table below). |

Stock `me_logoColour` per sidewall material (`teams/common.erp`): `sidewall` and `sidewall_medium` (0.972, 0.972, 0.963), `sidewall_soft` (0.995, 0.909, 0.123), `sidewall_hard` (0, 0.828, 0.948), `sidewall_super_soft` (0.948, 0.351, 0.396), `sidewall_ultra_soft` (0.787, 0.587, 0.762), `sidewall_hyper_soft` (0.982, 0.85, 0.883), `sidewall_super_hard` (0.982, 0.775, 0.551), `sidewall_intermediate` (0.459, 0.839, 0.57), `sidewall_wet` (0, 0.696, 0.839). `me_logoType` is 1 for intermediate and wet, 0 for the rest.

## Vehicle shaders (f1_2025_vehicle_package)

### vehicle_paint_v2/vehicle_metallic_paint.nefx2

120 material(s), e.g. `eaid://f1_2025_vehicle_package/teams/fom_car/materials/liveries/livery_konnersport_livery25.material` in `f1_2025_vehicle_package/teams/common_fom.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_carbonRippleAmount` | 0.3 (84%) | 0 to 0.3 (5 values) |
| `me_dirtAlbedo` | (0.0999, 0.0999, 0.0999) (89%) | 0 to 0.1 (4 values) |
| `me_dirtRoughness` | 0.98 (91%) | 0.98 to 1 (2 values) |
| `me_flakesColourInfluence` | (0, 0, 0) |  |
| `me_flakesStrength` | 0 (92%) | 0 to 0.5 (7 values) |
| `me_flakesTile` | 1 (94%) | 1 to 10 (5 values) |
| `me_gsaaScreenSpaceVariance` | 0.1592 |  |
| `me_gsaaThreshold` | 0.5646 |  |
| `me_lacquerColour` | (1, 1, 1) (95%) | 0 to 1 (2 values) |
| `me_lacquerIor` | 1.3 (88%) | 1.1 to 1.5 (5 values) |
| `me_lacquerRoughness` | 0.9 (83%) | 0 to 0.9 (7 values) |
| `me_lacquerThickness` | 0.01 (88%) | 0.001 to 0.2 (4 values) |
| `me_metallicIor` | (1, 1, 1) (95%) | 0 to 1 (2 values) |
| `me_nightSpecularIntensity` | 2.5 (88%) | 2 to 2.5 (2 values) |
| `me_normalTile` | 20 (84%) | 10 to 200 (5 values) |
| `me_uOffset` | 0 (94%) | 0 to 0.5 (4 values) |
| `me_uPaintOffset` | 0 |  |
| `me_useMappedMetallicIor` | 1 |  |
| `me_vertexAo` | 1 |  |
| `me_vOffset` | 0.83 (84%) | 0 to 0.933 (11 values) |
| `me_vPaintOffset` | 0 |  |

### vehicle_paint_v2/vehicle_custom_paint.nefx2

65 material(s), e.g. `eaid://f1_2025_vehicle_package/teams/fom_car/materials/fom_car_paint.material` in `f1_2025_vehicle_package/teams/common_fom.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_carbonRippleAmount` | 0.2 |  |
| `me_dirtAlbedo` | (0.1, 0.1, 0.1) |  |
| `me_dirtRoughness` | 1 |  |
| `me_gsaaScreenSpaceVariance` | 0.1592 |  |
| `me_gsaaThreshold` | 0.5646 |  |
| `me_lacquerColour` | (1, 1, 1) (98%) | 0 to 1 (2 values) |
| `me_lacquerIor` | 1.5 (98%) | 1.3 to 1.5 (2 values) |
| `me_lacquerRoughness` | 0 (97%) | 0 to 0.2 (3 values) |
| `me_lacquerThickness` | 0.1 (98%) | 0.01 to 0.1 (2 values) |
| `me_metallicOverdrive` | 1 |  |
| `me_nightSpecularIntensity` | 2 |  |
| `me_normalTile` | 150 |  |
| `me_uOffset` | 0.5 (98%) | 0.5 to 0.9 (2 values) |
| `me_uPaintOffset` | 0 |  |
| `me_vertexAo` | 1 |  |
| `me_vOffset` | 0 |  |
| `me_vPaintOffset` | 0 |  |

### vehicle_paint_v2/vehicle_gloss_paint.nefx2

6 material(s), e.g. `eaid://f1_2025_vehicle_package/teams/fom_car/materials/liveries/livery_novelty_summer03_carbon2.material` in `f1_2025_vehicle_package/teams/common_fom.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_carbonRippleAmount` | 0.2 (67%) | 0.2 to 0.25 (2 values) |
| `me_dirtAlbedo` | (0.0999, 0.0999, 0.0999) (67%) | 0.0999 to 0.553 (2 values) |
| `me_dirtRoughness` | 0.98 |  |
| `me_gsaaScreenSpaceVariance` | 0.1592 |  |
| `me_gsaaThreshold` | 0.5646 |  |
| `me_lacquerColour` | (0, 0, 0) (67%) | 0 to 1 (2 values) |
| `me_lacquerIor` | 1.5 (33%) | 1.2 to 1.5 (3 values) |
| `me_lacquerRoughness` | 0.09 (67%) | 0.09 to 0.4 (2 values) |
| `me_lacquerThickness` | 0.01 |  |
| `me_nightSpecularIntensity` | 2 |  |
| `me_normalTile` | 150 (67%) | 1 to 150 (2 values) |
| `me_uOffset` | 0.1 (67%) | 0 to 0.1 (2 values) |
| `me_uPaintOffset` | 0 |  |
| `me_vertexAo` | 1 |  |
| `me_vOffset` | 0 (67%) | 0 to 0.0935 (2 values) |
| `me_vPaintOffset` | 0 |  |

### vehicle_carbon/vehicle_carbon.nefx2

53 material(s), e.g. `eaid://f1_2025_vehicle_package/teams/fom_car/materials/fom_car_carbon1.material` in `f1_2025_vehicle_package/teams/common_fom.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_carbonAnisotropyX` | 60 (56%) | 30 to 100 (4 values) |
| `me_carbonAnisotropyY` | 5 (70%) | 1 to 10 (3 values) |
| `me_carbonEffect` | 0.064 (91%) | 0.032 to 0.064 (2 values) |
| `me_carbonIor` | 1.5 (70%) | 1.3 to 1.5 (3 values) |
| `me_carbonTile` | 150 (28%) | 1 to 350 (11 values) |
| `me_carbonWidth` | 0.076 (91%) | 0.012 to 0.076 (2 values) |
| `me_dirtColour` | (0.3511, 0.3511, 0.3511) (60%) | 0.0345 to 0.3511 (3 values) |
| `me_dirtIor` | 1.5 |  |
| `me_dirtRoughness` | 0.98 |  |
| `me_gsaaScreenSpaceVariance` | 0.1592 |  |
| `me_gsaaThreshold` | 0.5646 |  |
| `me_lacquerIor` | 1.5 (56%) | 1.3 to 1.5 (3 values) |
| `me_lacquerLevel` | 0.15 (43%) | 0.025 to 1 (6 values) |
| `me_lacquerRoughness` | 0.4 (56%) | 0.1 to 0.5 (7 values) |
| `me_logoIor` | 1.5 (96%) | 1.4 to 1.5 (2 values) |
| `me_logoRoughness` | 0.5 (53%) | 0.1 to 0.5 (5 values) |
| `me_nightSpecularIntensity` | 2 |  |
| `me_uOffset` | 0 (94%) | 0 to 0.88 (3 values) |
| `me_vertexAo` | 1 |  |
| `me_vOffset` | 0 (64%) | 0 to 1 (5 values) |

### vehicle_carbon/vehicle_carbon_no_damage.nefx2

125 material(s), e.g. `eaid://f1_2025_vehicle_package/teams/fom_car/materials/fom_car_carbon4.material` in `f1_2025_vehicle_package/teams/common_fom.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_carbonAnisotropyX` | 30 (97%) | 30 to 100 (3 values) |
| `me_carbonAnisotropyY` | 5 (93%) | 1 to 5 (2 values) |
| `me_carbonEffect` | 0.064 (87%) | 0.064 to 0.2 (2 values) |
| `me_carbonIor` | 1.46 (84%) | 1.4 to 1.5 (3 values) |
| `me_carbonTile` | 1 |  |
| `me_carbonWidth` | 0.076 (90%) | 0.076 to 0.2 (2 values) |
| `me_dirtColour` | (0.3511, 0.3511, 0.3511) (94%) | 0.3511 to 0.887 (2 values) |
| `me_dirtIor` | 1.5 |  |
| `me_dirtRoughness` | 0.98 (90%) | 0.98 to 1 (2 values) |
| `me_gsaaScreenSpaceVariance` | 0.1592 |  |
| `me_gsaaThreshold` | 0.5646 |  |
| `me_lacquerIor` | 1.5 (97%) | 1.4 to 1.5 (2 values) |
| `me_lacquerLevel` | 0.15 (98%) | 0.15 to 1 (2 values) |
| `me_lacquerRoughness` | 0.1 (84%) | 0.1 to 0.5 (6 values) |
| `me_logoIor` | 1.5 (96%) | 1.3 to 1.5 (3 values) |
| `me_logoRoughness` | 0.5 (91%) | 0.1 to 0.5 (4 values) |
| `me_nightSpecularIntensity` | 2 |  |
| `me_uOffset` | 0 |  |
| `me_vertexAo` | 1 |  |
| `me_vOffset` | 0 |  |

### vehicle_paint_v2/vehicle_floating_decal.nefx2

2119 material(s), e.g. `eaid://f1_2025_vehicle_package/teams/fom_car/materials/fom_car_dummy_decal.material` in `f1_2025_vehicle_package/teams/common_fom.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_carbonRippleAmount` | 0.2 |  |
| `me_decalBias` | 0 | 0 to 0.01 (2 values) |
| `me_dirtAlbedo` | (0.1, 0.1, 0.1) | 0 to 0.1 (2 values) |
| `me_dirtRoughness` | 1 |  |
| `me_gsaaScreenSpaceVariance` | 0.1592 |  |
| `me_gsaaThreshold` | 0.5646 |  |
| `me_nightSpecularIntensity` | 2 |  |
| `me_normalTile` | 150 |  |
| `me_uOffset` | 0.4 (57%) | 0 to 0.4 (2 values) |
| `me_uPaintOffset` | 0 |  |
| `me_vertexAo` | 1 |  |
| `me_vOffset` | 0.08 (57%) | 0 to 0.08 (2 values) |
| `me_vPaintOffset` | 0 |  |

### vehicle_paint_v2/vehicle_floating_decal_driver_number.nefx2

4752 material(s), e.g. `eaid://f1_2025_vehicle_package/teams/fom_car/materials/number_sets/ansu01/ansu01_number_b_1.material` in `f1_2025_vehicle_package/teams/common_fom.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_carbonRippleAmount` | 0.2 |  |
| `me_decalBias` | 0 |  |
| `me_dirtAlbedo` | (0.1, 0.1, 0.1) |  |
| `me_dirtRoughness` | 1 |  |
| `me_driverNumberFront` | 0 (50%) | 0 to 1 (2 values) |
| `me_gsaaScreenSpaceVariance` | 0.1592 |  |
| `me_gsaaThreshold` | 0.5646 |  |
| `me_nightSpecularIntensity` | 2 |  |
| `me_normalTile` | 150 |  |
| `me_uOffset` | 0 |  |
| `me_uPaintOffset` | 0 |  |
| `me_vertexAo` | 1 |  |
| `me_vOffset` | 0 |  |
| `me_vPaintOffset` | 0 |  |

### vehicle_paint_v2/vehicle_floating_decal_surface_map.nefx2

198 material(s), e.g. `eaid://f1_2025_vehicle_package/teams/toro_rosso/materials/toro_rosso_number_1.material` in `f1_2025_vehicle_package/teams/toro_rosso/wep/toro_rosso.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_carbonRippleAmount` | 0.2 |  |
| `me_decalBias` | 0 |  |
| `me_dirtAlbedo` | (0.1, 0.1, 0.1) |  |
| `me_dirtRoughness` | 1 |  |
| `me_gsaaScreenSpaceVariance` | 0.1592 |  |
| `me_gsaaThreshold` | 0.5646 |  |
| `me_nightSpecularIntensity` | 2 |  |
| `me_normalTile` | 150 |  |
| `me_uOffset` | 0.4 (50%) | 0 to 0.4 (2 values) |
| `me_uPaintOffset` | 0 |  |
| `me_vertexAo` | 1 |  |
| `me_vOffset` | 0.08 (50%) | 0 to 0.08 (2 values) |
| `me_vPaintOffset` | 0 |  |

### vehicle_generic/vehicle_glass.nefx2

1 material(s), e.g. `eaid://f1_2025_vehicle_package/shared/generics/materials/glass.material` in `f1_2025_vehicle_package/teams/common_fom.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_cubeMapScale` | 1 |  |
| `me_nightSpecularIntensity` | 4 |  |
| `me_ssaoBias` | 0.7 |  |
| `me_ssaoScale` | 1 |  |
| `me_uvTile` | (1, 1) |  |
| `me_vertexAo` | 0 |  |

### vehicle_generic/vehicle_rear_light.nefx2

1 material(s), e.g. `eaid://f1_2025_vehicle_package/shared/generics/materials/rear_light.material` in `f1_2025_vehicle_package/teams/common_fom.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_cubeMapScale` | 1 |  |
| `me_nightSpecularIntensity` | 4 |  |
| `me_rearLightIntensity` | 1 |  |
| `me_ssaoBias` | 0.7 |  |
| `me_ssaoScale` | 1 |  |
| `me_uvTile` | (1, 1) |  |
| `me_vertexAo` | 0 |  |

### vehicle_generic/vehicle_indicators.nefx2

1 material(s), e.g. `eaid://f1_2025_vehicle_package/shared/generics/materials/indicator_lights.material` in `f1_2025_vehicle_package/teams/common_fom.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_cubeMapScale` | 1 |  |
| `me_nightSpecularIntensity` | 4000 |  |
| `me_ssaoBias` | 0.7 |  |
| `me_ssaoScale` | 1 |  |
| `me_uvTile` | (1, 1) |  |
| `me_vertexAo` | 0 |  |

### vehicle_generic/vehicle_lcd_screen.nefx2

1 material(s), e.g. `eaid://f1_2025_vehicle_package/shared/generics/materials/generic_lcd.material` in `f1_2025_vehicle_package/teams/common_fom.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_cubeMapScale` | 0 |  |
| `me_nightSpecularIntensity` | 0 |  |
| `me_screenSelfIllumination` | 0.3 |  |
| `me_ssaoBias` | 0 |  |
| `me_ssaoScale` | 1 |  |
| `me_useCarMonitor` | 0 |  |
| `me_uvTile` | (1, 1) |  |
| `me_vertexAo` | 0 |  |

### vehicle_generic/vehicle_revs_individual.nefx2

2 material(s), e.g. `eaid://f1_2025_vehicle_package/shared/generics/materials/vehicle_revs_type2.material` in `f1_2025_vehicle_package/teams/common_fom.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_cubeMapScale` | 1 |  |
| `me_nightSpecularIntensity` | 0 |  |
| `me_ssaoBias` | 0.7 |  |
| `me_ssaoScale` | 1 |  |
| `me_uvTile` | (1, 1) |  |
| `me_vertexAo` | 0 |  |

### vehicle_generic/vehicle_brakedisc.nefx2

1 material(s), e.g. `eaid://f1_2025_vehicle_package/shared/generics/materials/discs.material` in `f1_2025_vehicle_package/teams/common_fom.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_cubeMapScale` | 1 |  |
| `me_nightSpecularIntensity` | 2 |  |
| `me_ssaoBias` | 0.7 |  |
| `me_ssaoScale` | 1 |  |
| `me_uvTile` | (1, 1) |  |
| `me_vertexAo` | 0 |  |

### vehicle_generic/vehicle_wheel.nefx2

25 material(s), e.g. `eaid://f1_2025_vehicle_package/teams/fom_car/materials/fom_car_wheel_hub.material` in `f1_2025_vehicle_package/teams/common_fom.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_cubeMapScale` | 0.5 (64%) | 0.5 to 1 (2 values) |
| `me_nightSpecularIntensity` | 2.5 |  |
| `me_ssaoBias` | 0.1 (96%) | 0.1 to 0.2 (2 values) |
| `me_ssaoScale` | 1 (60%) | 0.9 to 2.5 (3 values) |
| `me_uvTile` | (1, 1) |  |
| `me_vertexAo` | 0 (84%) | 0 to 1 (2 values) |

### vehicle_generic/vehicle_generic.nefx2

26 material(s), e.g. `eaid://f1_2025_vehicle_package/shared/generics/materials/generics.material` in `f1_2025_vehicle_package/teams/common_fom.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_cubeMapScale` | 1 (65%) | 0.5 to 1 (2 values) |
| `me_nightSpecularIntensity` | 2 (92%) | 2 to 2.5 (2 values) |
| `me_ssaoBias` | 0 (92%) | 0 to 0.1 (2 values) |
| `me_ssaoScale` | 1 |  |
| `me_uvTile` | (1, 1) |  |
| `me_vertexAo` | 0 (77%) | 0 to 1 (2 values) |

### vehicle_rain_beads/vehicle_rain_beads.nefx2

1 material(s), e.g. `eaid://f1_2025_vehicle_package/shared/generics/materials/rainbeads.material` in `f1_2025_vehicle_package/teams/common_fom.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_beadsFakeAOMax` | 1 |  |
| `me_beadsFakeAOMin` | -1.25 |  |
| `me_beadsMinAngle` | 0.8 |  |
| `me_beadsTileUV` | (4, 4) |  |
| `me_beadsVibrationScale` | 0.18 |  |
| `me_recipOfOneMinusBeadsMinAngle` | 5 |  |
| `me_streaksMovementScale` | 1.75 |  |
| `me_streaksTileUV` | (128, 0.1) |  |

### vehicle_steering/vehicle_steering.nefx2

24 material(s), e.g. `eaid://f1_2025_vehicle_package/teams/fom_car/materials/fom_car_steeringwheel.material` in `f1_2025_vehicle_package/teams/common_fom.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_gsaaScreenSpaceVariance` | 0.1592 |  |
| `me_gsaaThreshold` | 0.5646 |  |
| `me_nightSpecularIntensity` | 2 (92%) | 2 to 2.5 (2 values) |

### tyre_sidewall/tyre_sidewall.nefx2

10 material(s), e.g. `eaid://f1_2025_vehicle_package/shared/tyres/materials/sidewall.material` in `f1_2025_vehicle_package/teams/common.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_damageBottomNoiseScale` | -0.7 |  |
| `me_damageIntensity` | 0 |  |
| `me_damageThreshold` | -0.1 |  |
| `me_damageTopNoiseScale` | -1 |  |
| `me_dirtColour` | (0, 0, 0) |  |
| `me_gsaaScreenSpaceVariance` | 0.1592 |  |
| `me_gsaaThreshold` | 0.5646 |  |
| `me_logoColour` | (0.9723, 0.9723, 0.9628) (20%) | 0 to 0.9954 (9 values) |
| `me_logoIOR` | 1.519 |  |
| `me_logoRoughness` | 0.2 (90%) | 0.15 to 0.2 (2 values) |
| `me_logoType` | 0 (80%) | 0 to 1 (2 values) |
| `me_nightSpecularIntensity` | 3 |  |
| `me_rubberIOR` | 1.4 |  |

### tyre_tread/tyre_tread.nefx2

9 material(s), e.g. `eaid://f1_2025_vehicle_package/shared/tyres/materials/tread_hard.material` in `f1_2025_vehicle_package/teams/common.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_damageBottomNoiseScale` | -0.7 |  |
| `me_damageIntensity` | 0 |  |
| `me_damageThreshold` | -0.1 |  |
| `me_damageTopNoiseScale` | -1 |  |
| `me_grassIOR` | 1.4 |  |
| `me_grassRoughness` | 0.5 |  |
| `me_gravelIOR` | 1.8 |  |
| `me_gravelRoughness` | 0.5 |  |
| `me_gsaaScreenSpaceVariance` | 0.1592 |  |
| `me_gsaaThreshold` | 0.5646 |  |
| `me_marblesRoughness` | 0.5 |  |
| `me_nightSpecularIntensity` | 6 |  |
| `me_rubberIOR` | 1.4 |  |
| `me_wearRoughnessCurve` | 0.25 (56%) | 0.01 to 0.5 (3 values) |
| `me_wearScale` | 1 (89%) | 0.9879 to 1 (2 values) |

## Track shaders (Melbourne and environment_package/common)

### object_shader/object_opaque.nefx2

418 material(s), e.g. `eaid://environment_package/cinematics/objects/materials/props/electronics/monitor_garage_a_02.material` in `environment_package/tracks/melbourne/melbourne_common.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_diffuseTint` | (1, 1, 1) (90%) | 0 to 4.595 (36 values) |
| `me_glowColour` | (1, 1, 1) (74%) | 0 to 1 (2 values) |
| `me_glowLevel` | 0 (95%) | 0 to 3000 (5 values) |
| `me_gsaaScreenSpaceVariance` | 0.1592 (99%) | 0.1592 to 1 (2 values) |
| `me_gsaaThreshold` | 0.5646 (99%) | 0.5646 to 1 (2 values) |
| `me_receiveSSRTShadows` | 0 (97%) | 0 to 1 (2 values) |
| `me_roughnessLinkedOcclusion` | 0 (97%) | 0 to 1 (2 values) |
| `me_selfIlluminationLevel` | 0 (97%) | 0 to 100 (4 values) |
| `me_suppressSSAO` | 0 (97%) | 0 to 1 (2 values) |
| `me_translucency` | 0 (67%) | 0 to 1 (8 values) |
| `me_useCubemaps` | 0 (97%) | 0 to 1 (2 values) |
| `me_useSSRT` | 0 (97%) | 0 to 1 (2 values) |

### object_shader/object_glass.nefx2

2 material(s), e.g. `eaid://environment_package/tracks/melbourne/objects/materials/mel_pit_mod_building_02.material` in `environment_package/tracks/melbourne/wep/melbourne_common.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_alphaThreshold` | 0.5 (50%) | 0.5 to 0.6 (2 values) |
| `me_diffuseTint` | (1, 1, 1) |  |
| `me_energyConservingTransparency` | 1 |  |
| `me_glowColour` | (1, 1, 1) |  |
| `me_glowLevel` | 0 (50%) | 0 to 1 (2 values) |
| `me_gsaaScreenSpaceVariance` | 0.1592 |  |
| `me_gsaaThreshold` | 0.5646 |  |
| `me_indexOfRefraction` | 2 |  |
| `me_metallic` | 0.5 |  |
| `me_receiveSSRTShadows` | 0 |  |
| `me_roughness` | 0.96 (50%) | 0.95 to 0.96 (2 values) |
| `me_roughnessLinkedOcclusion` | 0 |  |
| `me_selfIlluminationLevel` | 0 |  |
| `me_shadowAlphaThreshold` | 0.5 |  |
| `me_suppressSSAO` | 0 |  |
| `me_translucency` | 0.5 |  |
| `me_useCubemaps` | 0 |  |
| `me_useSSRT` | 0 |  |

### bulb/bulb.nefx2

1 material(s), e.g. `eaid://environment_package/core_assets/objects/materials/sponsor/sponsor_pitlaneclock_a_03.material` in `environment_package/tracks/melbourne/wep/melbourne_classic.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_colour` | (1, 1, 1) |  |
| `me_outputLevel` | 4000 |  |
| `me_useTexture` | 1 |  |

### bulb/bulb_area.nefx2

4 material(s), e.g. `eaid://environment_package/core_assets/objects/materials/garage/garage_main_a_04.material` in `environment_package/tracks/melbourne/melbourne_common.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_colour` | (1, 1, 1) |  |
| `me_outputLevel` | 7000 (50%) | 0 to 7000 (3 values) |
| `me_useTexture` | 1 (75%) | 0 to 1 (2 values) |

### lights_animated/lights_animated.nefx2

1 material(s), e.g. `eaid://environment_package/core_assets/shared_materials/effects/monitor_screen_bright_anim_a.material` in `environment_package/tracks/melbourne/wep/melbourne_classic.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_activeStrip` | 1 |  |
| `me_alphaThreshold` | 0.5 |  |
| `me_numFrames` | 4 |  |
| `me_numStrips` | 1 |  |
| `me_playbackSpeed` | 0.6 |  |
| `me_selfIlluminationLevel` | 4000 |  |

### lights_animated/lights_scrolling.nefx2

2 material(s), e.g. `eaid://environment_package/core_assets/shared_materials/effects/monitor_screen_bright_anim_b.material` in `environment_package/tracks/melbourne/wep/melbourne_classic.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_alphaThreshold` | 0.5 |  |
| `me_selfIlluminationLevel` | 4000 (50%) | 1000 to 4000 (2 values) |
| `me_USpeed` | 0.025 (50%) | 0 to 0.025 (2 values) |
| `me_VSpeed` | 0 |  |

### lights_switchable/lights_switchable.nefx2

2 material(s), e.g. `eaid://environment_package/core_assets/objects/materials/lights/lights_pit_exit_a_01.material` in `environment_package/tracks/melbourne/melbourne_common.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_selfIlluminationLevel` | 1000 (50%) | 1000 to 1e+04 (2 values) |

### lights_marshal/lights_marshal.nefx2

1 material(s), e.g. `eaid://environment_package/core_assets/objects/materials/lights/lights_marshal_a_02.material` in `environment_package/tracks/melbourne/melbourne_common.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_selfIlluminationLevel` | 1 |  |
| `me_selfIlluminationLevelWSM` | 4000 |  |
| `me_worldLightMapNDotLBias` | 0.5 |  |

### jumbotron/jumbotron.nefx2

1 material(s), e.g. `eaid://environment_package/core_assets/shared_materials/effects/live_feed_screen.material` in `environment_package/tracks/melbourne/melbourne_common.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_backgroundUVOffset` | (0, 0) |  |
| `me_backgroundUVScale` | (1, 1) |  |
| `me_brightness` | 2 |  |
| `me_contrast` | 0 |  |
| `me_effectTile` | 6 |  |
| `me_foregroundUVOffset` | (0, 0) |  |
| `me_foregroundUVScale` | (1, 1) |  |
| `me_indexOfRefraction` | 1.4 |  |
| `me_roughness` | 0.5 |  |
| `me_rttUVOffset` | (0, 0) |  |
| `me_rttUVScale` | (1, 1) |  |
| `me_selfIlluminationLevel` | 500 |  |
| `me_useDiffuseAlpha` | 0 |  |
| `me_useGame` | 1 |  |

### sponsor/sponsor_board.nefx2

16 material(s), e.g. `eaid://environment_package/core_assets/shared_materials/sponsors/banner/sp_banner_2x_aramco_blend.material` in `environment_package/tracks/melbourne/melbourne_common.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_backsideUVOffset` | (0, 0) |  |
| `me_backsideUVScale` | (1, 1) |  |
| `me_detailUVOffset` | (0, 0) |  |
| `me_detailUVScale` | (1, 1) |  |
| `me_gsaaScreenSpaceVariance` | 0.1592 |  |
| `me_gsaaThreshold` | 0.5646 |  |
| `me_normalUVOffset` | (0, 0) |  |
| `me_normalUVScale` | (0.25, 1) (75%) | 0.125 to 1 (2 values) |
| `me_receiveSSRT` | 0 |  |
| `me_receiveSSRTShadows` | 0 |  |
| `me_selfIlluminationLevelWSM` | 4000 |  |
| `me_suppressSSAO` | 0 |  |
| `me_surfaceUVOffset` | (0, 0) |  |
| `me_surfaceUVScale` | (0.25, 1) (75%) | 0.125 to 1 (2 values) |
| `me_worldLightMapNDotLBias` | 0 |  |

### sponsor/sponsor_screen_reflection_flip.nefx2

1 material(s), e.g. `eaid://environment_package/core_assets/shared_materials/sponsors/digital/sp_digital_c_refl_flip.material` in `environment_package/tracks/melbourne/wep/melbourne_classic.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_backsideUVOffset` | (0, 0) |  |
| `me_backsideUVScale` | (1, 1) |  |
| `me_blackPoint` | 0.0196 |  |
| `me_delay` | 3 |  |
| `me_effectBrightness` | 6000 |  |
| `me_effectStrength` | 0.7 |  |
| `me_effectTile` | (6, 6) |  |
| `me_gsaaScreenSpaceVariance` | 0 |  |
| `me_gsaaThreshold` | 0 |  |
| `me_normalUVOffset` | (0, 0) |  |
| `me_normalUVScale` | (1, 1) |  |
| `me_numSponsors` | 9 |  |
| `me_receiveSSRT` | 0 |  |
| `me_receiveSSRTShadows` | 0 |  |
| `me_selfIlluminationLevelWSM` | 0 |  |
| `me_smoothness` | 0.5 |  |
| `me_suppressSSAO` | 0 |  |
| `me_surfaceUVOffset` | (0, 0) |  |
| `me_surfaceUVScale` | (1, 1) |  |
| `me_useSurfaceMap` | 0 |  |
| `me_whitePoint` | 0.9804 |  |
| `me_worldLightMapNDotLBias` | 0.1 |  |

### terrain_shader/terrain_single.nefx2

8 material(s), e.g. `eaid://environment_package/core_assets/shared_materials/collision/physics_astroturf.material` in `environment_package/tracks/melbourne/wep/melbourne_common.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_diffuseTint` | (1, 1, 1) (88%) | 0.4287 to 1 (2 values) |
| `me_diffuseWSM` | 0 |  |
| `me_diffuseWSMScale` | 1 (88%) | 0.1 to 1 (2 values) |
| `me_fresnelColour` | (1, 1, 1) (88%) | 0.0003 to 1 (2 values) |
| `me_fresnelDistance` | 50 |  |
| `me_groundCoverBlendBias` | 1 (88%) | 0 to 1 (2 values) |
| `me_groundcoverColour2Amt` | 1 |  |
| `me_macroScale` | 1 (88%) | 0.5 to 1 (2 values) |
| `me_macroWSM` | 0 |  |
| `me_macroWSMScale` | 1 (88%) | 0.1 to 1 (2 values) |
| `me_normalWSM` | 0 |  |
| `me_normalWSMScale` | 1 (88%) | 0.1 to 1 (2 values) |
| `me_renderOrder` | 0 |  |
| `me_roughnessLinkedOcclusion` | 0 |  |
| `me_surfaceWSM` | 0 |  |
| `me_surfaceWSMScale` | 1 (88%) | 0.1 to 1 (2 values) |
| `me_translucencyAmt` | 0.2 |  |
| `me_useFresnelColour` | 0 |  |

### water/water.nefx2

2 material(s), e.g. `eaid://environment_package/tracks/melbourne/terrain/materials/material_283.material` in `environment_package/tracks/melbourne/wep/melbourne_common.erp`.

| Constant | Stock value | Range across materials |
|---|---|---|
| `me_gammaAlbedo` | 1 |  |
| `me_gsaaScreenSpaceVariance` | 0.1592 |  |
| `me_gsaaThreshold` | 0.5646 |  |
| `me_normalLayer1USpeed` | 0.012 (50%) | 0.0051 to 0.012 (2 values) |
| `me_normalLayer1VSpeed` | -0.017 (50%) | -0.017 to 0.0073 (2 values) |
| `me_normalLayer1WMSScale` | 0.0035 (50%) | 0.0035 to 0.0055 (2 values) |
| `me_normalLayer2USpeed` | -0.01 (50%) | -0.01 to -0.0046 (2 values) |
| `me_normalLayer2VSpeed` | 0.01 (50%) | -0.0046 to 0.01 (2 values) |
| `me_normalLayer2WMSScale` | 0.0005 (50%) | 0.0005 to 0.003 (2 values) |
| `me_useLocalIBL` | 0 |  |


## Open
- Most constants are named but unchecked in game; the table gives stock values, not effects.
- Track materials beyond Melbourne and `environment_package/common` are not scanned; `python3 tools/gfx_material.py catalog` lists every shader's layout.
