// Host test for fits.hpp and facts_file.hpp: the frozen fit rules (garages, grid clamped to the install's grid size,
// AI folders and difficulties, Time Trial), the world choice by setup mode and season, missing facts, the guard's
// popup text and a box made from a content.package record. run_track_lists_test.sh builds and runs it.
#include "fixture.hpp"

#include <map>

int main()
{
    std::string why;
    trk::Facts facts = trk::parse_facts(FACTS, &why);
    expect(facts.loaded && facts.grid_size == 22 && facts.layers.size() == 3 && facts.tracks.size() == 6,
           "track-facts.json parses: grid size, three layers, six tracks");
    expect(!trk::parse_facts(R"({"version": 2})", &why).loaded && why.find("version 2") != std::string::npos,
           "a version 2 file is not read");
    expect(!trk::parse_facts("[1]", &why).loaded, "a file that isn't an object is not read");
    std::string line;
    expect(!trk::facts_profile_matches(facts, "Other", &line) &&
               line == "[tracks] track-facts.json is for profile Classic Seasons; this build is Other: no facts",
           "a file for another profile gives no facts, with the C2 line");
    expect(trk::facts_profile_matches(facts, "Classic Seasons", nullptr) && trk::facts_profile_matches(facts, "", nullptr),
           "the same profile, or no index profile, reads the facts");

    trk::Base base = cs_base();
    trk::BoxInfo box = cs2016();
    expect(trk::worlds_for(facts, box) == std::vector<std::string>{"f1"}, "swofty_f1 in season_2016 loads the f1 world");
    trk::BoxInfo no_season = box;
    no_season.season.clear();
    expect(trk::worlds_for(facts, no_season) == std::vector<std::string>{"f1_26", "f1"},
           "without a season every swofty_f1 layer's world counts");
    trk::BoxInfo unknown_season = box;
    unknown_season.season = "season_1990";
    expect(trk::worlds_for(facts, unknown_season).size() == 2, "a season no layer names falls back to every layer");

    trk::Fit f = trk::fits(box, *base.by_code("melbourne"), trk::Mode::grand_prix, &facts);
    expect(f.ok && f.no_facts.empty(), "Melbourne fits the 11-team 2016 box");

    f = trk::fits(box, *base.by_code("igora_drive"), trk::Mode::grand_prix, &facts);
    expect(!f.ok && f.rule == trk::Rule::garages && f.reason == "its swofty_f1 world has 10 garages; the box has 11 teams",
           "an 11-team box and a 10-garage world: left out with the frozen reason");
    expect(trk::fits(cs2016(10), *base.by_code("igora_drive"), trk::Mode::grand_prix, &facts).ok,
           "a 10-team box fits the 10-garage world");

    f = trk::fits(box, *base.by_code("monaco"), trk::Mode::grand_prix, &facts);
    expect(!f.ok && f.rule == trk::Rule::grid && f.reason == "its swofty_f1 world has 20 grid slots; the box runs 22 cars",
           "a 22-car box and a 20-slot world");
    trk::BoxInfo big = cs2016(12);
    f = trk::fits(big, *base.by_code("okayama"), trk::Mode::grand_prix, &facts);
    expect(f.ok, "a 24-car box clamped to the install's 22 fits a 24-slot world");
    f = trk::fits(big, *base.by_code("melbourne"), trk::Mode::grand_prix, &facts);
    expect(!f.ok && f.rule == trk::Rule::garages, "a 12-team box needs 12 garages");
    trk::BoxInfo grid_only = cs2016();
    grid_only.cars = 24;
    f = trk::fits(grid_only, *base.by_code("bahrain"), trk::Mode::grand_prix, &facts);
    expect(f.ok, "24 cars clamp to the install grid of 22: a 22-slot world fits");

    trk::BoxInfo f1990 = cs2016(10);
    f1990.package = "swofty.f1-1990/f1_1990";
    for (auto &c : f1990.ai)
        c = {c.team, "f1_1990", "f1_1990_car"};
    f = trk::fits(f1990, *base.by_code("melbourne"), trk::Mode::grand_prix, &facts);
    expect(f.ok, "F1 1990 fits a track with its AI folder and difficulty");
    f = trk::fits(f1990, *base.by_code("bahrain"), trk::Mode::grand_prix, &facts);
    expect(!f.ok && f.rule == trk::Rule::ai && f.reason == "no AI data for f1_1990/f1_1990_car (team_0)",
           "F1 1990 is left out of a track without its AI folder");
    trk::Facts no_diff = facts;
    no_diff.tracks["melbourne"].ai_difficulties = {"f1_2025"};
    f = trk::fits(f1990, *base.by_code("melbourne"), trk::Mode::grand_prix, &no_diff);
    expect(!f.ok && f.rule == trk::Rule::difficulties && f.reason == "no ai_track_difficulties for f1_1990",
           "an AI folder without the package's difficulty is left out");

    f = trk::fits(box, *base.by_code("okayama"), trk::Mode::time_trial, &facts);
    expect(f.ok, "D5: a track made for 1.5 runs Time Trial");
    expect(trk::fits(box, *base.by_code("okayama"), trk::Mode::time_trial, nullptr).ok,
           "a track made for 1.5 runs Time Trial without facts too");
    trk::Facts no_world = facts;
    no_world.tracks["okayama"].time_trial = false;
    no_world.tracks["okayama"].time_trial_why = "its files hold none of the worlds the game's track layers load";
    f = trk::fits(box, *base.by_code("okayama"), trk::Mode::time_trial, &no_world);
    expect(!f.ok && f.rule == trk::Rule::time_trial &&
               f.reason == "no Time Trial support (its files hold none of the worlds the game's track layers load)",
           "a track whose facts say time_trial false is left out of Time Trial with the facts' reason");
    expect(trk::fits(box, *base.by_code("okayama"), trk::Mode::grand_prix, &no_world).ok,
           "time_trial false in the facts leaves Grand Prix alone");
    trk::Facts parsed_tt = trk::parse_facts(R"({"version": 1, "grid_size": 22, "layers": [], "tracks": {
        "a": {"worlds": {}, "time_trial": false, "time_trial_why": "no world"}, "b": {"worlds": {}}}})");
    expect(!parsed_tt.track("a")->time_trial && parsed_tt.track("a")->time_trial_why == "no world" &&
               parsed_tt.track("b")->time_trial && parsed_tt.track("b")->time_trial_why.empty(),
           "the facts' time_trial and time_trial_why parse; time_trial defaults to true");
    f = trk::fits(box, *base.by_code("bahrain24"), trk::Mode::time_trial, &facts);
    expect(!f.ok && f.rule == trk::Rule::time_trial, "a record track with time_trial false is left out of Time Trial");
    trk::BaseTrack tt = *base.by_code("bahrain24");
    tt.time_trial = true;
    expect(trk::fits(box, tt, trk::Mode::time_trial, &facts).ok, "a record track with time_trial true runs Time Trial");
    trk::Facts record_no_world = facts;
    record_no_world.tracks["bahrain24"].time_trial = false;
    expect(!trk::fits(box, tt, trk::Mode::time_trial, &record_no_world).ok,
           "a record track with time_trial true but no world Time Trial loads is left out");
    expect(trk::fits(box, *base.by_code("melbourne"), trk::Mode::time_trial, &facts).ok, "a stock track runs Time Trial");

    f = trk::fits(box, *base.by_code("melbourne_reverse"), trk::Mode::grand_prix, &facts);
    expect(f.ok && f.no_facts.size() == 1 && f.no_facts.front() == "no facts for melbourne_reverse",
           "a track with no facts passes and says so");
    f = trk::fits(box, *base.by_code("igora_drive"), trk::Mode::grand_prix, nullptr);
    expect(f.ok && f.no_facts.front() == "no track-facts.json", "no facts file: every rule passes");
    trk::BoxInfo other_mode = box;
    other_mode.setup_mode = "f2_mode";
    f = trk::fits(other_mode, *base.by_code("igora_drive"), trk::Mode::grand_prix, &facts);
    expect(f.ok && f.no_facts.front() == "no track layer for setup mode f2_mode", "a setup mode with no layer passes");
    trk::Facts built = trk::parse_facts(R"({"version": 1, "grid_size": 22, "layers": [
        {"label": "f1", "setup_modes": ["f1"], "seasons": ["season_2023", "season_2024", "season_2025"], "world": "f1"},
        {"label": "f1_26", "setup_modes": ["f1_26"], "seasons": ["season_2026"], "world": "f1_26"},
        {"label": "swofty_f1", "setup_modes": ["swofty_f1"], "seasons": ["season_2023", "season_2024", "season_2025"], "world": "f1_26"}],
      "tracks": {
        "melbourne": {"worlds": {"f1": {"grid": 22, "garages": 10, "pit_boxes": 10}, "f1_26": {"grid": 22, "garages": 11, "pit_boxes": 11}}},
        "igora_drive": {"worlds": {"f1": {"grid": 22, "garages": 10, "pit_boxes": 10}, "f1_26": {"grid": 22, "garages": 10, "pit_boxes": 10}}}}})");
    trk::BoxInfo cs = cs2016();
    cs.season = "season_2025";
    expect(trk::worlds_for(built, cs) == std::vector<std::string>{"f1_26"},
           "Classic Seasons' swofty_f1 layer (a copy of f1_26) gives an 11-team box the f1_26 world, not f1");
    expect(trk::fits(cs, *base.by_code("melbourne"), trk::Mode::grand_prix, &built).ok,
           "an 11-team box fits a stock track's 11-garage f1_26 world");
    f = trk::fits(cs, *base.by_code("igora_drive"), trk::Mode::grand_prix, &built);
    expect(!f.ok && f.rule == trk::Rule::garages && f.reason == "its swofty_f1 world has 10 garages; the box has 11 teams",
           "a track whose f1_26 world has 10 garages is left out of an 11-team box");
    trk::Facts no_layer = built;
    no_layer.layers.pop_back();
    f = trk::fits(cs, *base.by_code("igora_drive"), trk::Mode::grand_prix, &no_layer);
    expect(f.ok && f.no_facts.front() == "no track layer for setup mode swofty_f1",
           "facts without a swofty_f1 layer pass the world rules and say so; the core doesn't guess a layer");

    trk::BoxInfo stock;
    stock.package = "game/f1_2026";
    f = trk::fits(stock, *base.by_code("igora_drive"), trk::Mode::grand_prix, &facts);
    expect(f.ok && f.no_facts.size() >= 2, "a stock box without a record: unknown teams and cars pass");
    trk::BoxInfo half = box;
    half.ai[3].folder.clear();
    f = trk::fits(half, *base.by_code("melbourne"), trk::Mode::grand_prix, &facts);
    expect(f.ok && f.no_facts.size() == 1 && f.no_facts.front() == "the AI folder of team_3",
           "a car whose AI folder is unknown passes and is named");

    trk::Decision d = trk::decide(base.by_code("igora_drive"), box, trk::Mode::grand_prix,
                                  trk::fits(box, *base.by_code("igora_drive"), trk::Mode::grand_prix, &facts), "");
    expect(!d.ok && d.title == "Igora Drive can't run in F1 2016" && d.body == "Its garages hold 10 teams. F1 2016 has 11.",
           "the guard's popup is the one tracks.md shows");
    expect(d.line == "[tracks] guard: mod.miami24/igora_drive in swofty.classic-seasons-2016/f1_2016 (grand_prix): refused "
                     "(its swofty_f1 world has 10 garages; the box has 11 teams)",
           "the guard's refusal line names the track, box, mode and reason");
    d = trk::decide(base.by_code("monaco"), box, trk::Mode::grand_prix,
                    trk::fits(box, *base.by_code("monaco"), trk::Mode::grand_prix, &facts), "Monaco");
    expect(!d.ok && d.body == "Its grid holds 20 cars. F1 2016 runs 22.", "the grid popup");
    d = trk::decide(base.by_code("okayama"), box, trk::Mode::time_trial,
                    trk::fits(box, *base.by_code("okayama"), trk::Mode::time_trial, &facts), "");
    expect(d.ok, "D5: the guard lets a track made for 1.5 run Time Trial");
    d = trk::decide(base.by_code("bahrain24"), box, trk::Mode::time_trial,
                    trk::fits(box, *base.by_code("bahrain24"), trk::Mode::time_trial, &facts), "");
    expect(!d.ok && d.title == "Sepang can't run in F1 2016" && d.body == "It has no Time Trial support.",
           "a record track without time_trial: the guard's Time Trial popup");
    d = trk::decide(base.by_code("melbourne"), box, trk::Mode::grand_prix,
                    trk::fits(box, *base.by_code("melbourne"), trk::Mode::grand_prix, &facts), "");
    expect(d.ok && d.line.find(": fits") != std::string::npos, "a fitting track is allowed");
    d = trk::decide(nullptr, box, trk::Mode::grand_prix, {}, "mystery");
    expect(d.ok && d.line.find("not in the track base; allowed") != std::string::npos, "a track outside the base is allowed");

    std::map<std::string, Value> recs;
    recs["vehicle|cs/ferrari_2016"] = Value::object({{"label", Value("ferrari_2016")},
                                                     {"donor", Value("game/ferrari_25")},
                                                     {"package", Value("cs/pkg_2016")},
                                                     {"ai", Value::object({{"asset_filename", Value("auto")}})}});
    recs["vehicle.package|cs/pkg_2016"] = Value::object({{"ai_package_folder", Value("f1_2016")}});
    recs["vehicle|cs/mclaren_2016"] = Value::object({{"label", Value("mclaren_2016")},
                                                     {"ai", Value::object({{"asset_filename", Value("mclaren_car")}})}});
    trk::Lookup lookup = [&](const std::string &reg, const std::string &id) -> const Value * {
        auto it = recs.find(reg + "|" + id);
        return it == recs.end() ? nullptr : &it->second;
    };
    std::map<std::string, std::string> db = {
        {"entrants.vehicles.vehicle[_metadata_=ferrari_2016].asset_filename", "ferrari_car"},
        {"entrants.vehicles.vehicle[_metadata_=mclaren_2016].vehicle_package", "f1_2025"},
        {"entrants.vehicles.vehicle_packages.vehicle_package[_metadata_=f1_2025].ai_package_folder", "f1_2025"},
        {"entrants.entrant_lists.entrant_list[_metadata_=cs_2016].season", "season_2016"}};
    trk::TextRead read = [&](const std::string &p) -> std::optional<std::string> {
        auto it = db.find(p);
        return it == db.end() ? std::nullopt : std::optional<std::string>(it->second);
    };
    Value pkg = Value::object(
        {{"entrant_list", Value("cs_2016")},
         {"season", Value(2016)},
         {"title", Value::object({{"literal", Value("F1 2016")}})},
         {"teams", Value::array({Value::object({{"key", Value("ferrari")},
                                                {"vehicle", Value("cs/ferrari_2016")},
                                                {"drivers", Value::array({Value::object({}), Value::object({}), Value::object({})})}}),
                                 Value::object({{"key", Value("mclaren")}, {"vehicle", Value("cs/mclaren_2016")}}),
                                 Value::object({{"key", Value("haas")}})})}});
    trk::BoxInfo made = trk::box_from_package("cs/f1_2016", pkg, lookup, read);
    expect(made.title == "F1 2016" && made.setup_mode == "swofty_f1" && made.season == "season_2016" && made.teams == 3 &&
               made.cars == 7,
           "a box from a package record: title, default setup mode, the list's season, teams and cars (no seats: two)");
    expect(made.ai.size() == 3 && made.ai[0].folder == "f1_2016" && made.ai[0].asset == "ferrari_car" &&
               made.ai[1].folder == "f1_2025" && made.ai[1].asset == "mclaren_car" && made.ai[2].folder.empty(),
           "each car's AI folder from its package record or the database rows, its asset from the record or its row");

    std::map<std::string, int64_t> rows = {{"game_setup.game_setup_modes|swofty_f1|stat_id", 20},
                                           {"game_setup.game_setup_modes|f1|stat_id", 1},
                                           {"game_setup.game_setup_modes|broken|stat_id", 300}};
    std::vector<std::string> asked;
    trk::CellRead cell = [&](const std::string &t, const std::string &l, const std::string &f) -> std::optional<int64_t> {
        asked.push_back(t + "|" + l + "|" + f);
        auto it = rows.find(asked.back());
        return it == rows.end() ? std::nullopt : std::optional<int64_t>(it->second);
    };
    expect(trk::setup_mode_stat_of(cell, "swofty_f1") == 20 && trk::setup_mode_stat_of(cell, "f1") == 1 &&
               asked.front() == "game_setup.game_setup_modes|swofty_f1|stat_id",
           "a setup mode's stat id comes from its row by label, as an int (IG11: the [_metadata_=] text read missed it)");
    expect(trk::setup_mode_stat_of(cell, "f1_26") == -1 && trk::setup_mode_stat_of(cell, "broken") == -1 &&
               trk::setup_mode_stat_of(cell, "") == -1 && trk::setup_mode_stat_of({}, "swofty_f1") == -1,
           "no row, an id past 255, no mode or no database: -1");

    printf("%s\n", fails ? "fits_test: FAILED" : "fits_test: all passed");
    return fails ? 1 : 0;
}
