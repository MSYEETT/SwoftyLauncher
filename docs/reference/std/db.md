---
title: Database read
description: Read the loaded F1 25 game database from a plugin with swofty/db.hpp - scalar fields, whole values and table cursors.
---

# Database read

Once the game database has loaded, you can read a whole typed value or pick out a field whose type you know. To work with rows, look one up by its label or use a cursor to walk a table. You use the same dotted paths as the [database operations](/reference/ops/database), hashed with fnv1a64, to identify what to read.

| | |
|---|---|
| Header | `swofty/db.hpp` |
| Namespace | `swofty::db` |
| Table | `SwoftyApi::db` (`SwoftyDb`) |
| Feature | `db` — stable (shared with the database write operations) |
| API version | 5 |
| Thread | Caller thread |

Check the `swofty::db::Result` returned by every call: `ok`, `late`, `unavailable` or `invalid`. The [result codes](./#result-codes) explain each one. If you need to keep a returned string, copy it: its pointer stays valid only until the next `db` call on the same thread.

## Calls

| Call | Returns |
|---|---|
| `available()` | `true` once the database has loaded |
| `get(key, out)` / `get(hash, out)` | whole `SwoftyDbValue` by dotted key or pre-hashed key |
| `int_at(key, out)` | one `int64_t` field |
| `float_at(key, out)` | one `double` field |
| `string_at(key, out)` | one `const char *` field |
| `bool_at(key, out)` | one `int32_t` field |
| `open(table, cursor)` | positions a cursor at the first row |
| `row(table, label, cursor)` | positions a cursor at a labelled row |
| `step(cursor)` | advances; `late` when the table is exhausted |
| `cell(cursor, field, out)` | one field of the cursor's current row |
| `row_name(cursor)` | the current row's label, or `nullptr` |

When you read a whole `SwoftyDbValue`, its `tag` tells you which of `i`, `f` or `s` holds the value. The tag is a `swofty::db::Tag`: `integer`, `real`, `string`, `enumeration`, `id` or `boolean`. You own the `SwoftyDbCursor`, so zero-initialise it before calling `open` or `row`.

## Example

Here's how you read one field, then walk a table to list every row:

```cpp [native/src/my_plugin.cpp]
#include <swofty/db.hpp>

namespace db = swofty::db;

SWOFTY_ON_LOAD
{
    if (!db::available())
        return;

    int64_t secs = 0;
    if (db::int_at("rules.stop_go_penalty.penalty_duration", &secs) == db::Result::ok)
        swofty::log("[my-addon] stop-go penalty is %lld s", (long long)secs);

    db::Cursor cur{};
    for (db::Result r = db::open("tracks", &cur); r == db::Result::ok; r = db::step(&cur))
        swofty::log("[my-addon] track %s", db::row_name(&cur));
}

SWOFTY_ADDON()
```

The loop prints the 43 rows of `tracks`. Declare the capability in your plugin's manifest entry:

```json [swofty.json]
"operations": {
  "native": [{ "dll": "native/my_plugin.dll", "capabilities": ["read-db"] }]
}
```

Look up dotted keys in the [game database reference](/reference/gamedb). For a row count, use `<container>._<Row>_instancecount_`.

## Availability

The db-read engine lane hasn't landed yet, so every call on current builds returns `unavailable`. Guard every call with `available()`. See [the native-libraries design](/design/native-libraries#_4-database-read).
