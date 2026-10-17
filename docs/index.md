---
layout: home

hero:
  name: F1 Mod Manager
  text: Mods and maps for F1 25
  tagline: A mod launcher for F1 25 on Windows and on Linux through Steam and Proton. Modded sessions are offline, every change to the game folder is journalled and undone, and community track packages import unchanged.
  image:
    src: /logo.svg
    alt: F1 Mod Manager
  actions:
    - theme: brand
      text: Installation
      link: /guide/installation
    - theme: alt
      text: Importing maps
      link: /using/importing-maps
    - theme: alt
      text: Making mods
      link: /modding/manifest

features:
  - title: Mods and maps
    details: Mods change the database, assets, UI and native code. Maps are community track packages, imported from their zip as they ship.
  - title: Non-destructive
    details: Every change to the game folder is journalled before it happens. Switching to vanilla, a crash or swofty restore leaves the folder byte-identical to Steam's copy.
  - title: Offline only
    details: A modded session never goes online and never runs alongside the anticheat. Steam's own Play button stays vanilla on Windows.
  - title: Declarative mod format
    details: One swofty.json per mod, with database operations by name, ERP operations, UI node patches, loose files and native plugins.
  - title: SwoftyMod built in
    details: A MOD SETTINGS tile under Career, the F9 prompt, the bottom-left launcher panel and an in-game settings page.
  - title: A desktop app and a CLI
    details: Swofty Mod Launcher is a native Rust app with an editor-style interface. The swofty CLI does everything headless, including the Steam launch wrapper.
---

## Where to start

- [Installation](/guide/installation): requirements, then [Windows](/guide/windows) or [Linux and Steam Deck](/guide/linux).
- [Mods and maps](/using/mods-and-maps): the two kinds of content the launcher manages.
- [Importing maps](/using/importing-maps): install a community track package from its zip.
- [Manifest](/modding/manifest): the `swofty.json` reference for mod authors.
- [The mod template](/modding/template): a sample mod to copy, under the MIT licence.
- [Design v0.3](/design/): the full design of the launcher and the in-game loader.

F1 Mod Manager is free software under the AGPL-3.0-or-later. The plugin SDK and the mod template are MIT.
