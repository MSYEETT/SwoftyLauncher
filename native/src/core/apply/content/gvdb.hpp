#pragma once

#include <cstdint>
#include <cstring>
#include <string>
#include <string_view>
#include <unordered_map>

// The game database file (game_package/gamedatabases/gamedatabases.2025.bin): a record count, then per
// record its FNV-1a-64 key (low and high halves), its tag, the masked text's length, the text and a 0. The
// mask runs over every record's text in turn (tools/gamedb.py).

namespace swofty_core::apply::content::gvdb {

struct Record {
    int tag = -1;
    std::string text;
};

inline uint64_t fnv64(std::string_view s)
{
    uint64_t h = 0xcbf29ce484222325ull;
    for (unsigned char c : s) {
        h ^= c;
        h *= 0x100000001b3ull;
    }
    return h;
}

inline bool read(const unsigned char *data, std::size_t size, std::unordered_map<uint64_t, Record> &out)
{
    static constexpr char MASK[] = "tG5MYmBZEZZnrH7yg2qTC4qj42Jcq9";
    constexpr std::size_t n = sizeof MASK - 1, start = 10;
    if (size < 4)
        return false;
    uint32_t count;
    std::memcpy(&count, data, 4);
    std::size_t p = 4, stream = 0;
    out.reserve(out.size() + count);
    for (uint32_t i = 0; i < count; i++) {
        if (p + 13 > size)
            return false;
        uint32_t lo, hi, len;
        std::memcpy(&lo, data + p, 4);
        std::memcpy(&hi, data + p + 4, 4);
        int tag = data[p + 8];
        std::memcpy(&len, data + p + 9, 4);
        if (p + 14 + len > size || data[p + 13 + len] != 0)
            return false;
        Record r{tag, std::string(len, '\0')};
        for (uint32_t k = 0; k < len; k++)
            r.text[k] = char((data[p + 13 + k] - unsigned(MASK[(start + stream + k) % n])) & 0xff);
        stream += len;
        out[(uint64_t(hi) << 32) | lo] = std::move(r);
        p += 14 + len;
    }
    return p == size;
}

}
