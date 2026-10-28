// SPDX-License-Identifier: MIT
#pragma once
#include <optional>
#include <swofty/registry.hpp>
#include <swofty/systems/forward.hpp>

// Sessions: in 1.6.0 only the rules a Grand Prix or Time Trial box carries (content::Package::rules),
// applied when a session loads in that box. The rest of the sessions system ships later.

namespace swofty::sessions {

struct Rules {
    Ref<Mode> mode;
    Ref<Points> points;
    Ref<power::DrsRule> drs;
    Ref<tyres::Class> tyre_class;
    Ref<RulesProfile> rules_profile;
    bool refuelling = false;
    std::optional<float> race_fuel_kg;
};
SWOFTY_RECORD(Rules, mode, points, drs, tyre_class, rules_profile, refuelling, race_fuel_kg)

}
