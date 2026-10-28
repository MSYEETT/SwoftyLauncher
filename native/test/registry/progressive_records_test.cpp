#include "../../src/core/apply/content/teams.hpp"
#include "../../src/core/registry/declare_all.hpp"
#include "../../src/core/registry/older.hpp"
#include "../../src/core/registry/table.hpp"

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <map>
#include <set>
#include <sstream>
#include <string>
#include <vector>

using namespace swofty_core::registry;

static int failures = 0;
#define CHECK(c, ...)                          \
    do {                                       \
        if (!(c)) {                            \
            std::printf("FAIL: " __VA_ARGS__); \
            std::printf("\n");                 \
            failures++;                        \
        }                                      \
    } while (0)

namespace {

std::string slurp(const std::string &path)
{
    std::ifstream f(path, std::ios::binary);
    std::stringstream ss;
    ss << f.rdbuf();
    return ss.str();
}

const char *STOCK_LABELS[] = {"mercedes",   "ferrari",    "redbull",    "mclaren",    "forceindia", "renault",
                              "williams",   "tororosso",  "sauber",     "haas",       "mercedes_24", "ferrari_24",
                              "redbull_24", "mclaren_24", "forceindia_24", "renault_24", "williams_24", "tororosso_24",
                              "sauber_24",  "haas_24",    "mercedes_26", "ferrari_26", "redbull_26", "mclaren_26",
                              "forceindia_26", "renault_26", "williams_26", "tororosso_26", "sauber_26", "haas_26",
                              "gm_cadillac_26"};

std::vector<std::string> stock_labels()
{
    std::vector<std::string> labels(std::begin(STOCK_LABELS), std::end(STOCK_LABELS));
    for (int yy = 16; yy <= 24; yy++)
        for (const char *team : {"mercedes", "ferrari", "redbull", "mclaren", "force_india", "lotus", "williams", "toro_rosso",
                                 "sauber", "haas"})
            labels.push_back("cs" + std::to_string(yy) + "_" + team);
    labels.push_back("cs16_manor");
    return labels;
}

void seed_stock(Store &st)
{
    namespace content = swofty_core::apply::content;
    std::map<std::string, std::string> db;
    int n = 0;
    for (const std::string &label : stock_labels()) {
        std::string at = std::string(content::TEAMS) + ".team_" + std::to_string(n++);
        db[at + "._metadata_"] = label;
        db[at + ".display_name"] = label;
    }
    db[std::string(content::TEAMS) + "._team_instancecount_"] = std::to_string(n);
    content::Read read = [&db](const std::string &path) -> std::optional<std::string> {
        auto it = db.find(path);
        if (it == db.end())
            return std::nullopt;
        return it->second;
    };
    for (auto &c : content::CONSTRUCTOR_SEEDS)
        st.add_base("team.constructor", c.key,
                    Value::object({{"name", Value(std::string(c.name))}, {"country", Value(std::string(c.country))},
                                   {"team", Value()}}));
    for (auto &t : content::stock_teams(read))
        st.add_base("team.identity", t.label, t.record);
    st.stock_loaded("team.identity", true);
}

int add_all(Store &st, const std::string &addon, const Value &calls)
{
    int added = 0;
    for (auto &c : *calls["add"].if_array()) {
        std::string id;
        int rc = st.add(addon, std::string(c["registry"].as_string()), std::string(c["key"].as_string()), c["value"], &id);
        if (rc != 0)
            std::printf("refused: %s %s\n", std::string(c["registry"].as_string()).c_str(),
                        std::string(c["key"].as_string()).c_str());
        added += rc == 0;
    }
    return added;
}

}

