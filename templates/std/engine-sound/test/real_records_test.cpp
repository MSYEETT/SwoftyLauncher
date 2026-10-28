// SPDX-License-Identifier: MIT
#include <cstdio>
#include <fstream>
#include <sstream>
#include <string>

#include <swofty/fake_core.hpp>

#include "../native/src/esp.cpp"
#include "../../../../native/src/core/registry/declare_all.hpp"
#include "../../../../native/src/core/registry/older.hpp"
#include "../../../../native/src/core/registry/table.hpp"

static int failures = 0;

static void check(bool ok, const char *what)
{
    std::printf("%s  %s\n", ok ? "ok  " : "FAIL", what);
    failures += !ok;
}

static std::string slurp(const std::string &path)
{
    std::ifstream f(path, std::ios::binary);
    std::stringstream ss;
    ss << f.rdbuf();
    return ss.str();
}

int main()
{
    namespace reg = swofty_core::registry;
    const std::string root = "../../..";
    reg::reset_store();
    auto &st = reg::store();
    st.set_order({"swofty.classic-seasons-2017", "mod.esp"});
    st.set_validator(&reg::schema_validator());
    swofty_core::registry::declare_system_registries([&](const std::string &name, const std::string &schema) {
        if (st.declared(name))
            return false;
        return st.declare(name, swofty::json::parse(schema), false, "") == 0;
    });
    swofty_core::registry::older::declare(st);
    check(reg::load_data_file("swofty.classic-seasons", "registry/drivers.json", R"j({ "driver.identity": { "stoffel_vandoorne": {
        "names": { "first": { "literal": "Stoffel" }, "last": { "literal": "Vandoorne" }, "full": { "literal": "Stoffel Vandoorne" }, "code": "VAN" } } } })j") == SWOFTY_ASSET_OK, "stands in for the base addon's driver");
    std::string text = slurp(root + "/addons/classicseasons/classic-seasons-2017/registry/content.json");
    check(!text.empty(), "reads Classic Seasons 2017's real content.json");
    check(reg::load_data_file("swofty.classic-seasons-2017", "registry/content.json", text) == SWOFTY_ASSET_OK, "loads the real records");

    swofty::fake::Core core{"mod.esp"};
    core.run_register();
    st.end_register();

    auto pkg = st.get("content.package", "mod.esp/f1_2017_v10");
    check(pkg.status == reg::Status::ok, "the F1 2017 (V10) variant is valid");
    auto base = st.get("content.package", "swofty.classic-seasons-2017/f1_2017");
    check(base.status == reg::Status::ok, "the real 2017 package is valid");
    auto box = st.get("era.box", "mod.esp/f1_2017_v10");
    check(box.status == reg::Status::ok, "its era box resolves");
    if (pkg.status == reg::Status::ok) {
        const auto &teams = pkg.value["teams"];
        bool merc = false, ferrari = false;
        for (std::size_t i = 0; i < teams.size(); i++) {
            if (teams[i]["key"].as_string() == "mercedes")
                merc = teams[i]["vehicle"].as_string() == "mod.esp/mercedes_v10";
            if (teams[i]["key"].as_string() == "ferrari")
                ferrari = teams[i]["vehicle"].as_string() == "swofty.classic-seasons-2017/ferrari";
        }
        check(merc, "the Mercedes drives the V10 variant");
        check(ferrari, "the Ferrari stays the 2017 Ferrari");
        check(pkg.value["entrant_list"].as_string() == "esp_f1_2017_v10", "its own entrant list");
    }
    auto ferrari = st.get("vehicle", "swofty.classic-seasons-2017/ferrari");
    check(ferrari.status == reg::Status::ok && ferrari.value["engine_sound"].as_string() == "mod.esp/v10_screamer",
          "the stand-in layer puts the V10 on the real 2017 Ferrari");
    check(swofty::content::packages.schema().find("\"type\":[\"number\",\"null\"]") != std::string::npos,
          "an optional field's schema accepts null");
    return failures ? 1 : 0;
}
