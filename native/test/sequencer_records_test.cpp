#include "sequencer_records_logic.hpp"

#include <cstdio>

static int failures;

#define CHECK(cond, msg)                                                                                                \
    do {                                                                                                                \
        if (cond) {                                                                                                     \
            printf("ok  : %s\n", msg);                                                                                  \
        } else {                                                                                                        \
            printf("FAIL: %s\n", msg);                                                                                  \
            failures++;                                                                                                 \
        }                                                                                                               \
    } while (0)

int main()
{
    using namespace runtime::sequencer_records;
    CHECK(skips(0), "an empty record collection is skipped");
    CHECK(!skips(0x7ff600001000), "a collection with a first record runs the game's code");
    bool said = false;
    CHECK(says(0, said), "the first empty collection is logged");
    CHECK(!says(0, said), "a second empty collection is not logged");
    bool other = false;
    CHECK(!says(0x1000, other) && !other, "a collection with records is not logged");
    return failures ? 1 : 0;
}
