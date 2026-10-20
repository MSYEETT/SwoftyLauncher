// SPDX-License-Identifier: AGPL-3.0-or-later
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>

#include "fq_tyres.hpp"
#include "weekend.hpp"

using namespace f1990::tyres;

static int failures;

static void check(const char *name, bool ok)
{
    std::printf("%s %s\n", ok ? "ok  " : "FAIL", name);
    failures += !ok;
}

struct Set {
    uint32_t tag = 0;
    uint8_t flags = 0;
    float wear = 0.f;
};

struct Alloc {
    std::vector<uint8_t> mem = std::vector<uint8_t>(0x100);
    std::vector<std::vector<uint8_t>> sets;

    explicit Alloc(const std::vector<std::vector<Set>> &compounds)
    {
        uint64_t n = compounds.size();
        std::memcpy(mem.data() + 0xd8, &n, sizeof n);
        sets.resize(compounds.size());
        for (size_t i = 0; i < compounds.size(); i++) {
            auto &v = sets[i];
            v.resize(compounds[i].size() * kSetSize);
            for (size_t k = 0; k < compounds[i].size(); k++) {
                uint8_t *s = v.data() + k * kSetSize;
                std::memcpy(s, &compounds[i][k].tag, 4);
                s[9] = compounds[i][k].flags;
                std::memcpy(s + 0x18, &compounds[i][k].wear, 4);
            }
            uint8_t *b = v.data(), *e = v.data() + v.size();
            std::memcpy(mem.data() + 0x10 + i * 0x28 + 0x10, &b, sizeof b);
            std::memcpy(mem.data() + 0x10 + i * 0x28 + 0x18, &e, sizeof e);
        }
    }

    uint32_t tag(size_t c, size_t k) const
    {
        uint32_t t;
        std::memcpy(&t, sets[c].data() + k * kSetSize, 4);
        return t;
    }
    uint8_t flags(size_t c, size_t k) const { return sets[c][k * kSetSize + 9]; }
};

int main()
{
    check("shootout sessions are 10..14",
          is_shootout_session(10) && is_shootout_session(13) && is_shootout_session(14) && !is_shootout_session(9) &&
              !is_shootout_session(15) && !is_shootout_session(8) && !is_shootout_session(0));

    Alloc a({{{10, kHandedBack}, {5, 0}, {1, kHandedBack | kUsed}, {11, kHandedBack, 0.2f}, {0, 0}},
             {{12, kHandedBack}},
             {},
             {{0, kHandedBack}},
             {{0, kHandedBack}}});
    int n = free_unused_dry_sets(a.mem.data());
    check("three unused dry sets changed", n == 3);
    check("handed-back shootout set freed", a.tag(0, 0) == 0 && !(a.flags(0, 0) & kHandedBack));
    check("tagged qualifying set untagged", a.tag(0, 1) == 0);
    check("used set left alone", a.tag(0, 2) == 1 && (a.flags(0, 2) & kHandedBack));
    check("worn set left alone", a.tag(0, 3) == 11 && (a.flags(0, 3) & kHandedBack));
    check("medium set freed", a.tag(1, 0) == 0 && !(a.flags(1, 0) & kHandedBack));
    check("inters and wets untouched", (a.flags(3, 0) & kHandedBack) && (a.flags(4, 0) & kHandedBack));
    check("allocation marked dirty", a.mem[0xe8] == 1);

    Alloc b({{{0, 0}, {3, kUsed}}});
    check("nothing to change keeps the allocation clean", free_unused_dry_sets(b.mem.data()) == 0 && b.mem[0xe8] == 0);

    Alloc c({{{0, kUsedOutsideGarage}}});
    check("set used outside the garage left alone", free_unused_dry_sets(c.mem.data()) == 0);

    std::vector<uint8_t> weekend(0x300), sessions(0xa0 * 5);
    auto session = [&](size_t i, uint32_t id, uint8_t group) {
        std::memcpy(sessions.data() + i * 0xa0 + 8, &id, 4);
        sessions[i * 0xa0 + 0x98] = group;
    };
    session(0, 1, 0);
    session(1, 2, 0);
    session(2, 13, 2);
    session(3, 8, 1);
    session(4, 15, 3);
    uint8_t *sb = sessions.data(), *se = sessions.data() + sessions.size();
    std::memcpy(weekend.data() + 0x220, &sb, sizeof sb);
    std::memcpy(weekend.data() + 0x228, &se, sizeof se);
    check("1990 weekend recognised", f1990::is_1990_weekend(weekend.data()));
    session(4, 16, 3);
    check("sprint weekend with race_2 rejected", !f1990::is_1990_weekend(weekend.data()));
    session(4, 15, 3);
    session(2, 3, 0);
    check("weekend without a shootout group rejected", !f1990::is_1990_weekend(weekend.data()));

    if (failures)
        std::printf("%d failure(s)\n", failures);
    return failures ? EXIT_FAILURE : EXIT_SUCCESS;
}
