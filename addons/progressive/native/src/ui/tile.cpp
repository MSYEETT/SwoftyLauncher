// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career - the career tile and wizard (progressive_ui.dll). Its own DLL, so a binding
// another feature can't resolve never drops the tile.
//
// The tile is a career content of the core (swofty/career_content.hpp): data group
// f1_career_progressive (the career's entrant list, so its own save slots), mode
// driver_career_progressive, the 2026 model, Driver Career only, suffix PG26 (Choose Driver lists
// F1CAREER_PG26, F2CAREER_PG26, ICONS_PG26; progressive_suffix.dll turns the 26 into the start
// year). The wizard library, for this data group only: start in F1, no Official/Custom Driver
// step, no F2 and no Icons tab on Choose Driver, and with the option setup_page the tile opens the
// launcher's career setup page (route career_wizard_driver_career_progressive) instead of the
// stock wizard's first step.

#include <swofty/career_content.hpp>
#include <swofty/career_wizard.hpp>
#include <swofty/mixin.hpp>

SWOFTY_SETTING(bool, setup_page, false);

namespace {

namespace content = swofty::career::content;
namespace wz = swofty::career::wizard;

constexpr const char *kGroup = "f1_career_progressive";
constexpr const char *kSetupRoute = "career_wizard_driver_career_progressive";

bool register_tile()
{
    content::Result r = content::add({.id = "progressive",
                                      .title = "lng_pg_career_content",
                                      .description = "lng_pg_career_content_description",
                                      .data_group = kGroup,
                                      .suffix = "PG26",
                                      .mode = "driver_career_progressive",
                                      .model = content::Model::f1_2026,
                                      .careers = content::Career::driver});
    wz::Result w[5] = {wz::start_in_f1(kGroup), wz::skip_driver_type(kGroup), wz::hide_driver_tab(kGroup, "header_f2"),
                       wz::hide_driver_tab(kGroup, "header_icon"),
                       setup_page ? wz::tile_route(kGroup, kSetupRoute) : wz::Result::ok};
    swofty::log("[progressive] tile: content 'progressive' %s; wizard: start in F1 %s, driver type step %s, F2 tab %s, "
                "Icons tab %s, setup page %s",
                swofty::career::name(r), wz::name(w[0]), wz::name(w[1]), wz::name(w[2]), wz::name(w[3]),
                setup_page ? wz::name(w[4]) : "off (setup_page)");
    return true;
}

} // namespace

SWOFTY_ADDON(register_tile)
