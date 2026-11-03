#pragma once

#include <cstdint>

namespace runtime::sequencer_records {

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

}
