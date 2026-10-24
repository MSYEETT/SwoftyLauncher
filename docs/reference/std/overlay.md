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
| `text(paint, x, y, font_size, rgba, utf8)` | text in the built-in font in a box `font_size` pixels tall; `(x, y)` is its top-left. `\n` starts a new line. Characters the font lacks draw as `?` |
| `measure(paint, font_size, utf8, &w, &h)` | writes the bounds the text would occupy |
| `load_image(id, file)` | loads a PNG or uncompressed 32-bit DDS from the addon folder under `id`; `invalid` for a bad id, a missing or unreadable file, or an image over 4096 pixels on a side |
| `image(paint, id, x, y, w, h, tint = 0xffffffff)` | draws a loaded texture into the rectangle, multiplied by `tint`; `invalid` for an unknown id |

## Text

The built-in font is Titillium Web SemiBold ([SIL Open Font License 1.1](https://openfontlicense.org)), drawn from a signed distance field atlas, so it stays sharp from 12 to 72 pixels and beyond. It covers ASCII, Latin-1, curly quotes, dashes, the bullet, the ellipsis, `€` and `™`, and applies the font's kerning. `font_size` is the font's em size in pixels and the height of one line box; the capitals and descenders sit centred in it. Lines after the first advance by `line_height × font_size`.

## v2 calls

A core that exports `SwoftyCoreOverlayVersion` (overlay version 2) implements these calls. `v2()` returns `true` there. On an older core, `rect_rounded`, `rect_rounded_outline` and `gradient` draw the nearest v1 rectangle, `polyline` draws v1 lines, styled `text` and `measure` use the v1 calls, and the other calls draw nothing.

| Call | Does |
|---|---|
| `v2()` | `true` when the core implements the v2 calls |
| `rect_rounded(paint, x, y, w, h, radius, rgba)` | anti-aliased filled rectangle with rounded corners; a radius over half the shorter side makes a pill or circle |
| `rect_rounded_outline(paint, x, y, w, h, radius, thickness, rgba)` | its outline, drawn inside the rectangle |
| `gradient(paint, x, y, w, h, from, to, horizontal = false, radius = 0)` | rectangle filled with a linear gradient, top to bottom or left to right; colours interpolate per channel |
| `circle(paint, cx, cy, radius, rgba)` | anti-aliased filled circle |
| `circle_outline(paint, cx, cy, radius, thickness, rgba)` | circle outline, drawn inside the radius |
| `arc(paint, cx, cy, radius, from, to, thickness, rgba)` | arc stroke centred on the radius. Angles are radians, 0 points right and angles grow clockwise on screen. A sweep of 2π is a closed ring |
| `polyline(paint, xy, count, thickness, rgba, closed = false)` | line strip through `count` points (`xy` is `x0, y0, x1, y1, …`) with mitred joins |
| `polygon(paint, xy, count, rgba)` | filled convex polygon |
| `text(paint, x, y, style, utf8)` | text laid out by a `TextStyle` and anchored at `(x, y)` by its alignment |
| `measure(paint, style, utf8, &w, &h)` | the widest line and the height of all lines |
| `text_in(paint, rect, style, utf8)` | text aligned inside a `Rect` |
| `image(paint, id, x, y, w, h, u0, v0, u1, v1, tint = 0xffffffff)` | draws the part `(u0, v0)`–`(u1, v1)` of a loaded texture, in 0–1 texture coordinates |
| `image_nine(paint, id, x, y, w, h, left, top, right, bottom, scale = 1, tint = 0xffffffff)` | nine-slice panel: the corners (insets in texture pixels) keep their size times `scale`, the edges stretch along one axis and the centre along both. Insets shrink when the panel is smaller than them |
| `image_size(id, &w, &h)` | pixel size of a loaded texture |
| `push_clip(paint, x, y, w, h)` / `pop_clip(paint)` | clips later drawing to a rectangle, intersected with the current clip. Under a transform the clip is the rectangle's screen bounds |
| `push_transform(paint, m)` / `pop_transform(paint)` | multiplies the current transform by `m = {a, b, c, d, e, f}`: `x' = a·x + c·y + e`, `y' = b·x + d·y + f` |
| `push_translate(paint, dx, dy)`, `push_scale(paint, sx, sy, cx = 0, cy = 0)`, `push_rotate(paint, angle, cx = 0, cy = 0)` | push a translation, a scale about `(cx, cy)` or a clockwise rotation about `(cx, cy)`; pop each with `pop_transform` |
| `push_opacity(paint, opacity)` / `pop_opacity(paint)` | multiplies the alpha of later drawing by `opacity` (0–1) |
| `ClipScope`, `TransformScope`, `OpacityScope` | push in the constructor and pop in the destructor |
| `safe_area(paint)` | the frame inset by 3.5% on each side, as a `Rect` |
| `ui_scale(paint)` | `min(width / 1920, height / 1080)`: multiply sizes laid out for 1920×1080 by it |

Strokes, rounded rectangles, circles, arcs and polygons have a one-pixel anti-aliased edge. A stroke of thickness `t` is opaque over `t - 1` pixels with a one-pixel fade on each side; a stroke under one pixel is a faded hairline. v1 `rect`, `rect_outline` and `line` keep their hard edges. Transforms apply to every call, v1 calls included; stroke thicknesses scale with the transform.

Each paint callback starts with no clip, no transform and full opacity, whatever the previous callback left pushed. Pop what you push inside the same callback. A stack holds 64 entries; a push past that is ignored along with its pop.

### TextStyle

| Field | Default | Meaning |
|---|---|---|
| `size` | 16 | font size in pixels: one line box tall |
| `rgba` | `0xffffffff` | text colour |
| `align` | `align::left \| align::top` | one of `align::left`, `align::center`, `align::right` combined with one of `align::top`, `align::middle`, `align::baseline`, `align::bottom`. Each line aligns to `x`; the block aligns to `y`, and `baseline` puts the first line's baseline on `y` |
| `tracking` | 0 | extra pixels after each glyph |
| `line_height` | 1.2 | line advance as a multiple of `size` |
| `outline`, `outline_px` | 0, 0 | outline colour and width in pixels; drawn when the colour's alpha is non-zero |
| `shadow`, `shadow_dx`, `shadow_dy`, `shadow_blur` | 0, 0, 0, 0 | drop shadow colour, offset and blur in pixels; drawn when the colour's alpha is non-zero |
| `max_width` | 0 | wraps at the last space that keeps a line inside it, mid-word when one word is wider; 0 does not wrap |

The C table reads a `SwoftyTextStyle` up to its `size` field.

## Limits

A frame holds up to 131,072 vertices across every paint callback; primitives past that are dropped, and `swofty/loader.log` says so once. The core holds up to 255 loaded images. Each image uploads to the GPU once, the first frame after `load_image`; loading the same `id` again replaces it.

## Callbacks

Your callback, `fn`, has the signature `void(SwoftyPaint *paint, void *user)`. Use the draw calls, `size` and `measure` only inside a paint callback, passing that callback's `paint` handle. To draw a texture, first call `load_image` outside paint. The id then stays loaded for the lifetime of the process. The build copies an image into the game folder only when its file or folder is listed in `files` on your `operations.native` entry, for example `{ "dll": "native/my_plugin.dll", "files": ["images"] }`; see [files in the game folder](/reference/addons/native-plugins#files-in-the-game-folder).

## Example

To draw a translucent panel with a label in the safe area, register a callback like this:

```cpp [native/src/my_plugin.cpp]
#include <swofty/overlay.hpp>

namespace ov = swofty::overlay;

static void paint(SwoftyPaint *p, void *)
{
    float s = ov::ui_scale(p);
    ov::Rect safe = ov::safe_area(p);
    ov::Rect panel{safe.x, safe.y, 260 * s, 52 * s};
    ov::rect_rounded(p, panel.x, panel.y, panel.w, panel.h, 10 * s, 0x000000a0);
    ov::rect_rounded_outline(p, panel.x, panel.y, panel.w, panel.h, 10 * s, 2 * s, 0xff2d9affu);
    ov::TextStyle label;
    label.size = 24 * s;
    label.align = ov::align::center | ov::align::middle;
    ov::text_in(p, panel, label, "Swofty overlay");
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

See [feature gates](/design/native-libraries#feature-gates).

The launcher can turn the overlay off for one addon: an addon id in `"overlay_off"` in `swofty/assets.json` gets `false` from `available()` and `unavailable` from `on_paint`, and the core skips its paint callbacks.

## When the overlay draws

The core draws the overlay only where it knows the drawing is ordered with the game's own frame. Where it doesn't draw, paint callbacks don't run, and `swofty/loader.log` says why on an `[overlay]` line.

| Case | Overlay |
|---|---|
| A swapchain the game created after the core loaded | Draws, on the command queue the swapchain was created with |
| A swapchain created before the core hooked swapchain creation | Doesn't draw on it |
| A swapchain that a frame generation module (DLSS Frame Generation, FSR or XeSS frame generation) created or presents | Doesn't draw on it |
| A swapchain created through Streamline's frame generation hook (`sl.dlss_g.dll`, every NVIDIA Streamline build of F1 25) | Draws while the game's `hardware_settings_config.xml` has `frame_gen mode="0"`; stops for the session once it reads anything else |
| The GPU still holds the previous frames' overlay work | Skips drawing that frame |
| The GPU was removed | Stops for the rest of the session; an `[overlay]` line says how long after the overlay last drew |

Colours are `0xRRGGBBAA` in sRGB. On HDR output the core converts them, so overlay white matches the game's own HUD white (350 nits by default, `SWOFTY_OVERLAY_NITS` to change it):

| Back buffer | Output |
|---|---|
| R10G10B10A2 with the HDR10 colour space (`SetColorSpace1`), or with no colour space set while the game's `hardware_settings_config.xml` has `hdr mode` above 0 | BT.2020 PQ |
| R16G16B16A16_FLOAT | Linear scRGB |
| Anything else | sRGB as given |
