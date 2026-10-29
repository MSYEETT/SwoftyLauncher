#pragma once
#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <map>
#include <memory>
#include <string>
#include <vector>

// Pure logic of the era picker addon row (era_picker.cpp): the box check, the box order, the image
// default, the press guard, the links of the row's two node handles and the DRS group swap of the host
// setup mode. No game calls, so the unit test runs on the host.

namespace runtime::era {

constexpr const char *HOST = "swofty_f1";
constexpr const char *SELECTED_ADDON_TILE = "selected_addon_tile";
constexpr const char *ADDON_TILES_VISIBLE = "addon_tiles_visible";
constexpr const char *IMAGE_ARRAY = "#overrides.home_screen.tiles_solo_6x6";
constexpr size_t MAX_BOXES = 64;
constexpr size_t LIST_NAME_MAX = 63;
constexpr size_t TEXT_MAX = 255;
constexpr size_t NODE_REFS = 0x24;
constexpr size_t NODE_HOLDERS = 0x28;
constexpr size_t HOLDER_PREV = 0x10;

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

struct Handle {
    uint8_t *node;
    void *next, *prev;
};
static_assert(offsetof(Handle, prev) == HOLDER_PREV);

inline void link(Handle &h, uint8_t *node)
{
    h = {node, *reinterpret_cast<void **>(node + NODE_HOLDERS), nullptr};
    if (h.next)
        *reinterpret_cast<void **>(static_cast<uint8_t *>(h.next) + HOLDER_PREV) = &h;
    *reinterpret_cast<void **>(node + NODE_HOLDERS) = &h;
    __atomic_fetch_add(reinterpret_cast<int16_t *>(node + NODE_REFS), int16_t(1), __ATOMIC_SEQ_CST);
}

// clear (FeTile_Clear_EraTileSet_Thunk in game) unlinks and releases both handles from the nodes they still
// hold, whichever view model those belong to, before the list handle is linked to this build's node.
template <class Set, class Clear> void relink(Set &set, uint8_t *list, Clear clear)
{
    if (set.list.node || set.selected.node)
        clear(set);
    set = {};
    link(set.list, list);
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

// A box's DRS rule (content.package rules.drs) applies through the host setup mode's DRS group. A game setup mode
// object keeps each string field as a holder whose +0x10 is the char * label (re/db structs/GameSetupMode.h); its
// +0x1e0 holder names the drs.drs_allowed_lap group the DRS system looks up at session load, at a full safety car
// end and at a red flag restart. The swap points that field at a holder of the rule's group label and remembers the
// game's holder; put_back restores it wherever the field still holds the swap's holder.
constexpr size_t MODE_NAME = 0xd8;
constexpr size_t MODE_DRS_GROUP = 0x1e0;
constexpr size_t HOLDER_TEXT = 0x10;
constexpr size_t DRS_LABEL_MAX = 63;

struct DrsHolder {
    void *unused[2] = {};
    const char *text = nullptr;
    uint8_t pad[0x40] = {};
};
static_assert(offsetof(DrsHolder, text) == HOLDER_TEXT);

class DrsSwaps {
public:
    // A holder per label, at a stable address for the life of the process.
    DrsHolder *holder(const std::string &label)
    {
        if (label.empty() || label.size() > DRS_LABEL_MAX || label.find('\0') != std::string::npos)
            return nullptr;
        auto &h = holders_[label];
        if (!h) {
            h = std::make_unique<Entry>();
            h->label = label;
            h->holder.text = h->label.c_str();
        }
        return &h->holder;
    }

    void swap(uint8_t *mode, DrsHolder *holder)
    {
        if (!mode || !holder)
            return;
        void *&field = *reinterpret_cast<void **>(mode + MODE_DRS_GROUP);
        if (field == holder)
            return;
        bool known = false;
        for (auto &s : swaps_)
            known = known || s.mode == mode;
        if (!known)
            swaps_.push_back({mode, field});
        field = holder;
    }

    size_t put_back()
    {
        size_t n = 0;
        for (auto &s : swaps_) {
            void *&field = *reinterpret_cast<void **>(s.mode + MODE_DRS_GROUP);
            if (!owns(field))
                continue;
            field = s.original;
            n++;
        }
        swaps_.clear();
        return n;
    }

    size_t active() const { return swaps_.size(); }

    bool owns(const void *holder) const
    {
        for (auto &[label, e] : holders_)
            if (&e->holder == holder)
                return true;
        return false;
    }

private:
    struct Entry {
        std::string label;
        DrsHolder holder;
    };
    struct Swap {
        uint8_t *mode;
        void *original;
    };
    std::map<std::string, std::unique_ptr<Entry>> holders_;
    std::vector<Swap> swaps_;
};

inline const char *holder_text(const void *holder)
{
    return holder ? *reinterpret_cast<const char *const *>(static_cast<const uint8_t *>(holder) + HOLDER_TEXT) : nullptr;
}

}
