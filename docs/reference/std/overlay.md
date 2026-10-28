---
title: Overlay
description: Draw a 2D overlay on top of F1 25 each frame with swofty/overlay.hpp.
---

# Overlay

The overlay library calls your paint callback once per presented frame and draws the rectangles, lines and text you issue on top of the game's picture.

```cpp [native/src/my_plugin.cpp]
#include <swofty/mixin.hpp>
#include <swofty/overlay.hpp>

#include <cstdio>

namespace ov = swofty::overlay;

static unsigned g_frames;

// Runs on the render thread once per presented frame; use the draw calls only in here
static void paint(SwoftyPaint *p, void *)
{
    float w = 0, h = 0;
    ov::size(p, &w, &h); // the drawable size changes with the window: ask every frame
    if (++g_frames == 1)
        swofty::log("[my-addon] paint %u at %.0fx%.0f", g_frames, w, h);

    ov::rect(p, 40, 40, 520, 150, 0x000000b0);                   // filled, 0xRRGGBBAA
    ov::rect_outline(p, 40, 40, 520, 150, 3, 0xff2d9affu);       // 3 px stroke
    ov::text(p, 60, 60, 32, 0xffffffffu, "SWOFTY OVERLAY");      // (60, 60) is the text's top-left
    char line[64];
    snprintf(line, sizeof line, "frame %u  %.0fx%.0f", g_frames, w, h);
    ov::text(p, 60, 110, 24, 0x7fff7fffu, line);
    ov::line(p, 40, 200, 560, 260, 4, 0xffff00ffu);
    ov::rect(p, w - 140, h - 140, 100, 100, 0x00ff0080u);        // pinned to the bottom-right corner
}

SWOFTY_ON_LOAD
{
    bool on = ov::available(); // false unless an addon lists "overlay" in experimental_features
    ov::Result r = on ? ov::on_paint(paint) : ov::Result::unavailable;
    swofty::log("[my-addon] overlay available=%d on_paint=%s", on ? 1 : 0, ov::name(r));
}

SWOFTY_ADDON()
```

```json [swofty.json]
{
  "schema_version": 1,
  "id": "author.my-addon",
  "name": "My Addon",
  "version": "1.0.0",
  "author": "Your name",
  "kind": ["native"],
  "compat": { "game": "f1_25" },
  "requires": [{ "id": "swofty.core", "version": ">=0.1.0" }],
  "experimental_features": ["overlay"],
  "operations": { "native": [{ "dll": "native/my_plugin.dll" }] }
}
```

`swofty/loader.log`:

```text
[overlay] api: author.my-addon on_paint -> ok (token 1)
[my-addon] overlay available=1 on_paint=ok
[overlay] the game's hardware settings (C:\users\steamuser\Documents\My Games\F1 25\hardwaresettings\hardware_settings_config.xml) have frame generation off (frame_gen mode 0, multi_frame_gen 0)
[overlay] drawing on swapchain 0000000064CB2270 with the direct queue 0000000064CB25A0 it was created with (1908x1028, format 28, 4 buffer(s), created by sl.dlss_g.dll <- sl.interposer.dll <- F1_25.exe <- kernel32.dll <- ntdll.dll)
[overlay] swapchain 0000000064CB2270 output: SDR, paper white 350 nits (format 28, no SetColorSpace1 seen)
[my-addon] paint 1 at 1908x1028
[overlay] first overlay frame submitted: 1014 vertices
[overlay] last 60 s: 2953 frame(s) drawn, 0 skipped while the GPU still held the slot; GPU 0.012 ms per drawn frame (2953 timed)
```

- **Header:** `swofty/overlay.hpp`, namespace `swofty::overlay`
- **Table:** `SwoftyApi::overlay` (`SwoftyOverlay`), API version 5
- **Feature:** `overlay`, experimental, on while an addon lists it in `experimental_features`
- **Thread:** the render thread (the swapchain present path) for paint callbacks

