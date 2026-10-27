---
title: UI and strings
description: Era boxes, screens, tiles, texture slices, options, HUD widgets and strings as registries, with the UI hook points.
---

# UI and strings

```text
registries   era.box, ui.screen, ui.tile, ui.texture_slice (one slice of a texture array), ui.route,
             ui.option, ui.theme, hud.widget, lang.string
hook points  ui.era.tiles, ui.home.tiles, ui.career_content.tiles, ui.car_category.tiles, ui.screen.built,
             ui.vm.resolve (Vm_Resolve_RegistryNode), ui.route.open, hud.mfd.pages, lang.lookup
operations   ui::set_text, ui::set_bool, ui::set_ref, popup::show, hud::toast
ready        ready for tiles, era boxes, view models, routes, popups and strings; HUD partial
limits       native screens with fixed counts (era tiles, four R&D departments, eight MFD pages)
```
