// SPDX-License-Identifier: AGPL-3.0-or-later
//
// R&D: Tyres & Thermal Management - department 4's icon and colour in the R&D lists.
// The sites are always on; install() runs once from SWOFTY_ON_LOAD with the game's module
// base and logs them. See rnd_dept_tags.cpp.
#pragma once

#include <cstdint>

namespace rnd_dept_tags {

void install(uintptr_t base);

} // namespace rnd_dept_tags
