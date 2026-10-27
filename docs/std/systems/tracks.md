---
title: Tracks
description: Tracks in code - tracks, venues, per-season track data, each GP/TT box's view of the tracks, world layers, slots and AI lines as typed registries, with imported tracks, the track events and operations, in standard library 2.0.
---

# Tracks

A track is a `tracks::Track` record that provides one or more venues, with a `tracks::TrackData` block in every data set the game has.

## The header

<Example root="swofty-sdk" code-file="include/swofty/systems/tracks.hpp">
<template #code>

```cpp
#include <swofty/registry.hpp>

namespace swofty::tracks {

struct Venue {                                     // a real circuit; the core owns the venue/ ids
    std::string name;                              // "Hockenheimring"
    std::string country;                           // "germany"
    Ref<Track> track;                              // resolved: the first enabled track that provides it, in load order
};

struct TrackNames {                                // TrackNameData: menus read names here, not from tracks.Track
    Text title;                                    // the track's title
    Text gp;                                       // the grand prix's name
    std::string acronym;                           // "HOC"
    Text city;                                     // the city
};

struct Track {                                     // a tracks.Track row, its files, and its rows in every data set
    std::string code;                              // the track code: its folder and row label
    std::string name;                              // shown in logs and the conflicts report
    Ref<Track> base;                               // the stock track its rows are cloned from, in every data set (a variant)
    Ref<Track> slot;                               // a dormant stock slot to fill (game/sochi); empty: a new row
    std::vector<Ref<Venue>> provides;              // the real circuits it is
    TrackNames names;                              // its names
    Path files;                                    // track/<code>.erp, mipmaps/, ui/
    float length_metres = 0;                       // length_metres
    int corners = 0;                               // numberOfCorners
    float latitude = 0, longitude = 0;             // for daylight and weather
    StatId stat_id = StatId::automatic;            // pinned for the install, so saved careers keep their rounds
};

struct Fuel { float burn_per_lap; };               // TrackGeneralData.fuel_burn_per_lap (+ RaceFuelSettings)
struct Tyres { std::string softer, medium, harder; };   // TyreData: compound_softer / _medium / _harder
struct Pit { int speed_limit; float stop_time_dry, stop_time_wet; };   // TrackPitData
struct Date { int day, month; };                   // weekend_day, weekend_month

struct TrackData {                                 // one TrackData block; id "<track id>/<data set>"
    Ref<Track> track;                              // the track
    std::string data_set;                          // a TrackDataSet: "f1_2026", "sprint_f1_2026", "classic" ...
    int laps = 0;                                  // number_of_laps
    Fuel fuel;                                     // fuel
    Tyres tyres;                                   // the dry compounds
    Ref<tyres::Class> tyre_class;                  // or a tyre class (see Tyres)
    Pit pit;                                       // pit lane
    Ref<cars::Vehicle> safety_car;                 // TrackSafetyCarData.safety_car_vehicle
    TrackNames names;                              // per-set names
    Date date;                                     // the weekend's date
};

struct PackageTrack {                              // one GP/TT box's view of one track; id "<package id>/<track code>"
    Ref<content::Package> package;                 // the box's package
    Ref<Track> track;                              // the track
    bool offered = true;                           // in this box's track select
    int order = 0;                                 // its place in the box's list; 0 keeps the base order
    Ref<Layer> world;                              // the layout or world version for this box
    std::optional<int> laps;                       // empty fields come from the base track data, layers included
    std::optional<Fuel> fuel;
    Ref<tyres::Class> tyre_class;
    std::optional<Pit> pit;
    std::optional<TrackNames> names;
    std::optional<Date> date;
};

struct World { std::string asset_group; std::string eri; };   // worldAssetGroup, worldEri

struct Layer {                                     // a track_data.track_layers row: a world per mode
    Ref<Track> track;
    std::string mode;                              // "f1", "f2", "myteam", "swofty_f1" ...
    World world;                                   // <track>_f1_26.wep (22 grid, 11 garages) or _myteam (24, 12)
    int grid = 22;                                 // grid slots the world has
    int garages = 11;                              // garages the world has
};

struct Slots {                                     // grid slots, garages and pit boxes added to one world
    Ref<Layer> layer;
    std::vector<Transform> grid, garages, pit_boxes;
};

struct AiLine {                                    // baked AI data: ai/<package folder>/<asset_filename>/*.erp
    Ref<Track> track;
    Ref<cars::Package> package;
    Path archive;
};

inline Registry<Track> tracks{"track", { .applies = Applies::boot }};
inline Registry<Venue> venues{"track.venue", { .applies = Applies::boot }};
inline Registry<TrackData> data{"track.data", { .applies = Applies::session }};
inline Registry<PackageTrack> per_package{"track.per_package", { .applies = Applies::content }};
inline Registry<Layer> layers{"track.layer", { .applies = Applies::boot }};
inline Registry<Slots> slots{"track.slots", { .applies = Applies::session }};   // when a bigger grid is active
inline Registry<AiLine> ai_lines{"track.ai_line", { .applies = Applies::session }};

Result set_data(Ref<Track> track, Edit<TrackData> edit);         // the loaded track object; put back on unload

}

namespace swofty::tracks::events {

struct DataGet {                                   // Game_TrackData_GetTrack: after
    static constexpr const char *point = "track.data.get";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    std::string_view data_set;                     // the set the game reads
    Ref<Track> track;
    TrackData &data;                               // the live object; after handlers may change it
};

}
```