`on_paint`, `remove`, `load_image` and `image` return `swofty::overlay::Result` (`ok`, `late`, `unavailable`, `invalid`); see the [result codes](./#result-codes). The draw calls, `size` and `measure` return nothing.

## Coordinates and colours

```cpp
float w = 0, h = 0;
ov::size(p, &w, &h);                         // framebuffer pixels, origin at the top-left
ov::rect(p, w / 2 - 50, 0, 100, 20, 0xff0000ffu); // 0xRRGGBBAA in sRGB: opaque red
```

Position your overlay in framebuffer pixels, measured from the top-left. The drawable size changes with the window, so query it each frame. Colours are `0xRRGGBBAA` in sRGB; on HDR output the core converts them (see [HDR output](#hdr-output)).

## available

```cpp
if (ov::available())
    ov::on_paint(paint);
```

`true` once the feature is on, the core implements it, and its present hook is armed.

## on_paint and remove

```cpp
// void(SwoftyPaint *paint, void *user)
uint32_t token = 0;
ov::on_paint(paint, /*user*/ nullptr, &token); // ok; unavailable while the feature is off; invalid for a null fn
ov::remove(token);                             // ok, or invalid for an unknown token
```

```text
[overlay] api: w1.overlay-probe on_paint -> ok (token 1)
```

Use the draw calls, `size` and `measure` only inside a paint callback, with that callback's `paint` handle. Your callback runs on the render thread, so it must return at once.

## Shapes

```cpp
ov::rect(p, x, y, w, h, rgba);                       // filled rectangle
ov::rect_outline(p, x, y, w, h, thickness, rgba);    // rectangle outline
ov::line(p, x0, y0, x1, y1, thickness, rgba);        // line segment
```

## Text

```cpp
ov::text(p, 60, 60, 32, 0xffffffffu, "Lap 12/57"); // font_size 32: glyphs 32 px tall, (60, 60) top-left

float tw = 0, th = 0;
ov::measure(p, 24, "Box this lap", &tw, &th);      // the bounds the text would occupy at 24 px
ov::text(p, w - tw - 20, 20, 24, 0xffffffffu, "Box this lap"); // right-aligned
```

Text uses the built-in font, a 5×8 pixel font with ASCII and Latin-1 (U+0020 to U+007E and U+00A0 to U+00FF), scaled to `font_size` pixels tall. Other characters draw as `?`.

## Images

```cpp
SWOFTY_ON_LOAD
{
    // outside paint: a path relative to swofty/addons/<your addon id>/
    ov::load_image("logo", "images/logo.png"); // ok; invalid for an empty id or a missing file
}

static void paint(SwoftyPaint *p, void *)
{
    ov::image(p, "logo", 20, 20, 128, 64);              // tint 0xffffffff by default
    ov::image(p, "logo", 20, 100, 128, 64, 0xffffff80); // multiplied by the tint: half transparent
}
```

`load_image(id, file)` registers a texture from a file in your addon folder under `id`, and the id stays loaded for the lifetime of the process. The file must be a relative path inside `swofty/addons/<id>/` (no leading `/`, no `:`, no `..`), and your DLL must belong to an addon that `swofty/settings.json` lists. `image(paint, id, x, y, w, h, tint)` returns `invalid` for an unknown id.

Current builds draw nothing for `image()`. The core says so when it loads one:

```text
[overlay] api: author.my-addon load_image logo (swofty/addons/author.my-addon/images/logo.png) -> ok (slot 0, not painted in this build)
[overlay] api: author.my-addon load_image logo -> invalid: swofty/addons/author.my-addon/images/logo.png is missing
```

## Availability

`overlay` is experimental. List it in your addon's `swofty.json`:

```json [swofty.json]
"experimental_features": ["overlay"]
```

While your addon is on, the build turns the overlay on for that profile, and the Addons page tags your addon **Uses experimental features**. Players install your addon and nothing else. This needs launcher 1.3.7 or later. With no addon listing it, the core logs:

```text
[overlay] the overlay feature is experimental and off; the HUD present hook is not armed
```

Rectangles, outlines, lines and text draw. `image()` draws nothing yet. See [feature gates](/design/native-libraries#feature-gates). `SWOFTY_OVERLAY_KILL` turns overlay parts off ([logging switches](./logging)).

## When the overlay draws

The core draws the overlay only where it knows the drawing is ordered with the game's own frame. Where it doesn't draw, paint callbacks don't run, and `swofty/loader.log` says why on an `[overlay]` line.

- **A swapchain the game created after the core loaded:** draws, on the command queue the swapchain was created with.

  ```text
  [overlay] drawing on swapchain 0000000064CB2270 with the direct queue 0000000064CB25A0 it was created with (1908x1028, format 28, 4 buffer(s), created by F1_25.exe <- kernel32.dll <- ntdll.dll)
  ```

- **A swapchain created before the core hooked swapchain creation:** doesn't draw on it.

  ```text
  [overlay] swapchain 00000000645C2340 was created before the core hooked swapchain creation, so its present queue is unknown; the overlay does not draw on it
  ```

- **A swapchain that a frame generation module (DLSS Frame Generation, FSR or XeSS frame generation) created or presents:** doesn't draw on it.

  ```text
  [overlay] frame generation (sl.dlss_g.dll) owns swapchain 0000000064CB2270; the overlay does not draw on it
  ```

- **A swapchain created through Streamline's frame generation hook** (`sl.dlss_g.dll`, every NVIDIA Streamline build of F1 25): draws while the game's `hardware_settings_config.xml` has `frame_gen mode="0"`; stops for the session once it reads anything else. The core re-reads the file at most every 2 seconds.

  ```text
  [overlay] the game's hardware settings now have frame generation on (frame_gen mode 1); the overlay stops drawing for this session
  ```

- **The GPU still holds the previous frames' overlay work:** skips drawing that frame. The per-minute line counts the skips.

  ```text
  [overlay] last 60 s: 138 frame(s) drawn, 0 skipped while the GPU still held the slot; GPU 0.015 ms per drawn frame (135 timed)
  ```

- **The GPU was removed:** stops for the rest of the session; the line says how long after the overlay last drew.

  ```text
  [overlay] the GPU was removed (present 0x887a0005, reason 0x887a0006) 0.4 s after the overlay last drew (swapchain by F1_25.exe <- kernel32.dll <- ntdll.dll); the overlay stops drawing
  ```

## HDR output

Colours are `0xRRGGBBAA` in sRGB. On HDR output the core converts them, so overlay white matches the game's own HUD white (350 nits by default; `SWOFTY_OVERLAY_NITS` sets 80 to 1000):

| Back buffer | Output |
|---|---|
| R10G10B10A2 with the HDR10 colour space (`SetColorSpace1`), or with no colour space set while the game's `hardware_settings_config.xml` has `hdr mode` above 0 | BT.2020 PQ |
| R16G16B16A16_FLOAT | Linear scRGB |
| Anything else | sRGB as given |

The core logs the output it picked for each swapchain:

```text
[overlay] swapchain 0000000064CB2270 output: SDR, paper white 350 nits (format 28, no SetColorSpace1 seen)
```
