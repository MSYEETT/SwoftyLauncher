// SPDX-License-Identifier: AGPL-3.0-or-later
//
// R&D: Tyres & Thermal Management - the Tyres department's own icon frames.
// probe() runs once from SWOFTY_ON_LOAD; the frame helpers then return the Tyres frame when
// the icon arrays are installed and the borrowed stock frame otherwise. See rnd_tyres_icon.cpp.
#pragma once

namespace rnd_tyres_icon {

void probe();
void probe_art();
bool part_art_live();
bool part_mask_live();
bool category_live();
bool live();
const char *dept(const char *borrowed);
const char *wheel(const char *borrowed);
const char *strip(const char *borrowed);

} // namespace rnd_tyres_icon
