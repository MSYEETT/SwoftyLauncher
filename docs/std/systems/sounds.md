---
title: Sounds
description: Engine sounds and a car's audio data as registries, and the bundle hook point.
---

# Sounds

```json
// engine_sound "mod.esp/v10_screamer"
{ "bundle": "sounds/v10_screamer.bdl", "settings_from": "game/ferrari_2025", "tuning_from": "game/ferrari_2025" }
```

```text
registries   engine_sound, audio.car (a car's switch name and its audio_car_data)         build
             audio.commentary                                                          later
hook points  audio.bundle.resolve (Game_Build_AudioBundleRegistry, Game_AudioF1Car_Setup)
ready        build: ready. runtime: unknown (L1)
limits       the reader expects 16-layer engine parts (2016-2018 bundles are widened by the applier);
             a car reaches its bundle through its switch name
```
