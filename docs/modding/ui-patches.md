# UI patches

UI patches describe changes to menus, labels and routes. Save them as `*.pkgpatch.json` and list them in `operations.ui` in your [manifest](/modding/manifest).

The current builder skips UI patches. This is the format for the planned build step.

## Example

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
      "value": "Launched with Swofty Launcher, press", "localise": false }
  ]
}
```

`targets` lists the resources to change. The home screen has two copies, so both are listed. Node paths are relative to `root`. Edits run in order.

## Screen edits

| Edit | Purpose |
|---|---|
| `clone-node` | Copy `src` to a sibling named `as`; optionally change properties with `set` |
| `set-text` | Set a label's `node`, `value` and `localise` flag |
| `set-props` | Change properties on `node` using `set` |
| `add-signal` | Add a `signal` to a node's `event` |
| `remove-signal` | Remove signals from a node's `event` to a target `to` |

Signals name a target node (`to`) and pin (`pin`). Optional fields are `from` and `attrs`.

## Routes and UI databases

For `.uir` and `.uidb` files, use `clone-entry` or `set-value`. Paths use dots for keys and `[field=value]` to select an array entry.

## Text and prompts

Use `set-text` with `"localise": false` for literal text. Game language files are not editable yet.

Keyboard and controller icons come from entries in `tool_tips.uidb`. See [`!pkg` resources](/reference/pkg) for the file format.
