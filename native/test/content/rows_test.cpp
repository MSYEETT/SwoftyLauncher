#include "../../src/core/apply/content/boot.hpp"
#include "../../src/core/apply/ids.hpp"
#include "../../src/core/registry/table.hpp"
#include "../../src/core/registry/validate.hpp"

#include <cstdio>
#include <fstream>
#include <sstream>

using namespace swofty_core;
using swofty::json::Value;
namespace content = apply::content;
namespace rows = apply::content::rows;

static int failures = 0;

static void check(bool ok, const std::string &what)
{
    std::printf("%s  %s\n", ok ? "ok  " : "FAIL", what.c_str());
    failures += !ok;
}

static std::string slurp(const std::string &path)
{
    std::ifstream f(path, std::ios::binary);
    std::stringstream s;
    s << f.rdbuf();
    return s.str();
}

struct Db {
    std::map<uint64_t, rows::Rec> recs;
    void put(const std::string &path, int tag, const std::string &text) { recs[content::gvdb::fnv64(path)] = {tag, text}; }
    rows::Base base()
    {
        return [this](uint64_t key) -> std::optional<rows::Rec> {
            auto it = recs.find(key);
            if (it == recs.end())
                return std::nullopt;
            return it->second;
        };
    }
};

static std::optional<std::string> written(const rows::Engine &e, const std::string &path)
{
    for (auto &w : e.writes())
        if (w.path == path)
            return w.text;
    return std::nullopt;
}

static Value op(const char *json) { return swofty::json::parse(json); }

static void engine_tests()
{
    rows::Keys keys;
    keys.parse("@t.rows.row_N\n_metadata_\nstat_id\nspeed\n+part\n_part_instancecount_\n"
               "@t.rows.row_N.part_N\n_metadata_\nweight\n");
    Db db;
    db.put("t.rows._row_instancecount_", 0, "2");
    for (int i = 0; i < 2; i++) {
        std::string r = "t.rows.row_" + std::to_string(i);
        db.put(r + "._metadata_", 4, i ? "beta" : "alpha");
        db.put(r + ".stat_id", 0, std::to_string(40 + i));
        db.put(r + ".speed", 1, i ? "2.5" : "1.5");
        db.put(r + "._part_instancecount_", 0, "1");
        db.put(r + ".part_0._metadata_", 4, "p");
        db.put(r + ".part_0.weight", 0, i ? "9" : "8");
    }
    db.put("t.list.items", 13, "a, b");
    rows::Engine e(db.base(), keys);
    e.attribute("mod.a", "vehicle");
    std::string why = e.apply(op(R"({"op":"add-row","table":"t.rows","from":"t.rows.row[_metadata_=beta]",
                                     "set":{"_metadata_":"gamma"}})"));
    check(why.empty(), "add-row copies a row by its label (" + why + ")");
    check(written(e, "t.rows.row_2._metadata_") == "gamma", "at the table's next index, with the set value");
    check(written(e, "t.rows.row_2.speed") == "2.5" && written(e, "t.rows.row_2.part_0.weight") == "9",
          "every key of the source row, nested rows included");
    check(written(e, "t.rows.row_2._part_instancecount_") == "1", "with the nested table's count");
    check(written(e, "t.rows._row_instancecount_") == "3", "and the table's count goes up by one");
    check(written(e, "t.rows.row_2.stat_id") == "42", "a top-level table with unique ids gives the highest plus one");
    bool fresh = false, count_fresh = true;
    for (auto &w : e.writes()) {
        if (w.path == "t.rows.row_2.speed")
            fresh = w.fresh && w.owner == "mod.a" && w.tag == 1;
        if (w.path == "t.rows._row_instancecount_")
            count_fresh = w.fresh;
    }
    check(fresh && !count_fresh, "new keys are fresh, the count is not; the source's tag and the owner are kept");

    why = e.apply(op(R"({"op":"add-row","table":"t.rows","from":"t.rows.row[_metadata_=alpha]",
                         "set":{"_metadata_":"delta","stat_id":"77"}})"));
    check(why.empty() && written(e, "t.rows.row_3.stat_id") == "77", "a set stat_id wins");
    check(e.apply(op(R"({"op":"add-row","table":"t.rows","from":"t.rows.row[_metadata_=zeta]"})")).find("no row matches") !=
              std::string::npos,
          "a missing source row fails");
    check(e.apply(op(R"({"op":"add-row","table":"t.rows","from":"t.rows.row[_metadata_=alpha]","set":{"colour":"1"}})"))
                  .find("unknown field") != std::string::npos,
          "a set field the row doesn't have fails");
    check(written(e, "t.rows.row_4._metadata_") == std::nullopt, "a failing operation writes nothing");

    std::size_t mark = e.mark();
    e.attribute("mod.b", "vehicle");
    check(e.apply(op(R"({"op":"set","select":"t.rows.row[_metadata_=gamma]","field":"speed","value":"7"})")).empty(),
          "set resolves a row the engine added");
    check(e.apply(op(R"({"op":"list-append","select":"t","field":"list.items","value":"b,c"})")).empty() &&
              written(e, "t.list.items") == "a,b,c",
          "list-append adds a missing value once");
    e.rollback(mark);
    check(written(e, "t.rows.row_2.speed") == "2.5" && written(e, "t.list.items") == std::nullopt,
          "rollback takes back every write since the mark");
    check(e.apply(op(R"({"op":"set","select":"t.rows.row_0","field":"nothing","value":"1"})")).find("no such key") !=
              std::string::npos,
          "set on a missing key fails");
    check(e.apply(op(R"({"op":"put-record","key":"t.extra","type":2,"value":"6869"})")).empty() &&
              written(e, "t.extra") == "hi",
          "put-record writes the hex value under the path");
    db.put("t.rows.row_9._metadata_", 4, "beta");
    rows::Engine twice(db.base(), keys);
    check(twice.apply(op(R"({"op":"set","select":"t.rows.row[_metadata_=beta]","field":"speed","value":"1"})")).empty(),
          "only rows under the count are searched");
    db.put("t.rows.row_1._metadata_", 4, "alpha");
    check(twice.apply(op(R"({"op":"set","select":"t.rows.row[_metadata_=alpha]","field":"speed","value":"1"})"))
                  .find("2 rows match") != std::string::npos,
          "two rows with the label fail");
    check(rows::shape_of("a.b_12.c.d_3") == "a.b_N.c.d_N" && rows::shape_of("audio.x_2025.y") == "audio.x_N.y",
          "row shapes");

    rows::Engine guard(db.base(), keys);
    guard.attribute("a.x", "vehicle", "a.x/one");
    guard.apply(op(R"({"op":"put-record","key":"audio.car.sw.bundle","type":2,"value":"61"})"));
    guard.attribute("b.y", "vehicle", "b.y/two");
    check(guard.apply(op(R"({"op":"put-record","key":"audio.car.sw.bundle","type":2,"value":"62"})")).find("a.x/one created it") !=
                  std::string::npos &&
              written(guard, "audio.car.sw.bundle") == "a",
          "a record never writes a key another record created");
    check(guard.apply(op(R"({"op":"put-record","key":"t.list.items","type":13,"value":"62"})")).empty(),
          "keys the game holds stay shared");
}

