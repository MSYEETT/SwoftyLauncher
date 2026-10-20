// SPDX-License-Identifier: AGPL-3.0-or-later
//
// R&D: Tyres & Thermal Management - the development-potential pie's 5th (Tyres)
// slice, native half. Pairs with the addon's install_fifth_pie piece (buffer grow,
// 5th slot construct, populate guard relax) and with the 5-sector
// rnd_dev_potential.idf / laptop_r_and_d.ui edits. See PIE_INTEGRATION.md.
#pragma once

#include <cstdint>

namespace rnd_fifth_pie {

// Call once from SWOFTY_ON_LOAD, right AFTER install_fifth_pie(base). Every
// handler is a no-op until then.
void install(uintptr_t base);

// True while the last department focused on R&D > Performance (or the Team HQ
// research overview) is the Tyres tile, i.e. while the stock selection index
// g_Career_RnDOverviewSelectedDept is stale for it. False before install().
bool tyres_focus_pending();

// True once the Tyres focus tracking is live (after install()).
bool open_tracking_live();

} // namespace rnd_fifth_pie
