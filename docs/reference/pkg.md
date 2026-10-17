# `!pkg` resources

Every front-end UI resource is `!pkg`: screens (`UIDF`, `.ui`), UI databases (`UIDB`, `.uidb`), routes (`UIRO`, `.uir`), presets, messages, config, content groups, string tables and animation clips. World layers use `!pkg` variants too.

`tools/pkg.py` reads and writes the format and round-trips all 435 unique `.ui` files byte for byte. `tools/uinode.py` holds the node model and id rules. The Rust port in `packages/formats` is in progress.

## Layout

```
"!pkg"
u8[4]  name                 zero for UI; "ANST" for string tables, "ANIM" for clips
u32    root offset          0x80000000 | offset
u32    !vca offset          0x80000000 | offset
chunks...
"!vca"  value pools
```

Chunks:

| Chunk | Contents |
|---|---|
| `!idi` | An object. Header `count | version << 24`. Version 1 entries are 12 bytes: key ref, value ref, `fnv1a32(key)`. Other versions are 8 bytes |
| `!ili` | An array of value refs |
| `!iar` | 8 raw bytes |
| `!vca` | The table of typed value pools |
| `!sbi` | The string pool: each string is a length byte, the bytes and a NUL |
| `!vbi` | A typed pool: FourCC type, count, bytes per datum, data |

A value ref is `pool << 24 | offset`. Pool `0x80` means a chunk at that offset. Pool types include `stri`, `bool`, `ui08`, `ui16`, `ui32`, `si32`, `ui64`, `si64`, `fp32`, `rgba`, `vec3`, `vec4`, `quat`, `woid`, `shnm`, `bbox` and `mat4`.

## Rules the writer follows

- **Breadth first.** Each level's tables are written before the next level's. A depth-first writer produces different bytes.
- **Hash-sorted keys.** Every version 1 `!idi` object's keys are sorted by `fnv1a32(key)` (basis `0x811c9dc5`, prime `0x01000193`). This held for all 33,677 objects with more than two keys. New keys are inserted in hash order.
- **Pooled values.** Values are interned into their pools, reusing an existing entry.
- **Alignment.** Each pool is aligned to its type (4, 8, 16, 32 or 64 bytes), and the file is padded to 4 bytes.

## The UI node model

A `.ui` screen is a node graph. Each node is an 8-slot `!ili`:

| Slot | Contents |
|---|---|
| 0 | Properties: `name`, `type` (a FourCC such as `UISC` screen, `UILB` label, `UIKL` key listener, `UITL` tooltip, `UIAC` action, `TILE`), and `{value, dbentry?}` properties that may bind to a view model or UIDB path |
| 1 | Children |
| 2 | Commands: native code sends these into the screen |
| 3 | Events: each maps to signals `{to_object, to_object_name, to_pin, to_pin_name, from_pin_name, attributes}` |
| 4 | Layout attach path |
| 5 | The component to instantiate |
| 6 | Native behaviour modules |
| 7 | Unused |

Ids are 32-bit djb2 hashes (`h = 5381; h = h * 33 + c`):

| Field | Hash of |
|---|---|
| `id` | The node's dotted path, `fe_home.tooltips.settings` |
| `nameId` | The node's name |
| `to_object`, `to_pin` | The target node path and pin name |
| `event`, `command` | The event or command name |
| layout `nodeId` | The layout node's name |

Recomputing every id reproduces all 435 screens byte for byte.

## UI databases and routes

A `.uidb` is a dictionary of typed variables, `{Name, Contents, DBType, ValueType, ReadOnly, Value}`. Screens bind to it with `#<db>.<path>`. Input prompts come from `tool_tips.uidb` entries, which name an input action and a text.

A `.uir` holds route groupings. A route is a graph of commands (`OpenScreen`, `CloseScreen`, `Migrate`, `Wait`, ...), and a `UIAC` node's `route` pin fires a route of the screen's grouping by its key.