// A small database with the stock rows a package copies.
static void stock_package_rows(Db &db)
{
    auto row = [&](const std::string &prefix, std::initializer_list<std::pair<const char *, std::string>> fields) {
        for (auto &[f, v] : fields)
            db.put(prefix + "." + f, std::string_view(f) == "_metadata_" ? 4 : 2, v);
    };
    db.put("entrants.entrant_lists.entrants._entrant_instancecount_", 0, "1");
    row("entrants.entrant_lists.entrants.entrant_0",
        {{"_metadata_", "max_verstappen"}, {"driver", "max_verstappen"}, {"driver_customisation", "x"}, {"car_number", "1"}});
    db.put("entrants.standings._standings_instancecount_", 0, "1");
    row("entrants.standings.standings_0", {{"_metadata_", "f1_teams_2024"}});
    db.put("entrants.standings.standings_0._team_reference_instancecount_", 0, "2");
    for (int i = 0; i < 2; i++)
        row("entrants.standings.standings_0.team_reference_" + std::to_string(i),
            {{"_metadata_", "t" + std::to_string(i)}, {"team", "stock"}});
    db.put("entrants.entrant_lists._entrant_list_instancecount_", 0, "1");
    row("entrants.entrant_lists.entrant_list_0", {{"_metadata_", "f1_2025"},
                                                  {"display_name", "x"},
                                                  {"previous_season_team_standings", "x"},
                                                  {"previous_season_driver_standings", "x"},
                                                  {"leaderboard", "f1"}});
    db.put("entrants.entrant_lists.entrant_list_0.stat_id", 0, "104");
    db.put("entrants.entrant_lists.entrant_list_0._entrant_instancecount_", 0, "2");
    for (int i = 0; i < 2; i++)
        row("entrants.entrant_lists.entrant_list_0.entrant_" + std::to_string(i),
            {{"_metadata_", "e"},
             {"team", "t"},
             {"vehicle", "v"},
             {"entrant_reference", "r"},
             {"team_position", "0"},
             {"car_number", "1"},
             {"driver", "d"},
             {"driver_customisation", "c"}});
    db.put("game_setup.scenarios._scenario_instancecount_", 0, "1");
    row("game_setup.scenarios.scenario_0", {{"_metadata_", "f1"},
                                            {"display_name", "x"},
                                            {"entrant_list", "f1_2025"},
                                            {"game_setup_mode", "f1"},
                                            {"race_class", "f1"},
                                            {"use_custom_vehicle_spec", "False"}});
    db.put("game_setup.scenarios.scenario_0.statId", 0, "14");
    std::string opt = "options_system.options.multiplayer.options_folder_2";
    db.put(opt + "._option_instancecount_", 0, "1");
    row(opt + ".option_0", {{"_metadata_", "category"}});
    db.put(opt + ".option_0._value_instancecount_", 0, "1");
    row(opt + ".option_0.value_0", {{"_metadata_", "f1_2025"},
                                    {"confirmation_dialogue", ""},
                                    {"description_override", ""},
                                    {"game_value", "f1_2025"},
                                    {"is_default", "False"},
                                    {"loc_tag", "x"},
                                    {"loc_tag_alternate", ""},
                                    {"vr_is_default", "False"}});
    db.put("entrants.entrant_setup_modes._entrant_setup_mode_instancecount_", 0, "1");
    row("entrants.entrant_setup_modes.entrant_setup_mode_0", {{"_metadata_", "swofty_f1"}});
    db.put("entrants.entrant_setup_modes.entrant_setup_mode_0.entrant_lists", 13, "f1_2025");
    db.put("entrants.entrant_setup_modes.entrant_setup_mode_0.selectable_entrant_lists", 13, "f1_2025");
    db.put("audio.audio_bundle_settings._audio_bundle_settings_instancecount_", 0, "1");
    row("audio.audio_bundle_settings.audio_bundle_settings_0",
        {{"_metadata_", "mercedes_2025"}, {"bundle_name", "mercedes_2025"}, {"era", "x"}, {"engine", "e"}, {"exhaust", "x"}, {"cabin", "c"}});
    for (auto &[f, v] : std::initializer_list<std::pair<const char *, const char *>>{
             {"_metadata_", "f1_m_mercedes"}, {"bundle_name", "mercedes_2025.bdl"}, {"engine_offset_x", "0.1"}})
        db.put(std::string("audio.audio_car_data.f1_m_mercedes.") + f, std::string_view(f) == "engine_offset_x" ? 1 : 2, v);
    db.put("entrants.vehicles._vehicle_instancecount_", 0, "1");
    row("entrants.vehicles.vehicle_0", {{"_metadata_", "mercedes_2025"}, {"performance_tier", "0"}, {"tyre_wear_scale", "1"}});
    db.put("entrants.vehicles.vehicle_0.stat_id", 0, "508");
    db.put("content_sets._content_set_instancecount_", 0, "1");
    row("content_sets.content_set_0", {{"_metadata_", "2025"}});
    db.put("content_sets.content_set_0.available_entrant_lists_0.entrant_lists", 13, "f1_2025");
}

