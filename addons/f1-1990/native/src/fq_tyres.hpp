// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include <algorithm>
#include <cstdint>
#include <cstring>

namespace f1990::tyres {

constexpr size_t kSetSize = 0x6c;
constexpr uint64_t kDryCompounds = 3;
constexpr uint8_t kUsed = 0x01;
constexpr uint8_t kHandedBack = 0x08;
constexpr uint8_t kUsedOutsideGarage = 0x10;
constexpr size_t kWear[] = {0x0c, 0x18, 0x24, 0x30};

inline bool is_shootout_session(uint32_t id)
{
    return id - 10 < 5;
}

inline bool unused_set(const uint8_t *set)
{
    if (set[9] & (kUsed | kUsedOutsideGarage))
        return false;
    for (size_t off : kWear) {
        float w;
        std::memcpy(&w, set + off, sizeof w);
        if (w != 0.f)
            return false;
    }
    return true;
}

// Every unused dry set (softer, medium, harder) of one car's allocation becomes usable in any
// session: the handed-back flag is cleared and the session tag set to 0 ("any"). Returns how many
// sets changed; a change marks the allocation dirty (+0xe8).
inline int free_unused_dry_sets(uint8_t *alloc)
{
    uint64_t compounds;
    std::memcpy(&compounds, alloc + 0xd8, sizeof compounds);
    int changed = 0;
    for (uint64_t i = 0; i < std::min(compounds, kDryCompounds); i++) {
        uint8_t *block = alloc + 0x10 + i * 0x28, *b, *e;
        std::memcpy(&b, block + 0x10, sizeof b);
        std::memcpy(&e, block + 0x18, sizeof e);
        for (; b && b + kSetSize <= e; b += kSetSize) {
            uint32_t tag;
            std::memcpy(&tag, b, sizeof tag);
            if (!unused_set(b) || (!(b[9] & kHandedBack) && !tag))
                continue;
            b[9] &= uint8_t(~kHandedBack);
            std::memset(b, 0, sizeof tag);
            changed++;
        }
    }
    if (changed)
        alloc[0xe8] = 1;
    return changed;
}

}
