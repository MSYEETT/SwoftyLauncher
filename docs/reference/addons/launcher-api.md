---
title: Launcher plugins
description: A launcher plugin is a helper program named in launcher.program, run as a subprocess that speaks newline-delimited JSON. It adds sidebar pages, an importer, a build hook, swofty commands and a profile summary. The steps to make one, and the full protocol.
---

# Launcher plugins

A **launcher plugin** is a prebuilt program inside your addon, named in the `launcher` block of `swofty.json`. The launcher runs it as a subprocess and exchanges newline-delimited JSON with it over stdin and stdout. A plugin can add any of:

- pages in the launcher's sidebar;
- an importer for files dropped on the window;
- asset ops contributed when a profile builds;
- `swofty` subcommands;
- a line on the profile card.

You can write the plugin in any language that reads and writes a line of JSON. The examples here use [`templates/launcher-addon`](./template#templates-launcher-addon), a Rust helper that depends only on `serde_json`.

## Make a launcher plugin

1. **Copy the template.** Start from `templates/launcher-addon`, or from the smaller helper in [Your first addon](./first-addon) ([Copy the template](#_1-copy-the-template)).
2. **Name it in `swofty.json`.** Set `launcher.program` for each OS, and list the `sections`, `imports`, `build` and `commands` you answer ([Name it](#_2-name-it-in-swofty-json)).
3. **Answer the calls.** Reply to `hello`, then to `view`, `event`, `import`, `build`, `command` and `summary` as you need them ([The protocol](#the-protocol)).
4. **Build the binary.** Build one binary per OS and copy it to the path in `program` ([Build the binary](#_4-build-the-binary)).
5. **Install and open it.** Install and enable the addon, open its sidebar page, and read `helper.log` when something fails ([Install and open it](#_5-install-and-open-it), [Isolation and failures](#isolation-and-failures)).
6. **Package it.** Ship the binaries in the `.swoftyaddon` ([Packaging and publishing](./publishing)).

## What's on this page

| Section | What it covers |
|---|---|
| [How the launcher runs the plugin](#how-the-launcher-runs-the-plugin) | When the plugin starts, its working directory and its log |
| [The protocol](#the-protocol) | Messages, the calls the launcher sends and the host calls the plugin sends back |
| [Views](#views) | The blocks a sidebar page is built from, and its events and effects |
| [Dashboard blocks](#dashboard-blocks) | Hero, pills, tiles, meters, cards and grids |
| [Forms](#forms) | Number, text, slider, select and toggle fields with an Apply button |
| [Tables](#tables) | Sortable columns, searchable rows and editable cells |
| [Live pages](#live-pages) | `tick` from the launcher and `invalidate` from the plugin |
| [The build reply](#the-build-reply) | Asset ops returned at build |
| [Importers and drag-and-drop](#importers-and-drag-and-drop) | Claiming dropped files and folders |
| [Commands](#commands) | `swofty` subcommands |
| [Profile summary](#profile-summary) | A line on the profile card |
| [Changes at runtime](./runtime-assets#from-a-build-hook) | How build-hook ops reach the game while it boots |

## Build a plugin

### 1. Copy the template

```sh
cp -r templates/launcher-addon my-addon
cd my-addon
```

### 2. Name it in `swofty.json`

Tell the launcher where to find your plugin with `launcher.program`. It maps each OS to a path inside the addon folder, so you'll need to ship one binary per OS you support:

```json [swofty.json]
{
  "schema_version": 1,
  "id": "example.launcher-addon",
  "name": "Example Launcher Addon",
  "version": "0.1.0",
  "author": "Your Name",
  "compat": { "game": "f1_25" },
  "launcher": {
    "program": { "windows": "bin/example-helper.exe", "linux": "bin/example-helper" },
    "sections": [{ "id": "items", "title": "Example", "icon": "package" }],
    "imports": ["exmod"],
    "build": true,
    "commands": [{ "name": "example", "about": "List the example items" }]
  }
}
```

| Field | Type | What it does |
|---|---|---|
| `program` | `{ os: path }` | Where to find your plugin inside the addon folder for each OS. Use at least one of `linux`, `windows`, `macos` as a key. |
| `sections` | `[{ id, title, icon }]` | Each entry adds a sidebar page. The launcher lists these without starting your plugin. |
| `imports` | `[string]` | Extensions your plugin claims when a file is dropped. Leave out the dot. |
| `build` | `bool` | Set to `true` if your plugin answers `build`. |
| `commands` | `[CommandSpec]` | `swofty` subcommands. |
| `claims` | `bool` | Set to `true` to decide yourself which dropped files and folders you import ([Importers](#importers-and-drag-and-drop)). |

### 3. Implement the methods

Your plugin answers the launcher's calls — `hello`, `view`, `event`, `import`, `build`, `command`, `summary`. While handling one, it can make host calls back to the launcher. You'll find every message in [The protocol](#the-protocol); for a working implementation, read `templates/launcher-addon/src/main.rs`, which covers all of them. Its **Overview** section uses every [dashboard block](#dashboard-blocks), a [form](#forms), an editable [table](#tables), `tick` and `invalidate`.

### 4. Build the binary

```sh
cargo build --release
mkdir -p bin
cp target/release/example-helper bin/
```

For Windows, build `example-helper.exe` and copy it to `bin/`. If you support players on both platforms, ship both binaries.

### 5. Install and open it

```sh
swofty addons install .
swofty addons enable example.launcher-addon
```

Enable the addon in a modded profile and you'll see its `Example` section in the sidebar, under the addon's name. Open that section to start the plugin and call `view`.

## How the launcher runs the plugin

Your plugin starts when the launcher first needs it: when the launcher starts with one of its sections in the sidebar, or when someone drops a file routed to it, builds a profile that enables it, or runs one of its commands. It keeps running until the launcher exits or the addon is reinstalled. The launcher picks `program[<os>]` for the host OS (`linux`, `windows`, `macos`). If you haven't supplied that entry, the plugin fails with `has no helper for <os>`.

- The working directory is the addon folder, `$DATA/addons/<id>/`.
- stderr is redirected to `$DATA/cache/addons/<id>/helper.log`.
- On Windows the process gets no console window.
- The launcher asks for a section's `view` at start, when the section opens, after a change, and after an [`invalidate`](#live-pages). It keeps the last view and shows it at once while it asks for a new one. While another call is running, such as an `import` or a `build`, it shows the last view and asks again afterwards.
- It asks for a `summary` only when the plugin is already running and idle; a summary never starts the plugin. While the plugin is busy, the profile card keeps its last summary.
- The plugin exits when stdin closes.

## The protocol {#the-protocol}

Send each message as one JSON object on one line, encoded as UTF-8. There are three message types:

- A **request** has `id`, `method` and `params`.
- A **reply** uses the request's `id` to match it with either a `result` or an `error` (a string).
- A **notification** has `method` and `params`, but no `id`.

The launcher sends one request at a time. You can make host calls back to it before replying, as the session below shows.

Every session starts with `hello`:

```text
launcher  {"id":0,"method":"hello","params":{"api":1,"addon":"example.launcher-addon","data_dir":"/home/you/.local/share/swofty/addon-data/example.launcher-addon","cache_dir":"/home/you/.local/share/swofty/cache/addons/example.launcher-addon","profile":{"name":"default","vanilla":false,"enabled":true}}}
plugin    {"id":0,"result":{"api":1}}
launcher  {"id":1,"method":"view","params":{"section":"items"}}
plugin    {"method":"progress","params":{"done":0.0,"text":"Loading"}}
plugin    {"id":2,"method":"get_state"}
launcher  {"id":2,"result":{"on":["okayama.exmod"]}}
plugin    {"id":1,"result":{"subtitle":"1 item, 1 on","body":{"list":{"rows":[],"reorder":false}}}}
```

`hello.params.api` is `1`, and your plugin must reply `{"api":1}`. If you answer `hello` with another number, the launcher stops the plugin.

### Calls the launcher sends

| `method` | `params` | `result` |
|---|---|---|
| `hello` | `{ api, addon, data_dir, cache_dir, profile }` | `{ "api": 1 }` |
| `profile` *(notification)* | `{ name, vanilla, enabled }` | — |
| `view` | `{ section }` | a [View](#views) |
| `event` | `{ section, event }` | an [Effect](#events-and-effects) |
| `import` | `{ paths }` | `[string]` — lines to show |
| `build` | `{ options, features }` | `{ ops, items, lines, warnings, fields }` ([build reply](#the-build-reply)) |
| `command` | `{ path, args }` | a string to print |
| `summary` | — | `{ label, items }` or `null` |
| `tick` | `{ section }` | `null` ([Live pages](#live-pages)) |

From `hello.params`, you get your addon's id, its two directories, the active profile and the launcher's version:

```json
{ "api": 1, "addon": "example.launcher-addon",
  "data_dir": "…/addon-data/example.launcher-addon",
  "cache_dir": "…/cache/addons/example.launcher-addon",
  "profile": { "name": "default", "vanilla": false, "enabled": true },
  "launcher": "1.0.10" }
```

`launcher` is missing before 1.0.10. Everything this page marks *1.0.10* needs that launcher: set `"compat": { "launcher": ">=1.0.10" }` in `swofty.json` when you use it. An older launcher fails a view that holds a block it doesn't know.

A launcher that answers the [addon host calls](#managing-addons) also lists them in `host_calls`: `["addons", "install_addon", "set_addon_enabled", "remove_addon"]`. An older launcher leaves `host_calls` out.

- Every profile shares `data_dir`. Use `cache_dir` for anything your plugin caches; it also holds the plugin's `helper.log`.
- Check `profile.enabled` to see whether your addon is on in the active profile. `profile.vanilla` identifies the untouched profile: it takes no addons and refuses state writes.
- If the active profile has changed since the last call, you'll receive a `profile` notification before the next request:

```json
{ "method": "profile", "params": { "name": "wet-race", "vanilla": false, "enabled": true } }
```

When the launcher asks for a `view`, it names the open section:

```json
{ "method": "view", "params": { "section": "items" } }
```

For `event`, you get the section and the [event](#events-and-effects) its control fired:

```json
{ "method": "event", "params": { "section": "items", "event": { "toggle": { "id": "okayama.exmod", "on": true } } } }
```

Handle the absolute paths in `import` and return a line per result:

```json
{ "method": "import", "params": { "paths": ["/home/you/Downloads/okayama.exmod"] } }
```

To prepare your `build` reply, you get the profile's value for every option your addon declares, along with the installed loader's runtime [features](#the-build-reply):

```json
{ "method": "build", "params": { "options": { "size": 24 }, "features": { "stable": ["file"], "experimental": [] } } }
```

Use the path and arguments in `command` to identify what to run. The path starts with the top-level name:

```json
{ "method": "command", "params": { "path": ["example"], "args": [] } }
```

`summary` has no params.

### Host calls the plugin sends back

While handling a request, you can call back to the launcher. Include an `id` when you expect a reply; send `progress` without an `id`.

| `method` | `params` | `result` |
|---|---|---|
| `progress` *(notification)* | `{ done, text }` | — |
| `get_state` | — | the addon's per-profile state, or `null` |
| `set_state` | the new state value | `null`, or an error on the vanilla profile |
| `game_build_key` | — | the game build's key, or `null` |
| `game_exe_version` | — | the `F1_25.exe` version, or `null` |
| `game_is_stock` | `{ rel }` | `true` or `false` |
| `game_original` | `{ rel }` | a path to a clean copy of the file |
| `game_original_lang` | `{ code }` | a path to the stock `language_<code>.lng` |
| `game_dir` *(1.0.10)* | — | the game folder, or `null` |
| `addon_game_dir` *(1.0.10)* | — | `<game>/swofty/addons/<id>/` for your addon, or `null` |
| `game_status` *(1.0.10)* | — | `{ running, pid, since, session }` |
| `steam_save_dir` *(1.0.10)* | — | `<steam>/userdata/<steamid3>/3059520/remote/`, or `null` |
| `profile_tracks` *(1.0.12)* | — | the tracks in the active profile's last build, or `null` ([Profile tracks](#profile-tracks)) |
| `addons` | — | the installed addons ([Managing addons](#managing-addons)) |
| `install_addon` | `{ path }` | the addon it installed |
| `set_addon_enabled` | `{ id, on }` | `null`, or an error on the vanilla profile |
| `remove_addon` | `{ id }` | `null` |
| `invalidate` *(notification, 1.0.10)* | `{ section }` | — ([Live pages](#live-pages)) |

```json
{ "method": "progress", "params": { "done": 0.5, "text": "Importing okayama.exmod" } }
{ "id": 2, "method": "get_state" }
{ "id": 3, "method": "set_state", "params": { "on": ["okayama.exmod"] } }
{ "id": 4, "method": "game_build_key" }
{ "id": 5, "method": "game_original", "params": { "rel": "ui/tool_tips.erp" } }
{ "id": 6, "method": "addon_game_dir" }
{ "id": 7, "method": "game_status" }
```

- Send `progress` to fill a progress card, with `done` from `0.0` to `1.0`. Including an `id` is an error.
- Use `get_state` to read your addon's entry in the active profile's `state`; you'll get `null` if the profile has none. `set_state` replaces the whole value, whose contents the launcher never reads. Duplicating or renaming a profile copies its state.
- Use the five `game_*` calls when you need the game's own clean files, rather than the modded folder. If no game is found, `game_build_key`, `game_exe_version` and `game_is_stock` return `null` or `false`. `game_original` and `game_original_lang` return `error: "the game is not available"` instead. A build key looks like `25251167-d873a1c8d204`; an exe version looks like `1.0.141.7865`.
- `game_dir` is the folder the launcher plays: the Steam install, or the one picked with **Locate F1 25**. It is `null` when no game is found.
- `addon_game_dir` is your addon's own folder inside the game, the one a native plugin reads and writes with the [storage library](./native-plugins). The launcher doesn't create it; it may not exist until a session has run the addon, so create it yourself before you write to it. Restoring the game folder removes it, so keep anything that must outlast a session in `data_dir`.
- `steam_save_dir` is the game's Steam Cloud save folder for the most recent Steam user (from Steam's `loginusers.vdf`, else the user folder changed last). It is `null` when that folder doesn't exist. The saves are encrypted per Steam account, so a backup restores only on the same account.
- `game_status` reports the running game:

```json
{ "running": true, "pid": 48211, "since": 1760941234.5, "session": "2026-10-20T14-22-33" }
```

| Field | Value |
|---|---|
| `running` | `true` while `F1_25.exe` runs. The launcher's Play button uses the same check. |
| `pid` | The game's process id, or `null` when it isn't running. |
| `since` | When the process started, in Unix seconds, or `null` when it isn't running or the OS doesn't say (Windows). |
| `session` | The Swofty session that started this run: its folder name under `$DATA/sessions/`. A session exists when the game started through Swofty, from **Play** or from Steam with the `swofty run` wrapper. It is `null` when the game isn't running or started without Swofty. |

#### Profile tracks {#profile-tracks}

`profile_tracks` reads the game database in the active profile's last build, or the game's own database before the profile's first build. It is `null` when no game is found.

```json
{ "built": true, "buildhash": "5d5224af722f2bfc", "tracks": [
  { "code": "suzuka", "stat_id": 13, "events": ["rw_suzuka", "rw_suzuka_26"], "playable": true, "modded": false, "name": null, "addon": null },
  { "code": "okayama", "stat_id": 43, "events": [], "playable": true, "modded": true, "name": "Okayama", "addon": "teamsimplified.mods" }
] }
```

| Field | Value |
|---|---|
| `built` | `false` before the profile's first build: the tracks are the stock ones. |
| `buildhash` | The build the tracks come from, or `null`. |
| `code` | The `tracks.Track` row's label. |
| `stat_id` | Its `statId`, the track id the game stores. |
| `events` | Career events (`schedule.all_track_events`) whose race-weekend activity names this track. |
| `playable` | `true` when the `2025` or `2026` content set lists the track for Grand Prix. |
| `modded` | `true` when a track package in this build added or filled the row. |
| `name`, `addon` | The package's `track_name` and the addon that added it, for a modded track. |

#### Managing addons {#managing-addons}

The four addon calls let a plugin install, list, turn on and off, and remove other addons, such as a page that gathers the data addons that add a track. Send them only when `hello.params.host_calls` lists them: a launcher that doesn't know a host call stops the plugin. A Rust helper gets them from `Cx::addons()`, which is `None` on such a launcher.

`addons` lists every installed addon, with its folder and whether it is on in the active profile:

```json
[ { "id": "yourname.okayama-named", "name": "Okayama with its own name", "version": "1.0.0", "author": "Your Name",
    "dir": "/home/you/.local/share/swofty/addons/yourname.okayama-named", "enabled": true } ]
```

- `install_addon` installs the addon at `path` as a drop on the window does: a `.swoftyaddon`, a zip that holds an addon, or a folder with `swofty.json`. It replaces an installed copy with the same id and returns the addon in the shape above. It doesn't turn the addon on.
- `set_addon_enabled` turns an addon on or off in the active profile. It refuses to turn off an addon that is always on, or one that [keeps data in the saves](./manifest#saves), since only the **Addons** page warns about that.
- `remove_addon` deletes the addon and takes it out of every profile, as **Remove** on the **Addons** page does. A plugin can't remove its own addon, a built-in one, one the launcher manages or one that keeps data in the saves.

The template uses state to track which items are on, reading it here and writing it when the selection changes:

```rust [src/main.rs]
fn enabled(&self, host: &mut Host) -> Result<Vec<String>, String> {
    let state = host.call("get_state", Value::Null)?;
    Ok(serde_json::from_value(state["on"].clone()).unwrap_or_default())
}
```

## Views {#views}

Return one page from `view`, describing its header, content and footer:

```json
{
  "subtitle": "1 item, 1 on",
  "actions": [ { "id": "pick", "label": "Import…", "icon": "upload",
    "kind": { "pick": { "title": "Import example items", "extensions": ["exmod"], "folders": false, "multiple": true } } } ],
  "notices": [ { "text": "Team Simplified is off in this profile.", "tone": "warn",
    "action": { "id": "enable", "label": "Turn on" } } ],
  "body": { "list": { "rows": [], "reorder": false } },
  "footer": "By Team Simplified."
}
```

| Field | Renders as |
|---|---|
| `subtitle` | The page header's line. A list body also shows its row count. |
| `actions` | Buttons in the header. |
| `notices` | Bars under the header, each with an optional button. `tone` colours the bar. |
| `body` | One of the three bodies below. |
| `footer` | A muted line under the body. |
| `tick` *(1.0.10)* | `true` asks for a `tick` about once a second while the page is open ([Live pages](#live-pages)). Default `false`. |

Choose one of these shapes for `body`:

```json
{ "list": { "rows": [ … ], "reorder": false, "search": { "placeholder": "Search mods" } } }
{ "empty": { "title": "No example items", "text": "Drop a .exmod file on the launcher, or import one.", "action": { "id": "pick", "label": "Import…" } } }
{ "blocks": [ … ] }
```

| Body | Shows |
|---|---|
| `list` | Rows with art, tags, a trailing control and an expandable `detail`. A `reorder` list adds **Move up** / **Move down** to each expanded row, firing a `move` event. An optional `search` *(1.0.10)* puts a search box above the rows ([Search](#search)). |
| `empty` | A title, a line and one button. |
| `blocks` | The blocks below, one after another. |

For each **row** in a list, supply its content and controls:

```json
{ "id": "okayama.exmod", "title": "Okayama", "subtitle": "41 kB",
  "art": { "monogram": "O" },
  "tags": [ { "text": "Track", "tone": "muted" } ],
  "trailing": { "switch": { "id": "okayama.exmod", "on": true, "enabled": true } },
  "detail": [ { "stats": [ { "label": "Size", "value": "41 kB" } ] } ] }
```

You must supply `id`, `title` and `subtitle`. Leave out `art`, `tags`, `trailing` or `detail` and they default to empty. For `art`, use `"none"`, `{ "monogram": "O" }` or `{ "image": "art/okayama.png" }`.

To give a row a **trailing control**, choose one of these:

```json
{ "switch": { "id": "okayama.exmod", "on": true, "enabled": true } }
{ "choice": { "id": "livery", "options": ["Title sponsor", "Tier 1", "Replica"], "current": 0 } }
{ "button": { "id": "remove", "label": "Remove", "style": "danger" } }
```

| Control | Fires |
|---|---|
| `switch` | `{ "toggle": { "id", "on" } }` |
| `choice` | `{ "choose": { "id", "index" } }` |
| `button` | depends on the button's `kind` |

Use these **blocks** to fill a row's `detail` or a `blocks` body:

| Block | JSON |
|---|---|
| Large label/value pairs | `{ "stats": [ { "label", "value" } ] }` |
| Small label/value pairs | `{ "facts": [ { "label", "value" } ] }` |
| A section heading | `{ "label": "WHAT IT CHANGES" }` |
| A line in a tone | `{ "text": { "text": "…", "tone": "warn" } }` |
| A heading and a name list | `{ "names": { "label": "Mods", "items": ["A", "B"] } }` |
| A table | `{ "table": { "columns": ["File", "Op"], "rows": [["a.erp", "patch"]] } }`, with sorting, search and editable cells from 1.0.10 ([Tables](#tables)) |
| A row of buttons | `{ "buttons": [ { "id", "label" } ] }` |
| A page header with status *(1.0.10)* | `{ "hero": { "title", "subtitle", "pills", "facts", "buttons" } }` ([Dashboard blocks](#dashboard-blocks)) |
| Status pills *(1.0.10)* | `{ "pills": [ { "text": "LIVE", "tone": "good", "dot": true } ] }` |
| Stat tiles *(1.0.10)* | `{ "tiles": [ { "label", "value", "delta", "hint" } ] }` |
| Progress bars *(1.0.10)* | `{ "meters": [ { "label", "value", "max", "text" } ] }` |
| A card holding blocks *(1.0.10)* | `{ "card": { "title", "blocks": [ … ] } }` |
| Blocks side by side *(1.0.10)* | `{ "grid": { "columns": 2, "blocks": [ … ] } }` |
| A form *(1.0.10)* | `{ "form": { "id", "fields": [ … ] } }` ([Forms](#forms)) |
| A tab bar *(1.0.10)* | `{ "tabs": { "id", "items": [ { "id", "label" } ], "active" } }` ([Tabs](#tabs)) |

Give each **button** an `id` and `label`, with an optional `icon`. Its `style` is `primary`, `outline`, `ghost` or `danger` (default `outline`); its `kind` determines what happens when you click:

```json
{ "id": "import", "label": "Import…", "kind": { "pick": { "title": "Import items", "extensions": ["exmod"], "folders": false, "multiple": true } } }
```

| `kind` | On click |
|---|---|
| omitted (default) | fires `{ "click": { "id" } }` |
| `{ "confirm": { "title", "text" } }` | asks in a dialog, then fires `{ "click": { "id" } }` |
| `{ "pick": { "title", "extensions", "folders", "multiple" } }` | opens the file picker, then fires `{ "picked": { "id", "paths" } }` |

`tone` (`neutral`, `muted`, `accent`, `good`, `warn`, `bad`) colours tags, notices and text. `icon` is one of a fixed set: `activity`, `arrow-right-left`, `chevron-down`, `chevron-up`, `circle-alert`, `circle-check`, `copy`, `eraser`, `folder-open`, `gamepad`, `layers`, `lock`, `map`, `package`, `pencil`, `play`, `plus`, `puzzle`, `settings`, `shield-check`, `trash`, `upload`. From 1.0.10 it can also be `calendar`, `car`, `clock`, `cloud-rain`, `flag`, `gauge`, `history`, `info`, `refresh`, `save`, `search`, `timer`, `trending-down`, `trending-up`, `triangle-alert`, `trophy`, `undo`, `user`, `users`, `wallet`, `wrench` or `zap`.

### Events and effects {#events-and-effects}

When you handle `event`, you receive one event and return one effect to tell the launcher what to do next.

| Event | JSON |
|---|---|
| A button or ghost row click | `{ "click": { "id": "remove" } }` |
| A switch flip | `{ "toggle": { "id": "okayama.exmod", "on": true } }` |
| A choice pick | `{ "choose": { "id": "livery", "index": 2 } }` |
| A row reorder | `{ "move": { "row": "okayama.exmod", "to": 0 } }` |
| A file-picker result | `{ "picked": { "id": "pick", "paths": ["/home/you/okayama.exmod"] } }` |
| A form's Apply, a live field or an edited table cell *(1.0.10)* | `{ "submit": { "form": "finance", "values": { "cash": 15000000 }, "changed": ["cash"] } }`, plus `"row"` for a table cell ([Forms](#forms), [Tables](#tables)) |
| A search box, when it asks for events *(1.0.10)* | `{ "search": { "id": "drivers", "query": "ver" } }` ([Search](#search)) |
| A tab click *(1.0.10)* | `{ "tab": { "id": "page", "tab": "drivers" } }` ([Tabs](#tabs)) |

| Effect | The launcher |
|---|---|
| `"none"` | does nothing |
| `"reload"` | asks every section for its view again |
| `{ "notice": "Imported okayama.exmod" }` | shows a notification and reloads |
| `{ "set_addon_enabled": true }` | turns the addon on or off in the active profile |
| `{ "open": "/home/you/.local/share/swofty/addon-data/example.launcher-addon/items" }` | opens the file or folder with the system handler |

Answer `view` and `summary` within 5 seconds. For `event`, `import`, `build` and `command`, you have 300 seconds; each message your plugin sends resets the wait. Until you return an effect, the control that fired the event shows busy.

```rust [src/main.rs]
if let Some(toggle) = event.get("toggle") {
    let name = toggle["id"].as_str().unwrap_or_default().to_owned();
    let mut on = self.enabled(host)?;
    on.retain(|n| *n != name);
    if toggle["on"].as_bool() == Some(true) { on.push(name); }
    host.call("set_state", json!({ "on": on }))?;
    return Ok(json!("reload"));
}
```

## Dashboard blocks {#dashboard-blocks}

*Launcher 1.0.10.* These blocks go wherever a block goes: in a `blocks` body, a row's `detail`, a `card` or a `grid`. Every field but the ones marked required can be left out.

```json
{ "blocks": [
  { "hero": {
      "title": "Scuderia Swofty",
      "subtitle": "My Team · 2026 season · Round 7 of 24",
      "art": { "monogram": "S" },
      "pills": [ { "text": "LIVE", "tone": "good", "dot": true }, { "text": "In Team HQ", "tone": "accent" } ],
      "facts": [ { "label": "Next race", "value": "Monaco" }, { "label": "Last save", "value": "2 min ago" } ],
      "buttons": [ { "id": "backup", "label": "Back up now", "icon": "save" } ] } },
  { "tiles": [
      { "label": "Cash", "value": "$12.4M", "delta": "+$2.0M", "hint": "since Bahrain" },
      { "label": "Resource points", "value": "340", "hint": "of 500", "progress": 0.68, "tone": "accent" },
      { "label": "Constructors", "value": "P4", "delta": "-1", "delta_tone": "bad", "icon": "trophy" } ] },
  { "grid": { "columns": [2, 1], "blocks": [
      { "card": { "title": "Facilities", "blocks": [
          { "meters": [ { "label": "Wind tunnel", "value": 3, "max": 4, "text": "Phase 3 of 4", "tone": "good" } ] } ] } },
      { "card": { "title": "Session", "pills": [ { "text": "Offline", "tone": "muted" } ], "blocks": [
          { "facts": [ { "label": "Mode", "value": "My Team" } ] } ] } } ] } }
] }
```

**`hero`**: a header for the page's subject, on its own surface.

| Field | Shows |
|---|---|
| `title` *(required)* | The large name. |
| `subtitle` | A muted line under the title. |
| `art` | `"none"` (default), `{ "monogram": "S" }` or `{ "image": "art/team.png" }`, at the left. |
| `pills` | [Pills](#pills) beside the title. |
| `facts` | `{ label, value }` pairs in a row along the bottom. |
| `buttons` | [Buttons](#views) at the right. |

**`pills`** {#pills}: a row of status badges. Each pill is `{ text, tone, dot, icon }`: `text` is required, `tone` colours it (default `neutral`), `dot: true` puts a status dot before the text, and `icon` puts an [icon](#views) there instead.

**`tiles`**: stat tiles in equal columns: up to 4 in one row, 5 or 6 in rows of 3, more in rows of 4. A narrow page uses fewer columns.

| Field | Shows |
|---|---|
| `label` *(required)* | A small heading. |
| `value` *(required)* | The large figure, as text. |
| `tone` | The figure's colour (default `neutral`). |
| `delta` | A change beside the figure, such as `+$2.0M`. |
| `delta_tone` | The delta's colour. Default: `good` when it starts with `+`, `bad` when it starts with `-` or `−`, else `muted`. |
| `hint` | A muted line under the figure. |
| `icon` | An [icon](#views) in the corner. |
| `progress` | `0.0` to `1.0`: a bar along the foot of the tile, in `tone` (`accent` when neutral). |

**`meters`**: labelled progress bars, one under another. Each is `{ label, value, min, max, text, tone }`: `value` is required, `min` defaults to `0` and `max` to `100`, `text` is shown at the right (default `<value> / <max>`), and `tone` colours the bar (default `accent`). The bar is clamped to the range.

**`card`**: a bordered surface that holds blocks under an optional header. `{ title, subtitle, pills, buttons, blocks }`; `blocks` defaults to empty. A table in a card loses its own border.

**`grid`**: blocks side by side. `{ columns, blocks }`: `columns` is a count from 1 to 4 for equal columns (default 2), or a list of relative widths such as `[2, 1]`. The blocks fill the columns left to right, then the next row; blocks in a row share its height. On a page too narrow for about 280 px per column, the grid becomes one column.

**`image`** *(1.0.12)*: a picture from your plugin's `cache_dir` or `data_dir`. `{ path, width, height, caption, fit }`: `path` (absolute), `width` and `height` (in pixels) are required. `width` shrinks to the space the block is in. `fit` is `cover` (default), which fills the box and crops, or `contain`, which shows the whole picture inside the box. `caption` is a muted line under it. A path outside those two folders shows an empty box. An older launcher fails the view.

```json
{ "image": { "path": "/home/you/.local/share/swofty/cache/addons/example.launcher-addon/helmets/helmet_042-91c0.png", "width": 240, "height": 135, "caption": "Carbon stripe", "fit": "contain" } }
```

## Tabs {#tabs}

*Launcher 1.0.10.* A `tabs` block draws a row of tabs, so one sidebar page can hold several views:

```json
{ "tabs": {
  "id": "page",
  "items": [
    { "id": "overview", "label": "Overview", "icon": "gauge" },
    { "id": "drivers", "label": "Drivers", "icon": "users" },
    { "id": "history", "label": "History", "icon": "history", "badge": "2" },
    { "id": "weather", "label": "Weather", "icon": "cloud-rain", "disabled": true }
  ],
  "active": "overview"
} }
```

| Field | Does |
|---|---|
| `id` *(required)* | Names the tab bar in its events. |
| `items` *(required)* | The tabs, left to right: `{ id, label, icon, badge, disabled }`. `id` and `label` are required; `badge` is a short count or word shown after the label; a `disabled` tab is greyed out and can't be clicked. |
| `active` | The highlighted tab's `id` (default: the first). |

Clicking a tab that isn't active fires `{ "tab": { "id": "page", "tab": "drivers" } }`. The launcher doesn't switch anything itself: keep the active tab in your plugin, return `"reload"`, and build the next view with that tab's blocks under the bar. Forms and searches keep their state per form `id`, so give forms on different tabs different ids.

## Forms {#forms}

*Launcher 1.0.10.* A `form` block holds input fields and an **Apply** button:

```json
{ "form": {
  "id": "finance",
  "columns": 2,
  "fields": [
    { "number": { "id": "cash", "label": "Cash", "value": 12400000, "min": 0, "max": 2147483647, "step": 100000, "unit": "$", "help": "Posted as a bank transaction." } },
    { "slider": { "id": "rp", "label": "Resource points", "value": 340, "min": 0, "max": 500, "step": 10, "unit": "RP" } },
    { "text": { "id": "name", "label": "Team name", "value": "Swofty Racing", "placeholder": "Team name", "max_len": 32 } },
    { "select": { "id": "weather", "label": "Weather", "value": "dry", "options": [ { "value": "dry", "label": "Dry" }, { "value": "wet", "label": "Heavy rain" } ] } },
    { "toggle": { "id": "lock", "label": "Lock budget", "value": false } }
  ],
  "apply": "Apply",
  "live": false,
  "disabled": false
} }
```

| Form field | Does |
|---|---|
| `id` *(required)* | Names the form in its `submit` events. |
| `fields` *(required)* | The fields, in order. |
| `columns` | Lays the fields out in 1 to 3 columns (default 1). |
| `apply` | The Apply button's label (default `Apply`). |
| `live` | `true` drops the Apply button: each field submits on its own as soon as it's committed. Default `false`. |
| `disabled` | `true` greys out every field and the button, such as while the game can't take edits. Default `false`. |

Every field takes `id` *(required: its key in `values`)*, `label`, `help` (a muted line under it) and `disabled`, plus its own keys:

| Field | Own keys | Sends |
|---|---|---|
| `number` | `value`, `min`, `max`, `step` (default `1`), `unit` (shown after the number) | a number |
| `slider` | `value`, `min` (default `0`), `max` (default `100`), `step` (default `1`), `unit` | a number |
| `text` | `value`, `placeholder`, `max_len` | a string |
| `select` | `value`, `options`: strings, `{ value, label }` objects (with an optional `image` *(1.0.12)*), or `{ heading }` group titles *(1.0.12)*; `search` *(1.0.12)* | the chosen option's `value` (a string option is its own value) |
| `toggle` | `value` | `true` or `false` |

A `{ "heading": "Modded tracks" }` option starts a group in the select's menu: its title shows above the options that follow it, with a line above it unless it's the first group, and it can't be picked. A search that matches a group's title keeps the whole group. An older launcher fails the view, so send it only to 1.0.12 or later.

*Launcher 1.0.12.* A `{ value, label }` option can carry `"image"`: the absolute path of a PNG or JPEG under your plugin's `cache_dir` or `data_dir`. The menu shows it as a thumbnail before the label, and the closed field shows the picked option's thumbnail. When some options have an image, the others get an empty square. A path outside those two folders shows no image. The launcher caches an image by its path, so write changed content under a new file name. An older launcher ignores `image`.

A number is clamped to `min`–`max` and rounded to `step` when it's committed, and is sent as an integer when the step and the value are whole. Up and Down step a focused number field.

A select's list opens below it, at most 320 pixels tall, and scrolls when the options don't fit. Up and Down move through the options, Enter picks one and Escape closes the list. *Launcher 1.0.12* adds `"search": true`, which puts a search box at the top of the list: typing filters the options to those whose label contains the text, ignoring case. The field still shows the current value while you search. An older launcher ignores `search` and shows the full list.

```json
{ "select": { "id": "helmet", "label": "Helmet", "value": "helmet_042", "search": true,
  "options": [
    { "heading": "Team" },
    { "value": "helmet_001", "label": "Classic red", "image": "/home/you/.local/share/swofty/cache/addons/example.launcher-addon/helmets/helmet_001-3fa2.png" },
    { "heading": "Custom" },
    { "value": "helmet_042", "label": "Carbon stripe", "image": "/home/you/.local/share/swofty/cache/addons/example.launcher-addon/helmets/helmet_042-91c0.png" } ] } }
```

How a form behaves:

- Edits stay in the form until you press **Apply**. Apply turns on once a field differs from the view; **Reset** beside it puts the view's values back.
- Apply fires one `submit` event. `values` holds every field; `changed` lists the fields that differ from the view, in field order:

```json
{ "submit": { "form": "finance", "values": { "cash": 15000000, "rp": 340, "name": "Swofty Racing", "weather": "dry", "lock": false }, "changed": ["cash"] } }
```

- In a `live` form, a field is committed by Enter or leaving it (number, text), releasing it (slider), picking an option (select) or flipping it (toggle). Its event holds only that field, in both `values` and `changed`.
- While the event runs, the form shows busy. When the effect comes back, the form drops its edits and shows the values of the next view, so return `"reload"` or a notice to show what the edit did. If the event fails, the edits stay and the error shows as a notification.
- A new view, such as one after an `invalidate`, updates every field you haven't edited and leaves edited fields alone.

## Tables {#tables}

*Launcher 1.0.10* extends `table`. The old shape, plain column names and rows of strings, still works and looks as before. Any of the keys below turns on the new table, which fills the width it's given:

```json
{ "table": {
  "id": "drivers",
  "columns": [
    "Driver",
    { "key": "team", "title": "Team" },
    { "key": "pace", "title": "Pace", "align": "right", "width": 96, "sort": true }
  ],
  "rows": [
    { "id": "ver", "cells": [
      "Max Verstappen",
      { "text": "Red Bull", "tone": "muted" },
      { "text": "95", "bar": 0.95, "edit": { "number": { "min": 0, "max": 100 } } } ] },
    ["Lando Norris", "McLaren", "93"]
  ],
  "sort": { "column": "pace", "desc": true },
  "search": { "placeholder": "Search drivers" },
  "compact": true,
  "empty": "No drivers match"
} }
```

| Table field | Does |
|---|---|
| `id` | Names the table in `submit` and `search` events (default `table`). |
| `columns` *(required)* | Column names, or column objects. |
| `rows` *(required)* | Rows: a list of cells, or `{ id, cells, tone }`. |
| `sort` | The starting sort: `{ column, desc }`, with `column` a column's key. |
| `search` | A search box above the table ([Search](#search)). |
| `compact` | `true` for shorter rows and smaller text. |
| `empty` | The line shown when no row is left after a search (default `No matches`). |

A **column** is a name, or `{ key, title, align, width, sort }`: `key` names the column in events and in `sort` (default: the title), `align` is `left` (default), `right` or `center`, `width` fixes its width in pixels (other columns share the rest), and `sort: true` makes the header clickable. A click sorts by that column, a second click reverses it. Sorting is numeric when both cells are numbers (commas allowed) and by text otherwise; give cells a `sort` key for formatted values such as `$12.4M`.

A **row** is a list of cells, or `{ id, cells, tone }`: `id` names the row in events (default: its position in `rows`, from `"0"`), and `tone` colours its text. A **cell** is a string, or:

| Cell field | Does |
|---|---|
| `text` | The text shown. |
| `tone` | Its colour. |
| `sort` | A number or string to sort by instead of `text`. |
| `pill` | `true` draws the text as a [pill](#pills) in `tone`. |
| `bar` | `0.0` to `1.0`: a small bar before the text, in `tone` (`accent` when neutral). |
| `edit` | Makes the cell editable: `{ "number": { value, min, max, step, unit } }` or `{ "text": { value, placeholder, max_len } }`. `value` defaults to the cell's text. |

Click an editable cell to edit it. Enter or leaving the cell commits it, and Escape cancels. A committed value that differs fires:

```json
{ "submit": { "form": "drivers", "row": "ver", "values": { "pace": 96 }, "changed": ["pace"] } }
```

The cell shows busy until the effect comes back, as a form does.

### Search {#search}

A `list` body or a `table` takes `search`, which puts a search box above its rows:

```json
{ "search": { "id": "drivers", "placeholder": "Search drivers", "event": false, "filter": true } }
```

| Field | Does |
|---|---|
| `id` | Names the box in its events. Default: the table's `id`, or `list`. |
| `placeholder` | The hint in the empty box (default `Search`). |
| `filter` | `true` (default): the launcher hides rows that don't match. A row matches when every word of the query appears in it, ignoring case: in a list row's title, subtitle and tags, or in a table row's cells. |
| `event` | `true` sends `{ "search": { "id", "query" } }` once typing pauses for 300 ms, and when the box is cleared. Default `false`. Set `filter` to `false` when you filter the rows yourself. |

The query stays in the box when the view refreshes, until the page is left.

## Live pages {#live-pages}

*Launcher 1.0.10.* A page can follow something that changes on its own, such as a file the game writes, in two ways: the launcher **ticks** it, and the plugin **invalidates** it.

**`tick`**: return `"tick": true` in a view and, while that page is open, the launcher sends a `tick` request about once a second. Reply `null`; the launcher ignores the result.

```text
launcher  {"id":12,"method":"tick","params":{"section":"overview"}}
plugin    {"method":"invalidate","params":{"section":"overview"}}
plugin    {"id":12,"result":null}
launcher  {"id":13,"method":"view","params":{"section":"overview"}}
```

- A tick has the 5-second budget of `view`. Keep it cheap: check a file's modified time and return.
- A tick is skipped, not queued, while another call runs or the page isn't on screen.
- A tick may make host calls, such as `game_status`, like any other call.

**`invalidate`**: a notification that tells the launcher a section's view is out of date. Send it whenever you know, whether during a call (in reply to a tick, or after an event) or on your own between calls, such as from a thread that watches a file. Write each message as one whole line.

```json
{ "method": "invalidate", "params": { "section": "overview" } }
{ "method": "invalidate", "params": {} }
```

- With `section`, the launcher asks for that section's view again; without one, for every section of your plugin.
- It asks only for sections that are open or that it has already loaded, and only once for several invalidations that arrive together. While another call runs, it asks once that call ends.
- An `invalidate` has no `id` and gets no reply.

## The build reply {#the-build-reply}

Return your addon's asset ops from `build`, along with lines and warnings:

```json
{
  "ops": [ { "op": "file.put", "source": { "root": "cache", "path": "enabled.txt" }, "target": "example-launcher-addon/enabled.txt" } ],
  "lines": ["1 item on"],
  "warnings": []
}
```

*Launcher 1.0.12* adds `fields`, the field names a `db.add-row` copies, in the same shape as an [op file's `fields`](/reference/ops/database): an object keyed by `<table>.<row>`, each holding a list of field names:

```json
{
  "ops": [ { "op": "db.add-row", "table": "entrants.teams",
             "from": "entrants.teams.team[_metadata_=mclaren_26]",
             "set": { "_metadata_": "team12_26" } } ],
  "fields": { "entrants.teams.team": ["_metadata_", "stat_id", "display_name_code"] }
}
```

- Write each op exactly as you would in an [op file](./asset-api#op-files): set the `op` field to `<kind>.<name>`, such as `file.put`, `db.set`, `erp.patch-pkg`, `lang.set` or `warn`.
- For a `source` inside the addon folder, use a relative path (`"art/okayama.dds"`). Use `{ "root": "data" | "cache", "path": "…" }` to read from `data_dir` or `cache_dir`.
- The launcher tags every op with this addon's id as its origin, routes them like declared ops, and reports conflicts by origin ([Asset API](./asset-api#deliveries)).
- If your addon loads several items, such as the mods in a library, you can add `items`: the item each op comes from, by position in `ops`, with `null` for an op that belongs to no item. The launcher adds the item to the op's origin, so each item gets its own runtime overlay archive and conflicts name it. Leave `items` out, or shorter than `ops`, and the remaining ops have no item.
- Check `features.stable` and `features.experimental` before returning ops. They list the op `kind`s the installed loader supports: `group`, `erp`, `db`, `lang`, `file`. Drop any ops it cannot apply.
- The build calls `build` for each enabled addon in load order, after that addon's declared ops. It prints `lines` and `warnings` with the addon id as a prefix.
- `fields` applies to every `db.add-row` of your addon, from the reply and from your op files. An op file's own `fields` takes precedence for the tables it lists. An `add-row` whose table is in neither fails with `field names unknown`. A launcher before 1.0.12 ignores `fields`; set `"compat": { "launcher": ">=1.0.12" }` when you return `add-row` ops.

```rust [src/main.rs]
fn build(&self, host: &mut Host) -> Result<Value, String> {
    let on = self.enabled(host)?;
    let file = self.cache_dir.join("enabled.txt");
    std::fs::write(&file, on.join("\n")).map_err(|e| e.to_string())?;
    Ok(json!({
        "ops": [{ "op": "file.put",
            "source": { "root": "cache", "path": "enabled.txt" },
            "target": "example-launcher-addon/enabled.txt" }],
        "lines": [format!("{} items on", on.len())]
    }))
}
```

## Importers and drag-and-drop

List your file extensions in `imports` to claim dropped files that match. The launcher routes each drop in this order:

1. The launcher claims addons first: a folder with `swofty.json`, a `.swoftyaddon` file (or a legacy `.swoftymod`), or a zip that holds an addon. Those install as addons.
2. It checks every plugin with `imports` or `claims`, in registry order. A plugin with `imports` claims a file when its extension matches. A plugin with `claims` decides for itself (see below).
3. A claimed file goes to that plugin's `import`. An unclaimed one is reported: `No addon imports <file>`.

Matching extensions can't tell a mod's zip from any other zip, and can't claim a folder. To decide for yourself, set `claims` to `true`. The launcher then runs your program on its own for each drop, as `<program> claims <path>`, and reads one line: `yes`, `maybe` or `no`. A `maybe` wins only when no other plugin says `yes` or `maybe`. Answer within 5 seconds, without the protocol: there is no `hello` and no profile. If your program fails, stalls or prints anything else, the launcher falls back to `imports`. A Rust helper that uses `swofty_addon::helper::run_helper` answers from its `Importer::claims` for you.

You'll receive files routed to your plugin together in one `import` call. Return one line per file and report `progress` as you work:

```rust [src/main.rs]
for (i, path) in paths.iter().enumerate() {
    host.progress(i as f32 / paths.len() as f32, &format!("Importing {name}"));
    std::fs::copy(path, self.items_dir().join(name)).map_err(|e| format!("{name}: {e}"))?;
    lines.push(format!("Imported {name}"));
}
```

## Commands

To add a top-level `swofty` subcommand, put an entry in `launcher.commands`:

```json [swofty.json]
"commands": [ { "name": "example", "about": "List the example items",
  "args": [], "subcommands": [] } ]
```

```
CommandSpec { name, about, args: [ArgSpec { name, help, required, many }], subcommands: [CommandSpec] }
```

In `command`, you receive the path of names (`["example"]`, or `["mods", "import"]` for a subcommand) and the arguments. Return the text you want printed. Choose a name that core and earlier addons haven't used, or it will be refused. Commands run against `--profile` (default: the active profile) and the Steam install of the game.

## Profile summary

To add a line to the profile card, return `{ "label": "EXAMPLE", "items": ["Okayama", "Suzuka"] }` from `summary`, or return `null` for no summary. Your line appears after `ADDONS`. Team Simplified uses this to show `MODS` with its enabled mod names.

## Isolation and failures

Because your plugin runs in a subprocess, a crash, a hang or malformed output stops the plugin without taking the launcher down. The launcher marks it failed when it:

- does not start, or has no `program` for this OS;
- does not answer `hello`, `view`, `tick` or `summary` within 5 seconds, or any other call within 300 seconds;
- exits or closes stdout (`stopped (exit status: 3)`);
- writes a line that is not a message, replies to an `id` it was not sent, or returns a result of the wrong shape.

When your plugin fails, its sections show the error in a bad-tone notice with a **Restart** button (id `swofty.restart`). The body is empty and reads `<name> is not running`. Click Restart to clear the failure; the next call starts the plugin again. Reinstalling the addon also stops its plugin.

To fail just one call, reply with `{ "id": …, "error": "…" }`. The launcher shows the error as a notification and keeps your plugin running. A failed `tick` shows nothing; the launcher goes on ticking.

## The internal Addon trait

Inside the launcher, every plugin is driven through the Rust trait `Addon` (`packages/addon-api/src/launcher.rs`). You don't implement this trait in your plugin. The launcher's `HelperAddon` implements `Addon` for you, turning each method into a JSON message using the protocol above. The methods map one to one onto the calls:

| `Addon` method | Call |
|---|---|
| `sections`, `view`, `event` | sidebar pages |
| `importer` → `claims`, `import` | drag-and-drop |
| `build` | `build` |
| `commands`, `command` | `swofty` subcommands |
| `summary` | profile card |

Write a launcher plugin: compiled-in Rust addons that implement `Addon` directly are legacy. Team Simplified is moving to a launcher plugin, and `addons/simplified` is being removed.

Start from [`templates/launcher-addon`](./template).
