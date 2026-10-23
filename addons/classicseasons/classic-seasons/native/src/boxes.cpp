// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Classic Seasons - one box per season in the Addons row of the Grand Prix / Time Trial era picker.
//
// The core builds the row, presses a box like a stock one and narrows its host setup mode, swofty_f1, to the
// pressed season. Each season addon adds its entrant list to swofty_f1 in its database operations; the core skips
// the box of a season that is not installed.

#include <iterator>

#include <swofty/era.hpp>

#include "seasons.hpp"

namespace {

bool init()
{
    if (!swofty::era::available()) {
        swofty::log("[classic-seasons] this core has no era picker boxes; the seasons get no box");
        return true;
    }
    int added = 0;
    for (const auto &s : classic_seasons::kSeasons)
        added += swofty::era::add_box({s.entrant_list, s.title, s.image, s.order}) == swofty::era::Result::ok;
    swofty::log("[classic-seasons] %d of %zu season box(es) registered", added, std::size(classic_seasons::kSeasons));
    return true;
}

}

SWOFTY_ADDON(init)
