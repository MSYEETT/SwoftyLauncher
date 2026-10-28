#pragma once
#include <cstdint>
#include <functional>
#include <string>

#include <swofty/systems/cars.hpp>
#include <swofty/systems/sounds.hpp>

// Builds the C-layout payloads of the core's car and sound events and fires them through `fire` (the
// core's hooks::fire, or a test table). Text and record ids point at the caller's strings for the call;
// what handlers change comes back through the reference arguments.

namespace swofty_core::hooks::payloads {

using PartSink = std::function<int32_t(const char *node, float detach_at, bool debris)>;

template <class Fire>
bool upgrades_fit(Fire &&fire, int32_t car, const char *vehicle, swofty::cars::Department department, float driver_stat,
                  float &rating)
{
    swofty::cars::events::UpgradesFit e{};
    e.car = car;
    e.vehicle_id = vehicle;
    e.department = department;
    e.driver_stat = driver_stat;
    e.rating = rating;
    bool ok = fire(e);
    rating = e.rating;
    return ok;
}

template <class Fire>
bool vtf_read(Fire &&fire, const swofty::cars::live::Car &car, const char *section, const char *field,
              const char *upgrade, float &value)
{
    swofty::cars::events::VtfRead e{};
    e.car = car;
    e.section_str = section;
    e.field_str = field;
    e.upgrade_str = upgrade ? upgrade : "";
    e.value = value;
    bool ok = fire(e);
    value = e.value;
    return ok;
}

template <class Fire> bool built(Fire &&fire, const swofty::cars::live::Car &car)
{
    swofty::cars::events::Built e{};
    e.car = car;
    return fire(e);
}

template <class Fire> bool reset(Fire &&fire, const swofty::cars::live::Car &car, bool upgrade)
{
    swofty::cars::events::Reset e{};
    e.car = car;
    e.upgrade = upgrade;
    return fire(e);
}

template <class Fire>
bool parts_register(Fire &&fire, int32_t car, const char *vehicle, void *front_wing, const PartSink &sink)
{
    swofty::cars::events::PartsRegister e{};
    e.car = car;
    e.vehicle_id = vehicle;
    e.front_wing = front_wing;
    e.add_fn = [](void *user, const char *node, float detach_at, bool debris) -> int32_t {
        auto &s = *static_cast<const PartSink *>(user);
        if (!node || !*node)
            return SWOFTY_ASSET_INVALID;
        return s ? s(node, detach_at, debris) : SWOFTY_ASSET_UNAVAILABLE;
    };
    e.user = const_cast<PartSink *>(&sink);
    return fire(e);
}

struct SoundSlot {
    std::string id;
    swofty::sounds::events::BundleResolve *payload;
};

template <class Fire>
bool bundle_resolve(Fire &&fire, int32_t car, const char *vehicle, const char *switch_name, std::string &engine_sound)
{
    swofty::sounds::events::BundleResolve e{};
    SoundSlot slot{engine_sound, &e};
    e.car = car;
    e.vehicle_id = vehicle;
    e.switch_name_str = switch_name;
    e.engine_sound_id = slot.id.c_str();
    e.set_engine_sound_fn = [](void *user, const char *id) -> int32_t {
        if (!id)
            return SWOFTY_ASSET_INVALID;
        auto *s = static_cast<SoundSlot *>(user);
        s->id = id;
        s->payload->engine_sound_id = s->id.c_str();
        return SWOFTY_ASSET_OK;
    };
    e.user = &slot;
    bool ok = fire(e);
    engine_sound = e.engine_sound_id ? e.engine_sound_id : "";
    return ok;
}

}
