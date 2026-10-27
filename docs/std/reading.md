---
title: Reading
description: Effective values, origins, listing and watching registry records in standard library 2.0.
---

# Reading

Every read returns the **effective value**: the base from the [provider](/std/providers) with every [layer](/std/layers) applied in load order. Inside a career, it returns that career's [snapshot](/std/scopes#career-snapshots).

## Effective values

```cpp
#include <swofty/registry.hpp>
#include <swofty/career/season.hpp>

using swofty::career::Season;

void log_grid(int year)
{
    std::string id = "f1_career_progressive/" + std::to_string(year);

    // typed, for the core's registries
    if (std::optional<Season> s = swofty::registry::get<Season>(id)) {
        for (const auto &t : s->teams)
            swofty::log("[grid] %d slot %d %s: %s, %s", year, t.slot, t.team.value_or("-").c_str(),
                        t.seats[0].c_str(), t.seats[1].c_str());
    }

    // untyped, for any registry
    if (std::optional<swofty::json::Value> v = swofty::registry::get("career.season", id))
        swofty::log("[grid] %s", v->dump().c_str());
}
```

`get` returns `std::nullopt` when the record doesn't exist, when no provider answers, or when the registry isn't declared. `swofty::registry::status(registry, id)` tells you which of these it was:

```cpp
switch (swofty::registry::status("career.season", id)) {
case swofty::registry::Status::ok:           break;
case swofty::registry::Status::no_record:    break; // no provider answered this id
case swofty::registry::Status::no_registry:  break; // nothing declared career.season (core too old?)
case swofty::registry::Status::invalid:      break; // every candidate value failed validation
}
```

## Origins

`origin` says which addon set a field of the effective value. It takes a [JSON Pointer](https://www.rfc-editor.org/rfc/rfc6901) into the value. For an `x-key` array, the pointer can name the element by its key instead of its position:

```cpp
std::string who = swofty::registry::origin("career.season", "f1_career_progressive/2017", "/teams/slot=1/seats/0");
// "tails.whatif-history": a layer from that addon set Mercedes' first seat
std::string base = swofty::registry::origin("career.season", "f1_career_progressive/2017", "/teams/slot=2/seats/0");
// "swofty.progressive": the provider's base value
```

Origins are what the launcher's conflicts view and Send Logs' `registry/effective.json` show.

## Listing

```cpp
// every id in a registry
for (std::string_view id : swofty::registry::ids("career.season"))
    swofty::log("%.*s", int(id.size()), id.data());

// the ids under a prefix: every Progressive season
for (std::string_view id : swofty::registry::ids("career.season", "f1_career_progressive/"))
    ...;
```

Listing includes ids from static providers and layers. A dynamic provider can also list the ids it answers, by passing a `list` callback:

```cpp
swofty::registry::provide(DriverRatings::registry, ratings, /* list */ [](auto add) {
    for (int year = 2016; year <= 2025; ++year)
        for (const std::string &d : progressive::history().drivers(year))
            add(std::string(progressive::kDataGroup) + "/" + std::to_string(year) + "/" + d);
});
```

## Watching

A watcher runs when a record's effective value changes, because a provider invalidated it, a layer changed, or a career loaded and its snapshot took over:

```cpp
static swofty::registry::Token g_watch;

SWOFTY_ON_LOAD
{
    swofty::registry::watch("career.season", [](std::string_view id) {
        if (id.starts_with("f1_career_progressive/"))
            progressive::cards::refresh();   // Choose Driver rebuilds its cards from the new grid
    }, g_watch);
}
```

- Watchers run on the flow thread, after the change is visible to `get`.
- One change can touch many records. The core calls a watcher once per changed id, in id order, then once with an empty id to say the batch is done.
- `swofty::registry::unwatch(g_watch)` stops it.

## Threads

- `get`, `status`, `origin` and `ids` work from any thread.
- Values are copied out. Nothing a read returns points into the registry.
- A read on the flow thread during a career load waits until the snapshot is in place, so it never sees a half-loaded career.
