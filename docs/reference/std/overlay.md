---
title: Overlay
description: Draw a 2D overlay on top of F1 25 each frame with swofty/overlay.hpp.
---

# Overlay

You draw an immediate-mode 2D overlay on top of the game by registering a paint callback. Once per presented frame, the core calls it with a `Paint` handle, then issues the draw calls you make through that handle.

| | |
|---|---|
| Header | `swofty/overlay.hpp` |
| Namespace | `swofty::overlay` |
| Table | `SwoftyApi::overlay` (`SwoftyOverlay`) |
| Feature | `overlay` — experimental, on while an addon lists it in `experimental_features` |
| API version | 5 |
| Thread | Render thread (the swapchain present path) for paint callbacks |

Position your overlay in framebuffer pixels, measured from the top-left. The drawable size changes with the window, so query it each frame. Colours use `0xRRGGBBAA`.

To check whether `on_paint`, `remove` or `load_image` succeeded, use the returned `swofty::overlay::Result` (`ok`, `late`, `unavailable`, `invalid`); see the [result codes](./#result-codes). The draw calls return nothing.

## Calls

| Call | Does |
|---|---|
| `available()` | `true` once the feature is on and implemented |
| `on_paint(fn, user = nullptr, token = nullptr)` | registers a per-frame callback and fills `token` |
| `remove(token)` | drops a paint callback; `invalid` for an unknown token |
| `size(paint, &w, &h)` | writes the drawable size in pixels |
| `rect(paint, x, y, w, h, rgba)` | filled rectangle |
| `rect_outline(paint, x, y, w, h, thickness, rgba)` | rectangle outline |
| `line(paint, x0, y0, x1, y1, thickness, rgba)` | line segment |
| `text(paint, x, y, font_size, rgba, utf8)` | text in the built-in font, a 5×8 pixel font with ASCII and Latin-1, scaled to `font_size` pixels tall; `(x, y)` is its top-left. Other characters draw as `?` |
| `measure(paint, font_size, utf8, &w, &h)` | writes the bounds the text would occupy |
| `load_image(id, file)` | loads a texture from a file in the addon folder under `id`; `invalid` for a bad id or missing file |
| `image(paint, id, x, y, w, h, tint = 0xffffffff)` | draws a loaded texture into the rectangle, multiplied by `tint`; `invalid` for an unknown id. Current builds draw nothing for it |

Your callback, `fn`, has the signature `void(SwoftyPaint *paint, void *user)`. Use the draw calls, `size` and `measure` only inside a paint callback, passing that callback's `paint` handle. To draw a texture, first call `load_image` outside paint. The id then stays loaded for the lifetime of the process.

## Example

To draw a translucent panel with a label at the top-left, register a callback like this:

```cpp [native/src/my_plugin.cpp]
#include <swofty/overlay.hpp>

namespace ov = swofty::overlay;

static void paint(SwoftyPaint *p, void *)
{
    float w = 0, h = 0;
    ov::size(p, &w, &h);
    ov::rect(p, 24, 24, 240, 52, 0x000000a0);
    ov::rect_outline(p, 24, 24, 240, 52, 2, 0xff2d9affu);
    ov::text(p, 36, 38, 24, 0xffffffffu, "Swofty overlay");
}

SWOFTY_ON_LOAD
{
    if (ov::available())
        ov::on_paint(paint);
}

SWOFTY_ADDON()
```

Then name your plugin in `operations.native` and list the overlay in `experimental_features`:

```json [swofty.json]
"experimental_features": ["overlay"],
"operations": {
  "native": [{ "dll": "native/my_plugin.dll" }]
}
```

## Availability

`overlay` is experimental. List it in your addon's `swofty.json`:

```json [swofty.json]
"experimental_features": ["overlay"]
```

While your addon is on, the build turns the overlay on for that profile, and the Addons page tags your addon **Uses experimental features**. Players install your addon and nothing else. This needs launcher 1.3.7 or later.

Rectangles, outlines, lines and text draw. `image()` draws nothing yet. See [feature gates](/design/native-libraries#feature-gates).

## When the overlay draws

The core draws the overlay only where it knows the drawing is ordered with the game's own frame. Where it doesn't draw, paint callbacks don't run, and `swofty/loader.log` says why on an `[overlay]` line.

| Case | Overlay |
|---|---|
| A swapchain the game created after the core loaded | Draws, on the command queue the swapchain was created with |
| A swapchain created before the core hooked swapchain creation | Doesn't draw on it |
| A swapchain that a frame generation module (DLSS Frame Generation, FSR or XeSS frame generation) created or presents | Doesn't draw on it |
| A swapchain created through Streamline's frame generation hook (`sl.dlss_g.dll`, every NVIDIA Streamline build of F1 25) | Draws while the game's `hardware_settings_config.xml` has `frame_gen mode="0"`; stops for the session once it reads anything else |
| The GPU still holds the previous frames' overlay work | Skips drawing that frame |
| The GPU was removed within 10 s of an overlay draw | Stops, and writes `swofty/overlay-gpu-removed.json` |
| `swofty/overlay-gpu-removed.json` exists at launch | Off for the session: `available()` is `false` and `on_paint` returns `unavailable` |

Deleting `swofty/overlay-gpu-removed.json` turns the overlay back on at the next launch. The launcher shows a notice while the file exists, with **Turn the overlay back on**, which deletes it; `swofty overlay reset` does the same. Rollback and Restore game folder keep the file. Colours are not converted for HDR output.
