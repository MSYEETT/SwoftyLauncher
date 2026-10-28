#pragma once
#include <string>

// The stat-id store's career side, as career snapshots use it. W6 owns the store (ids.json, one per
// install); until its implementation merges, ids_stub.cpp answers with an empty map and accepts every
// map. A map is a JSON object: "<registry>/<record id>" -> stat id.

namespace swofty_core::ids {

// The stat ids of every record the career refers to, as a JSON object, for the career's save entry.
std::string map_for_career(const std::string &career);

// A loaded career's saved map: the store checks its ids against it (a career synced from another PC
// must not swap a track or a driver). False with why when an id would change meaning.
bool adopt(const std::string &career, const std::string &map_json, std::string *why);

}
