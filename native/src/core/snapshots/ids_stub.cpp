#include "ids.hpp"

// Stub of W6's stat-id store (see ids.hpp). Delete this file from native/build.sh and
// native/test/run_snapshots_test.sh when W6's ids implementation merges.

namespace swofty_core::ids {

std::string map_for_career(const std::string &) { return "{}"; }

bool adopt(const std::string &, const std::string &, std::string *) { return true; }

}
