# F1 25 RE-Optimization

A Swofty Launcher addon port of [F1 25 RE-Optimization](https://github.com/xanpavle/F1-25-RE-Optimization) by [xanpavle](https://github.com/xanpavle), used and redistributed under its MIT licence (see `LICENSE`). Ported by Swofty with xanpavle's permission.

Offline, single-player only. It ships with the launcher and stays off until you turn it on. Supported on the Steam build of F1 25 (build 25251167, `F1_25.exe` sha256 `d873a1c8...`) only.

## What it does

| Part | Original | This port |
|---|---|---|
| Loading | proxy `dinput8.dll` | engine plugin `native/re_optimization.dll`, loaded by the launcher's `version.dll` loader at the early stage (inside the game's `CreateMutexW("Codemasters F1 25")` call) |
| Graphics profile | rewrites `hardware_settings_config.xml` on disk before the engine reads it, keeps a `.untuned` backup | hands the engine's XML parser a tuned copy of the text in memory; the file on disk is never written by the addon, and when the engine saves the file itself, every attribute that still holds a tuned value is written back with your own value |
| Engine overrides | detours the getters at RVA `0x03a43b90` (bool) and `0x03a43240` (32-bit), runs the original, then overrides the names in `[override]` | the same semantics through the mixin system, on the bindings `Ne_Get_SettingBool` and `Ne_Get_SettingU32`, found by signature |
| Upscaler | GPU detection and `[upscaler] mode` | the same detection and choice, as the Upscaler option |
| `f125_tune.ini` | settings | addon options on the Addons page and in game under ADDON SETTINGS |

The graphics profile lowers: volumetric fog map (to 512x288), world and night shadow maps (to 2048), shadow sampling (to 2), 32-bit sky shadow map, crowd shadows, SSR (quality 2), SSAO (algorithm 1), god rays, lens streak, motion blur v2, high particles, crowd billboard distance (2500), car reflection scales and cube-map update mode (one face per frame), procedural cloud RT scale (2.0), grass draw distance (80 m), hair, skidmark parallax and the texture streaming bias (0.25). It turns on the engine's generic GPU culling (`cs_culling`). Mirrors, ray tracing, rain and `aa_quality` are not touched.

## Options

| Option | Default | Effect |
|---|---|---|
| Apply the graphics profile | on | the tuned delta above, applied in memory at launch |
| Upscaler | Auto | Auto: DLSS on NVIDIA RTX 20 and newer, FSR 3 on NVIDIA GTX and AMD, XeSS on Intel Arc, FSR 3 on other Intel GPUs, unchanged on an unknown GPU. DLSS, FSR 3, XeSS, TAA and Off force a backend; Keep changes nothing |
| VRS and async compute | on | `Renderer.Gfx.enableVRS = 1`, `Renderer.Gfx.enableAsyncCompute = 1` |
| Grass and ground cover caps | on | `groundCoverForceUltraQuality = 0`, `groundCoverMaxInstances = 40000`, `groundCover2MaxInstances = 80000`, `groundCover2MaxMeshInstances = 4000` |
| Static instance caps | on | `Renderer.maxStaticInstances = 20000`, `Renderer.maxPVSRenderInstances = 20000` |
| Log setting reads | off | the original's discovery mode (`[log] all = 1`): every setting the engine reads, then each change, goes to `swofty/loader.log` |

The graphics profile and the upscaler apply when the game reads its settings, so a change applies on the next launch.

## Safety

- Every hook target is a signature binding in the launcher's knowledge base (`re/db/signatures.json`: `core.setting_get_bool`, `core.setting_get_u32`, `core.hw_xml_parse_call`, `core.hw_config_parse_call`, `core.hw_xml_write_call`), each matching exactly once in the Steam build. A binding that does not resolve leaves its feature off and logs `SAFE`. The graphics profile installs only when the save site resolves too.
- Log lines start with `[re-opt]` in `swofty/loader.log` in the game folder.

## Build and test

```sh
native/build.sh      # zig cross-build of native/re_optimization.dll
native/test/run.sh   # host tests: the delta, its reverse, the override table, GPU detection and the upscaler matrix
```
