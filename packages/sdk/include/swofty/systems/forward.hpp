// SPDX-License-Identifier: MIT
#pragma once

// Record types the 1.6.0 system headers refer to before their own system header ships, with the registry
// each belongs to. A Ref<T> needs only the declaration.

#define SWOFTY_FORWARD_RECORD_(ns, T, name)                                                                            \
    namespace swofty::ns {                                                                                             \
    struct T;                                                                                                          \
    [[maybe_unused]] constexpr const char *swofty_registry_name(const T *) { return name; }                            \
    }

SWOFTY_FORWARD_RECORD_(drivers, Identity, "driver.identity")
SWOFTY_FORWARD_RECORD_(drivers, Look, "driver.look")
SWOFTY_FORWARD_RECORD_(teams, Identity, "team.identity")
SWOFTY_FORWARD_RECORD_(tracks, Track, "track")
SWOFTY_FORWARD_RECORD_(tracks, Venue, "track.venue")
SWOFTY_FORWARD_RECORD_(liveries, Livery, "livery")
SWOFTY_FORWARD_RECORD_(power, Unit, "power_unit")
SWOFTY_FORWARD_RECORD_(power, DrsRule, "rules.drs")
SWOFTY_FORWARD_RECORD_(ai, TrackPack, "ai.track_pack")
SWOFTY_FORWARD_RECORD_(tyres, Class, "tyre.class")
SWOFTY_FORWARD_RECORD_(sessions, Mode, "session.mode")
SWOFTY_FORWARD_RECORD_(sessions, Points, "session.points")
SWOFTY_FORWARD_RECORD_(sessions, RulesProfile, "session.rules_profile")
SWOFTY_FORWARD_RECORD_(cars, Vehicle, "vehicle")
SWOFTY_FORWARD_RECORD_(cars, Package, "vehicle.package")
SWOFTY_FORWARD_RECORD_(cars, Handling, "vehicle.handling")
SWOFTY_FORWARD_RECORD_(sounds, EngineSound, "engine_sound")
SWOFTY_FORWARD_RECORD_(content, Package, "content.package")
SWOFTY_FORWARD_RECORD_(content, EraBox, "era.box")
SWOFTY_FORWARD_RECORD_(career, Calendar, "career.calendar")
SWOFTY_FORWARD_RECORD_(career, Season, "career.season")
SWOFTY_FORWARD_RECORD_(career, Content, "career.content")
SWOFTY_FORWARD_RECORD_(career, ModeRecord, "career.mode")
SWOFTY_FORWARD_RECORD_(career, DriverRatings, "career.driver_ratings")
