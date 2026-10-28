// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career: data/years.json as the career reads it. On a core with the registry (API 9,
// standard library 1.6) the seats, mid-season changes and ratings are the effective career.season and
// career.driver_ratings records, with every addon's layers (records_logic.hpp); on an older core, or
// when the registry has none of Progressive's records, it is the file as it is.

#pragma once
#include <string>

namespace pg::records {

bool read_years(std::string &text, const char *who);

}
