#include <swofty/systems/career.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/content.hpp>
#include <swofty/systems/sounds.hpp>

#include "core_events.hpp"
#include "hooks.hpp"

namespace swofty_core::hooks::events {
namespace {

constexpr bool same(const char *a, const char *b)
{
    while (*a && *a == *b)
        a++, b++;
    return *a == *b;
}

template <class E> constexpr bool declared(uint32_t kind)
{
    for (auto &e : catalogue::EVENTS)
        if (same(e.id, E::point))
            return e.kind == kind && e.version == E::version;
    return false;
}

namespace ce = swofty::career::events;
namespace ca = swofty::cars::events;
namespace so = swofty::sounds::events;

static_assert(declared<ce::SeasonStart>(SWOFTY_POINT_NOTIFY));
static_assert(declared<ce::AddActivity>(SWOFTY_POINT_CANCEL));
static_assert(declared<ce::StandingsRebuild>(SWOFTY_POINT_MODIFY));
static_assert(declared<ce::Created>(SWOFTY_POINT_NOTIFY));
static_assert(declared<ce::Loaded>(SWOFTY_POINT_NOTIFY));
static_assert(declared<ce::Unloaded>(SWOFTY_POINT_NOTIFY));
static_assert(declared<ce::SeasonEnd>(SWOFTY_POINT_NOTIFY));
static_assert(declared<ce::SeasonBreak>(SWOFTY_POINT_NOTIFY));
static_assert(declared<ce::Saving>(SWOFTY_POINT_MODIFY));
static_assert(declared<ca::UpgradesFit>(SWOFTY_POINT_MODIFY));
static_assert(declared<ca::VtfRead>(SWOFTY_POINT_MODIFY));
static_assert(declared<ca::Built>(SWOFTY_POINT_NOTIFY));
static_assert(declared<ca::Reset>(SWOFTY_POINT_NOTIFY));
static_assert(declared<ca::PartsRegister>(SWOFTY_POINT_NOTIFY));
static_assert(declared<so::BundleResolve>(SWOFTY_POINT_MODIFY));
static_assert(sizeof(ce::Loaded) == sizeof(Shell) && sizeof(ce::Created) == sizeof(Shell));

template <class E> bool shell()
{
    E e{};
    return fire(e);
}

}

bool career_created() { return shell<ce::Created>(); }
bool career_season_end() { return shell<ce::SeasonEnd>(); }
bool career_season_break() { return shell<ce::SeasonBreak>(); }
bool career_saving() { return shell<ce::Saving>(); }

bool career_season_start(int year, int season_index)
{
    ce::SeasonStart e{};
    e.year = year;
    e.season_index = season_index;
    return fire(e);
}

bool career_add_activity(ce::AddActivity &payload) { return fire(payload); }
bool career_standings_rebuild(ce::StandingsRebuild &payload) { return fire(payload); }
bool car_upgrades_fit(ca::UpgradesFit &payload) { return fire(payload); }
bool car_vtf_read(ca::VtfRead &payload) { return fire(payload); }
bool car_built(ca::Built &payload) { return fire(payload); }
bool car_reset(ca::Reset &payload) { return fire(payload); }
bool car_parts_register(ca::PartsRegister &payload) { return fire(payload); }
bool audio_bundle_resolve(so::BundleResolve &payload) { return fire(payload); }

}
