// SPDX-License-Identifier: AGPL-3.0-or-later
//
// years.json as Progressive's DLLs read it on a 1.6 core (records_logic.hpp), on the host: register.cpp
// registers its records into the core's registry (native/src/core/registry, through the SDK fake core),
// a second addon layers swofty.progressive/2017, and the overlaid years.json goes through the same
// parsers and season planner the DLL uses.
//   no layer:    every record read back leaves years.json exactly as it is;
//   a seat:      tails.what-if puts Alonso in the 2017 Mercedes' first seat: the plan seats him there,
//                Hamilton leaves the Mercedes and Alonso's McLaren seat goes to another driver;
//   a rating:    the same addon's pace 96 for Alonso in 2017 reaches the ratings parser;
//   a change:    a layer that drops a 2017 mid-season change drops it from what the planner reads;
//   no registry: no record means years.json as it is.

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>

#include <swofty/fake_core.hpp>
#include <swofty/storage.hpp>

#include "ratings_logic.hpp"
#include "records_logic.hpp"
#include "season_logic.hpp"

extern "C" int SwoftyAddonRegister(const SwoftyApi *a);

namespace {

namespace sj = swofty::json;
namespace career = swofty::career;
namespace pr = progressive::records;
namespace season = progressive::season;
namespace lineup = progressive::lineup;

int failures = 0;

void expect(bool ok, const char *what, int line)
{
    if (!ok) {
        std::printf("FAIL line %d: %s\n", line, what);
        ++failures;
    }
}

#define EXPECT(x) expect((x), #x, __LINE__)

std::string g_root;

bool slurp(const std::string &path, std::string &out)
{
    std::ifstream f(path, std::ios::binary);
    if (!f)
        return false;
    std::ostringstream s;
    s << f.rdbuf();
    out = s.str();
    return true;
}

int st_available() { return 1; }
int st_size(const char *path, uint64_t *out)
{
    std::string s;
    if (!slurp(g_root + "/" + path, s))
        return SWOFTY_ASSET_INVALID;
    *out = s.size();
    return SWOFTY_ASSET_OK;
}
int st_read(const char *path, void *buf, size_t cap, size_t *len)
{
    std::string s;
    if (!slurp(g_root + "/" + path, s) || s.size() > cap)
        return SWOFTY_ASSET_INVALID;
    std::memcpy(buf, s.data(), s.size());
    *len = s.size();
    return SWOFTY_ASSET_OK;
}
int st_none() { return SWOFTY_ASSET_UNAVAILABLE; }

const SwoftyStorage kStorage = {
    .available = st_available,
    .read = st_read,
    .write = reinterpret_cast<int (*)(const char *, const void *, size_t)>(&st_none),
    .append = reinterpret_cast<int (*)(const char *, const void *, size_t)>(&st_none),
    .remove = reinterpret_cast<int (*)(const char *)>(&st_none),
    .size = st_size,
    .list = reinterpret_cast<int (*)(const char *, char *, size_t, size_t *)>(&st_none),
};

std::optional<career::Season> season_of(const std::string &id) { return career::seasons.get(career::seasons[id]); }
std::optional<career::DriverRatings> rating_of(const std::string &id) { return career::ratings.get(career::ratings[id]); }

season::Data parse_season(const sj::Value &doc)
{
    season::Data d;
    std::string err;
    if (!season::parse(sj::dump(doc), d, err))
        std::printf("season parse: %s\n", err.c_str());
    return d;
}

season::Data g_data;

int stat(const std::string &label)
{
    const season::PoolDriver *p = g_data.driver(label);
    return p ? p->stat : -1;
}

season::Resolve resolver()
{
    season::Resolve r;
    r.driver_stat = stat;
    r.vehicle_stat = [](const std::string &label) { return 1000 + static_cast<int>(std::hash<std::string>{}(label) % 1000); };
    r.look_known = [](int) { return true; };
    r.rating = [](uint32_t) { return 75.0; };
    return r;
}

std::vector<season::CareerRow> career_rows(const std::string &player)
{
    std::vector<season::CareerRow> rows;
    const season::Year *y = g_data.year(2026);
    std::vector<int> seated;
    for (int i = 0; i < season::kSlots; ++i)
        for (auto &d : y->teams[i].drivers) {
            season::CareerRow r;
            r.driver = static_cast<uint32_t>(stat(d.driver));
            r.team = g_data.slots[i].team_stat;
            r.position = d.position;
            r.car = true;
            r.human = d.driver == player;
            r.number = static_cast<uint8_t>(d.number);
            rows.push_back(r);
            seated.push_back(stat(d.driver));
        }
    for (auto &p : g_data.pool)
        if (std::find(seated.begin(), seated.end(), p.stat) == seated.end()) {
            season::CareerRow r;
            r.driver = static_cast<uint32_t>(p.stat);
            r.number = p.number > 0 ? static_cast<uint8_t>(p.number) : season::kNoNumber;
            rows.push_back(r);
        }
    return rows;
}

bool in_slot(const season::Plan &p, int slot1, const std::string &driver)
{
    uint32_t s = static_cast<uint32_t>(stat(driver));
    return p.target[slot1 - 1][0] == s || p.target[slot1 - 1][1] == s;
}

void no_layer_reads_years_json_as_it_is(const sj::Value &file)
{
    sj::Value doc = file;
    pr::Summary n = pr::overlay(doc, season_of, rating_of);
    EXPECT(n.seasons == 11);
    EXPECT(n.ratings > 200);
    EXPECT(!n.changed());
    EXPECT(doc == file);
    for (auto &note : n.notes)
        std::printf("  note: %s\n", note.c_str());
}

void what_if_layer_changes_the_2017_seats(const sj::Value &file)
{
    sj::Value doc = file;
    pr::Summary n = pr::overlay(doc, season_of, rating_of);
    EXPECT(n.seats == 1);
    EXPECT(n.emptied == 1);
    EXPECT(n.ratings_changed == 1);
    EXPECT(n.changes == 1);
    g_data = parse_season(doc);
    const season::Year *y = g_data.year(2017);
    EXPECT(y && y->teams[0].drivers.size() == 2);
    if (y && !y->teams[0].drivers.empty())
        EXPECT(y->teams[0].drivers[0].driver == "fernando_alonso" && y->teams[0].drivers[0].look.stat < 0);
    EXPECT(y && y->teams[3].drivers.size() == 1);

    season::Plan p = season::plan(g_data, 2017, career_rows("max_verstappen"), resolver());
    EXPECT(p.error.empty());
    EXPECT(p.target[0][0] == static_cast<uint32_t>(stat("fernando_alonso")));
    EXPECT(in_slot(p, 1, "valtteri_bottas"));
    EXPECT(!in_slot(p, 1, "lewis_hamilton"));
    EXPECT(!in_slot(p, 4, "fernando_alonso"));
    EXPECT(in_slot(p, 4, "stoffel_vandoorne"));
    EXPECT(p.target[3][0] != lineup::kNone);
    bool look = false;
    for (auto &l : p.looks)
        look = look || (l.driver == static_cast<uint32_t>(stat("fernando_alonso")) && l.label == "2017-cs_mclaren_fernando_alonso");
    EXPECT(look);

    season::Data untouched = parse_season(file);
    season::Plan q = season::plan(untouched, 2017, career_rows("max_verstappen"), resolver());
    EXPECT(q.target[0][0] == static_cast<uint32_t>(stat("lewis_hamilton")));
    EXPECT(in_slot(q, 4, "fernando_alonso"));

    progressive::ratings::Data r;
    std::string err;
    EXPECT(progressive::ratings::parse(sj::dump(doc), r, err));
    const progressive::ratings::Values *v = r.find(2017, "fernando_alonso");
    EXPECT(v && std::fabs(v->v[3] - 96.0) < 1e-6);
    EXPECT(v && std::fabs(v->v[0] - 91.5) < 1e-6);
    EXPECT(v && v->overall > 87.0);
    const progressive::ratings::Values *h = r.find(2017, "lewis_hamilton");
    EXPECT(h && std::fabs(h->v[3] - 93.9) < 1e-6);

    EXPECT(g_data.year(2017)->changes.size() + 1 == parse_season(file).year(2017)->changes.size());
    EXPECT(parse_season(doc).year(2016)->changes.size() == parse_season(file).year(2016)->changes.size());
}

void no_registry_changes_nothing(const sj::Value &file)
{
    sj::Value doc = file;
    pr::Summary n = pr::overlay(
        doc, [](const std::string &) { return std::optional<career::Season>(); },
        [](const std::string &) { return std::optional<career::DriverRatings>(); });
    EXPECT(n.seasons == 0 && n.ratings == 0 && !n.changed());
    EXPECT(doc == file);
}

}

