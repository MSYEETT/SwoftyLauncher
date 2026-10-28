// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Progressive Career: years.json with the effective records in it (records.hpp).

#include <swofty/registry.hpp>
#include <swofty/storage.hpp>
#include <swofty/systems/career.hpp>

#include "records.hpp"
#include "records_logic.hpp"

namespace pg::records {

namespace {

namespace json = swofty::json;
namespace career = swofty::career;
namespace pr = progressive::records;

constexpr const char *kYearsPath = "data/years.json";

}

bool read_years(std::string &text, const char *who)
{
    if (swofty::storage::read(kYearsPath, text) != swofty::storage::Result::ok)
        return false;
    if (!swofty::detail::registry_table()) {
        swofty::log("[progressive] %s: %s as it is (the core has no registry)", who, kYearsPath);
        return true;
    }
    json::Error err;
    json::Value doc = json::parse(text, &err);
    if (!err.ok() || !doc.is_object())
        return true;
    pr::Summary n = pr::overlay(
        doc, [](const std::string &id) { return career::seasons.get(career::seasons[id]); },
        [](const std::string &id) { return career::ratings.get(career::ratings[id]); });
    if (!n.seasons && !n.ratings) {
        swofty::log("[progressive] %s: %s as it is (no swofty.progressive career.season or career.driver_ratings "
                    "record in the registry)",
                    who, kYearsPath);
        return true;
    }
    if (n.changed())
        text = json::dump(doc);
    swofty::log("[progressive] %s: %s with the effective records: %d career.season, %d career.driver_ratings; %d "
                "seat(s), %d emptied, %d change(s), %d rating(s) differ from the file",
                who, kYearsPath, n.seasons, n.ratings, n.seats, n.emptied, n.changes, n.ratings_changed);
    for (const std::string &note : n.notes)
        swofty::log("[progressive]   %s", note.c_str());
    return true;
}

}
