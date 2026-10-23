---
title: UI patterns
description: How stock F1 25 screens build multi-row lists, move focus between rows, block input, clip and fade scrolling content and host tab templates, what is proven in game and what is inferred, and how the launcher checks UI edits before they ship.
---

# UI patterns

These patterns are read from the stock `.ui` files and `.idf` scenes. The data behind every statement is in `tools/uicatalog/catalog.json` and on [UI node reference](./nodes). Each section marks what was **verified in game** and what is **inferred** from stock files alone.

## Tab containers

A container screen (`UISC`) holds a tab controller (`UITC`) and hosts tab templates from fragment 1 of the same `.ui`.

- The `UITC` binds the scene's content element with one layout entry, such as `screen_container`, an `eXRef`. `contentPoint` (`Scene Root.screen.screen_cc`) and `usedElement` (`screen_cc`) name where a tab template's scene is placed, and `tabContainer` names the stacker that holds the tab headers.
- The tab templates are `UIGP` roots in the `Templates` list, each with its own scene in the components slot. Which templates become tabs comes from the view model: stock bindings name them as `tab_controller.tab_<n>( <template> )`, such as `#f1_world_flow.f1_world_compendium_container.tab_controller.tab_1( f1_world_compendium_chapter_tab )`.
- The controller raises lifecycle commands on a tab template root. Stock `UIGP` nodes declare `onTabSelected` (43), `onShowCompleted` (30), `onTabActivated` (30), `onTabDeselectBegin` (10), `onTabDeselected` and `onTabDeactivated`, and forward them to their children. Stock `dynamic_era_select` declares only `onShowCompleted` (→ `tiles.trigger`, which spawns the row) and `onTabDeselectBegin` (→ `tiles.detach`, which despawns it).
- Fragment 1 of `f1_world_activity_container.ui` holds the tab templates `f1_world_activity` and `f1_world_activity_offline` (the play screen), `dynamic_era_select` (the era picker), `public_gp_select` and `time_trial_select`, and the tile templates they spawn (`activity_*_tile`, `era_select_tile`).

**Verified in game:** when the game rejects one edited tab template, the whole container fails without an error at warn level. The play screen's category boxes vanish even though every other template in the file is byte-identical to stock. With the era UI edit off, the play screen always shows. With the same edited build (two rows, earlier navigation, no masks), the play screen showed in one session and not in a later one.

