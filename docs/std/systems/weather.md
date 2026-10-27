---
title: Weather
description: Track climate and scripted forecasts as registries, and the weather setup hook point.
---

# Weather

```text
registries   weather.track (climate weights and temperatures per track; build), weather.forecast (a scripted
             forecast; runtime, career snapshot)
hook points  session.weather.setup (Sess_GameWeatherSystem_SetupSession)
operations   weather::set_seed, weather::script
ready        ready for setup and seeds (L2, shipped in Career Editor); dataset layout L1
```
