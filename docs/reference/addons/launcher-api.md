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

Your plugin answers the launcher's calls — `hello`, `view`, `event`, `import`, `build`, `command`, `summary`. While handling one, it can make host calls back to the launcher. You'll find every message in [The protocol](#the-protocol); for a working implementation, read `templates/launcher-addon/src/main.rs`, which covers all of them in about 230 lines.

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
- The launcher asks for a section's `view` at start, when the section opens, and after a change. It keeps the last view and shows it at once while it asks for a new one. While another call is running, such as an `import` or a `build`, it shows the last view and asks again afterwards.
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
| `build` | `{ options, features }` | `{ ops, items, lines, warnings }` ([build reply](#the-build-reply)) |
| `command` | `{ path, args }` | a string to print |
| `summary` | — | `{ label, items }` or `null` |

From `hello.params`, you get your addon's id, its two directories and the active profile:

```json
{ "api": 1, "addon": "example.launcher-addon",
  "data_dir": "…/addon-data/example.launcher-addon",
  "cache_dir": "…/cache/addons/example.launcher-addon",
  "profile": { "name": "default", "vanilla": false, "enabled": true } }
```

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

```json
{ "method": "progress", "params": { "done": 0.5, "text": "Importing okayama.exmod" } }
{ "id": 2, "method": "get_state" }
{ "id": 3, "method": "set_state", "params": { "on": ["okayama.exmod"] } }
{ "id": 4, "method": "game_build_key" }
{ "id": 5, "method": "game_original", "params": { "rel": "ui/tool_tips.erp" } }
```

- Send `progress` to fill a progress card, with `done` from `0.0` to `1.0`. Including an `id` is an error.
- Use `get_state` to read your addon's entry in the active profile's `state`; you'll get `null` if the profile has none. `set_state` replaces the whole value, whose contents the launcher never reads. Duplicating or renaming a profile copies its state.
- Use the five `game_*` calls when you need the game's own clean files, rather than the modded folder. If no game is found, `game_build_key`, `game_exe_version` and `game_is_stock` return `null` or `false`. `game_original` and `game_original_lang` return `error: "the game is not available"` instead. A build key looks like `25251167-d873a1c8d204`; an exe version looks like `1.0.141.7865`.

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

Choose one of these shapes for `body`:

```json
{ "list": { "rows": [ … ], "reorder": false } }
{ "empty": { "title": "No example items", "text": "Drop a .exmod file on the launcher, or import one.", "action": { "id": "pick", "label": "Import…" } } }
{ "blocks": [ … ] }
```

| Body | Shows |
|---|---|
| `list` | Rows with art, tags, a trailing control and an expandable `detail`. A `reorder` list adds **Move up** / **Move down** to each expanded row, firing a `move` event. |
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
| A table | `{ "table": { "columns": ["File", "Op"], "rows": [["a.erp", "patch"]] } }` |
| A row of buttons | `{ "buttons": [ { "id", "label" } ] }` |

Give each **button** an `id` and `label`, with an optional `icon`. Its `style` is `primary`, `outline`, `ghost` or `danger` (default `outline`); its `kind` determines what happens when you click:

```json
{ "id": "import", "label": "Import…", "kind": { "pick": { "title": "Import items", "extensions": ["exmod"], "folders": false, "multiple": true } } }
```

| `kind` | On click |
|---|---|
| omitted (default) | fires `{ "click": { "id" } }` |
| `{ "confirm": { "title", "text" } }` | asks in a dialog, then fires `{ "click": { "id" } }` |
| `{ "pick": { "title", "extensions", "folders", "multiple" } }` | opens the file picker, then fires `{ "picked": { "id", "paths" } }` |

`tone` (`neutral`, `muted`, `accent`, `good`, `warn`, `bad`) colours tags, notices and text. `icon` is one of a fixed set: `activity`, `arrow-right-left`, `chevron-down`, `chevron-up`, `circle-alert`, `circle-check`, `copy`, `eraser`, `folder-open`, `gamepad`, `layers`, `lock`, `map`, `package`, `pencil`, `play`, `plus`, `puzzle`, `settings`, `shield-check`, `trash`, `upload`.

### Events and effects {#events-and-effects}

When you handle `event`, you receive one event and return one effect to tell the launcher what to do next.

| Event | JSON |
|---|---|
| A button or ghost row click | `{ "click": { "id": "remove" } }` |
| A switch flip | `{ "toggle": { "id": "okayama.exmod", "on": true } }` |
| A choice pick | `{ "choose": { "id": "livery", "index": 2 } }` |
| A row reorder | `{ "move": { "row": "okayama.exmod", "to": 0 } }` |
| A file-picker result | `{ "picked": { "id": "pick", "paths": ["/home/you/okayama.exmod"] } }` |

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

## The build reply {#the-build-reply}

Return your addon's asset ops from `build`, along with lines and warnings:

```json
{
  "ops": [ { "op": "file.put", "source": { "root": "cache", "path": "enabled.txt" }, "target": "example-launcher-addon/enabled.txt" } ],
  "lines": ["1 item on"],
  "warnings": []
}
```

- Write each op exactly as you would in an [op file](./asset-api#op-files): set the `op` field to `<kind>.<name>`, such as `file.put`, `db.set`, `erp.patch-pkg`, `lang.set` or `warn`.
- For a `source` inside the addon folder, use a relative path (`"art/okayama.dds"`). Use `{ "root": "data" | "cache", "path": "…" }` to read from `data_dir` or `cache_dir`.
- The launcher tags every op with this addon's id as its origin, routes them like declared ops, and reports conflicts by origin ([Asset API](./asset-api#deliveries)).
- If your addon loads several items, such as the mods in a library, you can add `items`: the item each op comes from, by position in `ops`, with `null` for an op that belongs to no item. The launcher adds the item to the op's origin, so each item gets its own runtime overlay archive and conflicts name it. Leave `items` out, or shorter than `ops`, and the remaining ops have no item.
- Check `features.stable` and `features.experimental` before returning ops. They list the op `kind`s the installed loader supports: `group`, `erp`, `db`, `lang`, `file`. Drop any ops it cannot apply.
- The build calls `build` for each enabled addon in load order, after that addon's declared ops. It prints `lines` and `warnings` with the addon id as a prefix.

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
- does not answer `hello`, `view` or `summary` within 5 seconds, or any other call within 300 seconds;
- exits or closes stdout (`stopped (exit status: 3)`);
- writes a line that is not a message, replies to an `id` it was not sent, or returns a result of the wrong shape.

When your plugin fails, its sections show the error in a bad-tone notice with a **Restart** button (id `swofty.restart`). The body is empty and reads `<name> is not running`. Click Restart to clear the failure; the next call starts the plugin again. Reinstalling the addon also stops its plugin.

To fail just one call, reply with `{ "id": …, "error": "…" }`. The launcher shows the error as a notification and keeps your plugin running.

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
