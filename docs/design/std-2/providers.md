---
title: Providers
description: Static and dynamic providers fill a registry's base in standard library 2.0.
---

# Providers

A **provider** fills the base of a registry's records. A record has exactly one base. Every other addon changes it through a [layer](./layers).

There are two kinds, and an addon can use both for one registry.

## Static providers

A static provider is JSON in the addon, named in the manifest. The launcher reads it too, so launcher pages can show the base without the game running.

```json
// swofty.json
{
  "provides": [
    { "registry": "career.season", "file": "registry/seasons.json" },
    { "registry": "career.driver_ratings", "file": "registry/ratings.json" }
  ]
}
```

The file holds one or more records, by id:

```json
// registry/seasons.json
{
  "f1_career_progressive/2016": {
    "data_group": "f1_career_progressive",
    "year": 2016,
    "teams": [
      { "slot": 1, "team": "mercedes", "seats": ["lewis_hamilton", "nico_rosberg"] },
      { "slot": 3, "team": "redbull",  "seats": ["daniel_ricciardo", "daniil_kvyat"] },
      { "slot": 8, "team": "toro_rosso", "seats": ["carlos_sainz-jr", "max_verstappen"] }
    ],
    "changes": [
      { "round": 5, "slot": 3, "seat": 1, "out": "daniil_kvyat", "in": "max_verstappen", "kind": "swap" },
      { "round": 5, "slot": 8, "seat": 1, "out": "max_verstappen", "in": "daniil_kvyat", "kind": "swap" }
    ]
  },
  "f1_career_progressive/2017": { "...": "..." }
}
```

Progressive Career builds these files from its own `data/years.json` when it packages, with `tools/registry.py`. `years.json` stays Progressive's private format. The registry files are the public one.

## Dynamic providers

A dynamic provider is a callback. The core calls it the first time a record is read, and caches the result until the provider calls `invalidate`.

```cpp
#include <swofty/registry.hpp>
#include <swofty/career/ratings.hpp>

using swofty::career::DriverRatings;

// id: "<data group>/<year>/<driver>"
static std::optional<swofty::json::Value> ratings(std::string_view id)
{
    auto parts = swofty::registry::split(id);   // {"f1_career_progressive", "2017", "max_verstappen"}
    if (parts.size() != 3 || parts[0] != progressive::kDataGroup)
        return std::nullopt;                    // not ours: the core tries the next provider
    int year = std::stoi(std::string(parts[1]));
    std::optional<DriverRatings> r = progressive::ratings_model().season_start(std::string(parts[2]), year);
    if (!r)
        return std::nullopt;
    return r->to_json();
}

SWOFTY_ON_LOAD
{
    swofty::registry::provide(DriverRatings::registry, ratings);
}
```

When the data behind a dynamic record changes, the provider says so:

```cpp
void on_ratings_model_reloaded()
{
    swofty::registry::invalidate(DriverRatings::registry);                 // the whole registry
    swofty::registry::invalidate(DriverRatings::registry, "f1_career_progressive/2017/max_verstappen");
}
```

The cached value is dropped, the next read calls the provider again, and [watchers](./reading#watching) are told.

## Static and dynamic together

A provider can ship a static file for the launcher and compute the rest in game. When both answer the same id, the dynamic value wins in game and the static file is what the launcher sees:

```json
{
  "provides": [
    { "registry": "career.driver_ratings", "file": "registry/ratings.json", "dynamic": true }
  ]
}
```

Progressive does this for ratings. The static file holds every driver's start-of-season ratings for 2016 to 2025. In game, the dynamic provider adds drivers the file doesn't list, using the ratings model.

## Two providers for one record

Only one provider should answer an id. When two do, the later one in load order wins, and the core logs the clash:

```text
[registry] career.season f1_career_progressive/2017: provided by swofty.progressive and by someone.fork;
           someone.fork wins (later in load order). This is a conflict; the launcher lists it.
```

To change another addon's records, use a [layer](./layers), not a second provider.

## Threads and timing

- `provide` is accepted from `SwoftyAddonInit` on. A call made before the core's setup is queued and applied in load order.
- A dynamic provider runs on the thread that reads the record, under the registry's lock. It must not read the same registry (the core returns `late` to a re-entrant read) and must return quickly.
