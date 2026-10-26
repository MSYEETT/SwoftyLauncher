// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career: the per-career store, careers.json in the addon's storage folder. A career is
// keyed by pg-<seed>-<driver>: the WeatherAndTimeOfDayComponent rnd_seed (+0x2c4), which the game
// draws once for a career and keeps in its save, and the player's driver stat id. The profile
// options are copied in when the career is first seen at Team HQ, so later option changes leave
// running careers alone; the last applied season sits next to them, and the seasons the developer
// option test_next_season added on top of the game's season index.

#pragma once
#include <cstdint>
#include <string>
#include <vector>

namespace pg::store {

struct Career {
    std::string key;
    uint32_t driver = 0;
    uint32_t seed = 0;
    int start = 2026;
    std::string rnd = "real_life";
    std::string moves = "follow_driver";
    int created_season = 0;
    int applied_season = -1;
    int applied_year = 0;
    int test_seasons = 0;
};

bool load();
bool save();
Career *find(const std::string &key);
std::vector<Career *> by_driver(uint32_t driver);
Career &add(const Career &c);

} // namespace pg::store
