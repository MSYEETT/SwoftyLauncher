# UI patches

Every front-end UI resource in F1 25 is a `!pkg` resource: screens (`.ui`), routes (`.uir`), UI databases (`.uidb`) and more. A UI patch is a declarative transform over a resource's node graph, not a byte diff, so it survives the resource moving inside its ERP. The format is described in [`!pkg` resources](/reference/pkg).

## File format

A UI patch is a `*.pkgpatch.json` file listed in `operations.ui`:

```json
{
  "schema_version": 1,
  "targets": [
    { "erp": "2025_asset_groups/ui_package/flow_frontend.erp",
      "resource": "eaid://ui_package/screens/fe_home.ui", "fragment": 0 },
    { "erp": "2025_asset_groups/ui_package/screens/fe_home.erp",
      "resource": "eaid://ui_package/screens/fe_home.ui", "fragment": 0 }
  ],
  "root": "Screens[0]:fe_home",
  "edits": [
    { "op": "set-text", "node": "contextual_panels.offline_contextual_panel.subtitle",
      "value": "Launched with Swofty Mod Launcher, press", "localise": false }
  ]
}
```

| Field | Meaning |
|---|---|
| `targets` | Every copy of the resource to patch: game-relative `erp`, `resource` name, `fragment` (0 if absent). `fe_home.ui` ships twice, in `flow_frontend.erp` and in `screens/fe_home.erp`, and both copies need the patch |
| `root` | For `.ui` screens, the screen node: `Screens[<index>]:<name>`. Node paths in edits are relative to it |
| `edits` | Applied in order |

## Edits for screens (`.ui`)

Node paths are dotted names below the root, such as `tooltips.settings`.

| Op | Fields | Effect |
|---|---|---|
| `clone-node` | `src`, `as`, `set?`, `replace_event?` | Deep-copies node `src` as a sibling named `as`, rewrites references to its old path, sets properties from `set`, and replaces one event's signals with `replace_event {event, signals}` |
| `set-text` | `node`, `value`, `localise` | Sets a label's text. With `localise: false` the value is literal and its loc-tag binding is dropped |
| `set-props` | `node`, `set` | Sets property values, `{name: value}` |
| `add-signal` | `node`, `event`, `signal {to, pin, from?, attrs?}` | Adds a signal to an event |
| `remove-signal` | `node`, `event`, `to` | Removes the signals of an event that target node `to` |

A signal names its target node by full path (`fe_home.Action`), its pin (`route`), the pin it fires from (defaults to the event name) and attributes (`{"key": "mod_menu"}`).

## Edits for dictionary resources (`.uir`, `.uidb`)

Routes and UI databases are dictionaries and arrays rather than node trees. Paths use `.` for keys and `[field=value]` to pick an array element.

| Op | Fields | Effect |
|---|---|---|
| `clone-entry` | `at`, `src`, `as`, `set?` | In the container at `at`, copies the entry `src` and adds it as `as`, then sets fields from `set` |
| `set-value` | `path`, `value` | Sets one value |

The `mod_menu` route is a clone of `options` in the `fe_home` grouping of `routes_frontend_general.uir`:

```json
{
  "schema_version": 1,
  "targets": [
    { "erp": "2025_asset_groups/ui_package/persistent.erp",
      "resource": "eaid://ui_package/routes/routes_frontend_general.uir" }
  ],
  "edits": [
    { "op": "clone-entry", "at": "RouteGroupings[Id=fe_home].Routes",
      "src": "[Id=options]", "as": "mod_menu", "set": { "Id": "mod_menu" } }
  ]
}
```

## What the engine recomputes

The patch engine keeps the resource valid the same way the Python tools do:

- **Node ids.** Every id is a 32-bit djb2 hash (`h = 5381; h = h * 33 + c`): `id = djb2(dotted path)`, `nameId = djb2(name)`, and a signal's `to_object` and `to_pin`, an event's `event` and a command's `command` are the djb2 of their names. They are recomputed after every edit.
- **Key order.** Every `!idi` object keeps its keys sorted by `fnv1a32(key)`, which the game relies on. New keys are inserted in hash order.
- **Layout.** Chunks are written breadth first, and value pools are deduplicated, as the game's own files are.
- **The ERP.** The fragment is recompressed and the resource MD5 recomputed.

An empty edit list reproduces the original resource byte for byte.

## Literal text

The game's language files are not editable yet. A label shows literal text when its `localise` property is false and its text has no loc-tag binding; `set-text` with `"localise": false` does both. Glyphs for input prompts (F9, R3) come only from tooltip entries in `tool_tips.uidb`, so a prompt with a glyph needs a tooltip entry.