int main(int argc, char **argv)
{
    if (argc < 2) {
        std::printf("usage: progressive_records_test <captured.json from the register test>\n");
        return 2;
    }
    Store &st = store();
    declare_system_registries([&](const std::string &name, const std::string &schema) {
        return !st.declared(name) && st.declare(name, swofty::json::parse(schema), false, "") == 0;
    });
    older::declare(st);
    seed_stock(st);

    Value calls = swofty::json::parse(slurp(argv[1]));
    int added = add_all(st, "swofty.progressive", calls);
    {
        std::string id;
        st.add("ig.teampack", "team.identity", "ig_racing",
               swofty::json::parse(R"({"names":{"title":{"literal":"IG Racing"},"code":"IGR"},"livery_name":"ig_racing"})"), &id);
        auto layer = [&](const char *addon, const char *season, const char *patch) {
            LayerSpec sp;
            sp.addon = addon;
            sp.registry = "career.season";
            sp.id = season;
            sp.scope = Scope::profile;
            sp.change.is_patch = true;
            sp.change.patch = swofty::json::parse(patch);
            uint32_t handle = 0;
            return st.layer(sp, &handle);
        };
        int a = layer("tails.whatif-history", "swofty.progressive/2017",
                      R"({"teams":[{"slot":1,"seats":["game/fernando_alonso","game/valtteri_bottas"]}]})");
        int b = layer("ig.teampack", "swofty.progressive/2016",
                      R"({"teams":[{"slot":12,"team":"ig.teampack/ig_racing","seats":["game/lando_norris","game/alex_albon"]}]})");
        CHECK(a == 0 && b == 0, "the what-if seat layer and the added-team layer are accepted (%d, %d)", a, b);
    }
    st.end_register();
    std::printf("added %d record(s)\n", added);

    int seasons = 0, seasons_ok = 0, ratings = 0, ratings_ok = 0;
    std::set<std::string> reasons;
    for (auto &c : *calls["add"].if_array()) {
        std::string reg(c["registry"].as_string()), id(c["id"].as_string());
        Record r = st.get(reg, id);
        bool season = reg == "career.season";
        (season ? seasons : ratings)++;
        if (r.status == Status::ok) {
            (season ? seasons_ok : ratings_ok)++;
        } else if (reasons.insert(r.why).second) {
            std::printf("%s %s: %s (%s)\n", reg.c_str(), id.c_str(), status_name(r.status), r.why.c_str());
        }
    }
    std::printf("career.season %d/%d valid, career.driver_ratings %d/%d valid\n", seasons_ok, seasons, ratings_ok, ratings);
    CHECK(seasons == 11 && seasons_ok == 11, "all 11 Progressive seasons are valid");
    CHECK(ratings == 242 && ratings_ok == 242, "all 242 Progressive rating records are valid");

    Record s17 = st.get("career.season", "swofty.progressive/2017");
    CHECK(s17.status == Status::ok && s17.value["teams"][0]["seats"][0].as_string() == "game/fernando_alonso",
          "the effective 2017 season has Alonso in the first Mercedes seat (%s)", s17.why.c_str());
    CHECK(s17.status == Status::ok && s17.value["teams"][0]["seats"][1].as_string() == "game/valtteri_bottas",
          "and Bottas in the second");
    CHECK(s17.status == Status::ok && s17.value["teams"][1]["seats"][0].as_string() == "game/sebastian_vettel",
          "Ferrari is Progressive's");
    CHECK(st.origin("career.season", "swofty.progressive/2017", "/teams/slot=1/seats/0") == "tails.whatif-history",
          "the seat's origin is the what-if addon");

    Record s16 = st.get("career.season", "swofty.progressive/2016");
    bool team_added = false;
    if (s16.status == Status::ok)
        for (auto &t : *s16.value["teams"].if_array())
            team_added = team_added || (t["slot"].as_int() == 12 && t["team"].as_string() == "ig.teampack/ig_racing");
    CHECK(s16.status == Status::ok && team_added, "the effective 2016 season has the added team in slot 12 (%s)", s16.why.c_str());
    CHECK(s16.status == Status::ok && s16.value["teams"].size() == 12, "and 12 teams in all");

    Value report = st.effective_report();
    CHECK(report["career.season"]["swofty.progressive/2016"]["status"].as_string() == "ok", "effective.json lists 2016 as ok");

    Record bad = st.get("career.season", "nobody/none");
    CHECK(bad.status == Status::no_record, "an unknown season is no_record");
    st.add("ig.teampack", "career.season", "broken",
           swofty::json::parse(R"({"data_group":"f1_career_ig","year":2030,"teams":[{"slot":1,"team":"constructor/nobody"}]})"), nullptr);
    Record broken = st.get("career.season", "ig.teampack/broken");
    CHECK(broken.status == Status::invalid && broken.why.find("constructor/nobody does not resolve in team.identity") != std::string::npos,
          "an invalid record says why: %s", broken.why.c_str());
    CHECK(st.effective_report()["career.season"]["ig.teampack/broken"]["reason"].as_string() == broken.why,
          "and effective.json carries the reason");
    std::printf("%s: Progressive's records on the core's registries, with the what-if seat layer and an added team\n",
                failures ? "FAIL" : "ok");
    return failures ? 1 : 0;
}
