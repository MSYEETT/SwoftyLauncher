// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career: the per-career store, careers.json in the addon's storage folder. An entry's
// key is fixed when it is made (pg-<seed>-<driver>, with a suffix when taken); seed is the
// WeatherAndTimeOfDayComponent random_seed (+0x2c4) seen at the career's last load, a hint only
// since the game redraws it, and the player's driver stat id. season.cpp career_entry picks the
// entry of a loaded career (seed, then the year its seats show, then the last used entry of the
// driver). The profile options are copied in when the entry is made, so later option changes
// leave running careers alone; the last applied season sits next to them (an optimisation, the
// seats decide), and the seasons the developer option test_next_season added on top of the game's
// season index.

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
    uint64_t used = 0;
};

bool load();
bool save();
Career *find(const std::string &key);
std::vector<Career *> by_driver(uint32_t driver);
Career *by_seed(uint32_t seed, uint32_t driver);
Career &add(const Career &c);
std::string free_key(uint32_t seed, uint32_t driver);
uint64_t next_use();

} // namespace pg::store
