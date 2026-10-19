---
title: Addons that ship with the launcher
description: The addons every launcher release installs switched off, starting with xanpavle's F1 25 RE-Optimization - what it changes, its settings, and how to turn it on, off or remove it.
---

# Addons that ship with the launcher

Every launcher release carries a few addons in the `addons/` folder next to `bin/` and `loader/`. On start, the launcher installs each one into `$DATA/addons/<id>/`. They appear on the **Addons** page and stay off in every profile, new and existing, until you turn them on.

| Addon | Id | By |
|---|---|---|
| [F1 25 RE-Optimization](#f1-25-re-optimization) | `xanpavle.re-optimization` | [xanpavle](https://github.com/xanpavle), ported by Swofty |

## Turn one on, off or remove it

- **Turn it on:** on the **Addons** page, toggle its switch in the profile you want it in, then press **Play**. From a terminal: `swofty addons enable <id>`.
- **Turn it off:** toggle the switch again, or run `swofty addons disable <id>`.
- **Remove it:** expand its row and select **Remove addon**. It stays removed when the launcher updates. To get it back, install `addons/<id>.swoftyaddon` from the launcher's install folder.

A launcher update that carries a newer version of a bundled addon updates the installed copy. Your settings and the profiles it's on stay as they are.

## F1 25 RE-Optimization

F1 25 RE-Optimization is [xanpavle's renderer optimisation for F1 25](https://github.com/xanpavle/F1-25-RE-Optimization), released under the MIT licence. Swofty ported it to a launcher addon with his permission. It makes the renderer do less work: it lowers a set of quality settings that cost a lot of GPU time, overrides a few engine-level renderer limits, and picks the upscaler your GPU supports.

- It is off until you turn it on.
- It works offline and in single-player only, like every modded session.
- It is made for the Steam build of F1 25 (build 25251167). On any other build its hooks don't install, and `swofty/loader.log` says so with `SAFE`.
- It doesn't change `hardware_settings_config.xml` in your Documents folder. The addon lowers the settings in memory as the game reads the file. When the game saves the file, for example after you change a graphics option, the values the addon lowered go back to yours.

### What it lowers

The graphics profile lowers these on top of whatever preset you pick:

| Setting | Before (example) | With the profile |
|---|---|---|
| Volumetric fog map | 1920 × 1080 | 512 × 288 |
| World and night shadow maps | 4096 | 2048 |
| 32-bit sky shadow map | on | off |
| Shadow and spot shadow sampling | 3 | 2 |
| Crowd shadows | on | off |
| Screen-space reflections (SSR) quality | 4 | 2 |
| Ambient occlusion (SSAO) | 3 | 1 |
| Grass draw distance | 120 m | 80 m |
| Crowd billboard distance | 6000 | 2500 |
| High-quality particles | on | off |
| Hair | on | off |
| God rays, lens streak, motion blur v2 | on | off |
| Car reflection maps | | scale 1.0, one cube face updated per frame |
| Procedural cloud render scale | | 2.0 |
| Skid mark parallax | on | off |
| Texture streaming texel density bias | | 0.25 |
| GPU culling (`cs_culling`) | off | on |

It leaves resolution, your preset, mirrors, ray tracing, rain, the upscaler quality level (Quality, Balanced, Performance) and frame generation alone.

### Settings

You'll find these in the addon's row on the **Addons** page and under **ADDON SETTINGS** in game.

| Setting | Id | Default | Does |
|---|---|---|---|
| Apply the graphics profile | `apply_profile` | on | Lowers the settings in [What it lowers](#what-it-lowers) |
| Upscaler | `upscaler` | `auto` | Picks the upscaler: `auto`, `dlss`, `fsr3`, `xess`, `taa`, `off` (no anti-aliasing) or `keep` (your in-game choice) |
| VRS and async compute | `override_vrs_async` | on | Turns on variable rate shading (`Renderer.Gfx.enableVRS`) and async compute (`Renderer.Gfx.enableAsyncCompute`) |
| Grass and ground cover caps | `override_ground_cover` | on | Caps ground-cover instances: `groundCoverMaxInstances` 40000, `groundCover2MaxInstances` 80000, `groundCover2MaxMeshInstances` 4000, and turns off `groundCoverForceUltraQuality` |
| Static instance caps | `override_instance_caps` | on | Sets `Renderer.maxStaticInstances` and `Renderer.maxPVSRenderInstances` to 20000. Distant props can pop in |
| Log setting reads | `log_reads` | off | Writes every engine setting the game reads, and each change, to `swofty/loader.log` |

The graphics profile and the upscaler apply when the game reads its settings file at launch, so a change to either applies on the next launch. The upscaler applies with the profile on or off.

With `upscaler` set to `auto`, the addon picks by GPU:

| GPU | Upscaler |
|---|---|
| NVIDIA RTX 20, 30, 40 and 50 series | DLSS |
| NVIDIA GTX 10 and 16 series | FSR 3 |
| AMD Radeon | FSR 3 |
| Intel Arc | XeSS |
| Other Intel GPUs | FSR 3 |
| Not detected | Your in-game choice |

### Its log

The addon writes to `swofty/loader.log` in the game folder. Its lines start with `[re-opt]`: the GPU it detected, the upscaler it picked, the settings it lowered and each hook it installed or skipped.
