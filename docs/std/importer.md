---
title: The Simplified importer
description: How the Simplified importer turns imported mods into registry records others can stack on, keeps everything it can't recognise exactly as it builds today, and fixes the conflicts imports cause now.
---

# The Simplified importer

The importer turns a player's mod (a `.serps` package, a zip of game files, a track zip) into an addon. In 2.0 it also **recognises** what the mod is: a livery, a car model, handling, a helmet, an engine sound, a track, a team's identity. It writes those as registry records. Everything it doesn't recognise stays exactly as it builds today.

## What changes for one mod

Macarena Red Bull today converts into `mod.macarena-red-bull-v1-0`: one overlay that writes 325 resources into `redbull.erp`. Two of them are handling files, so the mod silently undoes R&D Tyres' handling for the Red Bull.

In 2.0 the importer writes a registry index next to the converted addon, and leaves the addon's own files alone:

```text
mod.macarena-red-bull-v1-0/
  swofty.json            unchanged
  ops/erp.json           unchanged: the overlay, 325 resources
  erp/overlay.erp        unchanged
  registry/content.json  new: the livery record
  registry/layers.json   new: point the Red Bull at the livery; the two handling files, as their own layer
  registry/raw.json      new: every op, split into slices, each claimed by a record or kept raw
```

```json
// registry/content.json: "redbull_2026" becomes mod.macarena-red-bull-v1-0/redbull_2026
{
  "livery": {
    "redbull_2026": {
      "vehicle": "game/redbull_2026",
      "textures": { "erp": "erp/overlay.erp", "prefix": "eaid://f1_2026_vehicle_package/teams/redbull/", "kinds": ["textures", "materials"] },
      "picker_name": "Macarena Red Bull"
    }
  }
}
```

```json
// registry/layers.json: a layer of vehicle "game/redbull_2026" and one of vehicle.handling "game/redbull_2026"
{
  "vehicle": { "game/redbull_2026": { "livery": "mod.macarena-red-bull-v1-0/redbull_2026" } },
  "vehicle.handling": { "game/redbull_2026": { "vtf": { "erp": "erp/overlay.erp", "resources": [
    "eaid://f1_2026_vehicle_package/teams/redbull/handling/redbull.vtf",
    "eaid://f1_2026_vehicle_package/teams/fom_car_redbull/handling/fom_car_redbull.vtf" ] } } }
}
```

Now the livery and the handling are separate fields. When R&D Tyres layers `vehicle.handling` and loads after the mod, R&D's handling wins and the livery still loads. The conflicts view offers the fix in one click:

```json
{
  "kind": "field", "registry": "vehicle.handling", "id": "game/redbull_2026", "path": "/vtf",
  "addons": ["swofty.rnd-tyres-thermal", "mod.macarena-red-bull-v1-0"], "wins": "mod.macarena-red-bull-v1-0",
  "fix": { "label": "Use only Macarena Red Bull's livery",
           "choice": { "mod": "mod.macarena-red-bull-v1-0", "drop": ["vehicle.handling"] } }
}
```

## Every year by default

What a player drops in becomes part of the **base**, which every GP/TT box and career inherits, unless an addon sets its own value for that year:

```text
dropped in                         becomes                                              shows in
Macarena Red Bull livery           livery record + layer on game/redbull_2026            every box whose Red Bull is the base car
Hockenheim track zip               track record (provides venue/hockenheimring)          every box's track select, with base data
V10 engine sound pack              engine_sound records + layers on the base cars        every box using those base cars
```

A year addon keeps its own look by setting its own value. Classic Seasons' 2017 Red Bull is a variant of the base car that sets its 2017 livery, and a variant's own value beats layers on its base:

```text
[registry] vehicle swofty.classic-seasons-2017/redbull: /livery set by the variant (2017 livery);
           layer on game/redbull_2026 from mod.macarena-red-bull-v1-0 not inherited for this field
[registry] vehicle game/redbull_2026: /livery -> mod.macarena-red-bull-v1-0/redbull_2026
```

To put a drop-in on a year's car too, the player picks that car in **Applies to** (see [Targeting another base](#targeting-another-base)), and then the drop-in wins there.

## Recognisers

Each recogniser lives next to the [applier](/std/build) it inverts, in `packages/core/src/std/`. A recogniser may only claim part of a mod if the applier rebuilds exactly that part, byte for byte. Anything else stays raw.

