// Host unit test for the unknown save container keeper (save_keep_logic.hpp): the game's obfuscated bool
// pair and the opt-out switch. run_events_test.sh builds and runs it.
#include "../src/core/save_keep_logic.hpp"

#include <cstdio>
#include <string>

namespace keep = runtime::save_keep;

static int fails;

static void expect(bool ok, const std::string &what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what.c_str());
    fails += !ok;
}

int main()
{
    expect(!keep::flag(0, 0x10), "byte 0, even key: false");
    expect(keep::flag(1, 0x10), "byte 1, even key: true");
    expect(!keep::flag(1, 0x11), "byte 1, odd key: false");
    expect(keep::flag(0, 0x11), "byte 0, odd key: true");
    expect(keep::flag(2, 0x10), "the game compares the whole byte: 2 with an even key reads true");

    for (uint64_t key : {0ull, 1ull, 0x8000000000000000ull, 0xfffffffffffffffeull, 0xffffffffffffffffull, 0x5d5e6971ull}) {
        uint8_t b = keep::true_byte(key);
        expect(keep::flag(b, key), "true_byte reads true for key " + std::to_string(key));
        expect(b <= 1, "true_byte is 0 or 1 for key " + std::to_string(key));
    }

    uint64_t r = 0x1234567890abcdefull;
    expect(!keep::flag(uint8_t(r & 1), r), "the root ctor's pair (byte r&1, key r) reads false");

    expect(keep::enabled(nullptr), "unset: on");
    expect(keep::enabled(""), "empty: on");
    expect(keep::enabled("1"), "1: on");
    expect(!keep::enabled("0"), "0: off");
    expect(!keep::enabled("off"), "off: off");
    expect(!keep::enabled("false"), "false: off");
    expect(!keep::enabled("no"), "no: off");

    printf("%s\n", fails ? "FAILED" : "all passed");
    return fails ? 1 : 0;
}
