#include "../../src/core/apply/content/logo_build.hpp"
#include "../../src/core/apply/content/png_straight.hpp"

#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <sstream>

namespace content = swofty_core::apply::content;
namespace lb = content::logo_build;
namespace ef = content::erpfile;
namespace fs = std::filesystem;

static int failures = 0;

static void check(bool ok, const std::string &what)
{
    std::printf("%s  %s\n", ok ? "ok  " : "FAIL", what.c_str());
    failures += !ok;
}

static std::string slurp(const fs::path &path)
{
    std::ifstream f(path, std::ios::binary);
    std::stringstream s;
    s << f.rdbuf();
    return s.str();
}

static std::shared_ptr<ef::Source> open_file(const fs::path &path)
{
    if (!fs::exists(path))
        return nullptr;
    auto in = std::make_shared<std::ifstream>(path, std::ios::binary);
    auto src = std::make_shared<ef::Source>();
    src->size = fs::file_size(path);
    src->read = [in](uint64_t off, size_t n, char *out) {
        in->clear();
        in->seekg(std::streamoff(off));
        in->read(out, std::streamsize(n));
        return size_t(in->gcount()) == n;
    };
    return src;
}

static std::string rewrite_to_string(const ef::Source &src, const ef::Archive &arc, const ef::Changes &changes, std::string &why)
{
    std::string out;
    ef::rewrite(src, arc, changes, [&](const char *p, size_t n) { out.append(p, n); return true; }, why);
    return out;
}

int main(int argc, char **argv)
{
    fs::path dir = argc > 1 ? argv[1] : "content/bc7";
    fs::path game = dir / "game";
    std::string why;

    std::string raw = slurp(dir / "shape64.rgba"), png_bytes = slurp(dir / "shape64.png"), decoded;
    const char *px = raw.data() + raw.find('\n') + 1;
    uint32_t w = 0, h = 0;
    check(content::png::decode(png_bytes, decoded, w, h, why) && w == 64 && h == 64 && decoded == std::string(px, 64 * 64 * 4),
          "a PNG decodes to straight alpha with zero colour under alpha 0");

    std::vector<std::string> list = {"2025_asset_groups/ui_package/databases.erp", "2025_asset_groups/ui_package/flow_a.erp",
                                     "2025_asset_groups/ui_package/flow_none.erp", "2025_asset_groups/ui_package/screens/b.erp"};
    for (auto &rel : list) {
        auto src = open_file(game / rel);
        ef::Archive arc;
        bool ok = src && ef::open(*src, arc, why);
        check(ok && rewrite_to_string(*src, arc, {}, why) == slurp(game / rel), rel + ": rewriting with no change gives the same bytes");
    }

    std::string uidb = slurp(dir / "expect/overrides.uidb");
    content::uidb::Pkg pkg;
    check(pkg.read(uidb) && pkg.write() == uidb, "overrides.uidb reads and writes to the same bytes");

    lb::Env env;
    env.list = [&] { return list; };
    env.open = [&](const std::string &rel) { return open_file(game / rel); };
    std::vector<lb::Want> wants(2);
    wants[0] = {"teams.driver_select_team_logos", "newteam", "", std::string(px, 64 * 64 * 4), 64, 64};
    wants[1].array = "teams.driver_select_team_logos";
    wants[1].key = "newalias";
    wants[1].donor = "b";
    lb::Result res;
    check(lb::build(env, wants, res) && res.built == 2 && res.refused == 0, "two slices are built");
    check(res.changes.size() == 3 && res.changes.count(lb::DATABASES) && res.changes.count(list[1]) && res.changes.count(list[3]) &&
              !res.changes.count(list[2]),
          "the two carriers and databases.erp change, the archive without the array does not");

    fs::path out = argc > 2 ? argv[2] : "../build/bc7/logo_out";
    fs::create_directories(out);
    for (auto &[rel, changes] : res.changes) {
        auto src = open_file(game / rel);
        ef::Archive arc;
        ef::open(*src, arc, why);
        std::string built = rewrite_to_string(*src, arc, changes, why);
        std::string name = rel.substr(rel.rfind('/') + 1);
        if (rel.find("screens/") != std::string::npos)
            name = "screens_" + name;
        std::ofstream(out / name, std::ios::binary) << built;
    }
    check(true, "archives written to " + out.string() + " for check_logo_build.py");

    {
        auto bad = wants;
        bad[0].width = 32;
        lb::Result r2;
        lb::build(env, bad, r2);
        check(r2.refused == 1 && r2.built == 1, "an image of the wrong size is refused and the alias still builds");
        bad = wants;
        bad[1].donor = "zz";
        lb::Result r3;
        lb::build(env, bad, r3);
        check(r3.refused == 1, "a donor key that is not listed is refused");
        bad = wants;
        bad[0].array = "teams.nothing";
        lb::Result r4;
        lb::build(env, bad, r4);
        check(r4.refused >= 1, "an array that is not in overrides.uidb is refused");
    }

    if (const char *g = std::getenv("F1_GAME")) {
        fs::path ui = fs::path(g) / "2025_asset_groups/ui_package";
        auto src = open_file(ui / "databases.erp");
        ef::Archive arc;
        check(src && ef::open(*src, arc, why) && rewrite_to_string(*src, arc, {}, why) == slurp(ui / "databases.erp"),
              "the game's databases.erp rewrites to the same bytes");
        const ef::Entry *e = arc.find(lb::OVERRIDES);
        std::string real;
        content::uidb::Pkg p;
        check(e && ef::fragment(*src, arc, e->frags[0], real, why) && p.read(real) && p.write() == real, "the game's overrides.uidb writes to the same bytes");
        std::vector<std::string> all;
        for (auto &it : fs::recursive_directory_iterator(ui))
            if (it.path().extension() == ".erp")
                all.push_back("2025_asset_groups/ui_package/" + fs::relative(it.path(), ui).generic_string());
        std::sort(all.begin(), all.end());
        lb::Env real_env;
        real_env.list = [&] { return all; };
        real_env.open = [&](const std::string &rel) { return open_file(fs::path(g) / rel); };
        std::vector<lb::Want> real_wants;
        for (auto *a : {"teams.driver_select_team_logos", "osd.timing_tower.team_badges_hud", "team_logos.driver_career_2024_team_select",
                        "teams.badges_broadcast"}) {
            lb::Want w;
            w.array = a;
            w.key = "w26_probe";
            w.donor = std::string(a) == "osd.timing_tower.team_badges_hud" ? "red_bull" : "williams";
            real_wants.push_back(w);
        }
        lb::Result rr;
        lb::build(real_env, real_wants, rr);
        for (auto &l : rr.lines)
            std::printf("      %s\n", l.c_str());
        check(rr.built == 4 && rr.refused == 0 && rr.changes.count(lb::DATABASES), "the game's own archives build four probe slices");
    }
    return failures ? 1 : 0;
}
