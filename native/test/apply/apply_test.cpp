#include "../../src/core/apply/database.hpp"
#include "../../src/core/apply/ids.hpp"
#include "../../src/core/apply/room.hpp"
#include "../../src/core/apply/scheduler.hpp"
#include "../../src/core/registry/table.hpp"

#include <cstdio>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

namespace ap = swofty_core::apply;
namespace reg = swofty_core::registry;
using swofty::json::Value;

namespace {

int failures = 0, checks = 0;

void check(bool ok, const std::string &what)
{
    checks++;
    if (!ok) {
        failures++;
        std::printf("FAIL: %s\n", what.c_str());
    }
}

std::vector<std::string> calls, logs;

struct TestApplier : ap::Applier {
    std::string name;
    reg::Applies kind;
    std::size_t keys;
    std::string seen;

    TestApplier(std::string n, reg::Applies k, std::size_t keys_ = 0) : name(std::move(n)), kind(k), keys(keys_) {}
    const char *registry() const override { return name.c_str(); }
    reg::Applies applies() const override { return kind; }
    std::size_t database_keys(reg::Store &) override
    {
        calls.push_back(name + ".keys");
        return keys;
    }
    void boot(reg::Store &, ap::Report &r) override
    {
        calls.push_back(name + ".boot");
        if (ap::db::staging().set("mod.a", name, "test.row.a", "int", "1"))
            r.applied++;
        else
            r.refused++;
    }
    void activate(reg::Store &st, const reg::Context &c, ap::Report &r) override
    {
        calls.push_back(name + ".activate " + (c.content.empty() ? std::string("-") : c.content.front()));
        reg::Record rec = st.get(name, "game/pkg");
        seen = rec.status == reg::Status::ok ? swofty::json::dump(rec.value["n"]) : "none";
        r.applied++;
        r.lines.push_back("pkg n = " + seen);
    }
    void deactivate(reg::Store &, ap::Report &r) override
    {
        calls.push_back(name + ".deactivate");
        r.put_back++;
    }
    void session_load(reg::Store &, const reg::Context &c, ap::Report &r) override
    {
        calls.push_back(name + ".session " + c.session_type);
        r.applied++;
    }
    void session_end(reg::Store &, ap::Report &r) override
    {
        calls.push_back(name + ".session_end");
        r.put_back++;
    }
    void install_read(reg::Store &, ap::Report &) override { calls.push_back(name + ".read"); }
};

std::string joined(const std::vector<std::string> &v)
{
    std::string s;
    for (auto &x : v)
        s += (s.empty() ? "" : " | ") + x;
    return s;
}

bool logged(const std::string &needle)
{
    for (auto &l : logs)
        if (l.find(needle) != std::string::npos)
            return true;
    return false;
}

void declare(reg::Store &st, const std::string &name, reg::Applies a)
{
    reg::Def d;
    d.name = name;
    d.applies = a;
    st.declare_def(d);
}

void test_sizing()
{
    namespace rm = ap::room;
    check(rm::grows_at(rm::STOCK_SLOTS) == 838860, "2^20 slots grow at 838,860");
    check(rm::grows_at(uint64_t(1) << 21) == 1677721, "2^21 slots grow at 1,677,721");
    check(rm::slots_for(742857) == rm::STOCK_SLOTS, "the stock file sizes 2^20");
    check(rm::slots_for(838860) == rm::STOCK_SLOTS, "838,860 records still fit 2^20");
    check(rm::slots_for(838861) == (uint64_t(1) << 21), "838,861 records size 2^21");
    check(rm::slots_for(843179) == (uint64_t(1) << 21), "the 1.5.11 file route's 843,179 records size 2^21");
    check(rm::slots_for(1677722) == (uint64_t(1) << 22), "1,677,722 records size 2^22");
    check(rm::slots_for(0) == 8 && rm::slots_for(6) == 8 && rm::slots_for(7) == 16, "the smallest map is 8 slots");
    auto t = rm::target(742857, 84112, 103984);
    check(t.want == 742857 + 84112 + 103984 + 32 + 4096 && !t.capped, "the target adds edits, records, probes and margin");
    check(t.want == 935081, "W7's example target is 935,081");
    check(rm::slots_for(t.want) == (uint64_t(1) << 21), "W7's example reserves 2^21");
    auto big = rm::target(742857, 900000, 200000);
    check(big.capped && big.want == rm::CAP && big.capped_from == 742857 + 900000 + 200000 + 32 + 4096,
          "the target is capped at 1,677,721");
    check(rm::slots_for(big.want) == (uint64_t(1) << 21), "the cap never asks for 2^22");
    auto small = rm::target(742857, 10, 0);
    check(rm::slots_for(small.want) == rm::STOCK_SLOTS, "a small profile keeps the stock 2^20 map");
    check(rm::left(rm::STOCK_SLOTS, 742857) == 838860 - 742857 - 4096, "the unsized room is about 92,000 records");
    check(rm::left(8, 100) == 0, "a full map has no room");
}

void test_budget()
{
    namespace rm = ap::room;
    auto b = rm::admit(100, {{"a", 40, 50}, {"b", 70, 70}, {"c", 60, 61}, {"d", 0, 5}});
    check(b.addons.size() == 4, "every addon gets a verdict");
    check(b.addons[0].admitted && !b.addons[1].admitted && b.addons[2].admitted && b.addons[3].admitted,
          "whole addons are admitted in load order while each fits what is left: " +
              std::to_string(b.addons[0].admitted) + std::to_string(b.addons[1].admitted) +
              std::to_string(b.addons[2].admitted) + std::to_string(b.addons[3].admitted));
    check(b.admitted_keys == 100 && b.refused_keys == 70 && b.asked_keys == 170, "the budget counts asked, admitted, refused");
    auto none = rm::admit(0, {{"a", 1, 1}, {"b", 0, 3}});
    check(!none.addons[0].admitted && none.addons[1].admitted, "an addon that only overwrites always fits");
}

void test_room_json()
{
    namespace rm = ap::room;
    rm::Report r;
    r.pe_timestamp = 1789118616;
    r.presize = "applied";
    r.profile = "Classic";
    r.loaded = true;
    r.slots = uint64_t(1) << 21;
    r.file_records = 742857;
    r.reserved_for = 935081;
    r.older_asked = 84112;
    r.older_new = 80120;
    r.registry_asked = 103984;
    r.registry_new = 103984;
    r.addons["swofty.f1-1990"] = {15020, 15020};
    r.probes = 7;
    r.count = 926968;
    r.info_inc = 32;
    Value v = rm::to_json(r);
    check(v["version"].as_int() == 1 && v["presize"].as_string() == "applied", "room.json carries version and presize");
    check(v["profile"].as_string() == "Classic", "room.json carries the profile");
    check(v["slots"].as_int() == 2097152 && v["grows_at"].as_int() == 1677721, "room.json carries slots and grows_at");
    check(v["room"].as_int() == 1677721 - 926968 - 4096, "room = grows_at - count - margin");
    check(v["registry"]["addons"]["swofty.f1-1990"]["applied"].as_int() == 15020, "room.json lists each addon");
    check(v["older_edits"]["new_keys"].as_int() == 80120, "room.json counts older edits' new keys");
    check(rm::line(r).find("value map sized for 1,677,721 records before load (older addons' edits 84,112; 1.6 "
                           "records 103,984)") != std::string::npos,
          "the room line says what it sized for: " + rm::line(r));
    rm::Report empty;
    Value e = rm::to_json(empty);
    check(e["presize"].as_string() == "not_needed" && e["profile"].as_string() == "default" && !e.contains("slots"),
          "a boot with nothing to size reports not_needed and no map numbers");
    check(rm::line(empty).find("nothing to size") != std::string::npos, "the empty room line");
    rm::Report un;
    un.presize = "unavailable";
    un.reason = "signature";
    un.loaded = true;
    un.slots = rm::STOCK_SLOTS;
    un.count = 81240;
    check(rm::line(un).find("pre-size unavailable (signature); 81,240 of 838,860 records used") != std::string::npos,
          "the unavailable room line: " + rm::line(un));
}

void test_ids()
{
    ap::ids::Store s;
    s.define_pool("vehicle", {{{515, 517}}});
    auto a = s.allocate("vehicle", "x/a", "2026-10-28");
    auto b = s.allocate("vehicle", "x/b", "2026-10-28");
    check(a.id == 515 && b.id == 516, "ids come from the pool in order");
    check(a.line.find("pool 515-517, 2 free after this") != std::string::npos, "the allocation line: " + a.line);
    check(s.allocate("vehicle", "x/a", "2026-10-29").id == 515, "a record keeps its id");
    ap::ids::Store t;
    check(t.load(s.dump()), "ids.json round-trips");
    check(t.get("vehicle", "x/b").id == 516 && t.get("vehicle", "x/b").how == "kept", "ids.json keeps ids across boots");
    t.define_pool("vehicle", {{{515, 517}}});
    auto map = t.map_for_career("career1");
    check(map["vehicle"]["x/a"] == 515 && map["vehicle"]["x/b"] == 516, "a career's map lists the live ids");
    t.set_live("vehicle", {"x/c"});
    check(t.allocate("vehicle", "x/c", "d").id == 517, "a new record takes a never-used id first");
    check(t.allocate("vehicle", "x/d", "d").id == -1, "ids a career uses are never reused, even for removed records");
    t.forget_career("career1");
    auto d = t.allocate("vehicle", "x/d", "d");
    check(d.id == 515 && d.how == "reclaimed", "an id is reused once no record and no career uses it");
    check(t.get("vehicle", "x/a").id == -1, "the reclaimed record loses its entry");
    ap::ids::Store u;
    u.define_pool("vehicle", {{{515, 520}}});
    u.allocate("vehicle", "x/a", "d");
    ap::ids::CareerMap synced{{"vehicle", {{"x/a", 515}, {"x/z", 518}, {"x/y", 515}}}};
    auto clashes = u.adopt("career2", synced, "d");
    check(clashes.size() == 1 && clashes[0].record == "x/y" && clashes[0].holder == "x/a",
          "a saved id another record holds stops the adopt and names both records");
    check(u.get("vehicle", "x/z").id == 518, "a free saved id is taken");
    check(u.allocate("vehicle", "x/new", "d").id == 516, "a taken saved id is not handed out again");
    auto pin = u.pin("vehicle", "x/explicit", 519, "d");
    check(pin.id == 519 && pin.how == "explicit", "an explicit stat_id is kept");
    check(u.allocate("vehicle", "x/next", "d").id == 517, "an explicit id is not allocated to others");
    ap::ids::Store none;
    check(none.allocate("vehicle", "x/a", "d").id == -1, "no pool, no id");
}

void test_scheduler()
{
    reg::Store &st = reg::store();
    st.set_order({"mod.a", "mod.b"});
    declare(st, "t.boot1", reg::Applies::boot);
    declare(st, "t.boot2", reg::Applies::boot);
    declare(st, "t.content", reg::Applies::content);
    declare(st, "t.session", reg::Applies::session);
    declare(st, "t.read", reg::Applies::read);
    declare(st, "content.package", reg::Applies::content);
    st.add_base("t.content", "pkg", Value::object({{"n", Value(0)}}));
    std::string id;
    st.add("mod.a", "content.package", "f1_2017",
           Value::object({{"entrant_list", Value("cs_f1_2017")}, {"season", Value(2017)}}), &id);
    check(id == "mod.a/f1_2017", "the test package is added: " + id);
    reg::Change ch;
    ch.patch = Value::object({{"entrant_list", Value("cs_f1_2017_v10")}});
    st.variant("mod.b", "content.package", "f1_2017_v10", "mod.a/f1_2017", ch, &id);
    reg::LayerSpec L;
    L.addon = "mod.b";
    L.registry = "t.content";
    L.id = "game/pkg";
    L.in = Value::array({Value::object({{"content", Value("mod.a/f1_2017")}})});
    L.change.patch = Value::object({{"n", Value(7)}});
    uint32_t handle = 0;
    check(st.layer(L, &handle) == SWOFTY_ASSET_OK, "an .in layer is registered");
    st.end_register();

    ap::clear_appliers();
    auto &b1 = ap::make<TestApplier>("t.boot1", reg::Applies::boot, 3);
    ap::make<TestApplier>("t.content", reg::Applies::content);
    ap::make<TestApplier>("t.boot2", reg::Applies::boot, 4);
    ap::make<TestApplier>("t.session", reg::Applies::session);
    ap::make<TestApplier>("t.read", reg::Applies::read);
    check(ap::appliers().size() == 5, "add() keeps every applier");

    ap::Scheduler s(st);
    s.set_log([](const std::string &l) { logs.push_back(l); });
    std::vector<reg::Context> facts;
    s.set_facts([&](const reg::Context &c) { facts.push_back(c); });
    calls.clear();
    check(s.plan_boot() == 7, "the plan sums database_keys of the boot appliers");
    check(s.plan_boot() == 7 && joined(calls) == "t.boot1.keys | t.boot2.keys", "database_keys runs once per boot applier, in order: " + joined(calls));
    check(s.boot_work(), "keys make boot work");
    calls.clear();
    ap::db::staging().open([](const std::string &p) { return p == "stock.row"; });
    check(ap::db::staging().exists("stock.row") && !ap::db::staging().exists("x"), "the staging answers exists");
    s.run_boot();
    auto rows = ap::db::staging().close();
    check(joined(calls) == "t.boot1.boot | t.boot2.boot", "boot runs every boot applier once, in order: " + joined(calls));
    check(rows.size() == 2 && rows[0].registry == "t.boot1", "boot appliers stage database rows");
    s.run_boot();
    check(calls.size() == 2, "boot runs once");
    calls.clear();
    s.run_boot();
    ap::Scheduler late(st);
    late.run_boot();
    check(b1.name == "t.boot1", "the applier stays alive");
    check(logged("[apply] t.boot1 boot: 1 applied, 0 put back, 0 refused"), "each applier logs its moment");
    check(logged("[apply] boot: 2 applier(s); 2 applied"), "the boot moment logs a summary");

    calls.clear();
    logs.clear();
    s.content_picked("cs_f1_2017");
    check(joined(calls) == "t.content.activate mod.a/f1_2017", "a box press activates its package: " + joined(calls));
    check(logged("pkg n = 7"), "an .in(content) layer applies while its content is active");
    check(st.context().content.size() == 1 && st.context().has_year && st.context().year == 2017,
          "the store's context carries the content and its year");
    check(!facts.empty() && facts.back().content.front() == "mod.a/f1_2017", "the event facts follow the moment");
    calls.clear();
    s.content_picked("cs_f1_2017_v10");
    check(joined(calls) == "t.content.deactivate | t.content.activate mod.b/f1_2017_v10",
          "another box puts the first back before it applies: " + joined(calls));
    check(st.context().content.size() == 2 && st.context().content[1] == "mod.a/f1_2017",
          "the content chain includes the variant's base");
    check(logged("pkg n = 7"), "content() matches the variant chain");
    calls.clear();
    s.content_picked("f1_2025");
    check(joined(calls) == "t.content.deactivate | t.content.activate -", "a stock box ends the addon content");
    check(logged("pkg n = 0"), "the .in layer is off once its content ends");
    check(st.context().content.empty(), "a stock box has no 1.6 content");

    calls.clear();
    s.career_loaded("f1_career_progressive/6f3e9a", "driver_career", "f1_career_progressive");
    check(joined(calls) == "t.content.deactivate | t.content.activate -", "a career load puts back and activates");
    check(st.context().career == "f1_career_progressive/6f3e9a" && st.context().data_group == "f1_career_progressive" &&
              st.context().mode == "driver_career",
          "the career's context reaches the store");
    calls.clear();
    s.career_season(2017);
    check(st.context().has_year && st.context().year == 2017, "a season start sets the year");
    check(joined(calls) == "t.content.deactivate | t.content.activate -", "a season start re-applies content");
    calls.clear();
    s.session_load("race", "game/monza");
    check(joined(calls) == "t.session.session race", "a session load runs the session appliers");
    check(st.context().session_type == "race" && st.context().track == "game/monza", "the session's context");
    calls.clear();
    s.session_load("qualifying", "");
    check(joined(calls) == "t.session.session_end | t.session.session qualifying",
          "a second session load ends the first: " + joined(calls));
    calls.clear();
    s.career_unloaded();
    check(joined(calls) == "t.session.session_end | t.content.deactivate", "a career unload ends the session and puts back");
    check(st.context().career.empty() && !st.context().has_year && st.context().content.empty(),
          "the career's context is cleared");
    calls.clear();
    s.install_read();
    s.install_read();
    check(joined(calls) == "t.read.read", "the read path installs once");
    calls.clear();
    s.deactivate("nothing active");
    check(calls.empty(), "nothing active, nothing to put back");
}

void test_stat_id_and_reports(const std::string &dir)
{
    reg::Store &st = reg::store();
    declare(st, "vehicle", reg::Applies::boot);
    ap::ids::Store ids;
    ids.define_pool("vehicle", {{{515, 538}}});
    std::string id;
    reg::Store fresh;
    reg::Def d;
    d.name = "vehicle";
    d.applies = reg::Applies::boot;
    fresh.declare_def(d);
    fresh.set_order({"mod.a"});
    fresh.add("mod.a", "vehicle", "ferrari", Value::object({{"label", Value("f")}}), &id);
    fresh.add("mod.a", "vehicle", "mclaren", Value::object({{"stat_id", Value(530)}}), &id);
    int64_t out = -1;
    std::string line;
    check(ap::stat_id(fresh, ids, "vehicle", "mod.a/ferrari", &out, "d", &line) == SWOFTY_ASSET_OK && out == 515,
          "stat_id allocates from the pool");
    check(line.find("vehicle mod.a/ferrari -> 515") != std::string::npos, "the stat id line: " + line);
    check(ap::stat_id(fresh, ids, "vehicle", "mod.a/mclaren", &out, "d", &line) == SWOFTY_ASSET_OK && out == 530,
          "an explicit stat_id is the record's id");
    check(ap::stat_id(fresh, ids, "vehicle", "mod.a/none", &out, "d", nullptr) != SWOFTY_ASSET_OK && out == -1,
          "a missing record has no stat id");

    int r = reg::write_reports(dir, [](Value &, Value &conflicts) {
        conflicts["status"] = Value::object({{"mod.a", Value::array({Value("12 vehicle record(s) refused")})}});
    });
    check(r == SWOFTY_ASSET_OK, "the reports are written");
    std::ifstream f(dir + "/conflicts.json");
    std::stringstream ss;
    ss << f.rdbuf();
    Value c = swofty::json::parse(ss.str(), nullptr);
    check(c["status"]["mod.a"][0].as_string() == "12 vehicle record(s) refused", "conflicts.json carries status lines");
    check(c["registry"].is_array(), "conflicts.json keeps its registry list");
    std::ifstream e(dir + "/effective.json");
    check(bool(e), "effective.json is written next to it");
}

}

int main(int argc, char **argv)
{
    std::string dir = argc > 1 ? argv[1] : ".";
    test_sizing();
    test_budget();
    test_room_json();
    test_ids();
    test_scheduler();
    test_stat_id_and_reports(dir);
    std::printf("apply: %d check(s), %d failure(s)\n", checks, failures);
    return failures ? 1 : 0;
}
