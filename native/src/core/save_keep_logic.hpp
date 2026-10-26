#pragma once
#include <cstdint>
#include <cstring>
#include <initializer_list>

// Pure logic of the unknown save container keeper (save_keep.cpp): the game's obfuscated bool pair and
// the opt-out switch. No game calls, so the unit test runs on the host.

namespace runtime::save_keep {

// A save-data node keeps its bools as a {byte, key} pair: true when the byte differs from the key's low
// bit (Game_SaveData_DeserialiseContainer at 0x14364fef0, Game_SaveData_SerialiseContainer at 0x14364e22f).
constexpr bool flag(uint8_t byte, uint64_t key)
{
    return uint64_t(byte) != (key & 1);
}

constexpr uint8_t true_byte(uint64_t key)
{
    return uint8_t((key & 1) ^ 1);
}

// SWOFTY_KEEP_UNKNOWN_SAVES: unset, empty or anything but an off word keeps the feature on.
inline bool enabled(const char *env)
{
    if (!env)
        return true;
    for (const char *off : {"0", "off", "false", "no"})
        if (!strcmp(env, off))
            return false;
    return true;
}

}
