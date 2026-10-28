// Compiles every standard library 1.6 header and instantiates its templates. Built for x86_64-windows-gnu
// (compile and link) and for the host (run): with no core, every call reports unavailable and nothing
// crashes; records round-trip through their JSON form; schemas carry the registry keywords.
#include <cassert>
#include <cstdio>
#include <string>

#include <swofty/events.hpp>
#include <swofty/hooks.hpp>
#include <swofty/registry.hpp>
#include <swofty/services.hpp>
#include <swofty/systems/career.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/content.hpp>
#include <swofty/systems/sessions.hpp>
#include <swofty/systems/sounds.hpp>

#if defined(_WIN32)
#include <swofty/assets.hpp>
#include <swofty/career_content.hpp>
#include <swofty/career_wizard.hpp>
#include <swofty/characters.hpp>
#include <swofty/db.hpp>
#include <swofty/hub.hpp>
#include <swofty/hud.hpp>
#include <swofty/lang.hpp>
#include <swofty/popup.hpp>
#include <swofty/storage.hpp>
#include <swofty/telemetry.hpp>
#endif

using namespace swofty;

static int failures = 0;
#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d: %s\n", __FILE__, __LINE__, #c);                                                   \
            failures++;                                                                                                \
        }                                                                                                              \
    } while (0)

struct ShopApi {
    static constexpr const char *name = "test.shop";
    static constexpr uint32_t version = 1;
    uint32_t size;
    int (*count)(void);
};

struct ShopPoint {
    static constexpr const char *point = "test.shop.open";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    int day;
};

namespace test {
struct Thing {
    static constexpr Version version{1, 2};
    std::string name;
    int n = 0;
    json::Value extra;
};
SWOFTY_RECORD(Thing, name, n)
inline Registry<Thing> things{"test.thing"};
}

SWOFTY_REGISTER
{
    auto v10 = sounds::engine_sounds.add("v10_screamer", {
                                                             .bundle = file("sounds/v10_screamer.bdl"),
                                                             .settings_from = sounds::engine_sounds["game/ferrari_2025"],
                                                         });
    cars::vehicles.layer(cars::vehicles["swofty.classic-seasons-2017/ferrari"], [=](cars::Vehicle &v) { v.engine_sound = v10; });
}

