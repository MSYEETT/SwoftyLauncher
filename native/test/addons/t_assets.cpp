#include "wine_test.hpp"
#include <swofty/assets.hpp>

namespace assets = swofty::assets;

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
}

SWOFTY_ADDON()
