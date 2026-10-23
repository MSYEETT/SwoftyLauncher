---
title: UI engine internals
description: How F1 25 loads a .ui resource, registers its screens and templates, builds node trees, instantiates tab and tile templates, links signals and binds layout paths, which checks reject a node, what each failure takes down, and how to trace a failure live.
---

# UI engine internals

This page describes the code in `F1_25.exe` that turns a `.ui` resource into live UI nodes. The [node grammar](/reference/pkg) and the [scene format](./idf) describe the data. Here the subject is what the engine accepts and what it does with a rejection.

All addresses are for the Steam build (`sha256 d873a1c8…`, image base `0x140000000`). Each claim is marked:

- **[read]**: read in the decompile, callers listed (KB level L2).
- **[traced]**: also observed in the running game with the [live trace](#live-trace) on 2026-10-23.
- **[scan]**: derived by a byte or immediate scan (KB L1).
- **[inferred]**: a reading of the code that no decompile or trace has confirmed.

The runtime-confirmed parts are the `NBVN` check, the instance build of a failing template root and the tab controller's two loops and instance names. The KB write-ups are `re/db/subsystems/vm.md` (engine side) and `re/db/subsystems/ui_system.md` (format side).

The machine-readable form of every check is [`re/db/ui/loader_rules.json`](https://github.com/Swofty-Developments/SwoftyLauncher/tree/main/re/db/ui/loader_rules.json): the rules, the error codes, the silently ignored cases, the order-dependent paths and a table of all 136 node classes.

## Load: one `.ui` resource

`Vm_Load_UidfResource` (`0x14449e140`) is the UIDF resource handler. It runs once per resource and walks the fragments in order **[read]**:

| fragment | handler | when |
|---|---|---|
| `temp` (screens, flows) | `Vm_Load_UidfScreens` `0x1441d7a90` | queues a 0xd0-byte build job on `mgr+0xb900`; the UI update builds it over later frames |
| `syst` (templates) | `Vm_Load_UidfTemplates` `0x144177470` | synchronous, in the handler |

A screen's node tree is built once, when its job runs. A template's node data is **not** parsed at load: `Vm_Load_UidfTemplates` only reads each template's `name` and stores `{NeStr name, resource id, pkg base, u32 node offset}` in the template store, a map keyed by the name string at `*(uisys->vt[0x48]() + 0x8168)` (map at `+0x28`, record array `+0x50`/`+0x58`) **[read]**. Templates are built each time something instantiates them.

The template store is **global across every loaded `.ui`**. A template whose name is already in the store makes `Vm_Load_UidfTemplates` return `0x58` (`0x144177f61`); the handler then destroys the resource's tracker and stops (`0x14449e292`), so every later template of that resource is never registered **[read]**.

## Node records and classes

A node is an 8-slot array: `[props, children, commands, events, layout, components, behaviours, slot7]`. `Vm_Read_UidfNode` (`0x14417f5f0`) reads one into a record: name `char*` at `+0x68`, class FourCC at `+0x70`, object at `+0x78`, children array at `+0x10` **[read]**. Slots 6 and 7 are optional.

The class FourCC selects a factory through `Vm_Find_NodeFactory` (`0x14416bc70`). 97 neon classes are registered by `Vm_Register_NodeClasses` (`0x14419b1d0`); the game registers its own (`F1xx`, `TILE`, `LCDC`, `OSPL`, `RSCM`, `TACM`, `TADM`, `UISS`) in `0x1437ea4a0` **[scan]**. Each class has exactly one vtable whose slot 16 (`+0x80`) returns its FourCC; slot 9 (`+0x48`) is its from-data init, slot 43 (`+0x158`) its message handler and slot 50 (`+0x190`) its pin handler **[scan]**. The JSON lists all of them with addresses.

20 classes use `Vm_ElementBase_Init` (`0x1441ffb90`) as their init and 106 more call it from theirs, 126 of the 136 **[scan]**. `UIGP`, `UIAM`, `UIAC`, `UIBT` and `UIPL` are plain element-base classes; `SVCD` (`0x144276f20`) and `F1TS` (`0x14382cff0`) add their own props after it **[read]**.

`UIAM` is the audio theme trigger: every stock `UIAM` node is named `audio_theme_trigger`, its pin handler implements `confirm`, `back`, `navigate`, `tab`, `toggle`, `slider` and their `…Fail` forms, and the class table names `NeUIAudioThemeTr…` **[scan]**.

## What rejects a node

A non-zero return from any of these fails the node. Codes: `3` a required key is missing, `5` unreadable data, a wrong value shape, a FourCC mismatch or no factory, `0x22` an empty layout path, `0x57` a bad enum value.

| check | code | where |
|---|---|---|
| the node array or any slot is not a readable `!pkg` value | 5 | `Vm_Read_UidfNode` |
| no factory for the FourCC, or the factory returns null | 5 | `0x144181765` (templates), `0x1441ddc1c` (screens) |
| element init: the class FourCC differs from the record's `type` | 5 | `Vm_ElementBase_Init` |
| layout (slot 4): `path = slot4[0]` is an empty array | 0x22 | `Vm_ElementBase_Init` |
| a path step with `isComponentId` lacks `componentId`; a step without it lacks `nodeId` | 3 | `Vm_ElementBase_Init` |
| components (slot 5): the first entry lacks `name` or `componentId` | 3 | `Vm_ElementBase_Init` |
| behaviours (slot 6): an entry lacks `type`; a created behaviour's own init fails | 3 / its rc | `Vm_ElementBase_Init` |
| commands (slot 2) / events (slot 3): an entry lacks `command` / `event`, or lacks `signals` | 3 | `Vm_Read_SignalList` `0x1442c17e0` |
| a signal lacks `to_pin` or `to_object`; a `mappings` entry lacks `value` | 3 | `Vm_Read_SignalList` |
| `UITG` `UITB` `UILT` `UIL2` `UIEL`: `data` has no `dbentry` key / is missing, not an object or empty | 3 / 5 | `Vm_ListBase_Init` `0x144208c80` |
| `SVCD`: `value` cannot be read | its rc | `0x144276f20` |
| `F1TS`: `inputType` is neither `normal` nor `dpadOnly` | 0x57 | `0x14382cff0` |
| slot 6 lists `NBVN` (visual navigation, type `1314275918`) but props have no `visualNav` object | 3 | `Vm_VisualNavBehaviour_Init` `0x144289830` |

All of these are **[read]**; the `NBVN` row is also runtime-confirmed (below). The loader never reads `commandName`, `eventName`, `to_object_name`, `to_pin_name`, `from_pin_name` or `debugName`; only the hashes count. An empty `signals` array is accepted. Multi-step layout paths are normal: stock nodes use paths of 2 to 6 steps.

### Behaviours (slot 6)

Each slot-6 `type` that is a registered behaviour gets a behaviour object, and its from-data init (vtable slot 5, `+0x28`) runs in the element init's loop (`0x14420254d`); a non-zero return rejects the node **[read]**:

| type | init | props it reads |
|---|---|---|
| `NBVN` visual navigation | `0x144289830` | `visualNav` object, **required**; then `autoFocus`, `allowXNav`, `allowYNav`, `xOffset`, `yOffset`, `navEnabled` |
| `NBMO` material override | `0x144280780` | `materialData`, optional (missing returns 0) |
| `NBEL` element attach | `0x14427fd50` | `hideOnUnbind`, optional |
| `NBAN` animation | `0x14427e320` | `handleRepeats`, optional |
| `NBAC` audio context | `0x14427dfd0` | `audioContext`, optional |
| `NBDC`, `NBVM`, `NBXA`, `NBTP` | `0x1438d50b0` | nothing |
| `NBMG`, `NBFC`, `NBOS` | none | not registered; skipped |

Stock pairs `visualNav` and `NBVN` exactly: every template root with the prop lists the behaviour, and `f1_world_activity` has neither. Props are found by case-insensitive string compare of the key names (`0x143927f90`), not by hash; a missing key returns 3.

## What a failure takes down

| failure | scope | log |
|---|---|---|
| node in a **screen** build job | the screen and every later screen of the same `.ui` (`site_Vm_UidfJob_ScreenDone` `0x1441de0d7`); `0x55` (same-name screen already registered) only skips that screen | none; swofty_core logs `[ui] engine screen '…' failed to build` |
| node in a **template instance** | the whole instance; the rc reaches every ancestor | none |
| duplicate template name at load | every later template of that resource | none |
| template name not in the store at instantiation | that one instance (the caller skips it) | none |

**Nothing is cleaned up [read].** `Vm_Build_TemplateNode` (`0x1441815c0`) attaches a node to its parent (`parent->vt[0x170]`, `0x14418169e`) after the node's own init succeeds and before its children are built. When a descendant fails, `Vm_Instantiate_Template_Body` (`0x153215c10`) jumps straight to its exit (`0x153215e71`) and the root, with every node built so far, stays attached to the parent without its `vt+0x58` setup, signal remap or `vt+0x50` link. When the root's own init fails, nothing is attached and the root object leaks; for a tab, the controller then has no content instance for that tab **[traced]**.

## Template instantiation

Every runtime instantiation calls `Vm_Instantiate_Template_Thunk` (`0x144177fe0`, a `jmp` to the `.data1` body `0x153215c10`) **[read]**. Its 9 callers include the tab controller (`0x144266e30`), the data-driven spawner `Vm_Spawn_TemplatesFromList` (`0x144178270`, reads `template_path`, `data_context_root_path`, `signal`, `signal_data` per list entry) and the `F1TS` tile spawner (`0x1438308c0`).

The body **[read]**:

1. builds the tree with `Vm_Build_TemplateNode`, under the given parent, naming the root after the instance name (for a tab, `tab_<n>( <template> )`);
2. gives each node a runtime id that continues the parent's djb2 with `"." + name`, and records the pair (authored id, runtime id);
3. on success, runs `Vm_Remap_InstanceSignals` (`0x144349300`): each signal's `to_object` that names a node inside the template is rewritten to the runtime id; ids outside the template stay as authored, without an error;
4. calls the root's `vt+0x58`, then `vt+0x50(ctx)`, whose return becomes the instance result (`0x153215dbd`).

### Tabs (`UITC`)

`Vm_UiTabs_OnMessage` (`0x144264d50`, `UITC` slot 43) handles the data-context message: it resolves the `tabs` array under the controller's data context and, for every entry, calls `Vm_UiTabs_AddTabTemplate` (`0x144266e30`) with `content_template`, then in a second loop with `header_template` **[read]**. `Vm_UiTabs_AddTabTemplate` reads the template name from the tab's VM node, looks it up (miss: `0x144267110`) and instantiates it under the controller (result: `0x14426708d`). Both a miss and a failed instance return NULL; the content result is discarded and the loop continues with the next tab **[traced]**. The instance name is always `tab_<n>( <content_template> )`, built from the `content_template` value even in the header loop, so a header instance carries the content template's name; `<n>` is the controller's child count plus one. A content instance whose root init failed attaches nothing, so the next instance gets the same `<n>` **[traced]**.

For `f1_world_activity_container`, `Game_F1WorldActivityContainer_AddTabs` (`0x141a76130`) creates seven tab view models, in this order **[read, traced]**:

| tab | template | title |
|---|---|---|
| 1 | `f1_world_activity` | `lng_f1_world_play` |
| 2 | `f1_world_activity_offline` | `lng_f1_world_play_offline` |
| 3 | `dynamic_era_select` | `lng_solo_grand_prix` |
| 4 | `public_gp_select` | `lng_public_grand_prix` |
| 5 | `dynamic_era_select` | `lng_time_trial` |
| 6 | `dynamic_era_select` | `lng_screen_era_select` |
| 7 | `session_create_dlc_select_tab` | `lng_create_session_dlc_select` |

`f1_world_activity`, `f1_world_activity_offline`, `dynamic_era_select` and `public_gp_select` live in fragment 1 (`syst`) of `f1_world_activity_container.ui`, whose fragment 0 is the screen; `session_create_dlc_select_tab` is registered by `f1_world_container.ui`. The `time_trial_select` template in the container `.ui` is not used by any tab. Every header instance builds `primary_nav_tab` from `templates/tab_header_templates.ui` **[traced]**.

## Signals, commands and events at runtime

- **Linking [read].** A signal holds a link handle to its target, not a lookup. `Vm_Link_NodeReference` (`0x144179da0`) resolves a node id; on a hit it logs `UIRef` `LINK node1 [%s][%u]` (info level); on a miss it registers a **pending** link (`0x1442238b0`) that completes when a node with that id is created.
- **Raising [read].** `Vm_Raise_NodeEvent` (`0x1442468d0`) finds the event id in the node's event list. An undeclared event, or one with no signals, is a no-op.
- **Delivery [read].** `Vm_Dispatch_EventSignals` (`0x144251800`) skips a signal whose link handle or target is null, and otherwise calls the target's pin handler (`vt+0x190`) and `vt+0x198`. It has no error path: a pin the target does not implement is ignored by the target's handler.
- **Native pins [scan].** The pins each class's handler compares against are in the JSON (`native_pins`), from the handler and one level of callees, with the pin ids read from the running game. It is a lower bound. A node's own slot-2 commands add pins of the same name.

## Layout binding

At init, the layout path is only stored: `Vm_GetOrCreate_AttachBehaviour` (`0x144179710`) returns the node's attach behaviour (vtable `0x145a73fa0`) and its `+0x20` vector receives one 0x30-byte step per path entry, `{u8 kind (0 = nodeId, 1 = componentId), u32 nodeId @+4, u64 componentId @+8}` **[read]**. A component (slot 5) adds one more step. No step is resolved against the `.idf` at load time, so no load-time check depends on the scene. Resolution happens when the element attaches to the scene (`attacherOpened` / `attacherClosed` pins, `UI_ATTACHER` channel transitions in `0x144218810`); how a step descends through `eXRef` and `eMask` elements is not mapped yet.

## Logs and debug switches

- No code on the screen-build or template path writes a log line on failure **[read]**. The template path has no channel to enable.
- The existing channels give context, not errors. `SWOFTY_GAME_LOG_CHANNELS=verbose` lowers `UI`, `UI_*`, `UIRef`, `UICom`, `UINodeTreeMessages`, `VM`, `FLOW`, `NeUI*` to info; that shows `UIRef LINK node1 …`, `OPEN SCREEN (…)`, `registerViewModelForViewModelBinding: […]` and `UI_ATTACHER` transitions. Add a channel at info with `name:info`, for example `SWOFTY_GAME_LOG_CHANNELS=default,UIRef:info,UI_ATTACHER:info`.
- swofty_core's screen hooks (`native/src/core/ui.cpp`) cover screens only. `SWOFTY_UI_DIAG=1` turns on its per-node lines.
- The engine has an errno trace: `Ne_Set_ErrnoTrace` (`0x14391ab70`) arms it for the calling thread, and failing readers then write `{return address, code}` into a 10-entry ring at `0x146107830` **[read]**. Nothing in the shipped game arms it for the UI thread.

## The era picker container failure

Observed: with an edited `dynamic_era_select` template, the play tab's category boxes in `f1_world_activity_container` can vanish, with no log.

**Cause of the rejection (runtime-confirmed 2026-10-23).** The edit removed the root's `visualNav` prop but kept `NBVN` in its slot 6. `Vm_VisualNavBehaviour_Init` reads `visualNav` first and returns 3 when it is missing, so the root's element init returns 3 and every content instance of `dynamic_era_select` fails. A live trace with the breaking build showed `TEMPLATE NODE 'dynamic_era_select' class UIGP init rejected it: rc=3` for each of the three era tabs. The stock template has `visualNav = {xOffset: 0, yOffset: 0}`; restoring it (or dropping `NBVN` from slot 6) satisfies the check.

**There is no second, successful build.** The same trace showed seven more instances named `tab_5( f1_world_activity )` … `tab_11( session_create_dlc_select_tab )`, three of them `tab_N( dynamic_era_select )`, all `ok`. These are the tab controller's header loop: their names embed the content template, but the template built is each tab's `header_template` [read]. The era content template fails on every build.

**What follows, [inferred].** Because a root whose init fails is never attached, the controller ends up with four content instances (two activity tabs, `public_gp_select`, `session_create_dlc_select_tab`) for seven tabs, while its headers are complete. How the controller maps a selected tab to its content instance is not mapped; a shifted mapping between the seven tabs and four contents would explain a missing play screen, and would make the result depend on which tab is selected first.

Other paths that depend on order or timing **[read]** unless marked:

- the template store is global by name, so two resident `.ui` resources that share a template name race at load (`0x58`);
- signal links to nodes created later stay pending, and signals fired before then are dropped;
- tab templates are instantiated on the controller's data-context message, which can come before or after `FeTile_Build_EraTiles` (`0x141a6e980`; core's `site_FeTile_EraTilesBuilt` `0x141a6f728` creates `addon_tiles` and `addon_tiles_visible` there);
- bindings such as `F1TS dataContext` and `SVCD dbEntry` store a path at init and resolve it later. `Vm_GetOrCreate_ListChild` (`0x1442b30e0`) keeps an existing child only if it is a list node, otherwise it creates one, so a node created earlier under the same name with another kind is not the node core fills **[inferred]**.

## Live trace

[`re/tools/ui_trace/ui_template_trace.sh`](https://github.com/Swofty-Developments/SwoftyLauncher/tree/main/re/tools/ui_trace) attaches gdb to the running game and arms 20 tracepoints that print and continue:

| tracepoint | address | prints |
|---|---|---|
| duplicate template name at load | `0x144177cfd` | the name |
| templates fragment of a resource failed | `0x14449e292` | rc, resource id |
| template node unreadable | `0x14418160a` | rc |
| no factory | `0x144181765` | node name, FourCC |
| class init rejected (non-root) | `0x144181657` | node name, FourCC, rc |
| every template root init | `0x144181657` | root name, instance, rc, pkg buffer, props, slot-6 count |
| behaviour init failed | `0x144202550`, `0x1442028c8`, `0x144202978` | node name, behaviour |
| `NBVN` init | `0x144289830`, `0x144289879` | node name, whether `visualNav` was found |
| failure chain | `0x14418176a` | each failing node and ancestor |
| instance link failed (hardware) | `0x153215dbf` | instance name, rc |
| every instance (hardware) | `0x153215e71` | instance name, rc, root class |
| tab template not found | `0x144267110` | template, instance |
| tab instance failed | `0x14426708d` | content or header, template, instance, rc |
| tab template record | `0x144266fa1` | content or header, instance, template built, record, pkg buffer, node offset |
| tab instance destroyed | `0x144265036` | instance |
| era tiles build | `0x141a6e980` | ordering marker |

Lines are numbered in the order they happen. The two `.data1` tracepoints are hardware breakpoints only; if the CPU refuses them they stay off.

1. Install the breaking build and start the game. Stay on the main menu.
2. Run `re/tools/ui_trace/ui_template_trace.sh [log file]`. It needs passwordless `sudo` for `kernel.yama.ptrace_scope`, which it sets back on exit. Wait for `[ui] armed`.
3. Open F1 World, then Play. Load-time lines appear when the F1 World flow loads, so the trace must be armed first.
4. Press <kbd>Ctrl</kbd>+<kbd>C</kbd> in the trace terminal. gdb prints hit counts, detaches and the game keeps running.

A failing root shows as `ROOT INIT '<name>' (XXXX) … init rc=…`, a failing behaviour as `BEHAVIOUR INIT failed …` or `NBVN init … visualNav MISSING`, a failing inner node as `TEMPLATE NODE '<name>' class XXXX init rejected it`, followed by `<- node …` lines and `TEMPLATE INSTANCE … FAILED` or `UITC TAB content|header … FAILED`. The behaviour tracepoint is hit for every element built, so the game can stutter while a flow loads. If every instance reports `ok`, the failure is not in node building or linking; compare the order of the `ERA TILES` line with the `dynamic_era_select` instances between a good and a bad session.
