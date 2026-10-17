---
title: Custom in-game UI
description: Add screens to F1 25, put a tile on the home screen, and add options to the game's menus.
---

# Custom in-game UI

An addon changes the game's menus in three ways:

| Goal | Tool |
|---|---|
| Settings for your addon | [`options`](/tutorials/addon-settings). SwoftyMod builds the addon's page in ADDON SETTINGS |
| A new screen | `operations.screens`. SwoftyMod registers the screen with the game's UI system |
| A tile on the home screen | A [mixin](/tutorials/hooking) on the home tile builder |
| A change to an existing screen | [UI patches](/modding/ui-patches) |

## Settings pages

Most addons need no screen of their own. Each [option](/modding/manifest#options) becomes a row on the addon's page in **ADDON SETTINGS**, and **Left** and **Right** change its value. The launcher generates these pages from the game's own files each time it builds the profile.

## Add a screen

A screen is a `.ui` file: the game's raw screen data, as stored in its UI archives. Declare it in `swofty.json`:

```json [swofty.json]
"operations": {
  "screens": [
    { "name": "my_page", "file": "ui/my_page.ui", "flow": "flow_options", "anchor": "settings_container" }
  ]
}
```

| Field | Meaning |
|---|---|
| `name` | The screen's name and route. `[a-z0-9_]`, unique across the profile's addons |
| `file` | The `.ui` file, relative to the addon folder |
| `flow` | The flow the screen belongs to. Default `flow_frontend` |
| `anchor` | A game screen in the same flow. The screen registers after that screen's resource loads |
| `view_model` | `true` asks SwoftyMod for a native view model |

When the profile is built, the launcher installs the file as `swofty/ui/<name>.ui` and lists it in the `ui` block of `swofty/settings.json`. When the flow loads, SwoftyMod registers the screen through the engine's own screen loader. A route to an addon screen that did not register is cancelled and the last screen reopens. `swofty/loader.log` lists every node the engine rejects, with its reason.

::: warning Reserved names
`swofty_settings`, `swofty_addon_<n>` and `swofty_map_<n>` belong to ADDON SETTINGS.
:::

### View models

A screen with `"view_model": true` gets a native view model when SwoftyMod has one whose prefix matches the screen's name. SwoftyMod has one, for `swofty_addon_` pages. It publishes `title` and, for each option `k`, `o<k>.label`, `o<k>.text`, `o<k>.value`, `o<k>.min` and `o<k>.max`, and it handles these commands:

| Command | Effect |
|---|---|
| `swofty_next:<addon>:<option>` | Next value |
| `swofty_prev:<addon>:<option>` | Previous value |
| `swofty_set:<addon>:<option>:<index>` | Value number `index` |

## Add a home tile

The home screen's tiles are built by `FeTile_Build_List`. SwoftyMod adds the ADDON SETTINGS tile with a site mixin just after the game adds the Career tile, then gives the new tile its own text and route. These are the steps from `native/src/core/swofty_core.cpp`.

**1. Name it.**

```cpp
using namespace swofty::game;

constexpr const char *ROUTE = "mod_menu";
constexpr const char *TITLE = "ADDON SETTINGS";
constexpr const char *DESCRIPTION = "Configure the addons installed by Swofty Launcher";
```

**2. Set its text.** A tile's `populate` method fills its view model node:

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

**3. Route it.** Its `get_action` method returns an action of type 1 whose string is the route to open:

```cpp
Action *mod_get_action(HomeItem *item, Action *out)
{
    out->type = 1;
    out->str = nullptr;
    Ne_Make_StrFromCStr(&out->str, ROUTE, 0, 0);
    return out;
}
```

**4. Add the tile.** `FeHome_Add_CareerTile` appends a copy of the Career tile, and the site handler moves the builder's counters past it. [`swofty::subclass`](/reference/std/mixin#subclass) then gives this one tile a copy of its virtual table with slot 4 (`populate`) and slot 5 (`get_action`) replaced. The other Career tiles keep the game's table.

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

SwoftyMod's own handler also checks that the list ends with a tile and that the new tile has the Career tile's virtual table before it changes anything.

Every function and struct used here is in the [Game API](/reference/game/): [`fe_home`](/reference/game/fe_home), [`fe_tiles`](/reference/game/fe_tiles), [`vm`](/reference/game/vm) and [`core`](/reference/game/core).

::: tip
SwoftyMod already owns `site_FeHome_AfterCareerCall`. A second site mixin there runs in the same chain, ordered by [priority](/tutorials/hooking#_4-order-the-chain).
:::

## Change an existing screen

[UI patches](/modding/ui-patches) describe edits to the game's screens, routes and UI databases: clone a node, set a label's text, change properties and add or remove signals. List them in `operations.ui`:

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
The build skips `operations.ui` and prints a warning. The same `clone-entry` and `set-value` edits already run on `!pkg` resources through the [`patch-pkg` ERP operation](/tutorials/erp-assets#edit-a-pkg-resource).
:::
