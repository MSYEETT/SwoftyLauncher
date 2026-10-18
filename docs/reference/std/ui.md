---
title: UI
description: Addon screens, ADDON SETTINGS pages and their view models, home tiles and UI patches.
---

# UI

You add screens to F1 25 through swofty_core, which uses the engine's own screen loader. To add a screen and a home tile, follow the [in-game UI tutorial](/reference/addons/in-game-ui).

## Screens

To tell the loader about your screens, list each one in `operations.screens` in `swofty.json`:

```json [swofty.json]
"screens": [{ "name": "my_page", "file": "ui/my_page.ui", "flow": "flow_options", "anchor": "settings_container", "view_model": false }]
```

| Field | Default | Meaning |
|---|---|---|
| `name` | | Your screen's route. Use `[a-z0-9_]` and keep it unique across the profile's addons |
| `file` | | Your screen's raw `.ui` data, relative to the addon folder |
| `flow` | `flow_frontend` | The flow that loads the screen |
| `anchor` | | A game screen in the same flow. Your screen registers after that game screen's resource loads |
| `view_model` | `false` | Ask swofty_core for a native view model |

The build installs each file as `swofty/ui/<name>.ui` and lists it in the `ui` block of `swofty/settings.json`. When the flow loads, swofty_core registers each listed screen. If the engine rejects any nodes, swofty_core logs each one with the reason.

A route to an unregistered addon screen is cancelled, reopening the last screen. When naming yours, leave `swofty_settings`, `swofty_addon_<n>` and `swofty_map_<n>` alone: they're reserved for ADDON SETTINGS.

## ADDON SETTINGS

Each time you build the profile, the launcher generates ADDON SETTINGS from the game's own screens. Here's what goes on each page:

| Screen | Holds |
|---|---|
| `swofty_settings` | One tile per enabled addon and map: name, author, version, description |
| `swofty_addon_<n>` | The page for `addons[n]` of `swofty/settings.json`: one row per option |
| `swofty_map_<n>` | A map's track code, author and version |

To open ADDON SETTINGS, its home tile routes to `mod_menu`.

## View models

Set `"view_model": true` to request a view model for your screen. You'll get one only if swofty_core has a view model whose prefix matches your screen's name; it registers that model under `view_model_data.<screen>`. swofty_core ships one view model, for `swofty_addon_` pages, with these properties:

| Property | Value |
|---|---|
| `title` | The addon's name |
| `o<k>.label` | Option `k`'s label |
| `o<k>.text` | The option's current value, as displayed |
| `o<k>.index`, `o<k>.value` | Its current value's index |
| `o<k>.min`, `o<k>.max`, `o<k>.step` | The index range: 0, the last index and 1 |
| `o<k>.is_slider` | `true` for a slider |

Use these commands to change options, enable or disable an addon, or restart the game:

| Command | Effect |
|---|---|
| `swofty_next:<addon>:<option>` | Select the next value |
| `swofty_prev:<addon>:<option>` | Select the previous value |
| `swofty_set:<addon>:<option>:<index>` | Select value number `index` |
| `swofty_enable:<addon>:<0 or 1>` | Turn the addon off or on |
| `swofty_restart` | Close the game so the launcher rebuilds and relaunches it |

## Home tiles

To build tiles for the home screen, use these [Game API](/reference/game/) bindings:

| Binding | Use |
|---|---|
| [`FeTile_Build_List`](/reference/game/fe_tiles#FeTile_Build_List) | Builds a tile list |
| [`site_FeHome_AfterCareerCall`](/reference/game/fe_home#site_FeHome_AfterCareerCall) | The instruction after the Career tile is added. `r14` is the `TileList *` |
| [`FeHome_Add_CareerTile`](/reference/game/fe_home#FeHome_Add_CareerTile) | Appends a Career tile to a list |
| [`FeTile_ItemBase_Populate`](/reference/game/fe_tiles#FeTile_ItemBase_Populate) | A tile's base `populate` |
| [`FeHome_Handle_Command`](/reference/game/fe_home#FeHome_Handle_Command) | Handles a home-screen command |
| [`Vm_Set_String`](/reference/game/vm#Vm_Set_String), [`Vm_Set_Bool`](/reference/game/vm#Vm_Set_Bool) | Set a view-model property |
| [`g_Vm_Registry`](/reference/game/vm#g_Vm_Registry) | The view-model registry |
| [`Ne_Make_StrFromCStr`](/reference/game/core#Ne_Make_StrFromCStr) | Makes an engine string |

A tile is a `HomeItem`. Its virtual table has `populate` in slot 4 and `get_action` in slot 5; you can replace both for one tile with [`swofty::subclass`](./mixin#subclass). To choose where the tile takes you, `get_action` returns an `Action` of type 1 whose `str` is the route to open.

## UI patches

[UI patches](/reference/ops/ui-patches) let you edit the game's own screens, routes and UI databases, but the build does not apply them yet.