```rust
// packages/core/src/std/vehicle.rs
pub fn apply(id: &RecordId, v: &Vehicle, ctx: &mut ApplyCtx) -> Result<(), ApplyError>;   // record -> ops
pub fn recognize(ops: &OpSlices, ctx: &RecognizeCtx) -> Vec<Claim>;                        // ops -> records

pub struct Claim {
    pub record: Option<Record>,     // a record under the mod's own key
    pub layer: Option<Layer>,       // or a layer on a game/ record or another addon's base
    pub slices: Vec<SliceRef>,      // the raw op slices it stands for
}
```

What each recogniser looks for:

```text
track          a track.add op from a track zip                          -> track mod.<id>/<track>
car archive    overlays of 2025_asset_groups/<pkg>/teams/<t>/wep/<t>.erp, split by resource path:
                 teams/<t>/textures/, materials/                        -> livery
                 teams/<t>/idf/, wep/                                    -> vehicle /model
                 teams/<t>/handling/*.vtf, fom_car_<t>/handling/*.vtf   -> vehicle.handling
                 teams/common*.erp, shared/generics/                    -> raw
               <pkg>/<t> is matched to a game/ vehicle through the stock database
               (entrants.vehicles: asset_filename, vehicle_package), never a hard-coded folder list
special livery se_livery and unlockable_se_livery rows plus <t><wep_override>.erp -> livery with "special"
helmet         helmet archives and their item rows                      -> helmet
engine sound   audio/2025/bundles/<maker>_<year>.bdl                    -> engine_sound layer on that maker
identity       named team and driver rows and their text                -> team.identity / driver.identity layers
tyre visuals   slick texture arrays and their mipmaps                   -> tyre.visual layer
season pack    car archives for 3 or more teams in one vehicle package  -> one vehicle per team, plus a dormant
                                                                           content.package variant
```

## Everything else stays raw

Every op the addon has is listed in `registry/raw.json`, in build order, split into slices. Each slice is claimed by a record or kept, with the reason:

```json
// mod.f1-2020-cars-for-f1-25/registry/raw.json (abridged)
{
  "recognizer": 1,
  "ops": [
    { "file": "ops/erp.json", "index": 0, "op": "overlay",
      "target": "2025_asset_groups/animation_package/scenes/incardriver/2025/incardriver_f1.erp",
      "slices": [ { "resources": 708, "claimed": null, "why": "no recognizer: in-car driver animation" } ] },
    { "file": "ops/erp.json", "index": 3, "op": "overlay",
      "target": "2025_asset_groups/f1_2025_vehicle_package/teams/ferrari/wep/ferrari.erp",
      "slices": [
        { "match": ["teams/ferrari/textures/", "teams/ferrari/materials/"], "claimed": "vehicle game/ferrari_2025 /livery" },
        { "match": ["teams/ferrari/idf/", "teams/ferrari/wep/"],            "claimed": "vehicle game/ferrari_2025 /model" },
        { "match": ["teams/ferrari/handling/", "teams/fom_car_ferrari/handling/"], "claimed": "vehicle.handling game/ferrari_2025 /vtf" } ] }
  ]
}
```

- **Raw stays raw.** Database drift from an older game build, in-car animation, shared archives, UI flows and tool junk are never claimed, and build exactly as before.
- **Mods that haven't converted yet** still load through the importer's build hook, and stay raw until they convert.

## Order and precedence

A mod's place is its addon's place in the profile's load order. The importer page lists imported addons in that order, and dragging one reorders the profile.

```text
raw vs raw        two imports, or an import and a 1.x addon: per target (resource, record, text key, file,
                  texture slice), the later one wins. Unchanged from 1.x.
layer vs raw      per field, the later one wins.
                  the 2.0 layer is later: the build masks the claimed raw slices of that field from earlier addons
                  the raw import is later: the layer's ops for that field aren't emitted
player choice     a target or a dropped part counts as a layer at the mod's own place
```

Masking needs one addition to `erp.overlay`: an `except` list of resources that the build skips. When nothing is masked, no `except` is written, and the build is byte-identical to today's.

Records an import recognises apply **in place**, at the import's own load-order position, through its own raw ops. So recognising a mod never changes a build. A build changes only when another addon's layer wins a field, or the player retargets the mod, drops a part, or gives a season its own entry.

## Targeting another base

A livery made for the stock Red Bull can go on Classic Seasons' 2017 Red Bull instead, from the mod or from the importer page.

From the mod, in a block that older importers skip:

