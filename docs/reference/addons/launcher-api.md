---
title: Launcher API
description: Launcher sections, views, importers and drag and drop, build hooks, commands, per-profile state and profile summaries, for built-in addons in Rust and third-party addons through a helper program.
---

# Launcher API

An addon can add pages to the launcher's sidebar, import files dropped on the window, add asset ops when a profile builds, add `swofty` subcommands and add a line to the profile cards.

- **Built-in addons** are Rust crates that implement the `Addon` trait from `swofty_addon::launcher` (`packages/addon-api`). They depend on `swofty-addon` and `swofty-formats`, never on `swofty-core`. Team Simplified (`addons/simplified`) is one.
- **Third-party addons** provide the same calls through a helper program named in `swofty.json` ([Third-party addons](#third-party-addons)).

## The Addon trait

```rust
pub trait Addon: Send + Sync {
    fn manifest(&self) -> &AddonManifest;
    fn sections(&self) -> Vec<SectionSpec>;
    fn view(&self, cx: &Cx, section: &str) -> Result<View>;
    fn event(&self, cx: &Cx, section: &str, event: Event) -> Result<Effect>;
    fn importer(&self) -> Option<&dyn Importer>;
    fn build(&self, cx: &BuildCx) -> Result<AssetBatch>;
    fn commands(&self) -> Vec<CommandSpec>;
    fn command(&self, cx: &Cx, path: &[String], args: &[String]) -> Result<String>;
    fn summary(&self, cx: &Cx) -> Option<Summary>;
    fn migrate(&self, data: &Path, version: u32) -> Result<Vec<String>>;
}
```

Every method except `manifest` has a default that does nothing.

| Method | Use |
|---|---|
| `sections`, `view`, `event` | Pages in the launcher's sidebar |
| `importer` | Files dropped on the launcher or picked in it |
| `build` | Asset ops for the profile being built |
| `commands`, `command` | `swofty` subcommands |
| `summary` | A list on the profile card |
| `migrate` | Moves the addon's data when `$DATA`'s `data_version` is older than the launcher's |

## Registration

`swofty_builtins::all()` (`addons/builtins`) returns every built-in addon, and the launcher and the CLI pass them to `Store::with_builtins`:

```rust
pub struct Builtin {
    pub addon: Arc<dyn Addon>,
    pub locked: bool,
    pub default_on: bool,
}
```

- A `locked` addon is always on in modded profiles, and its row on the Addons page has no switch.
- A `default_on` addon is on in profiles that do not list it. Team Simplified is `default_on`.
- Installing an addon with a built-in id is refused.

The core's registry (`swofty_core::registry`) holds every addon in id order: installed addons and SwoftyMod as declared addons (a manifest and its operations), built-ins, and third-party helpers.

## Context

```rust
pub struct Cx<'a> {
    pub addon: &'a str,
    pub profile: &'a ProfileInfo,
    pub data_dir: &'a Path,
    pub cache_dir: &'a Path,
    pub game: Option<&'a dyn GameFiles>,
}

impl Cx<'_> {
    pub fn state<T: DeserializeOwned + Default>(&self) -> Result<T>;
    pub fn set_state<T: Serialize>(&self, value: &T) -> Result<()>;
}
```

| Field | Meaning |
|---|---|
| `addon` | The addon's id |
| `profile` | The active profile: `name`, `vanilla`, and `enabled` (whether this addon is on in it) |
| `data_dir` | `$DATA/addon-data/<id>/`, shared by every profile |
| `cache_dir` | `$DATA/cache/addons/<id>/` |
| `game` | Clean game files, when a game is found |

`GameFiles` gives stock files only, never the game folder as modded:

| Method | Returns |
|---|---|
| `build_key` | The game build's key, such as `25251167-d873a1c8d204` |
| `exe_version` | The version resource of `F1_25.exe`, such as `1.0.141.7865` |
| `is_stock(rel)` | Whether the game ships that path |
| `original(rel)` | A clean copy of a game file |
| `original_lang(code)` | The stock `language_<code>.lng`, extracted from the archive indexed in `F1_25.exe` |

### Per-profile state

`state` reads the addon's entry in the active profile's `state` object, or `T::default()` when the profile has none. `set_state` writes it, and refuses on the vanilla profile. The core never reads inside it. Duplicating or renaming a profile copies its state.

```json [$DATA/profiles/default.json]
"state": {
  "teamsimplified.mods": { "mods": [{ "id": "okayama", "enabled": true }] }
}
```

## Sections and views

```rust
pub struct SectionSpec {
    pub id: String,
    pub title: String,
    pub icon: Icon,
}
```

Each section is one page and one sidebar entry. The sidebar lists **Profiles**, **Addons**, a divider, then every addon section in registry order. A section whose body is a list shows its row count. An addon that is off in the active profile still shows its sections.

`view` returns the page:

```rust
pub struct View {
    pub subtitle: String,
    pub actions: Vec<Button>,
    pub notices: Vec<Notice>,
    pub body: Body,
    pub footer: Option<String>,
}

pub enum Body {
    List { rows: Vec<Row>, reorder: bool },
    Empty { title: String, text: String, action: Option<Button> },
    Blocks(Vec<Block>),
}

pub struct Row {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub art: Art,
    pub tags: Vec<Tag>,
    pub trailing: Option<Control>,
    pub detail: Vec<Block>,
}
```

| Part | Renders as |
|---|---|
| `subtitle`, `actions` | The page header and its buttons |
| `notices` | Bars under the header, each with an optional button |
| `Body::List` | Rows with art (a monogram or an image), tags, a trailing control and an expandable body (`detail`). A `reorder` list gets **Move up** and **Move down** in each expanded row. |
| `Body::Empty` | An empty state with a title, a line and a button |
| `Body::Blocks` | The blocks below, one after another |
| `footer` | A muted line under the body |

| Block | Renders as |
|---|---|
| `Stats(facts)` | Large label and value pairs |
| `Facts(facts)` | Small label and value pairs |
| `Label(text)` | A section heading, such as `WHAT IT CHANGES` |
| `Text { text, tone }` | A line in a tone |
| `Names { label, items }` | A heading and a list of names |
| `Table { columns, rows }` | A table |
| `Buttons(buttons)` | A row of buttons |

| Control | Sends |
|---|---|
| `Switch { id, on, enabled }` | `Event::Toggle { id, on }` |
| `Choice { id, options, current }` | `Event::Choose { id, index }` |
| `Button(button)` | Depends on its `kind` |

A `Button` has `id`, `label`, an optional `icon`, a `style` (`primary`, `outline`, `ghost` or `danger`) and a `kind`:

| `ButtonKind` | On click |
|---|---|
| `Event` | Sends `Event::Click { id }` |
| `Confirm { title, text }` | Asks in a dialog, then sends `Event::Click { id }` |
| `Pick { title, extensions, folders, multiple }` | Opens the system file picker, then sends `Event::Picked { id, paths }`. `folders` allows folders. |

`Tone` (`neutral`, `muted`, `accent`, `good`, `warn`, `bad`) colours tags, notices and text from the launcher's palette. `Icon` is a fixed set: `activity`, `arrow-right-left`, `chevron-down`, `chevron-up`, `circle-alert`, `circle-check`, `copy`, `eraser`, `folder-open`, `gamepad`, `layers`, `lock`, `map`, `package`, `pencil`, `play`, `plus`, `puzzle`, `settings`, `shield-check`, `trash`, `upload`.

### Events and effects

`event` receives an `Event` and returns an `Effect`:

```rust
pub enum Event {
    Click { id: String },
    Toggle { id: String, on: bool },
    Choose { id: String, index: usize },
    Move { row: String, to: usize },
    Picked { id: String, paths: Vec<PathBuf> },
}

pub enum Effect {
    None,
    Reload,
    Notice(String),
    SetAddonEnabled(bool),
    Open(PathBuf),
}
```

| Effect | The launcher |
|---|---|
| `None` | Does nothing |
| `Reload` | Asks every section for its view again |
| `Notice(text)` | Shows a notification and reloads |
| `SetAddonEnabled(on)` | Turns the addon on or off in the active profile |
| `Open(path)` | Opens the file or folder with the system's handler |

- `event`, `import`, `command` and `build` run on a background thread. The control that sent an event shows as busy until its effect returns. An error shows as an error notification.
- The launcher asks for every view again after an effect, a profile switch or a store reload.

## Importers and drag and drop

```rust
pub trait Importer: Send + Sync {
    fn claims(&self, path: &Path) -> Claim;
    fn import(&self, cx: &Cx, paths: &[PathBuf], progress: &dyn Progress) -> Result<Vec<String>>;
}

pub enum Claim { No, Maybe, Yes }

pub trait Progress: Send + Sync {
    fn report(&self, done: f32, text: &str);
}
```

Files dropped anywhere on the launcher window are routed by `swofty_core::import`:

1. The core claims addons first: a folder with `swofty.json` at its root or in its only child folder, a `.swoftymod` file, or a zip that holds an addon. These install as addons.
2. Every addon with an importer is asked `claims` for each file, in registry order.
3. A file goes to the first `Yes`, else to the only `Maybe`. Otherwise the launcher names it: `No addon imports <file>`.

Files with the same route go to one `import` call. `claims` should only look at the path and the archive's file list. `import` returns lines to show as notifications, and `Progress` reports fill a progress card while it runs.

## Build hooks {#build-hooks}

```rust
pub struct BuildCx<'a> {
    pub cx: Cx<'a>,
    pub options: &'a BTreeMap<String, serde_json::Value>,
    pub features: &'a Features,
}

pub struct AssetBatch {
    pub ops: Vec<Op>,
    pub lines: Vec<String>,
    pub warnings: Vec<String>,
}
```

- The build calls `build` for each enabled addon in the profile's load order, after that addon's declared ops.
- `options` holds the profile's value of every option the addon declares. `features` holds the runtime features of the installed loader (`stable` and `experimental` kinds).
- Each `Op` carries its `Origin`: the addon and an optional item name, such as a mod id. The planner routes the ops like declared ones ([Asset API](./asset-api#deliveries)) and reports conflicts by origin.
- The build hash covers every returned op and the size and modification time of every source it names.
- The build prints `lines` and `warnings` prefixed with the addon id.

## Commands

```rust
pub struct CommandSpec {
    pub name: String,
    pub about: String,
    pub args: Vec<ArgSpec>,
    pub subcommands: Vec<CommandSpec>,
}

pub struct ArgSpec {
    pub name: String,
    pub help: String,
    pub required: bool,
    pub many: bool,
}
```

Each `CommandSpec` becomes a top-level `swofty` subcommand. `command` receives the path of names, starting with the top-level one (for example `["mods", "import"]`), and the arguments, and its text is printed. A name that core or an earlier addon already uses is refused, with a message. Commands run against `--profile` (default: the active profile) and the Steam install of the game.

## Profile summaries

`summary` returns a `Summary { label, items }`. The profile cards show it after `ADDONS`, as Team Simplified shows `MODS` with the names of the enabled mods.

## Third-party addons {#third-party-addons}

An installed addon gets pages, an importer, a build hook, commands and a summary by shipping a helper program and naming it in the `launcher` block of `swofty.json`:

```json [swofty.json]
"launcher": {
  "program": { "windows": "bin/example-helper.exe", "linux": "bin/example-helper" },
  "sections": [{ "id": "items", "title": "Example", "icon": "package" }],
  "imports": ["exmod"],
  "build": true,
  "commands": [{ "name": "example", "about": "List the example items" }]
}
```

| Field | Meaning |
|---|---|
| `program` | The helper's path inside the addon folder, per platform (`windows`, `linux`) |
| `sections` | `SectionSpec`s. The sidebar lists them without starting the helper. |
| `imports` | File extensions without the dot. A dropped file with one of them is claimed with `Yes`. |
| `build` | `true` when the helper has a build hook |
| `commands` | `CommandSpec`s added to `swofty` |

### Starting the helper

The launcher starts the helper on the first call that needs it: opening one of its sections, a drop routed to it, a build of a profile that enables it, or one of its commands. The working directory is the addon folder, and stderr goes to `$DATA/cache/addons/<id>/helper.log`. The launcher asks for a helper section's view only while that section is open, and for a summary only while the helper runs and is idle. The helper exits when stdin closes.

### Messages

Each message is one JSON object on one line, UTF-8, over the helper's stdin and stdout. A request has `id`, `method` and `params`. A reply has the request's `id` and either `result` or `error` (a string). A notification has no `id`. The host sends one request at a time.

```text
host   {"id":0,"method":"hello","params":{"api":1,"addon":"example.launcher-addon","data_dir":"…","cache_dir":"…","profile":{"name":"default","vanilla":false,"enabled":true}}}
helper {"id":0,"result":{"api":1}}
host   {"id":1,"method":"view","params":{"section":"items"}}
helper {"id":0,"method":"get_state"}
host   {"id":0,"result":{"on":["a.exmod"]}}
helper {"id":1,"result":{"subtitle":"1 item","body":{"list":{"rows":[…],"reorder":false}}}}
```

Host to helper:

| Method | `params` | `result` |
|---|---|---|
| `hello` | `api`, `addon`, `data_dir`, `cache_dir`, `profile` | `{"api": 1}` |
| `profile` (notification) | `name`, `vanilla`, `enabled` | |
| `view` | `section` | `View` |
| `event` | `section`, `event` | `Effect` |
| `import` | `paths` | Lines to show |
| `build` | `options`, `features` | `{"ops": [...], "lines": [], "warnings": []}` |
| `command` | `path`, `args` | Text to print |
| `summary` | none | `Summary` or `null` |

Helper to host, while a request is in flight:

| Method | `params` | `result` |
|---|---|---|
| `progress` (notification) | `done` (0 to 1), `text` | |
| `get_state` | none | The profile's state for the addon, or `null` |
| `set_state` | The new state | `null`, or an error on the vanilla profile |

- Values take the JSON forms of the Rust types above: enums are tagged in snake_case (`{"toggle":{"id":"a","on":true}}`, `"reload"`), `Icon` is kebab-case, and asset ops are written as in an [op file](./asset-api#op-files).
- Build ops get the origin `<id>`, and their sources resolve against the addon's `addon`, `data` and `cache` roots.
- The host sends `profile` before a request when the active profile changed since the last one.
- `api` is `swofty_addon::helper::API_VERSION`, 1. The host stops a helper that answers `hello` with another version.
- A helper answers a method it does not know with an `error`.

### Failures

A helper `error` fails that call only. The host stops the helper and marks it failed when it:

- does not start, or has no `program` for this platform;
- does not reply within 5 seconds to `hello`, `view` and `summary`, or within 300 seconds to the other calls (each message resets the wait);
- exits or closes stdout;
- writes a line that is not a message, replies to another `id` or sends a result of the wrong shape.

A failed helper's sections show its error in a notice with a **Restart** button (`swofty.restart`). Restart clears the failure, and the next call starts the helper again. Installing the addon again stops its helper.

### The example addon

[`templates/launcher-addon`](https://github.com/Swofty-Developments/SwoftyLauncher/tree/main/templates/launcher-addon) is a helper in Rust that depends only on `serde_json`:

- an **Example** page that lists imported `.exmod` files with a switch each, stored as per-profile state, and an **Import…** button;
- an importer that copies `.exmod` files into its `addon-data` folder and reports progress;
- a build hook that writes the names of the items that are on to `enabled.txt` in its cache folder and installs it with `file.put`;
- `swofty example`, which prints each item and whether it is on;
- the summary `EXAMPLE`.

Its README has the build and install steps.
