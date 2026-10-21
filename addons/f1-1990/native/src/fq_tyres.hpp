// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include <algorithm>
#include <cstdint>
#include <cstring>

namespace f1990::tyres {

constexpr size_t kSetSize = 0x6c;
constexpr uint64_t kDryCompounds = 3;
constexpr uint64_t kCompounds = 5;
constexpr size_t kBlockSize = 0x28;
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

inline uint64_t compound_count(const uint8_t *alloc)
{
    uint64_t n;
    std::memcpy(&n, alloc + 0xd8, sizeof n);
    return std::min(n, kCompounds);
}

// Compound block i of an allocation: +0x00 slot, +0x08 allocator, +0x10 begin, +0x18 end, +0x20 capacity.
inline uint8_t *block(uint8_t *alloc, uint64_t i)
{
    return alloc + 0x10 + i * kBlockSize;
}

inline const uint8_t *block(const uint8_t *alloc, uint64_t i)
{
    return alloc + 0x10 + i * kBlockSize;
}

inline const uint8_t *sets_begin(const uint8_t *blk)
{
    const uint8_t *b;
    std::memcpy(&b, blk + 0x10, sizeof b);
    return b;
}

inline size_t set_count(const uint8_t *blk)
{
    const uint8_t *b, *e;
    std::memcpy(&b, blk + 0x10, sizeof b);
    std::memcpy(&e, blk + 0x18, sizeof e);
    return b && e > b && size_t(e - b) % kSetSize == 0 ? size_t(e - b) / kSetSize : 0;
}

inline bool fresh_set(const uint8_t *set)
{
    return !(set[9] & (kUsed | kHandedBack | kUsedOutsideGarage));
}

struct Census {
    size_t total = 0, fresh = 0, tagged = 0;
    size_t sets[kCompounds] = {}, fresh_sets[kCompounds] = {};
};

// Sets per compound slot, how many are fresh (not used, not handed back) and how many fresh ones carry
// a session tag.
inline Census census(const uint8_t *alloc)
{
    Census c;
    for (uint64_t i = 0; i < compound_count(alloc); i++) {
        const uint8_t *blk = block(alloc, i), *s = sets_begin(blk);
        size_t n = set_count(blk);
        c.sets[i] = n;
        c.total += n;
        for (size_t k = 0; k < n; k++, s += kSetSize) {
            if (!fresh_set(s))
                continue;
            uint32_t tag;
            std::memcpy(&tag, s, sizeof tag);
            c.fresh_sets[i]++;
            c.fresh++;
            c.tagged += tag != 0;
        }
    }
    return c;
}

struct Heap {
    void *(*alloc)(void *allocator, size_t bytes);
    void (*release)(void *allocator, void *p);
};

// Copies every set of src into the same compound slots of dst, through each dst block's allocator,
// as new sets: not used, unworn, keeping the session tag and the handed-back flag. Returns the sets
// copied, or -1 when a block has no allocator or an allocation fails (dst keeps what was copied).
inline int copy_sets(uint8_t *dst, const uint8_t *src, const Heap &heap)
{
    int copied = 0, result = 0;
    for (uint64_t i = 0; i < std::min(compound_count(dst), compound_count(src)); i++) {
        size_t n = set_count(block(src, i));
        if (!n)
            continue;
        uint8_t *blk = block(dst, i), *b;
        void *allocator;
        uint64_t cap;
        std::memcpy(&allocator, blk + 0x08, sizeof allocator);
        std::memcpy(&b, blk + 0x10, sizeof b);
        std::memcpy(&cap, blk + 0x20, sizeof cap);
        if (!b || cap < n) {
            uint8_t *fresh = allocator ? static_cast<uint8_t *>(heap.alloc(allocator, n * kSetSize)) : nullptr;
            if (!fresh) {
                result = -1;
                continue;
            }
            if (b)
                heap.release(allocator, b);
            b = fresh;
            cap = n;
        }
        std::memcpy(b, sets_begin(block(src, i)), n * kSetSize);
        for (uint8_t *s = b; s < b + n * kSetSize; s += kSetSize) {
            s[9] &= uint8_t(~(kUsed | kUsedOutsideGarage));
            for (size_t off : kWear)
                std::memset(s + off, 0, sizeof(float));
        }
        uint8_t *e = b + n * kSetSize;
        std::memcpy(blk + 0x10, &b, sizeof b);
        std::memcpy(blk + 0x18, &e, sizeof e);
        std::memcpy(blk + 0x20, &cap, sizeof cap);
        copied += int(n);
    }
    if (copied)
        dst[0xe8] = 1;
    return result < 0 ? -1 : copied;
}

// Every fresh set with a session tag gets tag 0, which every session accepts. Returns the sets changed.
inline int untag_fresh_sets(uint8_t *alloc)
{
    int changed = 0;
    for (uint64_t i = 0; i < compound_count(alloc); i++) {
        uint8_t *blk = block(alloc, i), *b;
        std::memcpy(&b, blk + 0x10, sizeof b);
        size_t n = set_count(blk);
        for (size_t k = 0; k < n; k++, b += kSetSize) {
            uint32_t tag;
            std::memcpy(&tag, b, sizeof tag);
            if (!fresh_set(b) || !tag)
                continue;
            std::memset(b, 0, sizeof tag);
            changed++;
        }
    }
    if (changed)
        alloc[0xe8] = 1;
    return changed;
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
