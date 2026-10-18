---
title: Database read
description: Read the loaded game database from a native plugin with swofty/db.hpp or the C ABI.
---

# Database read

`swofty/db.hpp` reads the game's value database once it has loaded. Keys are the dotted path hashed
with fnv1a64, the same keys the [database operations](/reference/ops/database) write. Get a whole
typed value, one field of a known type, a row by its label, or iterate a table.

The feature is `db` (stable), shared with the database write operations. It needs loader version 5.
Each call returns a `swofty::db::Result`: `ok`, `late`, `unavailable` (older loader, feature off, or
the database has not loaded) or `invalid` (bad argument or a key that is not in the database).

## Availability

```cpp
#include <swofty/db.hpp>
namespace db = swofty::db;

if (db::available()) { /* the database has loaded */ }
```

`available()` is non-zero once the database has loaded. Before that every call returns
`unavailable`.

## Typed fields

```cpp
int64_t laps = 0;
if (db::int_at("entrants.entrant_lists.entrant_list_34._entrant_instancecount_", &laps) == db::Result::ok)
    swofty::log("grid size %lld", (long long)laps);

double x = 0;
db::float_at("ai.general_ai_settings.max_test_time", &x);

int32_t flag = 0;
db::bool_at("some.bool.key", &flag);

const char *name = nullptr;
db::string_at("some.string.key", &name);
```

`int_at`, `float_at`, `bool_at` and `string_at` read the record as the requested type. The caller
states the type; the record is read at its scalar offset without a class check, matching the game's
own field readers. A missing key is `invalid`.

## Whole value

```cpp
db::Value v;
if (db::get("ai.general_ai_settings.max_test_time", &v) == db::Result::ok) {
    // v.tag is SWOFTY_DB_INT, _FLOAT, _BOOL or _STRING
    // v.i integer, v.f float, v.s string text (or null), fnv32 label in v.i for a string
}
db::get(0x2273a5bae867b23bull, &v); // a pre-hashed key
```

`get` fills a `SwoftyDbValue`. The record's class fixes the tag. Integer, float and bool records set
`i` and `f`; a string-family record (`string`, `enum`, `id`) reports `SWOFTY_DB_STRING`, its fnv32
label in `i` and its inline text in `s` when present. Typing records needs the analysed build or the
database's record classes; on an unknown build without them `get` returns `unavailable` while the
typed field readers still work.

## Tables

A table is a container path (`entrants.teams`); its rows are `<table>.<stem>_<n>` and its count is
`<table>._<stem>_instancecount_`, where `<stem>` is the last path segment without a trailing `s`.

```cpp
db::Cursor c;
for (auto r = db::open("entrants.teams", &c); r == db::Result::ok; r = db::step(&c)) {
    const char *name = db::row_name(&c);
    db::Value stat;
    db::cell(&c, "stat_id", &stat);
}

db::Cursor row;
if (db::row("entrants.teams", "mercedes", &row) == db::Result::ok)
    db::cell(&row, "stat_id", &stat);
```

`open` positions a cursor at the first row and fills `count`; an empty table is `late`. `step`
advances and returns `late` at the end. `row` finds a row whose fnv32 metadata label matches. `cell`
reads one field of the current row. `row_name` returns the current row's label when the game exposes
it inline, otherwise null. A `Cursor` is caller-owned; string pointers from these calls are valid
until the next `db` call on the same thread.

## Manifest

Reading the database needs `read-db` in the manifest's `capabilities` (version 5).
