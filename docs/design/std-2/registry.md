---
title: The registry
description: Registries, records, schemas and typed records in standard library 2.0.
---

# The registry

A **registry** is a named collection of **records** that every addon in the profile shares. Every record has an id and a JSON value, and every value has to match the registry's **schema**.

```text
career.season                                  <- registry
  f1_career_progressive/2016                   <- record id
  f1_career_progressive/2017  { "data_group": "f1_career_progressive", "year": 2017, "teams": [...] }
  f1_career_progressive/2018
career.driver_ratings
  f1_career_progressive/2017/max_verstappen    { "overall": 88, "experience": 74, ... }
```

## Declaring a registry

The addon that **owns** a registry declares it: its name, its schema and its current version. The core owns the registries that describe the game (`career.*`, `team.*`, `lang.*`). An addon owns the registries of its own data, named under its id.

```json
// swofty.json of the addon that owns progressive.settings_presets
{
  "registries": [
    { "name": "progressive.settings_presets", "schema": "schemas/settings_presets.1.0.json" }
  ]
}
```

```json
// schemas/settings_presets.1.0.json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "x-registry": "progressive.settings_presets",
  "x-version": "1.0",
  "x-id": "{name}",
  "type": "object",
  "required": ["name", "start_year", "rnd", "team_moves"],
  "properties": {
    "name":       { "type": "string", "pattern": "^[a-z0-9_]+$" },
    "start_year": { "type": "integer", "minimum": 2016, "maximum": 2026 },
    "rnd":        { "enum": ["real_life", "hybrid"] },
    "team_moves": { "enum": ["follow_driver", "choose_team"] }
  }
}
```

A registry can also be declared from code. That's for a registry whose schema the addon builds at runtime, which is rare:

```cpp
swofty::registry::declare("progressive.settings_presets", "schemas/settings_presets.1.0.json");
```

## Records

A record id is a string, unique within its registry. The schema's `x-id` says how it's built from the value's fields, so the core can check that a record's id matches its value:

```json
"x-id": "{data_group}/{year}"            // career.season
"x-id": "{data_group}/{year}/{driver}"   // career.driver_ratings
```

Use `/` to build ids out of parts. Then a prefix lists a group: `f1_career_progressive/` lists every Progressive season (see [Listing](./reading#listing)).

## Schemas

A schema is a [JSON Schema](https://json-schema.org/draft/2020-12) using the common keywords: `type`, `properties`, `required`, `enum`, `minimum`, `maximum`, `pattern`, `items`, `minItems`, `maxItems`, `$ref` and `$defs`. The `x-` keywords add what a registry needs:

```json
{
  "x-registry": "career.season",          // the registry this schema belongs to
  "x-version": "1.0",                     // major.minor, see Versioning
  "x-id": "{data_group}/{year}",          // how a record id is built from its value
  "x-snapshot": "career",                 // freeze effective values into each career
  "x-snapshot-budget": 65536,             // bytes per career, compressed
  "properties": {
    "teams": {
      "type": "array",
      "x-key": "slot",                    // layers match elements by slot, not by position
      "items": { "$ref": "#/$defs/team" }
    }
  }
}
```

- **`x-key`** turns an array into a keyed list. A [layer](./layers) can then change one element by naming its key, without restating the whole array. A key can be several fields: `"x-key": ["round", "slot", "seat"]`.
- **`x-ref`** marks a string that must be the id of a record in another registry, or a game database label: `"x-ref": "db:drivers"`, `"x-ref": "career.team_identity"`. Validation checks it exists.

## Typed records

The C ABI passes every value as JSON text. In C++ you work with one of two things:

**`swofty::json::Value`** for any registry:

```cpp
#include <swofty/json.hpp>

std::optional<swofty::json::Value> v = swofty::registry::get("progressive.settings_presets", "my_2016_start");
if (v) {
    int year = (*v)["start_year"].as_int();             // 2016
    std::string moves = (*v)["team_moves"].as_string(); // "follow_driver"
}

swofty::json::Value patch = swofty::json::object({ { "start_year", 2018 } });
swofty::json::Value parsed = swofty::json::parse(R"({"rnd":"hybrid"})");
std::string text = patch.dump();                         // {"start_year":2018}
```

**Typed structs** for the core's registries, hand-written next to their schemas in `swofty/career/*.hpp`:

```cpp
#include <swofty/career/season.hpp>

namespace swofty::career {

struct SeasonTeam {
    int slot = 0;                            // 1-11, the career team that carries this season's team
    std::optional<std::string> team;         // the season's team key, e.g. "toro_rosso"; empty slot when absent
    std::array<std::string, 2> seats;        // seat 0, seat 1
};

enum class ChangeKind { swap, stand_in };

struct SeasonChange {
    int round = 0;
    int slot = 0;
    int seat = 0;
    std::string out, in;
    ChangeKind kind = ChangeKind::swap;
    std::optional<int> until_round;          // stand_in only
};

struct Season {
    static constexpr const char *registry = "career.season";
    static constexpr Version version{1, 0};

    std::string data_group;
    int year = 0;
    std::vector<SeasonTeam> teams;
    std::vector<SeasonChange> changes;
    json::Value extra;                       // fields from a newer minor version, kept for the round trip

    static std::optional<Season> from_json(const json::Value &v);
    json::Value to_json() const;
    std::string id() const;                  // "<data_group>/<year>"
};

} // namespace swofty::career
```

`swofty::registry::get<Season>(id)` reads the effective value and converts it. A field this header doesn't know lands in `extra`, so code built against 1.0 can read a 1.1 record and write it back without losing anything (see [the tolerant reader rule](./versioning#the-tolerant-reader-rule)).

## Rules

- **Registry names** are dotted and lowercase. The core uses `career.`, `team.`, `driver.`, `session.`, `lang.`, `era.`, `hub.` and `addon.`. An addon's registry starts with its own id.
- **Values** are JSON objects of at most 1 MB.
- **A record has one base** from one [provider](./providers). Every change on top of it is a [layer](./layers).
- **Every write is validated** against the schema before it becomes visible.