```json
// mod.serps
{
  "mod_name": "Ferrari 2017 Monza", "author": "someone", "version": "1.0", "game_version": "1.0.141.7865",
  "swofty": {
    "targets": [ { "kind": "livery", "from": "game/ferrari_2025", "to": "swofty.classic-seasons-2017/ferrari" } ],
    "requires": [ { "record": "content.package:swofty.classic-seasons-2017/f1_2017" } ]
  }
}
```

From the importer page:

```text
Macarena Red Bull                                       livery · handling
  Livery     Applies to  [ Red Bull 2026 (stock)  v ]   Red Bull 2026 (stock)
                                                        F1 2017 Red Bull (Classic Seasons 2017)   same model
                                                        F1 2016 Red Bull (Classic Seasons 2016)   same model
  Handling   [x] Use the mod's handling
```

The picker lists only targets that fit: a livery fits a vehicle whose `donor` chain reaches the livery's model, a helmet any driver, an engine sound any vehicle. A `mod.serps` target that doesn't fit is refused with the reason:

```text
[import] livery mod.ferrari-2017-monza/ferrari_2025: swofty.classic-seasons-2017/ferrari has model
         game/ferrari_2017_port, not game/ferrari_2025; the livery stays on game/ferrari_2025
```

The choice is saved in `importer.json`, so a re-import keeps it.

## Season packs and their own entry

A car pack for a whole season (fanfreluche's F1 2020 cars, F1 2012 Temporada) is recognised team by team. It also provides a dormant `content.package` variant:

```json
{
  "vehicle": {
    "ferrari": { "extends": "game/ferrari_2025", "model": { "erp": "erp/overlay-4.erp" },
                 "livery": "mod.f1-2020-cars-for-f1-25/ferrari", "handling": "mod.f1-2020-cars-for-f1-25/ferrari" }
  },
  "vehicle.handling": {
    "ferrari": { "vtf": { "erp": "erp/overlay-4.erp" } }
  },
  "content.package": {
    "f1_2020": { "extends": "game/f1_2025", "title": "F1 2020 cars for F1 25",
                 "teams": [ { "key": "ferrari", "vehicle": "mod.f1-2020-cars-for-f1-25/ferrari" } ] }
  }
}
```

- **In place** (the default, which is today's behaviour): the pack replaces the 2025 cars.
- **Own GP/TT entry** (a switch on the importer page): the pack gets its own cloned vehicle package and an era box, the way Classic Seasons builds `cs_2017_*`. Two car packs can then be installed side by side, where today they overwrite the same ten team slots.

## Existing imports

On upgrade, a startup pass indexes every already-imported mod in place:

```text
[import] index mod.macarena-red-bull-v1-0: 325 resources claimed (livery 323, vehicle.handling 2), 0 raw; round trip ok
[import] index mod.f1-2012-season-v1-0-bydanix: 2,114 claimed, 4,312 raw (database drift from an older game build)
```

- **Nothing is reconverted.** The pass writes only the new files; `ops/`, the archives and `swofty.json` keep their bytes, so the build hash doesn't change.
- **Downgrading to 1.5 still works.** `swofty.json` is untouched, and 1.5 never opens `registry/`.
- **The importer itself** ships a new major release that needs launcher 2.0. A 1.x launcher keeps getting the newest 0.11.x.

## What the importer provides

The importer owns three registries under its own id:

- **`teamsimplified.mods.package`:** one record per imported mod: its source file and hash, its parts and variants, the records it provides, its raw count and the player's choices;
- **`teamsimplified.mods.raw`:** each mod's `raw.json`, for Send Logs and the triage bot;
- **`teamsimplified.mods.rule`:** path rules other addons provide, so mods made against their files are targeted automatically. Classic Seasons 2017 can say that `cs_2017_vehicle_package/teams/<t>/` is `vehicle swofty.classic-seasons-2017/<t>`.

```text
swofty mods show <id>                       records and raw slices of one import
swofty mods target <id> <record> <base>     retarget a recognised record
swofty mods recognize --check [<id>]        re-run the round trip without writing
```

## The release gate for the importer

- **Round-trip tests:** for each recogniser, applying the claims of a real mod gives the exact ops the converter made.
- **Golden builds:** every imported mod in the player bundles the triage bot has, built on 1.5.19 and on 2.0, with byte-identical game files.
- **A downgrade build:** a 2.0-indexed profile builds on 1.5.19 exactly as before.
- **Masking, targeting and the season switch,** each with an in-game check.
