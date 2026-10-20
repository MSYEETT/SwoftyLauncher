#pragma once
#include <swofty/game/base.hpp>

namespace swofty::game {

using Sess_GameWeatherSystem_SetupSession_t = void(void *ws, void *settings, void *config);
inline constexpr fn<"Sess_GameWeatherSystem_SetupSession", Sess_GameWeatherSystem_SetupSession_t> Sess_GameWeatherSystem_SetupSession{}; // L2 experimental, GameWeatherSystem::setupSession.
using Sess_GameWeatherSystem_UpdateRandomSeed_t = void(void *ws, uint32_t seed, uint32_t weekend);
inline constexpr fn<"Sess_GameWeatherSystem_UpdateRandomSeed", Sess_GameWeatherSystem_UpdateRandomSeed_t> Sess_GameWeatherSystem_UpdateRandomSeed{}; // L2 experimental, GameWeatherSystem::updateRandomSeed.
using Sess_WeatherComponent_SetSystemRandomSeed_t = void(void *component);
inline constexpr fn<"Sess_WeatherComponent_SetSystemRandomSeed", Sess_WeatherComponent_SetSystemRandomSeed_t> Sess_WeatherComponent_SetSystemRandomSeed{}; // L2 experimental, WeatherAndTimeOfDayComponent::setWeatherSystemRandomSeed
inline constexpr call<"site_Sess_AiVehicleRegisterCall"> site_Sess_AiVehicleRegisterCall{}; // L2 experimental

}
