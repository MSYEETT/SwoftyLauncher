#pragma once

#include "bc7.hpp"

#include <cstdint>
#include <cstring>
#include <set>
#include <string>
#include <vector>

// Growing one stock logo texture array by slices: the three fragments of an array (the image header, the BC7 pixel
// data of every slice, and the view with its slice count) and the key list in overrides.uidb, as
// classic-seasons/tools/logos.py does it. The array is checked the same way (BC7, one mip, one slice of w*h bytes
// per key, the view's count equal to the slice count) and a key that is listed already is refused.

namespace swofty_core::apply::content::logo_array {

inline constexpr uint32_t FORMAT_BC7 = 70;
inline constexpr size_t HEAD_FORMAT = 8, HEAD_MIPS = 24, HEAD_COUNT = 28, HEAD_SIZE = 32, VIEW_COUNT = 0x14;

struct Stock {
    std::string head, data, view;
    std::vector<std::string> names;
};

struct Slice {
    std::string key, bc7;
};

struct Shape {
    uint32_t width = 0, height = 0, count = 0;
    size_t slice_bytes() const { return size_t(width) * height; }
};

inline uint32_t le32(const std::string &s, size_t at)
{
    uint32_t v = 0;
    std::memcpy(&v, s.data() + at, 4);
    return v;
}

inline void put32(std::string &s, size_t at, uint32_t v)
{
    std::memcpy(&s[at], &v, 4);
}

inline bool shape_of(const Stock &stock, Shape &out, std::string &why)
{
    if (stock.head.size() < HEAD_SIZE || stock.view.size() < VIEW_COUNT + 4)
        return why = "the stock fragments are shorter than an array header", false;
    uint32_t fmt = le32(stock.head, HEAD_FORMAT), w = le32(stock.head, HEAD_FORMAT + 4), h = le32(stock.head, HEAD_FORMAT + 8);
    uint32_t mips = le32(stock.head, HEAD_MIPS), count = le32(stock.head, HEAD_COUNT);
    if (fmt != FORMAT_BC7 || mips != 1)
        return why = "unexpected stock array (format " + std::to_string(fmt) + ", " + std::to_string(mips) + " mips)", false;
    if (!w || !h || w % 4 || h % 4)
        return why = "the stock array is " + std::to_string(w) + "x" + std::to_string(h) + ", not a multiple of 4", false;
    if (stock.data.size() != size_t(count) * w * h || stock.names.size() != count || le32(stock.view, VIEW_COUNT) != count)
        return why = "the stock array holds " + std::to_string(stock.data.size()) + " byte(s), " +
                     std::to_string(stock.names.size()) + " key(s) and a view count of " +
                     std::to_string(le32(stock.view, VIEW_COUNT)) + " for " + std::to_string(count) + " slice(s)",
               false;
    out = {w, h, count};
    return true;
}

inline bool grow(const Stock &stock, const std::vector<Slice> &add, Stock &out, std::string &why)
{
    Shape shape;
    if (!shape_of(stock, shape, why))
        return false;
    std::set<std::string> taken(stock.names.begin(), stock.names.end());
    for (auto &s : add) {
        if (!taken.insert(s.key).second)
            return why = "the array already lists " + s.key, false;
        if (s.bc7.size() != shape.slice_bytes())
            return why = s.key + " slice is " + std::to_string(s.bc7.size()) + " bytes, not " + std::to_string(shape.slice_bytes()),
                   false;
    }
    out = stock;
    uint32_t total = shape.count + uint32_t(add.size());
    put32(out.head, HEAD_COUNT, total);
    put32(out.view, VIEW_COUNT, total);
    for (auto &s : add) {
        out.data += s.bc7;
        out.names.push_back(s.key);
    }
    return true;
}

inline bool slice_from_rgba(const Shape &shape, const uint8_t *rgba, uint32_t w, uint32_t h, std::string &out, std::string &why)
{
    if (w != shape.width || h != shape.height)
        return why = "the image is " + std::to_string(w) + "x" + std::to_string(h) + ", the array's slices are " +
                     std::to_string(shape.width) + "x" + std::to_string(shape.height),
               false;
    out = bc7::encode_logo(rgba, int(w), int(h));
    return true;
}

}
