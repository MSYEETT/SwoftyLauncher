// SPDX-License-Identifier: AGPL-3.0-or-later
//
// R&D: Tyres & Thermal Management - upgrade types minted natively (ids 19+).
// The hooks are mixin sites; install() runs once from SWOFTY_ON_LOAD, builds the extended
// order table and turns ids 19+ live. See rnd_upgrade_types.cpp and native/HOOKS.md v0.9.0.
#pragma once

#include <cstdint>

namespace rnd_upgrade_types {

void install(uintptr_t base);

bool live();

int effect_dept(uint8_t type);

} // namespace rnd_upgrade_types
