---
title: Make an engine sound pack
description: Build a V10 sound addon for the stock 2025 Ferrari, or target only Classic Seasons' 2017 Ferrari.
---

# Make an engine sound pack

You'll build `mod.esp`, an addon that registers a V10 engine sound and puts it on a Ferrari: either the stock 2025 Ferrari, in every year that uses its sound, or only Classic Seasons' 2017 Ferrari. The player picks the Ferrari in Grand Prix or Time Trial and hears the V10 on track.

## The folder

Create an empty `mod.esp` folder. The finished layout shows both implementations: use `native/` for C++, or `registry/` for **Without code**; both use `swofty.json`, `registry.json` and the sound bundle.

```text
mod.esp/
  swofty.json
  registry.json
  sounds/
    v10_screamer.bdl
  native/
    build.sh
    esp.dll
    src/
      esp.cpp
  registry/
    sounds.json
    layers.json
```

## 1. Add the bundle and register the sound

<Split>

Put your V10 sound bundle at `sounds/v10_screamer.bdl`: one you made, or one from a sound mod you have permission to use. Sound mods keep their bundles at `audio/2025/bundles/<maker>_<year>.bdl`, which is how the [importer recognises them](/std/importer#recognisers).

`settings_from` copies the stock Ferrari sound's mixing settings, while `tuning_from` copies its engine, era, cabin and exhaust tuning; the V10 audio comes from your bundle ([Sounds](/std/systems/sounds)).

Save this complete file as `native/src/esp.cpp`, or save the two files in **Without code**.

<template #example>
<Example root="mod.esp" code-file="native/src/esp.cpp" data-file="registry/sounds.json" files="swofty.json, registry.json, native/build.sh, native/esp.dll, sounds/v10_screamer.bdl, registry/layers.json">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/sounds.hpp>
#include <swofty/systems/cars.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    // Registers mod.esp/v10_screamer.
    auto v10 = sounds::engine_sounds.add("v10_screamer", {
        .bundle = file("sounds/v10_screamer.bdl"),
        .settings_from = sounds::engine_sounds["game/ferrari_2025"],
        .tuning_from = sounds::engine_sounds["game/ferrari_2025"],
    });

    // Every car that inherits this sound receives the V10.
    cars::vehicles.layer(cars::vehicles["game/ferrari_2025"],
                         [=](cars::Vehicle &v) {
        v.engine_sound = v10;
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/sounds.json
{
  "engine_sound": {
    "v10_screamer": {
      "bundle": "sounds/v10_screamer.bdl",
      "settings_from": "game/ferrari_2025",
      "tuning_from": "game/ferrari_2025"
    }
  }
}
```

```json
// registry/layers.json
{
  "vehicle": {
    "game/ferrari_2025": {
      "engine_sound": "mod.esp/v10_screamer"
    }
  }
}
```

</template>
</Example>
</template>
</Split>

`SWOFTY_REGISTER` runs before the game loads its database; the vehicle's `engine_sound` supplies the audio records the core applies at boot ([Cars](/std/systems/cars)).

Layering `game/ferrari_2025` reaches every year that inherits that car's sound. A year that sets its own sound keeps it, including Classic Seasons' cars with period sounds. Targeting `swofty.classic-seasons-2017/ferrari` instead changes the 2017 Ferrari wherever that record is used. See [How changes reach seasons and careers](/std/inheritance).

## 2. Name the addon and declare its target

<Split>

Save these two files at the addon's root; `swofty.json` identifies the addon and its DLL, while `registry.json` requires the target vehicle before Play ([Manifest](/std/manifest)). For **Without code**, omit `operations` from `swofty.json`; the core discovers `registry/` by its folder name.

<template #example>
<Example root="mod.esp" data-file="swofty.json" files="registry.json, native/build.sh, native/src/esp.cpp, native/esp.dll, sounds/v10_screamer.bdl, registry/sounds.json, registry/layers.json">
<template #data>

```json
{
  "schema_version": 1,
  "id": "mod.esp",
  "name": "V10 Engine Sounds",
  "version": "1.0.0",
  "author": "you",
  "category": "cars",
  "compat": { "game": "f1_25", "launcher": ">=1.6.0" },
  "operations": { "native": [ { "dll": "native/esp.dll" } ] }
}
```

```json
// registry.json
{
  "requires": [
    { "record": "vehicle:game/ferrari_2025" }
  ]
}
```

</template>
</Example>
</template>
</Split>

## 3. Alternatively, target the 2017 Ferrari

<Split>

For the Classic Seasons version, replace `native/src/esp.cpp` with this file, or replace only `registry/layers.json` in **Without code**. Keep `registry/sounds.json` and the bundle from step 1; this [layer](/std/layers) changes the 2017 Ferrari's own sound, with later layers on the same field winning by load order.

<template #example>
<Example root="mod.esp" code-file="native/src/esp.cpp" data-file="registry/layers.json" files="swofty.json, registry.json, native/build.sh, native/esp.dll, sounds/v10_screamer.bdl, registry/sounds.json">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/sounds.hpp>
#include <swofty/systems/cars.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto v10 = sounds::engine_sounds.add("v10_screamer", {
        .bundle = file("sounds/v10_screamer.bdl"),
        .settings_from = sounds::engine_sounds["game/ferrari_2025"],
        .tuning_from = sounds::engine_sounds["game/ferrari_2025"],
    });

    // Target Classic Seasons' own car.
    cars::vehicles.layer(
        cars::vehicles["swofty.classic-seasons-2017/ferrari"],
        [=](cars::Vehicle &v) {
            v.engine_sound = v10;
        });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
{
  "vehicle": {
    "swofty.classic-seasons-2017/ferrari": {
      "engine_sound": "mod.esp/v10_screamer"
    }
  }
}
```

</template>
</Example>
</template>
</Split>

Replace the root manifests with these files. The addon requirement names Classic Seasons, and the record requirement checks that it still provides the Ferrari; omit `operations` for **Without code**.

<Split>

Both requirements are checked before Play ([Dependencies](/std/dependencies)).

<template #example>
<Example root="mod.esp" data-file="swofty.json" files="registry.json, native/build.sh, native/src/esp.cpp, native/esp.dll, sounds/v10_screamer.bdl, registry/sounds.json, registry/layers.json">
<template #data>

```json
{
  "schema_version": 1,
  "id": "mod.esp",
  "name": "V10 Engine Sounds",
  "version": "1.0.0",
  "author": "you",
  "category": "cars",
  "compat": { "game": "f1_25", "launcher": ">=1.6.0" },
  "requires": [
    { "id": "swofty.classic-seasons-2017" }
  ],
  "operations": { "native": [ { "dll": "native/esp.dll" } ] }
}
```

```json
// registry.json
{
  "requires": [
    { "record": "vehicle:swofty.classic-seasons-2017/ferrari" }
  ]
}
```

</template>
</Example>
</template>
</Split>

## Try it

For the C++ version, build `native/esp.dll` from inside `mod.esp`. `native/build.sh` comes from the [addon template](/reference/addons/template) and builds the DLL with zig. The **Without code** version skips this step.

```sh
native/build.sh
```

From the directory containing `mod.esp`, install and enable the addon, then run the game:

```sh
swofty addons install ./mod.esp
swofty addons enable mod.esp
swofty run --apply
```

Pick the stock 2025 Ferrari in Time Trial. For the alternative, enable Classic Seasons 2017 and pick its Ferrari in the F1 2017 box. Restart the game after changing the pack.

In `swofty/loader.log`, the native version reports its sound registration and the vehicle edit. For the stock target, look for these lines; registration timing varies:

```text
[registry] register: mod.esp (esp.dll) 1 engine_sound, 1 layer on vehicle (early stage, 0.4 ms)
[registry] vehicle game/ferrari_2025: edit by mod.esp changed /engine_sound -> mod.esp/v10_screamer
```

For the Classic Seasons version:

```text
[registry] register: mod.esp (esp.dll) 1 engine_sound, 1 layer on vehicle (early stage, 0.4 ms)
[registry] vehicle swofty.classic-seasons-2017/ferrari: edit by mod.esp changed /engine_sound -> mod.esp/v10_screamer
```

## When something's missing

If Classic Seasons 2017 is off, the alternative's `requires` entry stops Play and the launcher names the missing dependency. Turn Classic Seasons back on before launching.

If an update removes or renames the target Ferrari record, the `registry.json` requirement also stops Play and identifies the missing record. Update the layer and its record requirement together to a target the provider defines.

Turning off `mod.esp` removes its sound layer at the next game start, revealing the sound beneath it; a missing vehicle sound reference falls back to the donor's sound ([Missing references](/std/dependencies#when-a-reference-is-missing)).

## Publish

Follow [Packaging and publishing](/reference/addons/publishing) to publish the finished `mod.esp` addon to the Addon Browser.