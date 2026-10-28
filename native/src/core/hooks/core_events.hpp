#pragma once
#include <cstdint>
#include <string_view>

namespace swofty::cars::events {
struct UpgradesFit;
struct VtfRead;
struct Built;
struct Reset;
struct PartsRegister;
}
namespace swofty::sounds::events {
struct BundleResolve;
}
namespace swofty::career::events {
struct AddActivity;
struct StandingsRebuild;
}

// The firing helpers for the core's own 1.6.0 events that the 1.x code has no moment for yet. The
// apply paths (round 3) call them where each moment happens; each returns false when a replace
// handler handled the moment (cancel events) and true otherwise.

namespace swofty_core::hooks::events {

bool career_created();
bool career_season_start(int year, int season_index);
bool career_season_end();
bool career_season_break();
bool career_saving();
bool career_add_activity(swofty::career::events::AddActivity &payload);
bool career_standings_rebuild(swofty::career::events::StandingsRebuild &payload);
bool car_upgrades_fit(swofty::cars::events::UpgradesFit &payload);
bool car_vtf_read(swofty::cars::events::VtfRead &payload);
bool car_built(swofty::cars::events::Built &payload);
bool car_reset(swofty::cars::events::Reset &payload);
bool car_parts_register(swofty::cars::events::PartsRegister &payload);
bool audio_bundle_resolve(swofty::sounds::events::BundleResolve &payload);

}