static void variant_tests(const std::string &regs)
{
    registry::reset_store();
    auto &st = registry::store();
    st.set_order({"swofty.classic-seasons-2017", "mod.esp"});
    st.set_validator(&registry::schema_validator());
    const SwoftyRegistry *t = registry::table();
    for (const char *name : {"engine_sound", "vehicle.package", "vehicle", "content.package", "era.box"}) {
        std::string schema = slurp(regs + "/" + name + "/1.0.json");
        t->declare(name, SwoftyText{schema.data(), schema.size()});
    }
    int r1 = registry::load_data_file("swofty.classic-seasons-2017", "registry/content.json", R"j({
      "engine_sound": { "mercedes": { "bundle": "sounds/cs17_mercedes.bdl", "settings_from": "game/mercedes_2025" } },
      "vehicle": { "mercedes": { "label": "cs17_mercedes", "donor": "game/mercedes_2025", "stat_id": 520,
                                 "engine_sound": "swofty.classic-seasons-2017/mercedes",
                                 "audio_switch": "f1_M_mercedes", "audio_from": "f1_m_mercedes" } },
      "content.package": { "f1_2017": {
        "title": "lng_cs_f1_2017", "entrant_list": "cs_f1_2017", "season": 2017, "stat_id": 106,
        "teams": [ { "key": "mercedes", "vehicle": null,
                     "drivers": [ { "driver": "game/lewis_hamilton", "number": 44, "look": null } ] },
                   { "key": "redbull", "vehicle": null,
                     "drivers": [ { "driver": "game/max_verstappen", "number": 33, "look": null } ] } ],
        "database": { "entrant_list": "cs_f1_2017", "scenario_stat_id": 16, "entrant_prefix": "cs2017-",
                      "standings": "cs_f1_teams_2017", "content_sets": ["2025"],
                      "looks": { "mercedes/lewis_hamilton": "2017-cs_mercedes_lewis_hamilton",
                                 "redbull/max_verstappen": "2017-cs_redbull_max_verstappen" } } } } })j");
    int r2 = registry::load_data_file("mod.esp", "registry/content.json", R"j({
      "engine_sound": { "v10_screamer": { "bundle": "sounds/v10_screamer.bdl", "settings_from": "game/mercedes_2025" },
                        "mercedes_mix": { "extends": "swofty.classic-seasons-2017/mercedes", "settings_from": "game/mercedes_2025" } },
      "vehicle": { "mercedes_v10": { "extends": "swofty.classic-seasons-2017/mercedes", "label": "esp_cs17_mercedes",
                                     "engine_sound": "mod.esp/v10_screamer" },
                   "mercedes_copy": { "extends": "swofty.classic-seasons-2017/mercedes" } },
      "content.package": { "f1_2017_v10": {
        "extends": "swofty.classic-seasons-2017/f1_2017",
        "title": { "literal": "F1 2017 (V10)" }, "entrant_list": "esp_f1_2017_v10", "stat_id": "auto" } } })j");
    st.end_register();
    check(r1 == SWOFTY_ASSET_OK && r2 == SWOFTY_ASSET_OK, "the package and its variant register");
    auto v = st.get("content.package", "mod.esp/f1_2017_v10");
    check(v.status == registry::Status::ok && v.value["database"]["entrant_prefix"].as_string() == "cs2017-",
          "the variant inherits the base's private database block");

    Db db;
    stock_package_rows(db);
    auto pins = content::pins(st);
    check(pins.size() == 3 && pins[0].stat_id == 520 && pins[1].stat_id == 106 && pins[2].stat_id == 16 &&
              pins[2].registry == content::SCENARIO_IDS,
          "the bases pin their vehicle, entrant list and scenario ids; the variants inherit none");
    apply::ids::Store ids;
    auto text = [&](const std::string &path) -> std::optional<std::string> {
        auto r = db.base()(content::gvdb::fnv64(path));
        return r ? std::optional<std::string>(r->text) : std::nullopt;
    };
    for (auto &scan : content::scan_pools(text))
        ids.define_pool(scan.registry, {{{scan.lo, scan.hi}}});
    for (auto &p : pins)
        ids.pin(p.registry, p.id, p.stat_id, "test");
    content::StatIds stat = [&](const std::string &reg, const std::string &id, int64_t pin) -> int64_t {
        return pin >= 0 ? ids.pin(reg, id, pin, "test").id : ids.allocate(reg, id, "test").id;
    };
    content::Expansion x(db.base());
    content::expand(st, x, stat);
    int refused = 0;
    for (auto &[k, n] : x.refused)
        refused += n;
    check(refused == 0 && x.records["content.package"] == 2, "both packages apply");
    for (auto &l : x.lines["content.package"])
        std::printf("      %s\n", l.c_str());
    auto has_label = [&](const std::string &table, const std::string &row, const std::string &label) {
        for (int i = 0; i < 8; i++)
            if (written(x.engine, table + "." + row + "_" + std::to_string(i) + "._metadata_") == label)
                return table + "." + row + "_" + std::to_string(i);
        return std::string();
    };
    std::string base_list = has_label("entrants.entrant_lists", "entrant_list", "cs_f1_2017");
    std::string var_list = has_label("entrants.entrant_lists", "entrant_list", "esp_f1_2017_v10");
    check(!base_list.empty() && !var_list.empty(), "each gets its own entrant list");
    check(written(x.engine, base_list + ".stat_id") == "106", "the base keeps its pinned entrant list id");
    check(written(x.engine, var_list + ".stat_id") == "105", "the variant's comes from the pool above the table's ids");
    check(!has_label("entrants.entrant_lists.entrants", "entrant", "cs2017-lewis_hamilton").empty() &&
              !has_label("entrants.entrant_lists.entrants", "entrant", "esp_f1_2017_v10-lewis_hamilton").empty(),
          "the variant's entrants are named for its own list, not the base's prefix");
    check(written(x.engine, var_list + ".entrant_0.entrant_reference") == "esp_f1_2017_v10-lewis_hamilton",
          "and its list points at them");
    std::string var_standings = has_label("entrants.standings", "standings", "esp_f1_2017_v10_teams");
    check(!has_label("entrants.standings", "standings", "cs_f1_teams_2017").empty() && !var_standings.empty(),
          "each gets its own standings row");
    check(written(x.engine, var_list + ".previous_season_team_standings") == "esp_f1_2017_v10_teams",
          "the variant's list reads its own standings");
    std::string base_sc = has_label("game_setup.scenarios", "scenario", "cs_f1_2017");
    std::string var_sc = has_label("game_setup.scenarios", "scenario", "esp_f1_2017_v10");
    check(written(x.engine, base_sc + ".statId") == "16" && written(x.engine, var_sc + ".statId") == "15",
          "the base keeps its scenario id, the variant gets the first free one above the stock scenarios");
    check(written(x.engine, var_list + ".display_name") == "F1 2017 (V10)", "a literal title is the list's name");
    std::string car = has_label("entrants.vehicles", "vehicle", "cs17_mercedes");
    std::string v10 = has_label("entrants.vehicles", "vehicle", "esp_cs17_mercedes");
    check(!car.empty() && written(x.engine, car + ".stat_id") == "520", "the base car keeps its pinned stat id");
    check(!v10.empty() && written(x.engine, v10 + ".stat_id") == "510" &&
              written(x.engine, has_label("entrants.vehicles", "vehicle", "mercedes_copy") + ".stat_id") == "509",
          "its variant gets one of its own from the pool, not the base's");
    std::string base_sw = "audio.audio_car_data.f1_M_mercedes.", v10_sw = "audio.audio_car_data.esp_cs17_mercedes.";
    check(written(x.engine, base_sw + "_metadata_") == "f1_M_mercedes" &&
              written(x.engine, base_sw + "bundle_name") == "cs17_mercedes.bdl",
          "the base car's sound rows keep the base's sound");
    check(written(x.engine, v10_sw + "_metadata_") == "esp_cs17_mercedes" &&
              written(x.engine, v10_sw + "bundle_name") == "v10_screamer.bdl" &&
              written(x.engine, v10_sw + "engine_offset_x") == "0.1",
          "its variant's sound rows are under an audio switch of its own, named by its label");
    std::string copy = has_label("entrants.vehicles", "vehicle", "mercedes_copy");
    check(!copy.empty() && has_label("entrants.vehicles", "vehicle", "cs17_mercedes") == car &&
              written(x.engine, "audio.audio_car_data.mercedes_copy.bundle_name") == "cs17_mercedes.bdl",
          "a variant that inherits the label gets its key as its label and audio switch");
    int bundles = 0;
    for (int i = 0; i < 8; i++)
        bundles += written(x.engine, "audio.audio_bundle_settings.audio_bundle_settings_" + std::to_string(i) +
                                         "._metadata_") == "cs17_mercedes";
    check(bundles == 1 && !has_label("audio.audio_bundle_settings", "audio_bundle_settings", "v10_screamer").empty(),
          "an engine sound variant with its base's bundle adds no second row for it");
    int refused_rows = 0;
    for (auto &[k, n] : x.refused)
        refused_rows += n;
    check(refused_rows == 0 && x.records["vehicle"] == 3 && x.records["engine_sound"] == 3,
          "every base and variant applies");
    check(written(x.engine, "entrants.entrant_setup_modes.entrant_setup_mode_0.entrant_lists") ==
              "f1_2025,cs_f1_2017,esp_f1_2017_v10",
          "both lists join the setup mode, in load order");
    std::size_t base_keys = x.addon_fresh["swofty.classic-seasons-2017"], var_keys = x.addon_fresh["mod.esp"];
    check(base_keys > 0 && var_keys > 0 && x.fresh["vehicle"] > 0, "each addon's new keys are counted to it");

    auto adm = content::admit(st.order(), x.addon_fresh, base_keys + var_keys - 1);
    check(adm.skip.size() == 1 && adm.skip.count("mod.esp"), "past the room, the later addon is left out whole");
    content::Expansion y(db.base());
    content::expand(st, y, stat, {"swofty.classic-seasons-2017"});
    check(y.records["content.package"] == 1 && written(y.engine, "entrants.entrant_lists.entrant_list_1._metadata_") ==
                                                   "esp_f1_2017_v10",
          "with the first addon left out, the next one's rows start at the table's next index");
}

int main(int argc, char **argv)
{
    engine_tests();
    if (argc > 1)
        variant_tests(argv[1]);
    std::printf("%s\n", failures ? "FAILED" : "all passed");
    return failures ? 1 : 0;
}
