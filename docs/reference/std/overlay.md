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
| Feature | `overlay` — experimental, off by default |
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
| `text(paint, x, y, font_size, rgba, utf8)` | text in the built-in font; `(x, y)` is its top-left |
| `measure(paint, font_size, utf8, &w, &h)` | writes the bounds the text would occupy |
| `load_image(id, file)` | loads a texture from a file in the addon folder under `id`; `invalid` for a bad id or missing file |
| `image(paint, id, x, y, w, h, tint = 0xffffffff)` | draws a loaded texture into the rectangle, multiplied by `tint`; `invalid` for an unknown id |

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

Then declare the capability on your plugin's manifest entry:

```json [swofty.json]
"operations": {
  "native": [{ "dll": "native/my_plugin.dll", "capabilities": ["overlay"] }]
}
```

## Availability

`overlay` is experimental. To enable it, you need to list `overlay` under `"features"` in `swofty/assets.json`; see [feature gates](/design/native-libraries#feature-gates). The overlay engine implementation has yet to land, though, so on current builds `available()` returns `false` and your draw calls do nothing. See [the native-libraries design](/design/native-libraries#_2-overlay-draw).
