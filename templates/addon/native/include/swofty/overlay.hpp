// SPDX-License-Identifier: MIT
#pragma once
#include <windows.h>

#include <atomic>
#include <cmath>
#include <cstdint>
#include <string>
#include <string_view>
#include <swofty/mixin.hpp>

// Immediate-mode 2D overlay drawn on top of the game. Register a paint callback; the core
// invokes it once per presented frame, on the render thread, with a per-frame Paint handle.
// Issue draw calls through that handle. Coordinates are framebuffer pixels with the origin at
// the top-left; query the drawable size each frame. Colours are 0xRRGGBBAA. Load textures from
// the addon folder once (outside paint) and draw them by id.
//
// v2 calls (rounded rects, gradients, circles, arcs, polylines, styled text, image UVs and
// nine-slice, clip/transform/opacity stacks, safe area, UI scale) need a core that exports
// SwoftyCoreOverlayVersion; v2() says whether it does. On an older core the shape calls fall back
// to the nearest v1 call and the rest draw nothing.

namespace swofty::overlay {

enum class Result { ok, late, unavailable, invalid };

using Paint = SwoftyPaint;

namespace detail {

inline const SwoftyOverlay *table()
{
    return api && api->version >= 5 ? api->overlay : nullptr;
}

inline const SwoftyOverlay *table2()
{
    static std::atomic<int> found{-1};
    auto t = table();
    if (!t)
        return nullptr;
    int f = found.load();
    if (f < 0) {
        using VersionFn = uint32_t (*)(void);
        HMODULE core = GetModuleHandleW(L"swofty_core.dll");
        auto fn = core ? reinterpret_cast<VersionFn>(GetProcAddress(core, "SwoftyCoreOverlayVersion")) : nullptr;
        f = fn && fn() >= 2 ? 1 : 0;
        found = f;
    }
    return f ? t : nullptr;
}

inline Result result(int r)
{
    return r == SWOFTY_ASSET_OK ? Result::ok
           : r == SWOFTY_ASSET_LATE ? Result::late
           : r == SWOFTY_ASSET_UNAVAILABLE ? Result::unavailable
                                           : Result::invalid;
}

}

// The overlay library is usable (feature on and implemented by this core).
inline bool available()
{
    auto t = detail::table();
    return t && t->available();
}

// The core implements the v2 calls.
inline bool v2()
{
    return detail::table2() != nullptr;
}

// Register a per-frame paint callback. The token identifies it for remove; pass nullptr to
// ignore it.
inline Result on_paint(SwoftyPaintFn fn, void *user = nullptr, uint32_t *token = nullptr)
{
    auto t = detail::table();
    if (!t)
        return Result::unavailable;
    return detail::result(t->on_paint(fn, user, token));
}

// Drop a paint callback by its token.
inline Result remove(uint32_t token)
{
    auto t = detail::table();
    if (!t)
        return Result::unavailable;
    return detail::result(t->remove(token));
}

// Current drawable size in pixels.
inline void size(const Paint *paint, float *width, float *height)
{
    if (auto t = detail::table())
        t->size(paint, width, height);
}

// Filled rectangle.
inline void rect(Paint *paint, float x, float y, float w, float h, uint32_t rgba)
{
    if (auto t = detail::table())
        t->rect(paint, x, y, w, h, rgba);
}

// Rectangle outline of the given stroke thickness.
inline void rect_outline(Paint *paint, float x, float y, float w, float h, float thickness, uint32_t rgba)
{
    if (auto t = detail::table())
        t->rect_outline(paint, x, y, w, h, thickness, rgba);
}

// Line segment of the given stroke thickness.
inline void line(Paint *paint, float x0, float y0, float x1, float y1, float thickness, uint32_t rgba)
{
    if (auto t = detail::table())
        t->line(paint, x0, y0, x1, y1, thickness, rgba);
}

// Text in the built-in font at the given pixel size; the top-left of the text is (x, y).
inline void text(Paint *paint, float x, float y, float font_size, uint32_t rgba, std::string_view utf8)
{
    if (auto t = detail::table())
        t->text(paint, x, y, font_size, rgba, std::string(utf8).c_str());
}

// Bounds the text would occupy at the given pixel size.
inline void measure(const Paint *paint, float font_size, std::string_view utf8, float *width, float *height)
{
    if (auto t = detail::table())
        t->measure(paint, font_size, std::string(utf8).c_str(), width, height);
}

// Load a texture from a file in the addon folder and register it under an id. Call outside
// paint; the id is then available to image() for the lifetime of the process.
inline Result load_image(std::string_view id, std::string_view file)
{
    auto t = detail::table();
    if (!t)
        return Result::unavailable;
    return detail::result(t->load_image(std::string(id).c_str(), std::string(file).c_str()));
}

// Draw a loaded texture into the rectangle, multiplied by the tint colour (0xffffffff for none).
inline Result image(Paint *paint, std::string_view id, float x, float y, float w, float h, uint32_t tint = 0xffffffff)
{
    auto t = detail::table();
    if (!t)
        return Result::unavailable;
    return detail::result(t->image(paint, std::string(id).c_str(), x, y, w, h, tint));
}

inline const char *name(Result r)
{
    return r == Result::ok ? "ok" : r == Result::late ? "late" : r == Result::unavailable ? "unavailable" : "invalid";
}

// ----- v2 -----------------------------------------------------------------------------------------

namespace align {
constexpr uint32_t left = SWOFTY_TEXT_LEFT, center = SWOFTY_TEXT_CENTER, right = SWOFTY_TEXT_RIGHT;
constexpr uint32_t top = SWOFTY_TEXT_TOP, middle = SWOFTY_TEXT_MIDDLE, baseline = SWOFTY_TEXT_BASELINE,
                   bottom = SWOFTY_TEXT_BOTTOM;
}

// How text() lays out and draws a string. size is the font size in pixels: a line box that tall.
// align combines one horizontal and one vertical align:: value and places the text relative to (x, y).
// tracking adds pixels after each glyph; line_height is the line advance as a multiple of size;
// max_width wraps at spaces (0: no wrapping). An outline and a drop shadow draw when their colour's
// alpha is non-zero.
struct TextStyle {
    float size = 16;
    uint32_t rgba = 0xffffffff;
    uint32_t align = align::left | align::top;
    float tracking = 0;
    float line_height = 1.2f;
    uint32_t outline = 0;
    float outline_px = 0;
    uint32_t shadow = 0;
    float shadow_dx = 0, shadow_dy = 0, shadow_blur = 0;
    float max_width = 0;
};

struct Rect {
    float x = 0, y = 0, w = 0, h = 0;
};

namespace detail {

inline SwoftyTextStyle style(const TextStyle &s)
{
    SwoftyTextStyle c{};
    c.size = sizeof c;
    c.align = s.align;
    c.px = s.size;
    c.rgba = s.rgba;
    c.tracking = s.tracking;
    c.line_height = s.line_height;
    c.outline_rgba = s.outline;
    c.outline_px = s.outline_px;
    c.shadow_rgba = s.shadow;
    c.shadow_dx = s.shadow_dx;
    c.shadow_dy = s.shadow_dy;
    c.shadow_blur = s.shadow_blur;
    c.max_width = s.max_width;
    return c;
}

}

// Filled rectangle with corners of the given radius, anti-aliased.
inline void rect_rounded(Paint *paint, float x, float y, float w, float h, float radius, uint32_t rgba)
{
    if (auto t = detail::table2())
        t->rect_rounded(paint, x, y, w, h, radius, rgba);
    else
        rect(paint, x, y, w, h, rgba);
}

// Outline of a rounded rectangle, drawn inside its bounds.
inline void rect_rounded_outline(Paint *paint, float x, float y, float w, float h, float radius, float thickness,
                                 uint32_t rgba)
{
    if (auto t = detail::table2())
        t->rect_rounded_outline(paint, x, y, w, h, radius, thickness, rgba);
    else
        rect_outline(paint, x, y, w, h, thickness, rgba);
}

// Rectangle (rounded when radius > 0) filled with a linear gradient from one colour to another, top to
// bottom, or left to right when horizontal.
inline void gradient(Paint *paint, float x, float y, float w, float h, uint32_t from, uint32_t to,
                     bool horizontal = false, float radius = 0)
{
    if (auto t = detail::table2())
        t->rect_gradient(paint, x, y, w, h, radius, from, to,
                         horizontal ? SWOFTY_GRADIENT_HORIZONTAL : SWOFTY_GRADIENT_VERTICAL);
    else
        rect(paint, x, y, w, h, from);
}

inline void circle(Paint *paint, float cx, float cy, float radius, uint32_t rgba)
{
    if (auto t = detail::table2())
        t->circle(paint, cx, cy, radius, rgba);
}

// Circle outline, drawn inside the radius.
inline void circle_outline(Paint *paint, float cx, float cy, float radius, float thickness, uint32_t rgba)
{
    if (auto t = detail::table2())
        t->circle_outline(paint, cx, cy, radius, thickness, rgba);
}

// Arc stroke centred on the radius, from angle `from` to `to` in radians: 0 points right and angles grow
// clockwise on screen. A sweep of 2*pi or more is a closed ring.
inline void arc(Paint *paint, float cx, float cy, float radius, float from, float to, float thickness, uint32_t rgba)
{
    if (auto t = detail::table2())
        t->arc(paint, cx, cy, radius, from, to, thickness, rgba);
}

// Connected line strip through count points (xy holds x0, y0, x1, y1, ...), with mitred joins.
inline void polyline(Paint *paint, const float *xy, uint32_t count, float thickness, uint32_t rgba, bool closed = false)
{
    if (auto t = detail::table2()) {
        t->polyline(paint, xy, count, thickness, rgba, closed ? SWOFTY_POLYLINE_CLOSED : 0);
        return;
    }
    for (uint32_t i = 0; xy && i + 1 < count; i++)
        line(paint, xy[2 * i], xy[2 * i + 1], xy[2 * i + 2], xy[2 * i + 3], thickness, rgba);
}

// Filled convex polygon through count points.
inline void polygon(Paint *paint, const float *xy, uint32_t count, uint32_t rgba)
{
    if (auto t = detail::table2())
        t->polygon(paint, xy, count, rgba);
}

// Styled text anchored at (x, y) by style.align.
inline void text(Paint *paint, float x, float y, const TextStyle &style, std::string_view utf8)
{
    if (auto t = detail::table2()) {
        SwoftyTextStyle s = detail::style(style);
        t->text_styled(paint, x, y, &s, std::string(utf8).c_str());
        return;
    }
    float w = 0, h = 0;
    measure(paint, style.size, utf8, &w, &h);
    uint32_t ha = style.align & 0x0f, va = style.align & 0xf0;
    x -= ha == align::center ? w / 2 : ha == align::right ? w : 0;
    y -= va == align::middle ? h / 2 : va == align::bottom ? h : va == align::baseline ? h * 0.75f : 0;
    text(paint, x, y, style.size, style.rgba, utf8);
}

// Bounds styled text would occupy: the widest line and the height of all lines.
inline void measure(const Paint *paint, const TextStyle &style, std::string_view utf8, float *width, float *height)
{
    if (auto t = detail::table2()) {
        SwoftyTextStyle s = detail::style(style);
        t->measure_styled(paint, &s, std::string(utf8).c_str(), width, height);
        return;
    }
    measure(paint, style.size, utf8, width, height);
}

// Styled text aligned inside a rectangle by style.align (left/center/right, top/middle/bottom).
inline void text_in(Paint *paint, const Rect &r, const TextStyle &style, std::string_view utf8)
{
    uint32_t ha = style.align & 0x0f, va = style.align & 0xf0;
    float x = ha == align::center ? r.x + r.w / 2 : ha == align::right ? r.x + r.w : r.x;
    float y = va == align::middle ? r.y + r.h / 2 : va == align::bottom ? r.y + r.h : r.y;
    TextStyle s = style;
    if (va == align::baseline)
        s.align = ha | align::middle;
    text(paint, x, y, s, utf8);
}

// Draw the part (u0, v0)-(u1, v1) of a loaded texture, in 0..1 texture coordinates, into the rectangle.
inline Result image(Paint *paint, std::string_view id, float x, float y, float w, float h, float u0, float v0, float u1,
                    float v1, uint32_t tint = 0xffffffff)
{
    auto t = detail::table2();
    if (!t)
        return Result::unavailable;
    return detail::result(t->image_uv(paint, std::string(id).c_str(), x, y, w, h, u0, v0, u1, v1, tint));
}

// Nine-slice panel: the corners of a loaded texture (left/top/right/bottom insets in texture pixels)
// keep their size times scale, the edges stretch along one axis and the centre along both.
inline Result image_nine(Paint *paint, std::string_view id, float x, float y, float w, float h, float left, float top,
                         float right, float bottom, float scale = 1, uint32_t tint = 0xffffffff)
{
    auto t = detail::table2();
    if (!t)
        return Result::unavailable;
    return detail::result(
        t->image_nine(paint, std::string(id).c_str(), x, y, w, h, left, top, right, bottom, scale, tint));
}

// Pixel size of a loaded texture.
inline Result image_size(std::string_view id, float *width, float *height)
{
    auto t = detail::table2();
    if (!t)
        return Result::unavailable;
    return detail::result(t->image_size(std::string(id).c_str(), width, height));
}

// Clip later drawing to a rectangle (intersected with the current clip; under a transform, the
// rectangle's screen bounds). Pair every push with a pop inside the same paint callback.
inline void push_clip(Paint *paint, float x, float y, float w, float h)
{
    if (auto t = detail::table2())
        t->push_clip(paint, x, y, w, h);
}

inline void pop_clip(Paint *paint)
{
    if (auto t = detail::table2())
        t->pop_clip(paint);
}

// Multiply the current transform by m = {a, b, c, d, e, f}: x' = a*x + c*y + e, y' = b*x + d*y + f.
inline void push_transform(Paint *paint, const float m[6])
{
    if (auto t = detail::table2())
        t->push_transform(paint, m);
}

inline void pop_transform(Paint *paint)
{
    if (auto t = detail::table2())
        t->pop_transform(paint);
}

inline void push_translate(Paint *paint, float dx, float dy)
{
    const float m[6] = {1, 0, 0, 1, dx, dy};
    push_transform(paint, m);
}

inline void push_scale(Paint *paint, float sx, float sy, float cx = 0, float cy = 0)
{
    const float m[6] = {sx, 0, 0, sy, cx - sx * cx, cy - sy * cy};
    push_transform(paint, m);
}

// Rotate by angle radians (clockwise on screen) about (cx, cy).
inline void push_rotate(Paint *paint, float angle, float cx = 0, float cy = 0)
{
    float c = std::cos(angle), s = std::sin(angle);
    const float m[6] = {c, s, -s, c, cx - c * cx + s * cy, cy - s * cx - c * cy};
    push_transform(paint, m);
}

// Multiply the alpha of later drawing by opacity (0..1).
inline void push_opacity(Paint *paint, float opacity)
{
    if (auto t = detail::table2())
        t->push_opacity(paint, opacity);
}

inline void pop_opacity(Paint *paint)
{
    if (auto t = detail::table2())
        t->pop_opacity(paint);
}

// The part of the frame a HUD should stay inside: the frame inset by 3.5% on each side.
inline Rect safe_area(const Paint *paint)
{
    Rect r;
    if (auto t = detail::table2()) {
        t->safe_area(paint, &r.x, &r.y, &r.w, &r.h);
    } else {
        size(paint, &r.w, &r.h);
        r.x = std::round(r.w * 0.035f);
        r.y = std::round(r.h * 0.035f);
        r.w -= 2 * r.x;
        r.h -= 2 * r.y;
    }
    return r;
}

// Scale from a 1920x1080 layout to this frame: min(width / 1920, height / 1080).
inline float ui_scale(const Paint *paint)
{
    if (auto t = detail::table2())
        return t->ui_scale(paint);
    float w = 0, h = 0;
    size(paint, &w, &h);
    return w > 0 && h > 0 ? (w / 1920 < h / 1080 ? w / 1920 : h / 1080) : 1;
}

// Scoped pushes: each pops what it pushed when it goes out of scope.
struct ClipScope {
    Paint *paint;
    ClipScope(Paint *p, float x, float y, float w, float h) : paint(p) { push_clip(p, x, y, w, h); }
    ~ClipScope() { pop_clip(paint); }
    ClipScope(const ClipScope &) = delete;
    ClipScope &operator=(const ClipScope &) = delete;
};

struct TransformScope {
    Paint *paint;
    TransformScope(Paint *p, const float m[6]) : paint(p) { push_transform(p, m); }
    ~TransformScope() { pop_transform(paint); }
    TransformScope(const TransformScope &) = delete;
    TransformScope &operator=(const TransformScope &) = delete;
};

struct OpacityScope {
    Paint *paint;
    OpacityScope(Paint *p, float opacity) : paint(p) { push_opacity(p, opacity); }
    ~OpacityScope() { pop_opacity(paint); }
    OpacityScope(const OpacityScope &) = delete;
    OpacityScope &operator=(const OpacityScope &) = delete;
};

}
