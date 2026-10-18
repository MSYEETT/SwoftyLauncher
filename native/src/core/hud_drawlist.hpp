#pragma once
#include <cstdint>
#include <string>
#include <string_view>
#include <vector>

#include "swofty_api.h"

namespace hud {

enum class Op : uint8_t { rect, rect_outline, line, text, image };

struct Cmd {
    Op op;
    float x, y, w, h;
    float thickness;
    float font_size;
    uint32_t color;
    int image;
    std::string text;
};

struct Frame {
    float width = 0, height = 0;
    std::vector<Cmd> cmds;
};

constexpr float FONT_CELL = 8.0f;
constexpr float FONT_ADVANCE = 6.0f;

inline size_t glyphs(std::string_view s)
{
    size_t n = 0;
    for (unsigned char c : s)
        if ((c & 0xc0) != 0x80)
            n++;
    return n;
}

inline float text_width(float font_size, std::string_view s)
{
    return glyphs(s) * font_size * (FONT_ADVANCE / FONT_CELL);
}

inline void measure(float font_size, std::string_view s, float *width, float *height)
{
    if (width)
        *width = text_width(font_size, s);
    if (height)
        *height = font_size;
}

inline bool rgba_visible(uint32_t rgba)
{
    return (rgba & 0xff) != 0;
}

inline void rect(Frame &f, float x, float y, float w, float h, uint32_t rgba)
{
    if (w <= 0 || h <= 0 || !rgba_visible(rgba))
        return;
    f.cmds.push_back({Op::rect, x, y, w, h, 0, 0, rgba, -1, {}});
}

inline void rect_outline(Frame &f, float x, float y, float w, float h, float thickness, uint32_t rgba)
{
    if (w <= 0 || h <= 0 || thickness <= 0 || !rgba_visible(rgba))
        return;
    f.cmds.push_back({Op::rect_outline, x, y, w, h, thickness, 0, rgba, -1, {}});
}

inline void line(Frame &f, float x0, float y0, float x1, float y1, float thickness, uint32_t rgba)
{
    if (thickness <= 0 || !rgba_visible(rgba) || (x0 == x1 && y0 == y1))
        return;
    f.cmds.push_back({Op::line, x0, y0, x1, y1, thickness, 0, rgba, -1, {}});
}

inline void text(Frame &f, float x, float y, float font_size, uint32_t rgba, std::string_view s)
{
    if (font_size <= 0 || !rgba_visible(rgba) || s.empty())
        return;
    f.cmds.push_back({Op::text, x, y, 0, 0, 0, font_size, rgba, -1, std::string(s)});
}

inline void image(Frame &f, int slot, float x, float y, float w, float h, uint32_t tint)
{
    if (slot < 0 || w <= 0 || h <= 0 || !rgba_visible(tint))
        return;
    f.cmds.push_back({Op::image, x, y, w, h, 0, 0, tint, slot, {}});
}

struct Paintsub {
    uint32_t token;
    SwoftyPaintFn fn;
    void *user;
    std::string owner;
};

class Registry {
  public:
    uint32_t add(SwoftyPaintFn fn, void *user, std::string owner)
    {
        if (!fn)
            return 0;
        uint32_t token = next_++;
        subs_.push_back({token, fn, user, std::move(owner)});
        return token;
    }

    bool remove(uint32_t token)
    {
        for (size_t i = 0; i < subs_.size(); i++)
            if (subs_[i].token == token) {
                subs_.erase(subs_.begin() + i);
                return true;
            }
        return false;
    }

    size_t count() const { return subs_.size(); }

    const std::vector<Paintsub> &subs() const { return subs_; }

    int intern_image(const std::string &id)
    {
        for (size_t i = 0; i < images_.size(); i++)
            if (images_[i] == id)
                return int(i);
        images_.push_back(id);
        return int(images_.size() - 1);
    }

    int find_image(const std::string &id) const
    {
        for (size_t i = 0; i < images_.size(); i++)
            if (images_[i] == id)
                return int(i);
        return -1;
    }

    size_t image_count() const { return images_.size(); }

  private:
    std::vector<Paintsub> subs_;
    std::vector<std::string> images_;
    uint32_t next_ = 1;
};

inline void build(const std::vector<Paintsub> &subs, Frame &f, float width, float height)
{
    f.width = width;
    f.height = height;
    f.cmds.clear();
    for (const auto &s : subs)
        if (s.fn)
            s.fn(reinterpret_cast<SwoftyPaint *>(&f), s.user);
}

}
