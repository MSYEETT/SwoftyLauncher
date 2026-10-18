#include "wine_test.hpp"
#include <swofty/assets.hpp>
#include <swofty/telemetry.hpp>

namespace assets = swofty::assets;
namespace tel = swofty::telemetry;

namespace {

bool on()
{
    char v[4] = {};
    return GetEnvironmentVariableA("SWOFTY_TEST_ASSETS", v, sizeof v) && v[0] == '1';
}

}

SWOFTY_ON_LOAD
{
    if (!on())
        return;
    swofty::log("[t_assets] available: group %d, erp %d, db %d, lang %d, file %d, sound %d", assets::available("group"),
                assets::available("erp"), assets::available("db"), assets::available("lang"), assets::available("file"),
                assets::available("sound"));
    std::string_view erps[] = {"2025_asset_groups/t_assets/t.erp"};
    assets::Fragment frags[] = {{"erp/frag.bin"}};
    const std::byte one[] = {std::byte('1')};
    assets::Result r[] = {
        assets::register_group("asset_groups/t_assets/t.assetgroup", erps),
        assets::put_resource("eaid://t_assets/t.tif.image", frags, erps),
        assets::db_set("entrants.teams.team.t_assets", "int", "1"),
        assets::db_set(0xf01d03c7a1b2c3d4ull, 0, one),
        assets::lang_set("eng", "lng_t_assets", "Runtime assets API"),
        assets::lang_table("eng", "lang/extra.lng"),
        assets::overlay_file("t_assets/sponsor.dds", "files/sponsor.dds"),
        assets::lang_table("eng", "../escape.lng"),
    };
    for (auto x : r)
        swofty::log("[t_assets] result %s", assets::name(x));
}

extern "C" __declspec(dllexport) void t_assets_late()
{
    if (!on())
        return;
    assets::Fragment frags[] = {{"erp/frag.bin"}};
    swofty::log("[t_assets] late put_resource: %s",
                assets::name(assets::put_resource("eaid://t_assets/late.tif.image", frags)));

    tel::SessionState ss = {};
    tel::CarState cs = {};
    swofty::log("[t_assets] telemetry: available %d, session %s, car0 %s, car99 %s", tel::available(),
                tel::name(tel::session(&ss)), tel::name(tel::car(0, &cs)), tel::name(tel::car(99, &cs)));
    const char *feed = nullptr;
    tel::Result sm = tel::shared_memory(&feed);
    swofty::log("[t_assets] telemetry: shared_memory %s name=%s", tel::name(sm), feed ? feed : "(none)");
}

SWOFTY_ADDON()
