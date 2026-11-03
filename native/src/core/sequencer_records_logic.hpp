#pragma once

#include <cstdint>

namespace runtime::sequencer_records {

constexpr uint64_t SKIP_BYTES = 0x42;

inline bool skips(uint64_t first_match)
{
    return !first_match;
}

inline bool says(uint64_t first_match, bool &said)
{
    if (!skips(first_match) || said)
        return false;
    said = true;
    return true;
}

inline uint64_t resume(uint64_t site)
{
    return site + SKIP_BYTES;
}

}
