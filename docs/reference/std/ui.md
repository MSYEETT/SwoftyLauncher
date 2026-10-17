---
title: UI
description: Addon screens, ADDON SETTINGS pages and their view models, home tiles and UI patches.
---

# UI

SwoftyMod adds screens to F1 25 through the engine's own screen loader. The [tutorial](/tutorials/in-game-ui) adds a screen and a home tile.

## Screens

`operations.screens` in `swofty.json`:

```json
"screens": [{ "name": "my_page", "file": "ui/my_page.ui", "flow": "flow_options", "anchor": "settings_container", "view_model": false }]
```

| Field | Default | Meaning |
|---|---|---|
| `name` | | `[a-z0-9_]`, unique across the profile's addons. The screen's route |
| `file` | | The screen's raw `.ui` data, relative to the addon folder |
| `flow` | `flow_frontend` | The flow that loads the screen |
| `anchor` | | A game screen in the same flow. The screen registers after that screen's resource loads |
| `view_model` | `false` | Ask SwoftyMod for a native view model |

- The build installs each file as `swofty/ui/<name>.ui` and lists it in the `ui` block of `swofty/settings.json`.
- SwoftyMod registers each listed screen when its flow loads, and logs every node the engine rejects with its reason.
- A route to an addon screen that is not registered is cancelled and the last screen reopens.
- `swofty_settings`, `swofty_addon_<n>` and `swofty_map_<n>` are reserved for ADDON SETTINGS.

## ADDON SETTINGS

The launcher generates ADDON SETTINGS from the game's own screens each time it builds the profile:

| Screen | Contents |
|---|---|
| `swofty_settings` | One tile per enabled addon and map: name, author, version and description |
| `swofty_addon_<n>` | The page for `addons[n]` of `swofty/settings.json`: one row per option |
| `swofty_map_<n>` | A map's track code, author and version |

The home tile that opens it routes to `mod_menu`.

## View models

A screen with `"view_model": true` gets a view model when SwoftyMod has one whose prefix matches the screen's name. SwoftyMod registers it under `view_model_data.<screen>`. SwoftyMod has one view model, for `swofty_addon_` pages.

| Property | Value |
|---|---|
| `title` | The addon's name |
| `o<k>.label` | Option `k`'s label |
| `o<k>.text` | Its current value, as shown |
| `o<k>.index`, `o<k>.value` | Its current value's index |
| `o<k>.min`, `o<k>.max`, `o<k>.step` | The index range: 0, the last index and 1 |
| `o<k>.is_slider` | `true` for a slider |

| Command | Effect |
|---|---|
| `swofty_next:<addon>:<option>` | Next value |
| `swofty_prev:<addon>:<option>` | Previous value |
| `swofty_set:<addon>:<option>:<index>` | Value number `index` |
| `swofty_enable:<addon>:<0 or 1>` | Turn the addon off or on |
| `swofty_restart` | Close the game so the launcher rebuilds and relaunches it |

## Home tiles

These [Game API](/reference/game/) bindings build the home screen's tiles:

| Binding | Use |
|---|---|
| [`FeTile_Build_List`](/reference/game/fe_tiles#FeTile_Build_List) | Builds a tile list |
| [`site_FeHome_AfterCareerCall`](/reference/game/fe_home#site_FeHome_AfterCareerCall) | The instruction after the Career tile is added. `r14` is the `TileList *` |
| [`FeHome_Add_CareerTile`](/reference/game/fe_home#FeHome_Add_CareerTile) | Appends a Career tile to a list |
| [`FeTile_ItemBase_Populate`](/reference/game/fe_tiles#FeTile_ItemBase_Populate) | A tile's base `populate` |
| [`FeHome_Handle_Command`](/reference/game/fe_home#FeHome_Handle_Command) | Handles a home-screen command |
| [`Vm_Set_String`](/reference/game/vm#Vm_Set_String), [`Vm_Set_Bool`](/reference/game/vm#Vm_Set_Bool) | Set a view model property |
| [`g_Vm_Registry`](/reference/game/vm#g_Vm_Registry) | The view model registry |
| [`Ne_Make_StrFromCStr`](/reference/game/core#Ne_Make_StrFromCStr) | Makes an engine string |

A tile is a `HomeItem` whose virtual table has `populate` in slot 4 and `get_action` in slot 5. Replace them for one tile with [`swofty::subclass`](./mixin#subclass). `get_action` returns an `Action` of type 1 whose `str` is the route to open.

## UI patches

[UI patches](/modding/ui-patches) edit the game's own screens, routes and UI databases. The build does not apply them yet.
