#include "runtime.hpp"

#include <swofty/mixin.hpp>

// Stub for the overlay / draw lane. Every call returns unavailable, and the draw calls do
// nothing, until the lane hooks the D3D12 swapchain present and builds the immediate-mode
// draw list behind the Paint handle.

namespace {

int ov_available()
{
    return 0;
}

int ov_on_paint(SwoftyPaintFn, void *, uint32_t *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int ov_remove(uint32_t)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

void ov_size(const SwoftyPaint *, float *width, float *height)
{
    if (width)
        *width = 0;
    if (height)
        *height = 0;
}

void ov_rect(SwoftyPaint *, float, float, float, float, uint32_t)
{
}

void ov_rect_outline(SwoftyPaint *, float, float, float, float, float, uint32_t)
{
}

void ov_line(SwoftyPaint *, float, float, float, float, float, uint32_t)
{
}

void ov_text(SwoftyPaint *, float, float, float, uint32_t, const char *)
{
}

void ov_measure(const SwoftyPaint *, float, const char *, float *width, float *height)
{
    if (width)
        *width = 0;
    if (height)
        *height = 0;
}

int ov_load_image(const char *, const char *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int ov_image(SwoftyPaint *, const char *, float, float, float, float, uint32_t)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

const SwoftyOverlay table = {ov_available,  ov_on_paint, ov_remove,  ov_size,       ov_rect,       ov_rect_outline,
                             ov_line,       ov_text,     ov_measure, ov_load_image, ov_image};

}

namespace runtime {

const SwoftyOverlay *overlay_table()
{
    return &table;
}

}
