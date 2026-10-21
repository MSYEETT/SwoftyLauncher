// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include <string>
#include <string_view>

namespace f1990 {

constexpr std::string_view kUpgradePrefix = "f1990_";

// The R&D card binders name a card's frame <upgrade_name>, <upgrade_name>_in_progress or
// <upgrade_name>_complete (state <= 1, 2, other); masks are <upgrade_name>. Splits off that suffix.
inline std::string_view state_suffix(std::string_view name, std::string_view &base)
{
    for (std::string_view s : {std::string_view("_in_progress"), std::string_view("_complete")})
        if (name.size() > s.size() && name.substr(name.size() - s.size()) == s) {
            base = name.substr(0, name.size() - s.size());
            return s;
        }
    base = name;
    return {};
}

// The frame of a stock part for a card state, as the lists name them: the rnd_om_xm lists
// om_xm_<part>, om_xm_<part>_progress, om_xm_<part>_complete and the mask om_xm_<part>_m; the plain
// lists <part>, <part>_in_progress, <part>_complete and the mask <part>. donor is the stock part,
// with or without its om_xm_ prefix.
inline std::string donor_frame(std::string_view donor, std::string_view suffix, bool omxm, bool mask)
{
    std::string_view part = donor;
    if (part.substr(0, 6) == "om_xm_")
        part.remove_prefix(6);
    std::string out = omxm ? "om_xm_" : "";
    out += part;
    if (mask)
        out += omxm ? "_m" : "";
    else
        out += omxm && suffix == "_in_progress" ? std::string_view("_progress") : suffix;
    return out;
}

}