</template>
</Example>

Race fuel at session load is `session.fuel.load` and `session::set_track_fuel` on [Sessions and weekends](/std/systems/sessions).

## Hockenheim, in code

<Split>

Hockenheim is a new track, not a stock slot. It clones Austria's rows in every data set and provides `venue/hockenheimring`.

- **Every data set gets a block,** cloned from Austria's as a variant. Layers on Austria's data flow in until Hockenheim sets the field itself.
- **A selector** sets what holds in every set; an edit on one set changes that set only.
- **The core allocates the stat id** and pins it for the install.
- **The world** keeps the stock counts: `hockenheim_f1_26` has 22 grid slots and 11 garages, the My Team world 24 and 12.

<template #example>
<Example root="mod.hockenheim" code-file="native/src/track.cpp" data-file="registry/track.json" files="swofty.json, track/hockenheim.erp, mipmaps/hockenheim_d.tif.mipmaps, ui/minimap.json, ui/images.json, lang/track.json">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/tracks.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto hockenheim = tracks::tracks.add("hockenheim", {
        .code = "hockenheim",
        .name = "Hockenheimring",
        .base = tracks::tracks["game/austria"],
        .provides = { tracks::venues["venue/hockenheimring"] },
        .names = { .title = Text::key("lng_hockenheim_title"), .gp = Text::key("lng_hockenheim_gp"), .acronym = "HOC" },
        .files = file("track/"),
        .length_metres = 4574, .corners = 17, .latitude = 49.3278f, .longitude = 8.5658f,
    });

    // every data set: 67 laps, its own fuel and pit lane
    tracks::data.layer(under<tracks::TrackData>("mod.hockenheim/hockenheim/"), [](tracks::TrackData &d) {
        d.laps = 67;
        d.fuel.burn_per_lap = 1.62f;
        d.pit.speed_limit = 80;
    });

    // the 2025 and 2026 sets: the 2019 compounds
    for (const char *set : { "f1_2025", "sprint_f1_2025", "f1_2026", "sprint_f1_2026" })
        tracks::data.layer(tracks::data[std::string("mod.hockenheim/hockenheim/") + set], [](tracks::TrackData &d) {
            d.tyres = { .softer = "C4", .medium = "C3", .harder = "C2" };
        });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/track.json
{
  "track": {
    "hockenheim": {
      "code": "hockenheim", "name": "Hockenheimring", "extends": "game/austria", "slot": "new",
      "provides": ["venue/hockenheimring"],
      "names": { "title": "lng_hockenheim_title", "gp": "lng_hockenheim_gp", "acronym": "HOC" },
      "length_metres": 4574, "corners": 17, "latitude": 49.3278, "longitude": 8.5658
    }
  },
  "track.data": {
    "$select": [
      { "ids": "mod.hockenheim/hockenheim/*", "patch": { "laps": 67, "fuel": { "burn_per_lap": 1.62 }, "pit": { "speed_limit": 80 } } }
    ],
    "mod.hockenheim/hockenheim/f1_2025": { "tyres": { "softer": "C4", "medium": "C3", "harder": "C2" } },
    "mod.hockenheim/hockenheim/f1_2026": { "tyres": { "softer": "C4", "medium": "C3", "harder": "C2" } }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: mod.hockenheim (hockenheim.dll) 1 track, 5 layers on track.data (early stage, 0.3 ms)
[registry] track mod.hockenheim/hockenheim: base game/austria, new row, provides venue/hockenheimring
[tracks]   hockenheim: stat id 43 (pinned since 2026-10-27)
[registry] applied at boot: track 1 (19 data sets cloned from game/austria; 77 asset groups; 1,712 database records)
[registry] applied at session load: track.data mod.hockenheim/hockenheim/f1_2026 (laps 67, fuel 1.62, tyres C4/C3/C2)
```

## A calendar that uses it, optionally

<Split>

Classic Seasons 2016 races the 2016 calendar. Round 12 is the German Grand Prix at Hockenheim, which only a track addon provides. The round asks for the venue, optionally: with no addon that provides it, the round is dropped; with one, it resolves live, to whichever track provides it first in load order.

<template #example>
<Example root="swofty.classic-seasons-2016" code-file="native/src/calendar.cpp" data-file="registry/calendar.json" files="swofty.json, lang/season.json">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/career.hpp>
#include <swofty/systems/tracks.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    career::calendars.add("f1_2016", { .rounds = {
        { .round = 1,  .race = Text::key("lng_gp_australia"), .venue = tracks::venues["venue/albert_park"] },
        { .round = 4,  .race = Text::key("lng_gp_russia"),    .venue = tracks::venues["venue/sochi"].optional() },
        { .round = 12, .race = Text::key("lng_gp_germany"),   .venue = tracks::venues["venue/hockenheimring"].optional() },
        { .round = 16, .race = Text::key("lng_gp_malaysia"),
          .venue = tracks::venues["venue/sepang"].or_else(tracks::venues["venue/bahrain"]) },
    }});
}
```

</template>
<template #data>

```json
// registry/calendar.json
{
  "career.calendar": {
    "f1_2016": { "rounds": [
      { "round": 1,  "race": "lng_gp_australia", "venue": "venue/albert_park" },
      { "round": 4,  "race": "lng_gp_russia",    "venue": "venue/sochi?" },
      { "round": 12, "race": "lng_gp_germany",   "venue": "venue/hockenheimring?" },
      { "round": 16, "race": "lng_gp_malaysia",  "venue": "venue/sepang|venue/bahrain" }
    ] }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] career.calendar swofty.classic-seasons-2016/f1_2016: round 12 venue/hockenheimring -> mod.hockenheim/hockenheim
[registry] career.calendar swofty.classic-seasons-2016/f1_2016: round 4 dropped (venue/sochi: no enabled track provides it; optional)
```

A [guard](/std/dependencies) stops a saved career whose track has gone missing.

## A track is in every year

<Split>

A track goes into the base that every GP/TT box and career inherits. Hockenheim appears in every box's track select, 2016, 2017 and the stock 2025 and 2026 alike, with its base data, unless something year-specific changes it.

Each box has its own view of the tracks: `tracks::per_package`, keyed `<package>/<track>`.

- **Which tracks it offers, and in what order.** By default, all of the base's, so tracks other addons add later appear too.
- **Per-track data for that box only:** world version, laps, fuel, tyre class, pit, names, date. An empty field comes from the base track data, so layers on the base reach every box that doesn't set it.
- **A variant package** (F1 2017 V10) inherits its base package's view.
- **Live:** when a box is picked, the core applies its view to the track data the game reads (`Game_TrackData_GetTrack`), and puts it back when the box is left.

<template #example>
<Example root="swofty.classic-seasons-2016" code-file="native/src/tracks.cpp" data-file="registry/tracks.json" files="swofty.json, native/src/calendar.cpp">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/content.hpp>
#include <swofty/systems/tracks.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto box = content::packages["swofty.classic-seasons-2016/f1_2016"];

    // the 2016 order, by track code; tracks the box doesn't list keep the base order, after these
    int order = 1;
    for (const char *code : { "australia", "bahrain", "china", "sochi", "catalunya", "monaco", "montreal", "baku",
                              "austria", "silverstone", "hungaroring", "hockenheim", "spa", "monza" })
        tracks::per_package.add(std::string("f1_2016/") + code, { .package = box, .order = order++ });

    // 2016 data at Hockenheim; nothing happens while no track addon provides it
    tracks::per_package.layer(tracks::per_package["swofty.classic-seasons-2016/f1_2016/hockenheim"].optional(),
        [](tracks::PackageTrack &t) {
            t.laps = 67;
            t.fuel = tracks::Fuel{ .burn_per_lap = 1.71f };
        });
}
```

</template>
<template #data>

```json
// registry/tracks.json
{
  "track.per_package": {
    "f1_2016/australia":  { "package": "swofty.classic-seasons-2016/f1_2016", "order": 1 },
    "f1_2016/hockenheim": { "package": "swofty.classic-seasons-2016/f1_2016", "order": 12,
                            "laps": 67, "fuel": { "burn_per_lap": 1.71 } }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] content swofty.classic-seasons-2016/f1_2016 active: 14 scoped changes applied (track view: 21 tracks, 14 ordered;
           mod.hockenheim/hockenheim laps 67, fuel 1.71)
[registry] content swofty.classic-seasons-2016/f1_2016 ended: 14 scoped changes put back
[registry] content game/f1_2026 active: track view of the base (22 tracks, mod.hockenheim/hockenheim included, base data)
```

A 2018-only layout addon edits only the 2018 box's entry:

```cpp
tracks::per_package.layer(tracks::per_package["swofty.classic-seasons-2018/f1_2018/hockenheim"], [](tracks::PackageTrack &t) {
    t.world = tracks::layers["mod.hockenheim-2018/hockenheim_2018"];
});
```

The track select list per box comes from `ui.track_select.build` (`ui::events::TrackSelectBuild`), on the list the game builds.

## Imported tracks

<Split>

The importer turns a track zip into a track addon once. The core activates it live at every game start; the launcher only places its files.

1. **The importer converts the zip** into `swofty.json`, `ops/track.json` (`track.add`, kept so a 1.5 launcher still installs it), the track's files (`track/<code>.erp`, `mipmaps/`, `ui/`) and `registry/track.json`, a data-only track record.
2. **Package fixes happen once, at import:** stand-in worlds, mip paths, passing space on the AI line.
3. **The launcher places the files** at their game paths (`2025_asset_groups/environment_package/tracks/<code>/...`). With a track record present, a 2.0 launcher's `track.add` places files only and leaves the database to the core.
4. **At boot, before the database loads,** the core clones the base track's rows in every data set as a variant, registers the asset groups, adds the menu entries and names, pins the stat id and provides the venue. Extra grid slots, garages and pit boxes for a bigger grid are served in memory when that grid is active.
5. **Calendars** that ask for the venue resolve live. Per-season track data applies at session load.
6. **Already-imported tracks:** on upgrade the importer writes `registry/track.json` from the existing `track/manifest.json`, with no re-import. A 1.5 launcher still does the full activation from `track.add`.

<template #example>
<Example root="mod.sochi" code-file="native/src/track.cpp" data-file="registry/track.json" files="swofty.json, ops/track.json, track/sochi.erp, track/manifest.json, mipmaps/sochi_d.tif.mipmaps, README.md">
<template #code>

```cpp
// the same track, made as a normal addon instead of imported
#include <swofty/registry.hpp>
#include <swofty/systems/tracks.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    tracks::tracks.add("sochi", {
        .code = "sochi",
        .name = "Sochi Autodrom",
        .base = tracks::tracks["game/baku"],
        .slot = tracks::tracks["game/sochi"],                   // the stock row without assets
        .provides = { tracks::venues["venue/sochi"] },
        .files = file("track/"),
    });

    for (const char *set : { "f1_2025", "sprint_f1_2025", "f1_2026", "sprint_f1_2026" })
        tracks::data.layer(tracks::data[std::string("mod.sochi/sochi/") + set], [](tracks::TrackData &d) {
            d.tyres = { .softer = "C5", .medium = "C4", .harder = "C3" };
        });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/track.json, written by the importer
{
  "track": {
    "sochi": { "code": "sochi", "name": "Sochi Autodrom", "extends": "game/baku", "slot": "game/sochi",
               "provides": ["venue/sochi"] }
  },
  "track.data": {
    "mod.sochi/sochi/f1_2025":        { "tyres": { "softer": "C5", "medium": "C4", "harder": "C3" } },
    "mod.sochi/sochi/sprint_f1_2025": { "tyres": { "softer": "C5", "medium": "C4", "harder": "C3" } },
    "mod.sochi/sochi/f1_2026":        { "tyres": { "softer": "C5", "medium": "C4", "harder": "C3" } },
    "mod.sochi/sochi/sprint_f1_2026": { "tyres": { "softer": "C5", "medium": "C4", "harder": "C3" } }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[import]   mod.sochi: registry/track.json written from track/manifest.json (base game/baku, slot game/sochi, 4 data overrides)
[registry] track mod.sochi/sochi: fills game/sochi, base game/baku, provides venue/sochi
[registry] applied at boot: track 1 (19 data sets cloned from game/baku; 77 asset groups; 1,688 database records, 474 changed)
```

Two addons claiming one slot is a provider clash. The second is refused at boot, with both named:

```text
[registry] track mod.igora_d/igora: slot game/miami24 is already filled by mod.miami24/igora_drive; refused
```

## Worlds, slots and AI lines

A bigger grid needs more grid slots, garages and pit boxes than a world has. Custom Grid Size provides them per world; the core serves them when a grid that size is active:

```cpp
tracks::slots.add("monza_myteam_30", {
    .layer = tracks::layers["game/monza/myteam"],
    .grid = grid_cs::slots("monza", 30), .garages = grid_cs::garages("monza", 30), .pit_boxes = grid_cs::boxes("monza", 30),
});
```

```cpp
// the classic world for Hockenheim: a layer row per mode
tracks::layers.add("hockenheim_swofty_f1", {
    .track = tracks::tracks["mod.hockenheim/hockenheim"], .mode = "swofty_f1",
    .world = { "environment_package/tracks/hockenheim", "hockenheim_f1_26" }, .grid = 22, .garages = 11,
});
```

## Track data at run time

```cpp
// track.data.get: Progressive copies 2025 fuel into the 2026 track objects for a season
events::on<tracks::events::DataGet>().in(events::owner("swofty.progressive"))
    .after([](tracks::events::DataGet &g, const events::Context &ctx) {
        if (g.data_set == "f1_2026" && ctx.career()->year <= 2025)
            g.data.fuel = progressive::fuel_2025(g.track);
    });

tracks::set_data(tracks::tracks["game/monaco"], [](tracks::TrackData &d) { d.laps = 78; });   // put back on unload
```

```text
[ops] tracks::set_data game/monaco: laps 50 -> 78 (career 3f2a); put back on career.unloaded
```

## Readiness and limits

- **Records: ready.** About a dozen community tracks ship today.
- **Per-box data: ready.** `Game_TrackData_GetTrack` is L2; `CareerEntity+0xf58` (the TrackDataSystem) is L1 with an independent agree; the object layout (0x290 bytes, fuel at +0xf0/+0x100/+0x1a0/+0x1b4) is proven by Progressive 1.2.2.
- **The per-box track list: partial.** `content_sets` `available_tracks` and `sorted_track_order` are known in data; the runtime list builder behind `ui.track_select.build` isn't in the KB yet (an RE item).
- **Stock slots without assets:** `paul_ricard`, `portimao`, `sochi`, `hanoi`, `texas_nat`, `suzuka_east`, `bahrain_pad`, `silverstone_int`; `bahrain24` is empty.
- **Stack-built keys:** a new track must carry `CareerHubFirstVisit_N`, `CareerHubLastVisit_N`, `TrackHighlights_N` and `TrackVideo_N` (`tools/track_keys.py`).
- **Track data is found by mode mask,** and sub-tables inherit from the parent set one table at a time.
- **AI packs** are per car folder (`ai_package_folder/asset_filename`).
- **Worlds:** `_f1_26` worlds have 22 grid slots and 11 garages, `_myteam` 24 and 12, `_f1` 10 garages (a Classic Seasons crash in `Sess_Assign_GarageSlotsByPreference`, fixed by a layer copy).
- **Unknown:** the invitational slot limit (8 tracks without the loader), Time Trial on added tracks (crashes or freezes), and the 28-car session-enter crash on community maps.
- **Size:** a track adds about 1,650 to 1,884 records and changes about 470 to 490. Autopolis alone sets 400.
