---
title: In-game UI
description: Add a screen to F1 25 with operations.screens, put a tile on the home screen, and change an existing screen.
---

# In-game UI

You can use your addon to change the game's menus in three ways:

| Goal | Tool |
|---|---|
| Settings for your addon | [`options`](/reference/addons/settings). swofty_core builds the addon's page in ADDON SETTINGS |
| A new screen | `operations.screens`. swofty_core registers it with the game's UI system |
| A tile on the home screen | A [site mixin](/reference/addons/hooking#run-code-at-an-instruction) on the home-tile builder |
| A change to an existing screen | [UI patches](/reference/ops/ui-patches) |
| A change to a screen's layout | An edited [scene](/reference/ui/editing-menus), made in [Swofty UI](/reference/ui/swofty-ui) |

If you just need settings, each [option](/reference/addons/manifest#options) becomes a row on your addon's page in **ADDON SETTINGS**. Use **Left** and **Right** to change its value. The launcher generates those pages from the game's own files each time it builds the profile, so most addons don't need their own screen. Add one when you need more than a settings row.

## Add a screen

A screen starts with a `.ui` file, the raw screen data you'd find in the game's UI archives.

Declare your screens in `swofty.json`. Two steps need that list before your plugin runs: at build time, the launcher installs each `.ui` file into the game and lists it in `swofty/settings.json`; at boot, swofty_core registers each screen when its flow loads. That's why the declaration belongs in the manifest rather than your plugin. Your plugin handles the live parts: [mixins](/reference/addons/hooking), view-model data and the screen's commands.

### 1. Declare it

```json [swofty.json]
"operations": {
  "screens": [
    { "name": "my_page", "file": "ui/my_page.ui", "flow": "flow_options", "anchor": "settings_container" }
  ]
}
```

| Field | Default | Meaning |
|---|---|---|
| `name` | | `[a-z0-9_]`, unique across the profile's addons. The screen's name and route |
| `file` | | The `.ui` file, relative to the addon folder |
| `flow` | `flow_frontend` | The flow the screen belongs to |
| `anchor` | | A game screen in the same flow. The screen registers after that screen's resource loads |
| `view_model` | `false` | `true` asks swofty_core for a native view model |

### 2. Build and launch

Run `swofty build` to install the file as `swofty/ui/my_page.ui` and list it in the `ui` block of `swofty/settings.json`. When the flow loads, swofty_core registers your screen through the engine's own screen loader. Check `swofty/loader.log` if the engine rejects any nodes: it names each one and gives the reason. If you route to an addon screen that didn't register, the route is cancelled and the last screen reopens.

::: warning Reserved names
`swofty_settings`, `swofty_addon_<n>`, `swofty_map_<n>` and `swofty_mod_<n>` belong to ADDON SETTINGS.
:::

### View models

Set `"view_model": true` to request a native view model. Your screen gets one if swofty_core has one whose prefix matches the screen's name. It ships one for `swofty_addon_` pages, which publishes `title` and, for each option `k`, `o<k>.label`, `o<k>.text`, `o<k>.value`, `o<k>.min` and `o<k>.max`. It handles these commands to change option values:

| Command | Effect |
|---|---|
| `swofty_next:<addon>:<option>` | Next value |
| `swofty_prev:<addon>:<option>` | Previous value |
| `swofty_set:<addon>:<option>:<index>` | Value number `index` |

You'll find every view-model property and command in the [UI reference](/reference/std/ui).

## Add a home tile

To add a home tile, hook into `FeTile_Build_List`, which builds the home screen's tiles. That's how swofty_core adds ADDON SETTINGS: a [site mixin](/reference/addons/hooking#run-code-at-an-instruction) runs immediately after the game adds the Career tile, then gives the new tile its own text and route. Here's how it works in `native/src/core/swofty_core.cpp`.

**1. Name it.**

```cpp
using namespace swofty::game;

constexpr const char *ROUTE = "mod_menu";
constexpr const char *TITLE = "ADDON SETTINGS";
constexpr const char *DESCRIPTION = "Configure the addons installed by Swofty Launcher";
```

**2. Set its text.** Fill the tile's view-model node in its `populate` method:

```cpp
void set_string(void *reg, void *parent, const char *name, const char *text)
{
    NeStr *s = nullptr;
    Ne_Make_StrFromCStr(&s, text, 0, 0);
    Vm_Set_String(reg, parent, name, &s, 1);
    Ne_Make_StrFromCStr(&s, nullptr, 0, 0);
}

void mod_populate(HomeItem *item, void *node, uint32_t a7)
{
    FeTile_ItemBase_Populate(item, node, a7);
    auto vm_node = static_cast<uint8_t *>(item->button_node.node);
    if (!vm_node)
        return;
    void *reg = *g_Vm_Registry, *parent = vm_node + 8;
    uint8_t no = 0;
    set_string(reg, parent, "title", TITLE);
    Vm_Set_Bool(reg, parent, "title_localised", &no, 1);
    set_string(reg, parent, "description", DESCRIPTION);
    Vm_Set_Bool(reg, parent, "description_localised", &no, 1);
}
```

**3. Route it.** Return an action of type 1 from the tile's `get_action` method. The action's string tells it which route to open:

```cpp
Action *mod_get_action(HomeItem *item, Action *out)
{
    out->type = 1;
    out->str = nullptr;
    Ne_Make_StrFromCStr(&out->str, ROUTE, 0, 0);
    return out;
}
```

**4. Add the tile.** Call `FeHome_Add_CareerTile` to append a copy of the Career tile, then move the builder's counters past it in the site handler. Use [`swofty::subclass`](/reference/std/mixin#subclass) to give this tile a copy of its virtual table, replacing slot 4 (`populate`) and slot 5 (`get_action`). That gives your tile its own text and route while the other Career tiles keep the game's table.

```cpp
SWOFTY_SITE(site_FeHome_AfterCareerCall, swofty::Context &c)
{
    auto list = reinterpret_cast<TileList *>(c.r14);
    FeHome_Add_CareerTile(list, uint8_t(c.r12 + 1));
    c.r12 += 1;
    c.r15 += 1;
    HomeItem *item = list->items_end[-1].tile->item;
    swofty::subclass(item, {{4, reinterpret_cast<void *>(mod_populate)}, {5, reinterpret_cast<void *>(mod_get_action)}});
    Ne_Make_StrFromCStr(&item->id, ROUTE, 0, 0);
}
```

Look up every function and struct here in the [Game API](/reference/game/): [`fe_home`](/reference/game/fe_home), [`fe_tiles`](/reference/game/fe_tiles), [`vm`](/reference/game/vm) and [`core`](/reference/game/core).

::: tip
swofty_core already owns `site_FeHome_AfterCareerCall`. If you add a second site mixin there, it runs in the same chain, ordered by [priority](/reference/addons/hooking#chains).
:::

## Change an existing screen

Use [UI patches](/reference/ops/ui-patches) to describe edits to the game's screens, routes and UI databases. You can clone a node, set a label's text, change properties, or add or remove signals. List your patches in `operations.ui`:

```json [ui/example.pkgpatch.json]
{
  "schema_version": 1,
  "targets": [
    { "erp": "2025_asset_groups/ui_package/databases.erp", "resource": "eaid://ui_package/databases/tool_tips.uidb" }
  ],
  "edits": [
    {
      "op": "clone-entry",
      "at": "tool_tips.Contents.tooltips_dictionary.Contents",
      "src": "settings_fe_home",
      "as": "sample_mod_hint",
      "set": { "Name": "sample_mod_hint", "Contents.text.Value": "Sample Mod" }
    }
  ]
}
```

::: warning Not built yet
The build skips `operations.ui` and prints a warning. To apply the same `clone-entry` and `set-value` edits to `!pkg` resources, use the [`patch-pkg` ERP operation](/reference/mods/erp-assets#patch-a-pkg-resource).
:::
