---
layout: false
title: Swofty Launcher for F1 25
titleTemplate: Mods and tracks on Windows and Linux
description: A launcher and mod loader for F1 25 on Windows and Linux. Add community tracks, install addons, change addon settings in game and keep separate profiles.

screenshots:
  hero:
    src: /screenshots/hero-launcher.png
    alt: The Swofty Launcher for F1 25 launcher
  gallery:
    - src: /screenshots/gallery-ingame.png
      alt: The F1 25 home menu with ADDON SETTINGS under Career, the "Launched with Swofty Launcher" panel and the F9 prompt
      caption: ADDON SETTINGS in the game's home menu
      shape: square
    - src: /screenshots/gallery-launcher.png
      alt: The Swofty Launcher for F1 25 launcher
      caption: Addons, maps and profiles in the launcher
      shape: wide

hero:
  name: Swofty Launcher for F1 25
  tagline: Bringing <em>mod support</em> to
  game:
    text: F1 25
    link: https://store.steampowered.com/app/3059520/
  download: https://github.com/Swofty-Developments/F1ModManager/releases
  docs: /guide/

links:
  - { icon: book, text: Documentation, link: /guide/ }
  - { icon: github, text: Source Code, link: https://github.com/Swofty-Developments/F1ModManager }
  - { icon: tag, text: Releases, link: https://github.com/Swofty-Developments/F1ModManager/releases }

intro: Swofty Launcher for F1 25 is a <em>launcher and mod loader</em> for F1 25 on <em>Windows and Linux</em>. Add <em>community tracks</em>, install <em>addons</em>, change <em>addon settings in game</em> and keep separate <em>profiles</em>. Modded play is <em>offline only</em>, and launching from Steam stays vanilla.

platforms:
  - { icon: windows, text: Windows 10 and 11 }
  - { icon: linux, text: Linux with Steam and Proton }
  - { icon: deck, text: Steam Deck }

featuresTitle: Everything in <em>one launcher</em>
features:
  - icon: map
    title: Maps
    details: Import community track packages, such as <em>Sepang</em> and <em>Okayama</em>, and race on them as extra tracks.
    link: /using/importing-maps
  - icon: box
    title: Addons
    details: Install, enable and disable addons. The app <em>warns you</em> when two addons change the same thing.
    link: /using/mods-and-maps
  - icon: sliders
    title: In-game addon settings
    details: Open <em>ADDON SETTINGS</em> in the game's home menu, under Career, to change addon options without leaving the game.
    link: /using/swofty-mod
  - icon: layers
    title: Profiles
    details: Switch between separate sets of addons and maps, each with its own saves. The original game is always <em>restorable in one click</em>.
    link: /using/profiles
  - icon: offline
    title: Offline-only modded play
    details: Modded sessions always run <em>offline</em> with multiplayer off. Launching from Steam normally stays <em>completely vanilla</em>.
    link: /reference/safety
  - icon: play
    title: The launcher app
    details: Manage everything and press <em>Play</em> in one window, with <em>live logs</em> to help if the game crashes.
    link: /guide/installation

developers:
  text: Swofty Launcher for F1 25 is <em>open source</em>, with a hook system built for modders. Addons <em>hook game functions by name</em>, with <em>priorities</em> and <em>chaining</em> so several addons can change the same function.
  actions:
    - { icon: book, text: Modding docs, link: /modding/template, style: solid }
    - { icon: chip, text: Plugin SDK, link: /modding/native-plugins, style: outline }
    - { icon: code, text: Game API, link: /reference/game/, style: outline }

cta:
  text: Interested? Read the <em>installation guide</em> to set up Swofty Launcher for F1 25, or the <em>docs</em> to start making addons.
  actions:
    - { icon: download, text: Installation guide, link: /guide/installation }
    - { icon: book, text: Docs, link: /guide/ }
    - { icon: github, text: Source, link: https://github.com/Swofty-Developments/F1ModManager }
---

<Landing>
<template #code>

```cpp
#include <swofty/mixin.hpp>
#include <swofty/game/fe_home.hpp>

using namespace swofty::game;

SWOFTY_WRAP(FeHome_Handle_Command, uint64_t, FEHomeViewModel *vm, NeStr **cmd)
{
    if (swofty::str_eq(*cmd, "example.open_page"))
        return open_page(vm);
    return orig(vm, cmd);
}
```

</template>
</Landing>