static void unavailable_without_core()
{
    CHECK(!detail::registry_table());
    auto v10 = sounds::engine_sounds.add("v10", {.bundle = file("a.bdl")});
    CHECK(v10.empty());
    auto ferrari = cars::vehicles.variant("ferrari", cars::vehicles["game/ferrari_2025"], [](cars::Vehicle &v) { v.label = "x"; });
    CHECK(ferrari.empty());
    auto h = content::packages.layer(content::packages["swofty.classic-seasons-2017/f1_2017"],
                                     [](content::Package &p) { p.teams.at("ferrari").engine_sound = {}; });
    h.in(events::mode(events::Mode::time_trial)).in(events::content("swofty.classic-seasons-2017/f1_2017"));
    CHECK(h.result() == Result::unavailable);
    auto sel = cars::handling.layer(tagged<cars::Handling>(cars::handling.tag("c/f1_cars")), [](cars::Handling &x) {
        if (option<std::string>("wing_damage") == "heavy")
            x.wing_damage.damage_scale *= 2;
        x.upgrade("radUpgradeEngine").set("FuelTank", "FuelBsfcMin", '*', 0.85f);
    });
    CHECK(sel.result() == Result::unavailable);
    cars::vehicles.layer(under<cars::Vehicle>("game/").except({cars::vehicles["game/ferrari_2025"]}), [](cars::Vehicle &) {},
                         Scope::profile);
    cars::vehicles.layer(where<cars::Vehicle>([](const cars::Vehicle &v) { return v.performance_class == cars::Class::f1_26; }),
                         [](cars::Vehicle &) {});
    content::packages.layer(tagged(&content::Package::calendar, Tag<career::Calendar>("career.calendar", "c/x")),
                            [](content::Package &) {});
    CHECK(cars::handling.protect(cars::handling["mod.a/ferrari"], {"chassis", "f1_aero"}) == Result::unavailable);
    CHECK(!career::seasons.get(career::seasons["swofty.progressive/2017"]));
    CHECK(career::seasons.status(career::seasons["swofty.progressive/2017"]) == Status::no_registry);
    CHECK(career::seasons.all("swofty.progressive/").empty());
    CHECK(career::seasons.origin(career::seasons["swofty.progressive/2017"], "teams").empty());
    CHECK(!career::seasons.watch([](Ref<career::Season>) {}));
    CHECK(career::ratings.check([](const career::DriverRatings &r, Errors &e) {
              if (r.pace > 100)
                  e.add("pace", "0-100");
          }) == Result::unavailable);
    CHECK(career::seasons.provide([](std::string_view) -> std::optional<career::Season> { return std::nullopt; }) ==
          Result::unavailable);
    CHECK(career::seasons.invalidate() == Result::unavailable);
    CHECK(cars::vehicles.tag_add(cars::vehicles.tag("c/f1_cars"), cars::vehicles["mod.a/rb"]) == Result::unavailable);
    CHECK(!cars::vehicles.tag("c/f1_cars").contains(cars::vehicles["game/ferrari_2025"]));
    CHECK(cars::vehicles.tag("c/f1_cars").members().empty());
    CHECK(!cars::vehicles["game/ferrari_2025"].resolved());
    CHECK(option<int>("laps") == 0 && option<std::string>("x").empty() && !option<bool>("y"));
    CHECK(!cars::vehicles.stat_id(cars::vehicles["game/ferrari_2025"]));
    CHECK(last_error().empty());
    Registry<json::Value> untyped{"tails.whatif.presets"};
    CHECK(!untyped.get(untyped["tails.whatif-history/real_2016"]));
    CHECK((adapter<test::Thing, test::Thing>(test::things, test::things, [](test::Thing t) { return t; },
                                              [](test::Thing t) { return t; }) == Result::unavailable));
    CHECK((migrate<test::Thing, test::Thing>(test::things, test::things, [](test::Thing t) { return t; }) ==
           Result::unavailable));

    using events::any, events::content, events::Context, events::Mode, events::mode, events::not_, events::on,
        events::owner, events::track, events::year;
    CHECK(on<career::events::SeasonStart>()
              .in(content("swofty.progressive/f1_career_progressive"))
              .when(year(2016, 2019))
              .after([](const career::events::SeasonStart &, const Context &ctx) {
                  auto s = ctx.record<career::Season>();
                  (void)s;
                  (void)ctx.career();
                  (void)ctx.participants();
              }) == swofty::events::Result::unavailable);
    CHECK(on<career::events::AddActivity>().in(any({content("a/x"), not_(track("game/monaco"))})).replace(
              [](career::events::AddActivity &, const Context &) { return false; }) == swofty::events::Result::unavailable);
    CHECK(on<cars::events::Built>().before([](cars::events::Built &e, const Context &) { e.car.f1_aero().lift_scale = 1.01f; }) ==
          swofty::events::Result::unavailable);
    CHECK(on("swofty.progressive.seat_change").after([](std::span<const std::byte>, const Context &) {}) ==
          swofty::events::Result::unavailable);
    CHECK(events::define<ShopPoint>(owner("test")) == swofty::events::Result::unavailable);
    ShopPoint p{sizeof p, 1, 3};
    CHECK(events::fire(p));
    CHECK(!Context().career() && Context().participants().empty() && !Context().in(mode(Mode::grand_prix)));

    CHECK(hooks::before<career::events::AddActivity>([](career::events::AddActivity &) {}) == hooks::Result::unavailable);
    CHECK(hooks::before<career::events::AddActivity>(hooks::Priority{10}, [](career::events::AddActivity &) {}) ==
          hooks::Result::unavailable);
    CHECK(hooks::replace<ShopPoint>([](ShopPoint &) { return true; }) == hooks::Result::unavailable);
    CHECK(hooks::after<ShopPoint>([](const ShopPoint &, const hooks::Outcome &) {}) == hooks::Result::unavailable);
    CHECK(hooks::define<ShopPoint>() == hooks::Result::unavailable);
    CHECK(hooks::fire(p) && !hooks::live<ShopPoint>());

    static const ShopApi shop{sizeof(ShopApi), [] { return 3; }};
    CHECK(services::publish(shop) == services::Result::unavailable);
    CHECK(services::when<ShopApi>([](const ShopApi &) {}) == services::Result::unavailable);
    CHECK(!services::available());

    career::ApplyReport report;
    CHECK(career::apply_lineup(career::Season{}, &report) == career::Result::unavailable);
    CHECK(career::set_seat(7, 1, Ref<drivers::Identity>("game/pierre_gasly")) == career::Result::unavailable);
    CHECK(cars::reset_upgrades(0) == Result::unavailable);
    CHECK(!cars::live::available() && cars::live::cars().empty());
    CHECK(detail::run_register(nullptr) == 1);
}

