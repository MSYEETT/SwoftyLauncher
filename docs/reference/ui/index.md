---
title: Menus and UI
description: Change F1 25's menus from an addon. Find a screen's scene in Swofty UI, edit and export it, ship it with ERP operations or as a new screen, check a UI edit against the engine's rules, then test and debug it in game.
---

# Menus and UI

Every menu screen in F1 25 has a layout, its [scene](./idf), stored as an `.idf` in the game's UI archives. You change a menu by editing that scene in [Swofty UI](./swofty-ui) and shipping the result in an addon. The addon's operations put it in the game when the launcher builds a profile, so the game's own archives stay stock.

## Edit a menu

1. **Find the scene.** Open Swofty UI, press <kbd>Ctrl</kbd>+<kbd>K</kbd> and search for the screen ([Open the scene](./editing-menus#_1-open-the-scene)).
2. **Edit and preview it.** Move, resize, clone, rename or delete elements; Swofty UI draws the scene with the game's textures as you go ([Edit it](./editing-menus#_2-edit-it)).
3. **Export it.** Export writes the checked `.idf` and the `replace-resource` operations for every archive that holds it ([Export](./editing-menus#_3-export)).
4. **Ship it in an addon.** List the operations file under `operations.erp` in `swofty.json` ([Add it to your addon](./editing-menus#_4-add-it-to-your-addon)). Pick the route from the table below.
5. **Test it and debug it live.** Build, play, and read `swofty/loader.log`; when a count or position is fixed in native code, trace it with gdb ([Build and play](./editing-menus#_5-build-and-play), [Techniques](./techniques), [Debugging live](./debugging)). For a `.ui` edit, follow [Make a safe UI edit](#make-a-safe-ui-edit).

## Ways to ship a menu change

| Change | Operation | Page |
|---|---|---|
| A moved, resized or added element | `replace-resource` in an `erp` operation file, with the exported `.idf` | [Editing menus](./editing-menus), [ERP operations](/reference/ops/erp#scenes) |
| A cloned tooltip, label or list entry in a `!pkg` resource | `patch-pkg` in an `erp` operation file (`clone-entry`, `set-value`) | [ERP operations](/reference/ops/erp#editing-pkg-resources) |
| A new screen | `operations.screens`, with a `.ui` file | [In-game UI](/reference/addons/in-game-ui#add-a-screen) |
| A new home tile | A site mixin in an engine plugin | [In-game UI](/reference/addons/in-game-ui#add-a-home-tile) |
| Settings for your addon | `options`; ADDON SETTINGS builds the page | [Settings](/reference/addons/settings) |
| Edits to screens, routes and UI databases by node | `operations.ui` patches. The build skips them for now | [UI patches](/reference/ops/ui-patches) |

## Make a safe UI edit

The game rejects a `.ui` node it cannot load without writing a log line. A rejected node takes down its whole screen and every later screen of the same `.ui`, or its whole template instance; a tab whose content template is rejected loses its content. [UI engine internals](./engine) lists every check and what each failure takes down.

1. **Start from the stock resource.** Edit a copy of the stock `.ui` or `.idf` and keep every node, property and behaviour you do not mean to change. Properties the engine reads stay required: a node that lists the `NBVN` behaviour needs its `visualNav` property, and a list control needs `data` bound to an array ([What rejects a node](./engine#what-rejects-a-node)).
2. **Keep the ids and references in step.** Recompute node ids and signal hashes after renaming or moving a node; the engine reads only the hashes. Retarget an `eXRef` by setting `fileRef` and `xref` together ([References](./idf#references)).
3. **Check the build.** Run the checks on each stock archive and the archive your build writes in its place:

   ```sh
   cargo run --release -p swofty-core --example ui_check -- <stock.erp>=<built.erp>
   ```

   Hard findings cite the engine rule, its error code, what a failure takes down and the function address, and stop an addon build. Warnings list what no stock file does ([Checking UI edits](./patterns#checking-ui-edits)).
4. **Look up what stock does.** [UI node reference](./nodes) lists, per node type, the properties, commands, events, pins and layout bindings stock files use; [UI patterns](./patterns) shows how stock screens combine them.
5. **Trace a runtime failure.** A failure the checks pass, such as an ordering or binding problem, shows in the live trace: `re/tools/ui_trace/ui_template_trace.sh` attaches gdb to the running game and prints every rejected template node, failed instance and tab miss with its error code ([Live trace](./engine#live-trace), [Debugging live](./debugging)).

## What's in this section

| Page | What it covers |
|---|---|
| [UI scenes (.idf)](./idf) | The scene format: where scenes live, elements, identity, references, and `tools/idf.py` |
| [Swofty UI](./swofty-ui) | The scene viewer and editor: getting it, editing, exporting, its checks and its HTTP API |
| [Editing menus with an addon](./editing-menus) | One scene from Swofty UI to the game, step by step |
| [Techniques](./techniques) | Tracing what draws an element, counts fixed in native code, patching native builders safely |
| [Debugging live](./debugging) | Attaching gdb to the running game under Proton or Wine |
| [UI node reference](./nodes) | Every stock node type and scene element type: properties, commands, events, pins, wiring and layout bindings |
| [UI patterns](./patterns) | Tab containers, multi-row lists, row navigation, input blocking, masks, runtime bindings, and the checks every UI edit goes through |
| [UI engine internals](./engine) | How the game loads a `.ui`, registers and instantiates templates, which checks reject a node, what each failure takes down, and the live trace |
