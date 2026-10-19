// SPDX-License-Identifier: AGPL-3.0-or-later
//
// R&D: Tyres & Thermal Management - upgrade types minted natively (ids 19+).
// Install once from SWOFTY_ON_LOAD with the game's module base; gated by its own
// tyres_upgrade_types setting. See rnd_upgrade_types.cpp and native/HOOKS.md v0.9.0.
#pragma once

#include <cstdint>

namespace rnd_upgrade_types {

void install(uintptr_t base);

bool live();

int effect_dept(uint8_t type);

} // namespace rnd_upgrade_types
