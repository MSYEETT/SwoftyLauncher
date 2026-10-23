#pragma once
#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

// Pure logic of the era picker addon row (era_picker.cpp): the box check, the box order, the image
// default and the press guard. No game calls, so the unit test runs on the host.

namespace runtime::era {

constexpr const char *HOST = "swofty_f1";
constexpr const char *SELECTED_ADDON_TILE = "selected_addon_tile";
constexpr const char *ADDON_TILES_VISIBLE = "addon_tiles_visible";
constexpr const char *IMAGE_ARRAY = "#overrides.home_screen.tiles_solo_6x6";
constexpr size_t MAX_BOXES = 64;
constexpr size_t LIST_NAME_MAX = 63;
constexpr size_t TEXT_MAX = 255;

constexpr uint32_t djb2(const char *s)
{
    uint32_t h = 5381;
    while (*s)
        h = h * 33 + uint8_t(*s++);
    return h;
}

constexpr uint32_t ADDON_TILES = djb2("addon_tiles");
constexpr uint32_t ADDON_PRESS = djb2("addon_select_pressed");
constexpr uint32_t SELECT_PRESS = djb2("select_pressed");
static_assert(ADDON_TILES == 0xf2614e2b && ADDON_PRESS == 0x8fb68dff && SELECT_PRESS == 0x2b0357a);

struct Box {
    std::string entrant_list, title, image;
    int32_t order = 0;
    std::string owner;
    size_t seq = 0;
};

inline bool plain_name(const std::string &s)
{
    if (s.empty() || s.size() > LIST_NAME_MAX)
        return false;
    return std::all_of(s.begin(), s.end(), [](char c) {
        return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_';
    });
}

inline std::string check(Box &box, const std::vector<Box> &have)
{
    if (!plain_name(box.entrant_list))
        return "entrant_list must be 1 to 63 letters, digits or underscores";
    if (box.title.empty())
        box.title = "lng_" + box.entrant_list;
    if (box.title.size() > TEXT_MAX || box.image.size() > TEXT_MAX)
        return "title and image must be at most 255 characters";
    if (box.title.find('\0') != std::string::npos || box.image.find('\0') != std::string::npos)
        return "title and image must not hold NUL bytes";
    for (const auto &h : have)
        if (h.entrant_list == box.entrant_list)
            return "a box for this entrant list is already registered by " + h.owner;
    if (have.size() >= MAX_BOXES)
        return "the core holds at most 64 boxes";
    return {};
}

inline std::string image_of(const Box &box)
{
    return box.image.empty() ? std::string(IMAGE_ARRAY) + "[" + box.entrant_list + "]" : box.image;
}

inline std::vector<Box> ordered(std::vector<Box> boxes)
{
    std::stable_sort(boxes.begin(), boxes.end(), [](const Box &a, const Box &b) {
        return a.order != b.order ? a.order < b.order : a.seq < b.seq;
    });
    return boxes;
}

inline std::string press_refusal(bool same_view_model, bool nodes, size_t vector_tiles, size_t stock, size_t boxes)
{
    if (!same_view_model)
        return "the picker that raised it is not the one the addon row was built for";
    if (!nodes || !boxes)
        return "the addon row has no boxes";
    if (vector_tiles < stock + boxes)
        return "the tile vector holds " + std::to_string(vector_tiles) + " tiles, fewer than the " +
               std::to_string(stock) + " stock and " + std::to_string(boxes) + " addon tiles of the last build";
    return {};
}

}