static void round_trip()
{
    career::Season s{
        .data_group = "f1_career_progressive",
        .year = 2017,
        .box = content::packages["swofty.classic-seasons-2017/f1_2017"].or_else(content::packages["game/f1_2025"]),
        .calendar = career::calendars["swofty.progressive/2017"],
        .teams = {{.slot = 1,
                   .team = Ref<teams::Identity>("game/mercedes"),
                   .seats = {Ref<drivers::Identity>("game/lewis_hamilton"), Ref<drivers::Identity>("game/valtteri_bottas")}}},
        .changes = {{.round = 15,
                     .slot = 7,
                     .seat = 1,
                     .out = Ref<drivers::Identity>("game/daniil_kvyat"),
                     .in = Ref<drivers::Identity>("game/pierre_gasly")}},
        .extra = json::parse("{\"reserve\":\"game/x\"}"),
    };
    s.teams.at(2).team = Ref<teams::Identity>("game/ferrari");
    CHECK(s.teams.size() == 2 && s.teams.find(2) && !s.teams.find(3));
    CHECK(s.changes.find(15, 7, 1) && s.changes.erase(15, 7, 1) == true && s.changes.empty());
    std::string text = to_json(s);
    CHECK(text.find("\"box\":\"swofty.classic-seasons-2017/f1_2017|game/f1_2025\"") != std::string::npos);
    CHECK(text.find("\"reserve\":\"game/x\"") != std::string::npos);
    career::Season back;
    CHECK(from_json(text, back));
    CHECK(back.year == 2017 && back.teams.size() == 2 && back.teams.find(1)->seats[1].id() == "game/valtteri_bottas");
    CHECK(to_json(back) == text);

    cars::Handling h{.cls = cars::Class::f1_26};
    h.upgrade("radUpgradeEngine").set("Engine", "WearICERate", '*', 0.8f);
    h.engine.revs_power_spline = Curve::flat(0, 1200, 550000).scale_y(1.05f);
    std::string ht = to_json(h);
    CHECK(ht.find("\"class\":\"f1_26\"") != std::string::npos && ht.find("\"op\":\"*\"") != std::string::npos);
    cars::Handling hb;
    CHECK(from_json(ht, hb) && hb.cls == cars::Class::f1_26 && hb.rad_upgrades.size() == 1);

    content::Package pkg{.title = Text::literal("F1 2017 (V10)"), .entrant_list = "esp_f1_2017_v10", .season = 2017};
    pkg.teams.at("ferrari").engine_sound = Ref<sounds::EngineSound>("mod.esp/v10_screamer");
    std::string pt = to_json(pkg);
    CHECK(pt.find("{\"literal\":\"F1 2017 (V10)\"}") != std::string::npos && pt.find("\"stat_id\":\"auto\"") != std::string::npos);
    content::Package pb;
    CHECK(from_json(pt, pb) && pb.title.is_literal() && pb.teams.find(std::string("ferrari")));

    sounds::EngineSound es{.bundle = stock("audio/2025/bundles/redbull_2025.bdl")};
    CHECK(to_json(es).find("{\"stock\":\"audio/2025/bundles/redbull_2025.bdl\"}") != std::string::npos);

    career::Calendar cal{.rounds = {{.round = 1,
                                     .race = "lng_gp_usa",
                                     .venue = Ref<tracks::Venue>("venue/phoenix"),
                                     .date = std::chrono::year{1990} / 3 / 11}},
                         .seasons = 1};
    CHECK(to_json(cal).find("\"date\":\"1990-03-11\"") != std::string::npos);
    career::Calendar cb;
    CHECK(from_json(to_json(cal), cb) && cb.rounds.find(1)->date == cal.rounds.find(1)->date);

    career::RndDepartment d{.colour = Colour("#22A45D")};
    CHECK(to_json(d).find("\"colour\":\"#22A45D\"") != std::string::npos);
    CHECK(!from_json("{\"year\":", back));
}

static void schemas()
{
    std::string s = career::seasons.schema();
    CHECK(s.find("\"x-registry\":\"career.season\"") != std::string::npos);
    CHECK(s.find("\"x-version\":\"1.0\"") != std::string::npos);
    CHECK(s.find("\"x-applies\":\"read\"") != std::string::npos);
    CHECK(s.find("\"x-snapshot\":true") != std::string::npos && s.find("\"x-snapshot-budget\":65536") != std::string::npos);
    CHECK(s.find("\"x-key\":[\"slot\"]") != std::string::npos);
    CHECK(s.find("\"x-key\":[\"round\",\"slot\",\"seat\"]") != std::string::npos);
    CHECK(s.find("\"x-ref\":\"content.package\"") != std::string::npos);
    CHECK(s.find("\"x-ref\":\"driver.identity\"") != std::string::npos);
    std::string v = cars::vehicles.schema();
    CHECK(v.find("\"x-applies\":\"boot\"") != std::string::npos && v.find("\"enum\":[\"f1\",\"f1_26\"") != std::string::npos);
    CHECK(cars::handling.schema().find("\"class\":") != std::string::npos);
    CHECK(content::packages.schema().find("\"x-key\":[\"key\"]") != std::string::npos);
    CHECK(test::things.schema().find("\"x-version\":\"1.2\"") != std::string::npos);
    CHECK(Registry<json::Value>("x.y").schema().empty());
    CHECK(sounds::engine_sounds.at_major() == "engine_sound@1");
}

int main()
{
    unavailable_without_core();
    round_trip();
    schemas();
    if (failures) {
        std::printf("sdk_headers_test: %d failure(s)\n", failures);
        return 1;
    }
    std::printf("sdk_headers_test: ok\n");
    return 0;
}

#if defined(_WIN32)
SWOFTY_ADDON()
#endif
