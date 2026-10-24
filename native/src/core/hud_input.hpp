#pragma once
#include <cstdint>
#include <cstring>
#include <string>
#include <vector>

#include "swofty_api.h"

// Overlay input: window messages from the game's window procedure are queued as events and folded
// into one snapshot per presented frame. One addon at a time can hold input focus; while it does, the
// core keeps mouse and keyboard messages from the game and draws a cursor.

namespace hud::input {

enum class Ev : uint8_t { move, down, up, wheel, chr, key_down, key_up };

struct Event {
    Ev kind;
    uint32_t a;
    float x, y;
};

struct Snapshot {
    float x = -1, y = -1;
    float wheel = 0;
    uint32_t buttons = 0, pressed = 0, released = 0;
    std::vector<uint32_t> chars;
    uint8_t keys_down[32] = {};
    uint8_t keys_pressed[32] = {};
};

inline void set_bit(uint8_t *bits, uint32_t vk, bool on)
{
    if (vk >= 256)
        return;
    if (on)
        bits[vk >> 3] |= uint8_t(1u << (vk & 7));
    else
        bits[vk >> 3] &= uint8_t(~(1u << (vk & 7)));
}

inline bool bit(const uint8_t *bits, uint32_t vk)
{
    return vk < 256 && (bits[vk >> 3] >> (vk & 7) & 1);
}

// Folds the frame's events into s: edges and text start empty, held buttons and keys carry over, and
// positions arrive in client pixels and leave in framebuffer pixels.
inline void fold(Snapshot &s, const std::vector<Event> &events, float sx, float sy)
{
    s.wheel = 0;
    s.pressed = s.released = 0;
    s.chars.clear();
    memset(s.keys_pressed, 0, sizeof s.keys_pressed);
    for (const Event &e : events) {
        switch (e.kind) {
        case Ev::move:
            s.x = e.x * sx;
            s.y = e.y * sy;
            break;
        case Ev::down:
            s.x = e.x * sx;
            s.y = e.y * sy;
            s.buttons |= e.a;
            s.pressed |= e.a;
            break;
        case Ev::up:
            s.x = e.x * sx;
            s.y = e.y * sy;
            s.buttons &= ~e.a;
            s.released |= e.a;
            break;
        case Ev::wheel:
            s.wheel += e.x;
            break;
        case Ev::chr:
            if (e.a >= 0x20 && e.a != 0x7f && s.chars.size() < 16)
                s.chars.push_back(e.a);
            break;
        case Ev::key_down:
            if (!bit(s.keys_down, e.a))
                set_bit(s.keys_pressed, e.a, true);
            set_bit(s.keys_down, e.a, true);
            break;
        case Ev::key_up:
            set_bit(s.keys_down, e.a, false);
            break;
        }
    }
}

// Joins UTF-16 surrogate pairs from WM_CHAR into code points; returns 0 while half a pair is pending.
struct Utf16 {
    uint32_t high = 0;

    uint32_t push(uint32_t unit)
    {
        if (unit >= 0xd800 && unit < 0xdc00) {
            high = unit;
            return 0;
        }
        if (unit >= 0xdc00 && unit < 0xe000) {
            uint32_t cp = high ? 0x10000 + ((high - 0xd800) << 10) + (unit - 0xdc00) : 0xfffd;
            high = 0;
            return cp;
        }
        high = 0;
        return unit;
    }
};

// Which addon holds focus. A holder that has not painted for timeout_ms loses it.
struct Focus {
    std::string owner;
    uint64_t last_paint = 0;

    bool take(const std::string &who, bool on, uint64_t now)
    {
        if (on) {
            owner = who;
            last_paint = now;
            return true;
        }
        if (owner == who) {
            owner.clear();
            return true;
        }
        return false;
    }

    void painted(const std::string &who, uint64_t now)
    {
        if (!owner.empty() && owner == who)
            last_paint = now;
    }

    bool expire(uint64_t now, uint64_t timeout_ms)
    {
        if (owner.empty() || now - last_paint < timeout_ms)
            return false;
        owner.clear();
        return true;
    }

    bool held() const { return !owner.empty(); }
};

// The snapshot an addon sees: the focus holder gets everything; while nobody holds focus every addon
// gets the cursor and key edges; while another addon holds it, an addon gets nothing.
inline void view(const Snapshot &s, const Focus &f, const std::string &who, SwoftyInput &out)
{
    uint32_t size = out.size;
    memset(&out, 0, sizeof out);
    out.size = size;
    bool mine = f.held() && f.owner == who;
    out.focused = mine ? 1 : 0;
    out.x = out.y = -1;
    if (f.held() && !mine)
        return;
    out.x = s.x;
    out.y = s.y;
    memcpy(out.keys_down, s.keys_down, sizeof out.keys_down);
    memcpy(out.keys_pressed, s.keys_pressed, sizeof out.keys_pressed);
    if (!mine)
        return;
    out.wheel = s.wheel;
    out.buttons = s.buttons;
    out.pressed = s.pressed;
    out.released = s.released;
    out.char_count = uint32_t(s.chars.size());
    for (size_t i = 0; i < s.chars.size(); i++)
        out.chars[i] = s.chars[i];
}

}
