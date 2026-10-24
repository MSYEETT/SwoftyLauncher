---
title: Post-process passes
description: Run your own HLSL shader on the race picture after the game's tonemap and before the HUD.
---

# Post-process passes

A post-process pass is an HLSL compute shader in an addon. The launcher compiles it to DXIL with dxc when it builds the profile, and the in-game core runs it on every frame of the 3D picture after the game's tonemap and colour grade, before the HUD draws. Vignettes, saturation, sharpening, film grain and chromatic aberration are passes.

Start from [`templates/postfx-pass`](https://github.com/Swofty-Developments/SwoftyLauncher/tree/main/templates/postfx-pass): a vignette with saturation, and settings for both.

| | |
|---|---|
| Manifest | `postfx`, `"experimental_features": ["postfx"]` |
| Feature | `postfx`: experimental, on while an addon lists it in `experimental_features` |
| Launcher | Not in a release yet; an earlier launcher rejects the manifest |
| Build | dxc from `SWOFTY_DXC`, the launcher's `dxc/` folder, or `PATH` |
| Picture | The game's encoded picture: sRGB in SDR (R8G8B8A8), PQ in HDR (R10G10B10A2) |

## The manifest

```json [swofty.json]
{
  "schema_version": 1,
  "id": "me.my-vignette",
  "name": "My Vignette",
  "version": "1.0.0",
  "author": "Me",
  "category": "shaders",
  "experimental_features": ["postfx"],
  "compat": { "game": "f1_25" },
  "options": [
    { "id": "amount", "type": "slider", "label": "Amount", "min": 0, "max": 100, "step": 5, "default": 60,
      "binds": "db" }
  ],
  "postfx": [
    { "shader": "fx/vignette.hlsl", "options": ["{{amount}}", 100] }
  ]
}
```

| Field | Type | Value |
|---|---|---|
| `postfx[].shader` | string | The `.hlsl` file, a path inside the addon. |
| `postfx[].name` | string? | The pass's name in the build output and `loader.log`. Defaults to the file name without `.hlsl`. |
| `postfx[].options` | (number \| string)[]? | Up to 16 values the shader reads with `option(0)` to `option(15)`. A string is <span v-pre>`"{{id}}"`</span> naming a `db` option; a toggle reads as 1 or 0. |
| `postfx[].order` | integer? | Where the pass runs among every addon's passes. Lower runs first; equal values run in load order. Defaults to 0. |
| `postfx[].budget_ms` | number? | GPU milliseconds a frame, above 0 and at most 16. Defaults to 1.5. |

`postfx` needs `"postfx"` in `experimental_features`. Installing checks every pass: a `shader` that is not a `.hlsl` file inside the addon, more than 16 options, an unknown option or one that does not bind `db`, or a budget outside its range is refused.

## The shader

A pass includes `swofty/postfx.hlsli` and defines `effect`, which returns the colour of one pixel:

```hlsl [fx/vignette.hlsl]
#include "swofty/postfx.hlsli"

float4 effect(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    float3 rgb = to_linear(c.rgb);
    float2 d = uv - 0.5;
    d.x *= float(swofty_size.x) / float(swofty_size.y);
    rgb *= 1 - option(0) / 100 * smoothstep(0.3, 0.9, length(d));
    return float4(from_linear(rgb), c.a);
}
```

The header is in the SDK at `packages/sdk/include/swofty/postfx.hlsli`; the build puts it on dxc's include path. Files next to the shader can be included with `#include "name.hlsli"`.

| Name | Value |
|---|---|
| `scene(uv)` | The picture at `uv` (0 to 1), filtered. |
| `scene_at(pixel)` | The picture at a pixel, clamped to the edges. |
| `scene_depth(uv)` | The game's depth buffer at `uv`, as stored (`eaid://ego_package/gbuffer_depth_stencil.image`): 1 at the camera, falling towards 0 with distance. 0 everywhere when `has_depth()` is false. |
| `has_depth()` | Whether the depth buffer was readable for this frame. It is false while passes run at Present. |
| `swofty_depth_size` | The depth buffer's size in pixels. It is the render resolution, which is below the picture's with upscaling on. |
| `option(i)` | The pass's `options[i]`, 0 when not set. |
| `to_linear(c)`, `from_linear(c)` | Decode the picture to linear light and encode it back: sRGB in SDR, PQ in HDR10, unchanged in scRGB. In linear light 1 is SDR white, and 80 nits in HDR. |
| `luminance(c)` | Rec. 709 luminance of a linear colour. |
| `swofty_size`, `swofty_texel` | The picture's size in pixels, and 1 over it. |
| `swofty_time`, `swofty_frame` | Seconds since the passes started, and frames since the game started. |
| `swofty_encoding` | `SWOFTY_POSTFX_SDR`, `SWOFTY_POSTFX_HDR10` or `SWOFTY_POSTFX_SCRGB`. |
| `swofty_pass` | The pass's place in this frame's chain, from 0. |

A pass reads the result of the pass before it. The output is the picture's own format, so values outside 0 to 1 clip in SDR.

## Build

Launcher releases include dxc in a `dxc/` folder beside `bin/`, built from the open-source DirectX Shader Compiler with its DXIL validator. `SWOFTY_DXC` names another dxc, and without either the build uses `dxc` on `PATH`. A pass must compile to signed DXIL; dxc without its validator (`dxil.dll`, `libdxil.so`) beside it writes unsigned DXIL, which the build refuses.

To check a pass while writing it, compile the addon's passes the way the build does. The command prints dxc's errors and warnings in full and exits non-zero when a pass fails:

```sh
swofty postfx compile path/to/my-addon [--out dxil/]
```

```text
dxc: /home/me/SwoftyLauncher/dxc/bin/dxc
ok    vignette (fx/vignette.hlsl): 5984 bytes of signed DXIL
```

The build compiles each pass with `dxc -T cs_6_0 -E main -HV 2021 -O3` and keeps the DXIL in the addon's cache, so an unchanged pass is not compiled again. It prints one line per pass:

```text
postfx: My Vignette's pass vignette runs as pass 1 after the tonemap (5968 bytes of DXIL)
```

A pass that does not compile is left out, and the build warns with dxc's first error:

```text
My Vignette: pass vignette is skipped: dxc: fx/vignette.hlsl:6:12: error: use of undeclared identifier 'nope'
```

The compiled passes are installed as `swofty/postfx/00.dxil` onwards, and `swofty/assets.json` lists them under `postfx` in the order they run.

## In game

- `swofty/loader.log` shows `[postfx] 1 pass(es) wait for the game's tonemap`, `[postfx] My Vignette's pass vignette is ready`, then `[postfx] the picture now goes through 1 pass(es) before the HUD` once the race renders, and `[postfx] passes now run at Present on a snapshot of the picture before the HUD (queue 0000000066737380), so every frame gets them`.
- Every 60 s for the first 30 minutes, the log counts the frames presented and the snapshots taken: `[postfx] last 60 s at Present: 3852 frame(s); snapshots before the HUD: 3171 in the game's lists, 182 between them; 0 frame(s) without one`. Frames without a snapshot show the game's own picture; loading screens are most of them.
- With Frame Generation on, or unknown, in the game's settings, passes run on the split instead: `[postfx] swapchain 0000000062296D50: passes stay before the HUD, as frame generation is on or unknown in the game's settings`. Every 10 s for the first minute, the log then counts the frames the passes ran on and the frames they skipped: `[postfx] last 10 s: passes ran on 598 frame(s); 0 tonemap list(s) also drew UI and 0 left no back buffer to draw on, so passes skipped them`.
- With HDR on, the back buffer is R10G10B10A2 and holds PQ: `[postfx] passes run on the game's 1908x1028 picture (format 24)`, and `swofty_encoding` is `SWOFTY_POSTFX_HDR10`. A pass that works in `to_linear`/`from_linear` looks the same in SDR and HDR.
- With Frame Generation, the game keeps a HUD-less copy of the picture, `eaid://ego_package/hudless_backbuffer.image`; on the split, the passes' result goes into it too: `[postfx] the game's HUD-less copy of the picture gets the passes too, for Frame Generation`.
- A pass whose pipeline cannot be made is turned off: `[postfx] My Vignette's pass vignette is off: its pipeline could not be made`.
- The core measures each pass's GPU time. A pass whose average over 120 frames stays above `budget_ms` three times in a row is turned off: `[postfx] My Vignette's pass vignette is off: it took 2.410 ms of GPU time a frame, over its 1.50 ms budget`.
- A `swofty/postfx.off` file in the game folder turns every pass off within a second, and removing it turns them on again. `SWOFTY_POSTFX=0` in the game's environment turns them off for the session.
- `SWOFTY_POSTFX_CAPTURE=<folder>` writes the picture before and after the passes six times in the first minute, and once more each time a `swofty/postfx.capture` file appears in the game folder. At Present the pictures hold the HUD, and `postfx-NN-hudless.ppm` holds the snapshot before it; on the split they are without the HUD. `SWOFTY_FRAME_CAPTURE` writes the presented frame with the HUD ([logging switches](/reference/std/logging)).

## How it works

The game tonemaps the 3D picture into `eaid://ego_package/tonemap_output.image`, grades and encodes it into the back buffer, then draws the HUD into the back buffer. On some frames, and on most in garages, the picture and the HUD are in one command list.

Every frame, the core copies the back buffer into a snapshot of its own between the picture and the HUD. Where the game moves `eaid://ego_package/hudless_backbuffer.image` out of UAV while the back buffer is a render target, the copy goes into the game's list just before that barrier. Where a list in which `tonemap_output.image` goes from written to read ends with the back buffer still a render target, the core submits a copy list right after it on the same queue. Both add copies and barriers only, so the game's pipeline state stays as it was.

At Present, the core submits a list on the swapchain's queue: each pass runs on a copy of the snapshot, between two textures of its own, and a merge adds what the passes changed to the presented back buffer wherever it still matches the snapshot. The HUD keeps the game's colours, and so does the picture under translucent HUD panels.

With Frame Generation on, the core marks the list in which `tonemap_output.image` goes from written to read. When that list leaves the back buffer as a render target and draws no UI, the core submits a list of its own right after it on the same queue: it copies the back buffer, runs each pass, and copies the result back into the back buffer and the HUD-less copy. The HUD lists then run as the game submitted them. Frames where the game draws the picture and the HUD in one list keep the game's picture.
