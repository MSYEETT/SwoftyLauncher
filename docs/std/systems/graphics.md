---
title: Graphics, replays and telemetry
description: Looks, post-process passes, graphics settings, replay cameras and telemetry channels.
---

# Graphics, replays and telemetry

```text
registries   graphics.look (a LUT with strength, stacked in order), graphics.pass, graphics.setting
             replay.hero_shots, telemetry.channel (a value an addon publishes per car or session)
hook points  render.present, graphics.setting.read, replay.recordable.register, session.tick
ready        looks and passes ready (shipped); settings L2; replays and telemetry partial
limits       HDR and frame generation change the pipeline; replay holders size the grid
```
