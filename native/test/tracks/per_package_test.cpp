// Host test for per_package.hpp and the box fuel plan of data_session.cpp (fuel_plan.hpp): entry targets by track,
// venue or the id's last code, a variant package inheriting its base's entries with its own winning, and the
// writes and put-back order of the fuel plan over a fake data set. run_track_lists_test.sh builds and runs it.
#include "fixture.hpp"

#include "../../src/core/apply/tracks/fuel_plan.hpp"

int main()
{
    trk::Base base = cs_base();
    std::vector<std::pair<std::string, Value>> recs = {
        {"cs/f1_2016/hockenheim", Value::object({{"venue", Value("venue/albert_park")},
                                                {"fuel", Value::object({{"burn_per_lap", Value(1.9)}})}})},
        {"cs/f1_2016/monaco", Value::object({{"offered", Value(false)}})},
        {"cs/f1_2016/sochi", Value::object({{"venue", Value("venue/sochi?")}})},
        {"cs/f1_2016/okay", Value::object({{"track", Value("mod.okayama/okayama")}, {"order", Value(2)}})},
        {"cs/f1_2016_v10/melbourne", Value::object({{"fuel", Value::object({{"burn_per_lap", Value(2.1)}})}})},
        {"cs/f1_2016_v10/monaco", Value::object({{"offered", Value(true)}, {"fuel", Value()}})}};
    trk::Entries e = trk::resolve_entries(base, recs);
    expect(e.by_track.size() == 3, "three targets: Melbourne, Monaco and Okayama");
    expect(e.of("game/melbourne") && e.of("game/melbourne")->id == "cs/f1_2016_v10/melbourne" &&
               e.of("game/melbourne")->burn && *e.of("game/melbourne")->burn > 2.09f,
           "the variant's own entry for a target wins over its base's (venue/albert_park resolves to Melbourne)");
    expect(e.of("game/monaco") && e.of("game/monaco")->offered && !e.of("game/monaco")->burn,
           "the variant offers Monaco again; a null fuel writes nothing");
    expect(e.of("mod.okayama/okayama") && e.of("mod.okayama/okayama")->order == 2, "an entry by track id keeps its order");
    expect(e.lines.size() == 1 && e.lines[0] == "[tracks] cs/f1_2016/sochi: no track for venue/sochi? (optional); nothing applied",
           "an entry whose venue has no track is skipped with a line");
    expect(trk::entry_of_package("cs/f1_2016/monaco", "cs/f1_2016") && !trk::entry_of_package("cs/f1_2016_v10/monaco", "cs/f1_2016") &&
               !trk::entry_of_package("cs/f1_2016", "cs/f1_2016") && !trk::entry_of_package("cs/f1_2016/a/b", "cs/f1_2016"),
           "a package's entries are the ids one level under it");

    trk::fuel::Set set;
    set.tracks.push_back({0, 0x1000, 0x10, "melbourne", {}});
    set.tracks.push_back({1, 0x2000, 0x20, "monaco", {}});
    set.tracks[0].fuel.burn = 1.3f;
    set.tracks[0].fuel.count = 1;
    set.tracks[1].fuel.burn = 1.1f;
    set.tracks[1].fuel.count = 1;
    std::vector<trk::FuelWrite> plan = trk::plan_box_fuel(base, e, {&set}, "");
    expect(plan.size() == 1 && plan[0].object == 0x1000 && plan[0].after.burn > 2.09f && plan[0].before.burn < 1.31f &&
               plan[0].entry == "cs/f1_2016_v10/melbourne",
           "the box's burn per lap goes to its track in the set the session reads");
    plan = trk::plan_box_fuel(base, e, {&set}, "monaco");
    expect(plan.empty(), "only the session's track is written when it is known");
    set.tracks[0].fuel.burn = 2.1f;
    plan = trk::plan_box_fuel(base, e, {&set}, "melbourne");
    expect(plan.empty(), "a track already at the box's value is not written");
    set.tracks[0].fuel.count = 9;
    set.tracks[0].fuel.burn = 1.3f;
    plan = trk::plan_box_fuel(base, e, {&set}, "");
    expect(plan.empty(), "a track whose fuel rows don't read as fuel is not written");

    std::vector<trk::FuelWrite> done = {{0x1000, "melbourne", "a", {}, {}}, {0x2000, "monaco", "b", {}, {}}};
    std::vector<uintptr_t> order;
    for (auto it = done.rbegin(); it != done.rend(); ++it)
        order.push_back(it->object);
    expect(order == std::vector<uintptr_t>{0x2000, 0x1000}, "writes are put back in reverse order");

    printf("%s\n", fails ? "per_package_test: FAILED" : "per_package_test: all passed");
    return fails ? 1 : 0;
}