**Inferred:** the container binds all of its templates when it opens, so a template that fails to bind takes the container with it. A failure that changes from session to session points at a binding whose target exists only sometimes when the template binds; see [Runtime bindings](#runtime-bindings).

## Multi-row lists

A row is a `UIGP` holding a tile spawner (`F1TS`), magnets (`UIGP` nodes bound to `magnet_full_left`, `magnet_full_centre`, `magnet_full_right`) and a scrollbar (`UISB`).

- The `F1TS` binds the row's stacker, `item_spawner` or `spawner_list` (`eStacker`, 207 of 267 spawners; 19 more through an `eXRef`). `dataContext` binds the list the spawner builds from and `selection` the selected item.
- On `spawned` a spawner signals itself `trigger_children` and `focus_and_reset`, and the scrollbar `attach`.
- The row's scene is a component reached through an `eXRef` in the screen's scene. In `vehicle_type.idf` that is `play_categories_row`, which references `vehicle_type_spawner.idf` (`scroll_bar`, `item_spawner`, three magnets).
- `promo_store` in `unified_store.ui` has three spawned rows (`featured_item`, `row_1`, `row_2`). They spawn on the root commands `attach_spawner` and `detach_spawner`, which the store's native view model raises.

**Verified in game:** two half-scale rows at the top level of `vehicle_type.idf` (the stock `play_categories_row` and a clone), each `F1TS` bound to its own view-model list, draw and scroll.

## Navigation between rows

A spawner fires `navigate_up_failed`, `navigate_down_failed`, `navigate_left_failed` and `navigate_right_failed` when focus tries to leave it. Two stock ways route that to another row:

1. **Direct.** The failed event signals the other row's spawner `focus`. **Verified in game** in the era picker: `navigate_down_failed` on the Built In spawner focuses the Addons spawner and `navigate_up_failed` on Addons focuses Built In.
2. **Through the row (`promo_store`).** Each row `UIGP` declares the commands and events, and the spawner reports to its row:
   - spawner `navigate_*_failed` signals its row's `navUp`, `navDown`, `navLeft`, `navRight` pin; a signal to a `UIGP` pin named like an event raises that event;
   - the row declares `focus` → spawner `focus`, `block_input` → spawner `inputDisallow`, `unblock_input` → spawner `inputAllow`;
   - the row's `focused` event signals the other row's `unfocus`;
   - `row_1`'s `navDown` signals the `SVCD` `row1_items_visible` `query` and the `UIAM` `trigger_nav_on_up_down` `navigate`; that `SVCD` reads the view-model bool `row2_items_visible` and on `yes` raises the screen command `row_2_focused`, which native code handles;
   - `navLeft` and `navRight` raise screen commands (`row_navigate_left`, `row_navigate_right`);
   - the template root forwards `onTabDeselectBegin` to the rows' `block_input` and `onTabActivated` to their `unblock_input`.

   `row_1` declares no `navUp` event. Every stock `UIGP` `navUp`, `navDown`, `navLeft` and `navRight` event has at least one signal (106, 109, 22 and 102 of them).

**Inferred:** the promo_store chain depends on native handlers: `row_2_focused` and the `attach_spawner` commands come from the store's view model.

## Focus and input blocking

- `InputOnActPrt` is a `bool` set on `F1TS` (37), `UITG` (32) and `SCRL` (4) only. **Verified in game:** setting it on the era `F1TS` in place of `consumeStrategy` works.
- `consumeStrategy` is a `ui32` on 9 node types, mostly `DDMC` message containers (26).
- `inputAllow` and `inputDisallow` are pins on `F1TS`, `UIGP`, `TILE`, `UITC`, `UIKL` and others. `inputAllowed` is the matching property (a literal or bound `bool`).
- `focus`, `unfocus`, `focus_and_reset` and `focusFirstAvailable` are the spawner focus pins. **Verified in game:** dropping `visualNav` from the era template root and the magnet rename from `focused` to `unfocus` work.

## Masks and clipping

Three element types clip:

| Type | Stock elements | How stock uses it |
|---|---|---|
| `eMaskTexture` | 899 | A texture alpha mask (`eMaskTexture.maskTextureResource`, `eMaskTexture.maskHalfVec`, `width`, `height`). Its children are `eNull` (1,053), `eXRef` (81), `eStacker` (29) and `eScroller` (17) |
| `eWindow` | 491 | A clipping rectangle with rounded corners (`cornerCurves`, `cornerCurvesRadius`, `maskedWindow`). It holds 1,217 `eXRef` children |
| `eMask` | 3 | Only in `driver_of_the_day.idf` and `tut_overlay_mask_group.idf`; its children are `eNull` |

- In stock the mask sits inside the list's own component scene, between the transition nulls and the stacker: `play_button_list.idf` is `transition_panel_list` > `mask_list` (`eMaskTexture`, `play_side_panel_button_mask.tif`) > `transition_list` > `spawner_list` (`eStacker`). The screen scene places that component with one `eXRef`. Other examples: `con_info_button_list.idf` (`mask_list` > `transition_list` > `spawner_list`), the laptop message lists (`content_mask` > `scroller` > `item_stacker`) and the career rewards lists (`mask_list` > `scroller`).
- The UI side binds the stacker inside the masked component, through the `eXRef` when needed (`eXRef>eStacker`). Nodes bind an `eMaskTexture` itself only to animate or hide it: `UIGP` 40, `UIXA` 9, `UIL2` 2.
- Every resolved stock layout path with more than one entry passes only through `eXRef` elements before its last entry.

**Verified in game:** wrapping each row `eXRef` of `vehicle_type.idf` in an `eMaskTexture` cloned from `mask_list`, with layout paths `[row_mask_x, row]`, empties the play screen. That build also added a new fade texture resource for the masks.

## Runtime bindings

A binding names a view-model key relative to the template's view model, such as `tiles`, or a global path starting with `#`. Native code publishes the keys. A key that native code adds after the template is created, such as the era picker's `addon_tiles`, `selected_addon_tile` and `addon_tiles_visible` (published at `site_FeTile_EraTilesBuilt`), exists at bind time only if the native step ran first. Stock `dynamic_era_select` binds only `tiles` and `selected_tile`.

The launcher flags every view-model key an edited template binds that no stock node of the same template binds (`vm-key-new`).

## Checking UI edits

`check::ui_edit` (`.ui`) and `check::scene_edit` (`.idf`) in `packages/core/src/ui/check.rs` report two layers:

| Layer | Source | Effect |
|---|---|---|
| Hard | The format: node `id` and `nameId`, layout `nodeId`, event and command hashes, signal `to_object` and `to_pin` hashes, signals to nodes missing from their own root, and `eXRef` ids that disagree with `fileRef` | The build stops |
| Hard | `tools/uicatalog/loader_rules.json`: rules read from the engine's UI loader | The build stops; the finding cites the rule and its evidence |
| Warning | The stock catalogue | Listed in the build's warnings |
| Warning | A diff against the stock template: view-model keys stock never binds there | Listed in the build's warnings |

Catalogue warnings (code, what it flags):

| Code | Flags |
|---|---|
| `node-type`, `element-type` | A node or element type no stock file uses |
| `root-type`, `parent-type`, `element-parent` | A type as a screen or template root, or under a parent type, where stock never puts it |
| `prop`, `prop-shape`, `param`, `param-type` | A property or param stock never sets on that type, or a value shape stock never uses for it |
| `command`, `event` | A command or event name stock never declares on that type |
| `event-no-signals`, `command-no-signals` | An event or command with no signals where every stock one of that name on that type has signals |
| `event-signals`, `command-signals` | Signals on an event or command that never has any in stock |
| `pin-in`, `pin-out`, `link`, `pin-attributes` | A signal to a pin stock never signals on the target type, from a pin the source type never sends from, between types stock never links, or with attribute shapes stock never sends |
| `layout-shape`, `layout-element`, `layout-through` | A layout attach shape, a bound element-type chain, or a path through a non-`eXRef` element that stock never uses |
| `vm-key-new` | A view-model key that no stock node of the same template binds |

### Loader rules

`tools/uicatalog/loader_rules.json` holds rules read from the engine. A rule upgrades every finding it matches to a hard one:

```json
{
  "schema_version": 1,
  "rules": [
    {
      "id": "uigp-nav-event-needs-signals",
      "code": "event-no-signals",
      "node_type": "UIGP",
      "name": "navUp",
      "effect": "reject",
      "evidence": "FUN_14xxxxxxx L2",
      "message": "the event binder rejects an event with no signals"
    }
  ]
}
```

`code` is a finding code from the table above. `node_type` and `name` narrow the match, and `*` or leaving them out matches any. `effect` is `reject` or `drop`; `warn` keeps the finding a warning. Two codes only rules produce: `required-prop` (a node of `node_type` must set property `name`) and `forbidden-prop` (it must not).

### Where it runs

- Every addon build: after an addon's ERP operations apply, each changed `.ui` and `.idf` fragment is checked against its stock copy with the scenes of its archive. Hard findings stop the build.
- The core menu build: the hard checks run on every ADDON SETTINGS screen it generates.
- By hand:

```sh
cargo run --release -p swofty-core --example ui_check -- <archive.erp> <eaid resource> [--stock <stock archive.erp>] [--against <other archive.erp>]
```

`--stock` adds the runtime-binding diff, `--against` prints only findings the other archive does not have, and the exit status is 1 when there are hard findings.

### Rebuilding the catalogue

```sh
F1_25_DIR=<game folder> python3 tools/ui_catalog.py nodes --baseline <baseline.json> [--stock <pristine copy>]
python3 tools/ui_catalog.py nodes-doc
F1_25_DIR=<pristine game folder> cargo test --release -p swofty-core --test real_ui_catalog
```

The last command checks that every stock `.ui` and `.idf` passes with no findings.
