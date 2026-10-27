---
title: Build time and run time
description: Which registries the launcher resolves into game files when a profile builds, which the in-game core resolves while the game runs, and the appliers that turn records into changes, in standard library 2.0.
---

# Build time and run time

Some records describe game files: a car package, an engine bundle, a livery's textures. They have to be in place before the game starts. Others describe things that change while the game runs: a career's line-up, a driver's ratings. Each registry says when it resolves:

```json
{ "x-registry": "engine_sound",  "x-resolve": "build" }     // the launcher, when a profile builds
{ "x-registry": "career.season", "x-resolve": "runtime" }   // the core, in game
{ "x-registry": "era.box",       "x-resolve": "both" }      // built into the game's data, and readable in game
```

## Build time: the launcher applies records

When a profile builds, the launcher resolves every build-time registry, including layers, variants and load order, then hands each effective record to an **applier**. An applier turns a record into the build operations addons write by hand today: `file.put`, database rows, ERP edits.

```text
profile build
  resolve   engine_sound "swofty.classic-seasons-2017/ferrari"   base
            vehicle      "swofty.classic-seasons-2017/ferrari"   base + layer from mod.esp: engine_sound = mod.esp/v10_screamer
  apply     engine_sound "mod.esp/v10_screamer"
              file.put  mod.esp/sounds/v10_screamer.bdl -> audio/2025/bundles/esp_v10_screamer.bdl
              db add-row audio.audio_bundle_settings (copied from ferrari_2025, bundle esp_v10_screamer)
            vehicle "swofty.classic-seasons-2017/ferrari"
              db set   audio.audio_car_data.<cs17_ferrari switch>.*.bundle_name = esp_v10_screamer
  then      every addon's own operations, as in 1.x
```

**The core's appliers** live in the launcher and cover the core's content registries (`era.box`, `content.package`, `vehicle`, `engine_sound`, `livery`, `helmet`, `track`). They're Rust in `packages/core/src/std/`, each a function from an effective record to operations:

```rust
// packages/core/src/std/engine_sound.rs
pub fn apply(id: &RecordId, sound: &EngineSound, ctx: &mut ApplyCtx) -> Result<(), ApplyError> {
    let bundle = ctx.bundle_name(id);                         // "esp_v10_screamer"
    ctx.put_file(&sound.bundle, &format!("audio/2025/bundles/{bundle}.bdl"))?;
    let settings = ctx.db_row("audio.audio_bundle_settings", &sound.settings_from)?;
    ctx.db_add_row("audio.audio_bundle_settings", settings.with_label(&bundle))?;
    ctx.note(format!("engine sound {id}: bundle {bundle}, settings from {}", sound.settings_from));
    Ok(())
}
```

**An addon's own registries** get applied by its launcher plugin. The plugin declares `"build": true` in its `launcher` block, as it does in 1.x. In 2.0 the `build` call also carries the effective records of the registries it owns, and the plugin returns operations:

```text
launcher  {"id":7,"method":"build","params":{"profile":"default","registries":{"tails.whatif.presets":{"tails.whatif.presets/real_2016":{...}}}}}
plugin    {"id":7,"result":{"ops":[{"op":"file.put","source":"generated/presets.json","target":"swofty/addons/tails.whatif-history/presets.json"}]}}
```

**Order:** a record is applied at the load-order position of the addon that provides it, and a layer at the position of the addon that wrote it, interleaved with every addon's own operations. A profile of 1.x addons has no records, so its build is byte-identical to 1.x (see [Compatibility](./compatibility)). An imported mod's recognised records apply through its own raw operations, at its own position (see [The Simplified importer](./importer)).

## Run time: the core reads records

Runtime registries are read in game, through the [reading](./reading) calls. Their providers can be dynamic, and they can have [career snapshots](./scopes#career-snapshots). An [operation](./operations) or an addon's own code applies them.

## Both

A registry with `"x-resolve": "both"` is applied at build and also readable in game. An era box is built into the game's database at build time, and the core's era picker reads its records in game to build the Addons row. Dynamic providers and runtime layers can't change the build-time part: the build is fixed once the game starts. They can change what's read in game.

## Why this matters for addons

- **Pure data addons stay pure data.** An engine sound pack, a livery pack, a season variant: JSON plus their files, no DLL, no launcher plugin.
- **Nothing gets copied.** A pack that changes one car layers that car's record, instead of shipping its own copy of Classic Seasons' rows.
- **Every 1.x build path stays.** Hand-written `operations`, imports and launcher plugin build replies work as before, and run after the appliers.
