#include "runtime.hpp"

#include <swofty/mixin.hpp>

// Stub for the telemetry lane. Every call returns unavailable until the lane finds the
// per-car physics/telemetry struct, reads it each frame, and publishes the shared feed.

namespace {

int tel_available()
{
    return 0;
}

int tel_session(SwoftySessionState *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int tel_car(uint32_t, SwoftyCarState *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int tel_participant(uint32_t, SwoftyParticipant *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int tel_shared_memory(const char **)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

const SwoftyTelemetry table = {tel_available, tel_session, tel_car, tel_participant, tel_shared_memory};

}

namespace runtime {

const SwoftyTelemetry *telemetry_table()
{
    return &table;
}

}