int main(int argc, char **argv)
{
    if (argc < 2) {
        std::printf("records_test <addon folder>\n");
        return EXIT_FAILURE;
    }
    g_root = argv[1];
    std::string text;
    if (!slurp(g_root + "/data/years.json", text)) {
        std::printf("records_test: no data/years.json in %s\n", argv[1]);
        return EXIT_FAILURE;
    }
    const sj::Value file = sj::parse(text);

    {
        swofty::fake::Core core{"swofty.progressive"};
        const SwoftyFakeEngine *engine = swofty::fake::Core::default_engine();
        SwoftyApi api = core.api();
        api.storage = &kStorage;
        swofty::api = &api;
        engine->begin_addon("swofty.progressive");
        SwoftyAddonRegister(&api);
        no_layer_reads_years_json_as_it_is(file);

        engine->begin_addon("tails.what-if");
        career::seasons.layer(career::seasons["swofty.progressive/2017"], [](career::Season &s) {
            if (auto *team = s.teams.find(1))
                team->seats = {swofty::Ref<swofty::drivers::Identity>("game/fernando_alonso"),
                               swofty::Ref<swofty::drivers::Identity>("game/valtteri_bottas")};
            if (!s.changes.empty())
                s.changes.items().erase(s.changes.items().begin());
        }, swofty::Scope::profile);
        career::ratings.layer(career::ratings["swofty.progressive/2017/game/fernando_alonso"],
                              [](career::DriverRatings &r) { r.pace = 96; }, swofty::Scope::profile);
        engine->end_register();
        what_if_layer_changes_the_2017_seats(file);
        swofty::api = nullptr;
    }
    no_registry_changes_nothing(file);

    if (failures) {
        std::printf("records_test: %d failure(s)\n", failures);
        return EXIT_FAILURE;
    }
    std::printf("records_test: ok (no layer: years.json as it is; a 2017 layer moves Alonso into the Mercedes, "
                "his pace to 96 and drops a change)\n");
    return EXIT_SUCCESS;
}
