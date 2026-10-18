#include "runtime.hpp"

#include <swofty/mixin.hpp>

// Stub for the events + frame-scheduler lane. Every call returns unavailable until the lane
// wires subscribe/on_frame to Vm_Update_FlowStateMachine and the lifecycle hooks.

namespace {

int ev_available()
{
    return 0;
}

int ev_subscribe(const char *, SwoftyEventFn, void *, uint32_t *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int ev_on_frame(SwoftyFrameFn, void *, uint32_t *)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

int ev_unsubscribe(uint32_t)
{
    return SWOFTY_ASSET_UNAVAILABLE;
}

const SwoftyEvents table = {ev_available, ev_subscribe, ev_on_frame, ev_unsubscribe};

}

namespace runtime {

const SwoftyEvents *events_table()
{
    return &table;
}

}
