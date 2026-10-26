// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career: careers.json (store.hpp).

#include <cstdio>
#include <deque>

#include <swofty/mixin.hpp>
#include <swofty/storage.hpp>

#include "../../../../native/src/core/json.hpp"
#include "store.hpp"

namespace pg::store {

namespace {

constexpr const char *kPath = "careers.json";

std::deque<Career> g_careers;
bool g_loaded = false;

std::string quoted(const std::string &s)
{
    std::string out = "\"";
    for (char c : s) {
        if (c == '"' || c == '\\')
            out += '\\';
        if (static_cast<unsigned char>(c) >= 0x20)
            out += c;
    }
    return out + "\"";
}

} // namespace

bool load()
{
    if (g_loaded)
        return true;
    std::string text;
    swofty::storage::Result r = swofty::storage::read(kPath, text);
    if (r != swofty::storage::Result::ok) {
        g_loaded = swofty::storage::available();
        return g_loaded;
    }
    json::Value j;
    if (!json::parse(text, j) || j.type != json::Value::Obj) {
        swofty::log("[progressive] store: %s is not JSON; starting a new one (the old file is kept as careers.bad.json)",
                    kPath);
        swofty::storage::write("careers.bad.json", text);
        g_loaded = true;
        return true;
    }
    for (auto &c : j["careers"].a) {
        Career x;
        x.key = c["key"].str();
        x.driver = static_cast<uint32_t>(c["driver"].num());
        x.seed = static_cast<uint32_t>(c["seed"].num());
        x.start = static_cast<int>(c["start"].num(2026));
        x.rnd = c["rnd"].str("real_life");
        x.moves = c["moves"].str("follow_driver");
        x.created_season = static_cast<int>(c["created_season"].num());
        x.applied_season = static_cast<int>(c["applied_season"].num(-1));
        x.applied_year = static_cast<int>(c["applied_year"].num());
        x.test_seasons = static_cast<int>(c["test_seasons"].num());
        if (!x.key.empty())
            g_careers.push_back(x);
    }
    g_loaded = true;
    swofty::log("[progressive] store: %zu career(s) in %s", g_careers.size(), kPath);
    return true;
}

bool save()
{
    std::string out = "{\n  \"careers\": [";
    for (size_t i = 0; i < g_careers.size(); ++i) {
        const Career &c = g_careers[i];
        char b[384];
        std::snprintf(b, sizeof b,
                      "%s\n    {\"key\": %s, \"driver\": %u, \"seed\": %u, \"start\": %d, \"rnd\": %s, \"moves\": %s, "
                      "\"created_season\": %d, \"applied_season\": %d, \"applied_year\": %d, \"test_seasons\": %d}",
                      i ? "," : "", quoted(c.key).c_str(), c.driver, c.seed, c.start, quoted(c.rnd).c_str(),
                      quoted(c.moves).c_str(), c.created_season, c.applied_season, c.applied_year, c.test_seasons);
        out += b;
    }
    out += "\n  ]\n}\n";
    swofty::storage::Result r = swofty::storage::write(kPath, out);
    if (r != swofty::storage::Result::ok)
        swofty::log("[progressive] store: writing %s failed (%s)", kPath, swofty::storage::name(r));
    return r == swofty::storage::Result::ok;
}

Career *find(const std::string &key)
{
    for (auto &c : g_careers)
        if (c.key == key)
            return &c;
    return nullptr;
}

std::vector<Career *> by_driver(uint32_t driver)
{
    std::vector<Career *> out;
    for (auto &c : g_careers)
        if (c.driver == driver)
            out.push_back(&c);
    return out;
}

Career &add(const Career &c)
{
    g_careers.push_back(c);
    return g_careers.back();
}

} // namespace pg::store
