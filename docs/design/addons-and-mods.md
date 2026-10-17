# Addons and mods

This page is for contributors. It is the design for splitting today's mods into addons and mods, the Launcher API, the Asset API and the built-in Team Simplified addon. Nothing on this page is built yet unless a line says so.

## 1. Terms and the rename

| Term | Meaning |
|---|---|
| Addon | A package with a `swofty.json` manifest: native plugins, declared content, settings and in-game screens. Everything the product calls a mod today is an addon. The built-in addons are SwoftyMod (`swofty.core`) and Team Simplified (`teamsimplified.mods`). |
| Mod | A content package with no code: liveries, cars, text, shaders, database rows. An addon imports, stores and loads mods. Team Simplified handles `.serps` mods. |
| Map | A community track package. Tracks are mods in Team Simplified's library ([section 5](#_5-tracks-are-mods)). |
| Asset op | One change to game content: an asset group, an archive resource, a database record, a language string or a loose file. |
| Delivery | How an asset op reaches the game. A build delivery rewrites a game file and installs it through the journal. A runtime delivery has swofty_core apply the op in memory while the game boots. |

Assets reach the game in two ways only:

1. An addon declares asset ops in its manifest, and the build loads them at launch.
2. An addon produces asset ops itself, through the Asset API: a Rust build hook for built-in addons, or `swofty::assets` calls from a native plugin at runtime.

There is no core Assets page. Mods appear only in launcher sections that addons contribute.

### Renames

The renames and the compatibility rules below are built. `owned_items` entries in `swofty/settings.json` also name their addon under `addon` instead of `mod`.

| Area | Now | After |
|---|---|---|
| Core types | `ModManifest`, `ModOption` | `AddonManifest`, `AddonOption` |
| Store | `Store::mods()`, `mods_dir()`, `install_mod()` | `addons()`, `addons_dir()`, `install_addon()` |
| Profile | `ProfileMod`, `Profile.mods` | `ProfileAddon`, `Profile.addons` |
| Session settings | `SessionMod`, `SessionSettings.mods` | `SessionAddon`, `SessionSettings.addons` |
| Launcher | `ModItem`, `Library.mods`, `Shell.open_mod`, `Page::Mods`, `mods_page`, `mod_row`, `mod_body`, `install_mod` | `AddonItem`, `Library.addons`, `Shell.open_addon`, `Page::Addons`, `addons_page`, `addon_row`, `addon_body`, `install_addon` |
| CLI | `ModsCommand`, `swofty mods list/install/enable/disable` | `AddonsCommand`, `swofty addons list/install/enable/disable` |
| SDK | `SwoftyModInit`, `SwoftyModInitFn`, `INIT_SYMBOL`, `SWOFTY_MOD()` | `SwoftyAddonInit`, `SwoftyAddonInitFn`, `INIT_SYMBOLS`, `SWOFTY_ADDON()` |
| Native core | `ModSettings`, `mods[]` in `manager.cpp`, `settings.cpp`, `ui_vm.cpp` | `AddonSettings`, `addons[]` |
| Repository | `mods/swoftymod`, `mods/custom-grid-size`, `templates/mod` | `addons/swoftymod`, `addons/custom-grid-size`, `templates/addon` |
| Data dir | `$DATA/mods/<id>/` | `$DATA/addons/<id>/` |
| Data dir (new) | | `$DATA/addon-data/<id>/` for files an addon owns, `$DATA/cache/addons/<id>/` for its caches |
| Profile file | `mods` | `addons`, plus a `state` object keyed by addon id |
| Game folder | `swofty/mods/core/`, `swofty/mods/*.dll` | `swofty/addons/core/`, `swofty/addons/*.dll` |
| `swofty/settings.json` | `mods` | `addons` |
| Screen names | `swofty_mod_<n>` | `swofty_addon_<n>` |
| In game | `MOD SETTINGS` | `ADDON SETTINGS` |
| Launcher text | `Mods` (sidebar), `Install mod…`, `A mod folder that contains swofty.json.`, `<n> mods installed`, `MODS` and `No mods` on profile cards | `Addons`, `Install addon…`, `An addon folder that contains swofty.json.`, `<n> addons installed`, `ADDONS` and `No addons` |
| CLI text | `Swofty Launcher for F1 25: mods and maps`, `mod settings` | `Swofty Launcher for F1 25: addons, mods and maps`, `addon settings` |
| Manifest errors | `must be a relative path inside the mod` | `must be a relative path inside the addon` |
| SwoftyMod description | `mod loading` | `addon loading` |
| Docs titles | `Mods and maps`, `Making a mod`, `Configuring mods (swofty.json)`, `In-game mod settings`, `Mod manifest` | `Addons and maps` (`Addons, mods and maps` with the Mods section), `Making an addon`, `Configuring addons (swofty.json)`, `In-game addon settings`, `Addon manifest` |

These stay as they are: `swofty.json` and every field in it, manifest `schema_version` 1, the id `swofty.core`, the name SwoftyMod, the `swofty_set:` and `swofty_enable:` commands, the requests file, `$DATA/maps/`, the app id `net.swofty.ModLauncher`, and doc page paths.

### Compatibility and migration

- **Manifests.** Existing manifests load unchanged. `operations.assets` is a new optional field. `operations.database`, `operations.erp` and `operations.files` stay valid and load into the asset op model.
- **Data dir.** `Store::migrate()` runs when the launcher starts and when a `swofty` command parses. `--help`, `--version` and parse errors exit before it:
  1. If `mods/` exists and `addons/` does not, `mods/` is renamed to `addons/`.
  2. If both exist, each `mods/<x>/` whose manifest id is not in `addons/` moves there. The rest stay and are logged. `mods/` is removed when empty.
  3. `config.json` gets `"data_version": 2`.
- **Profiles.** `Profile.addons` also reads `mods`. The next save writes `addons` and `schema_version: 2`. Option values stay keyed by addon id.
- **Game folder.** The new layout needs no migration. The loader also reads `swofty/mods/` until the next minor release.
- **Plugins.** The loader calls `SwoftyAddonInit`, or `SwoftyModInit` when that is the only export. `SWOFTY_MOD()` stays as an alias of `SWOFTY_ADDON()`. Built plugins keep working.
- **Settings file.** swofty_core reads `addons`, or `mods` when `addons` is absent.
- **CLI.** `swofty mods` stays a hidden alias of `swofty addons` until Team Simplified provides `swofty mods`. From then until the next minor release, `swofty mods install <dir>` for a folder with `swofty.json`, and `swofty mods enable|disable <id>` for an addon id, run the `swofty addons` command and print a note.
- Older launchers cannot read migrated data.

## 2. Launcher API

A new crate, `packages/addon-api` (`swofty-addon`), holds the API. `manifest` and `ops` move there from core, and core re-exports them. Core depends on `swofty-addon` and runs addons. Built-in addons depend on `swofty-addon` and `swofty-formats`, never on `swofty-core`. The apps register them through `addons/builtins` (`swofty-builtins`). Core names no addon.

The types below, the registry, `Profile.state`, the `addon-data` and cache folders and the build hooks are built: `swofty_addon::launcher` and `swofty_addon::view`, core's `registry` module, and `Store::with_builtins`. The launcher shows sections, runs events and importers on the background executor, routes dropped files through `swofty_core::import` and shows summaries on the profile cards. The CLI adds addon commands and forwards the old `swofty mods` forms.

```rust
pub trait Addon: Send + Sync {
    fn manifest(&self) -> &AddonManifest;
    fn sections(&self) -> Vec<SectionSpec> { Vec::new() }
    fn view(&self, cx: &Cx, section: &str) -> Result<View> { Err(Error::NoSection(section.into())) }
    fn event(&self, cx: &Cx, section: &str, event: Event) -> Result<Effect> { Ok(Effect::None) }
    fn importer(&self) -> Option<&dyn Importer> { None }
    fn build(&self, cx: &BuildCx) -> Result<AssetBatch> { Ok(AssetBatch::default()) }
    fn commands(&self) -> Vec<CommandSpec> { Vec::new() }
    fn command(&self, cx: &Cx, path: &[String], args: &[String]) -> Result<String> { Err(Error::NoCommand(path.join(" "))) }
    fn summary(&self, cx: &Cx) -> Option<Summary> { None }
}

pub trait Importer: Send + Sync {
    fn claims(&self, path: &Path) -> Claim;
    fn import(&self, cx: &Cx, paths: &[PathBuf], progress: &dyn Progress) -> Result<Vec<String>>;
}

pub enum Claim { No, Maybe, Yes }

pub trait Progress: Send + Sync {
    fn report(&self, done: f32, text: &str);
}

pub struct Cx<'a> {
    pub addon: &'a str,
    pub profile: &'a ProfileInfo,
    pub data_dir: &'a Path,
    pub cache_dir: &'a Path,
    pub game: Option<&'a dyn GameFiles>,
    state: &'a dyn StateStore,
}

impl Cx<'_> {
    pub fn state<T: DeserializeOwned + Default>(&self) -> Result<T>;
    pub fn set_state<T: Serialize>(&self, value: &T) -> Result<()>;
}

pub struct ProfileInfo {
    pub name: String,
    pub vanilla: bool,
    pub enabled: bool,
}

pub trait GameFiles: Send + Sync {
    fn build_key(&self) -> String;
    fn exe_version(&self) -> Option<String>;
    fn is_stock(&self, rel: &str) -> bool;
    fn original(&self, rel: &str) -> Result<PathBuf>;
    fn original_lang(&self, code: &str) -> Result<PathBuf>;
}

pub struct BuildCx<'a> {
    pub cx: Cx<'a>,
    pub options: &'a BTreeMap<String, serde_json::Value>,
    pub features: &'a Features,
}

pub struct Summary {
    pub label: String,
    pub items: Vec<String>,
}

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

`GameFiles` gives clean files only: baseline objects, game files the baseline has not recorded, and the stock language files ([section 3.6](#_3-6-language-baseline)). `is_stock` is true for a path the game ships.

The core's addon registry holds every addon. Installed addons and SwoftyMod are `DeclaredAddon`s: a manifest and a folder, with the default methods. Built-in entries carry `locked` (SwoftyMod: always on) and `default_on` (Team Simplified: on in profiles that do not list it). `Store::profile` turns on a `default_on` addon that the profile does not list, and a `locked` one always.

### Sections and pages

```rust
pub struct SectionSpec {
    pub id: String,
    pub title: String,
    pub icon: Icon,
}

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

pub enum Block {
    Stats(Vec<Fact>),
    Facts(Vec<Fact>),
    Label(String),
    Text { text: String, tone: Tone },
    Names { label: String, items: Vec<String> },
    Table { columns: Vec<String>, rows: Vec<Vec<String>> },
    Buttons(Vec<Button>),
}

pub enum Control {
    Switch { id: String, on: bool, enabled: bool },
    Choice { id: String, options: Vec<String>, current: usize },
    Button(Button),
}

pub struct Button {
    pub id: String,
    pub label: String,
    pub icon: Option<Icon>,
    pub style: Style,
    pub kind: ButtonKind,
}

pub enum ButtonKind {
    Event,
    Confirm { title: String, text: String },
    Pick { title: String, extensions: Vec<String>, folders: bool, multiple: bool },
}

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

`Fact`, `Tag`, `Notice`, `Tone`, `Style`, `Art` and `Icon` are small value types: label and value, text and tone, a fixed icon set, a monogram or an image path. Every type is serde-serialisable.

- Each section is one page and one sidebar entry. The sidebar lists Profiles, Addons, a divider, then the addon sections in registry order. A section whose body is a list shows its row count.
- `footer` is a muted line under the body. `Effect::Open` opens a file or folder with the system's handler.
- The shell renders a `View` with the components the core pages use: page header, list, row, switch, choice button, stats, facts, notices and empty state. A row's `detail` is its expanded body. A `reorder` list gets a drag handle and Move up and Move down entries.
- The shell keeps each section's `View` and asks again after `Effect::Reload`, a profile switch or a store reload.
- `view` runs on the UI thread and reads in-memory state only. `event`, `import`, `command` and `build` run on the background executor. The control that sent an event shows as busy until its `Effect` returns.
- The sections of an addon that is off in the active profile are hidden.

### Import handlers

- A `ButtonKind::Pick` button opens the system file picker and sends `Event::Picked`.
- Files dropped on the window go to the importer whose `claims` returns `Yes`, else to the only one that returns `Maybe`. Otherwise the shell names the file and says no addon imports it. Core imports addon folders (`swofty.json`), `.swoftymod` files and zips that hold an addon, and is asked first. Team Simplified's importer claims track zips ([section 5](#_5-tracks-are-mods)).
- `Progress` reports appear as a notification with a progress bar.

### Per-profile state

- `Profile.state[<addon id>]` is a JSON value the addon owns. Core does not read inside it.
- `Cx::state` returns it, or `T::default()` when absent. `Cx::set_state` writes it to the active profile and refuses on the vanilla profile.
- Duplicating or renaming a profile copies its state.

### Build hooks

- The build visits enabled addons in resolver order. For each one it takes the manifest's declared ops, then the ops `build` returns, and hands both to the asset planner ([section 3](#_3-asset-api)).
- The build hash covers every op and the size and modification time of every source file an op names, and of every file in a folder source.
- `BuildCx.options` holds the profile's values of the addon's options.
- `BuildCx.features` holds the features from the loader's `features.json`. Hook ops go through the planner after the addon's declared ops, with the origin the hook gives them.
- An op file or hook result that uses a `patch-pkg` edit the build does not support yet (anything but `clone-entry` and `set-value`) has all its ERP ops skipped, with a warning.

### Commands and profile cards

- The CLI adds each `CommandSpec` as a top-level subcommand and passes the arguments to `command`. A name already taken by core or an earlier addon is refused when the addon registers.
- `summary` adds a list to the profile cards, after `ADDONS`.

### Third-party addons {#third-party-addons}

Built. An installed addon gets pages, an importer, a build hook, commands and a profile summary through a helper program it ships. Core runs the helper as `HelperAddon` (`swofty_core::helper`), an `Addon` backed by a process. The registry holds one `HelperAddon` per installed addon with a `launcher` block and marks its entry `helper`. The protocol types are in `swofty_addon::helper`. [`templates/launcher-addon`](https://github.com/Swofty-Developments/SwoftyLauncher/tree/main/templates/launcher-addon) is an example helper in Rust.

The manifest declares the helper and what it provides:

```json
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
| `program` | The helper's path inside the addon folder, keyed by `std::env::consts::OS` (`windows`, `linux`) |
| `sections` | `SectionSpec`s. The sidebar lists them without starting the helper. |
| `imports` | File extensions without the dot. A file with one of them gets `Claim::Yes`. |
| `build` | `true` when the helper has a build hook |
| `commands` | `CommandSpec`s added to the CLI |

**Start.** The host starts the helper on the first call that needs it: opening one of its sections, a drop routed to it, a build of a profile that enables it, or one of its commands. The working directory is the addon folder. stderr goes to `$DATA/cache/addons/<id>/helper.log`. The launcher asks for a helper section's view only while that section is open. A summary is asked only while the helper runs and has no call in flight. The helper exits when stdin closes.

**Messages.** Each message is one JSON object on one line, UTF-8, over the helper's stdin and stdout. A request carries `id`, `method` and `params`. A reply carries the request's `id` and either `result` or `error` (a string). A notification has no `id`. The host sends one request at a time and waits for its reply.

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
| `hello` | `api`, `addon`, `data_dir`, `cache_dir`, `profile` (`ProfileInfo`) | `{"api": <n>}` |
| `profile` (notification) | `ProfileInfo` | |
| `view` | `section` | `View` |
| `event` | `section`, `event` (`Event`) | `Effect` |
| `import` | `paths` | lines to show, as strings |
| `build` | `options`, `features` (`Features`) | `{"ops": [asset op], "lines": [], "warnings": []}` |
| `command` | `path`, `args` | text to print |
| `summary` | none | `Summary` or `null` |

Helper to host, while a request is in flight:

| Method | `params` | `result` |
|---|---|---|
| `progress` (notification) | `done` (0 to 1), `text` | |
| `get_state` | none | the profile's state for the addon, or `null` |
| `set_state` | the new state | `null`. An error on the vanilla profile. |

- Values use the serde forms of the Launcher API and Asset API types: enums are externally tagged in snake_case (`{"toggle":{"id":"a","on":true}}`, `"reload"`), `Icon` is kebab-case, and asset ops are written as in an ops file. Build ops get the origin `<id>`, and their sources resolve against the addon's `addon`, `data` and `cache` roots.
- `api` is `swofty_addon::helper::API_VERSION`, 1. The host stops a helper that answers `hello` with another version.
- The host sends `profile` before a request when the active profile changed since the last one.
- A helper answers a method it does not know with an `error`.

**Failures.** A helper error fails that call only. The host stops the helper and marks it failed when it does not start, does not reply within 5 seconds to `hello`, `view` and `summary` or within 300 seconds to the other calls (a message resets the wait), exits, closes stdout, writes a line that is not a message, replies to another `id` or sends a result of the wrong shape. A failed helper's sections show its error in a notice with a Restart button (`swofty.restart`). Restart clears the failure, and the next call starts the helper again. Other calls to a failed helper return the error. Installing the addon again stops its helper.

## 3. Asset API

### 3.1 Ops

| Op | Fields | Change |
|---|---|---|
| `group.register` | `group`, `erp` or `erps`, optional `deps`, `template` | Add an asset group to the engine's manifest |
| `erp.replace-resource`, `erp.add-resource`, `erp.copy-resource`, `erp.remove-resource`, `erp.add-mipmaps`, `erp.patch-pkg` | As in [archive edits](/reference/ops/erp) | Unchanged. All six are built (`packages/core/src/erpbuild.rs`) |
| `erp.put-resource` | `erp`, `resource`, `source`, optional `as` | Copy a resource from an archive in the addon, replacing or adding it |
| `db.set`, `db.add-row`, `db.remove-row`, `db.list-append`, `db.list-remove` | As in [database edits](/reference/ops/database) | Unchanged |
| `db.put-record` | `key` (path or `0x` and 16 hex digits), `type`, `value` (hex) | Write one raw record |
| `db.remove-record` | `key` | Remove one record |
| `lang.set` | `language` (code or `*`), `key`, `value` | Set one string |
| `lang.table` | `language`, `source` (a partial `.lng`) | Set every string in a table |
| `file.put` | `source`, `target` | Replace or add a loose game file |
| `warn` | `message` | Print a message at build |

Every op takes `when` and `{{id}}` placeholders, as database ops do. `source` paths are relative to the addon folder. The ops, `operations.assets` and the planner are built (`packages/core/src/planner.rs`).

### 3.2 Manifest

```json
"operations": {
  "assets": ["assets/livery.json"]
}
```

```json
{
  "schema_version": 1,
  "ops": [
    { "op": "erp.put-resource", "erp": "2025_asset_groups/f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso.erp",
      "resource": "eaid://f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso.wep", "source": "erp/car.erp" },
    { "op": "db.set", "select": "entrants.teams.team[_metadata_=toro_rosso_26]", "field": "display_name_code", "value": "AT" },
    { "op": "lang.set", "language": "*", "key": "lng_team_tor_hashtag", "value": "AlphaTauri" },
    { "op": "file.put", "source": "files/sponsor.dds", "target": "livery_package_shared/sponsor.dds" }
  ]
}
```

Files in `operations.database` hold `db.*` ops without the prefix, files in `operations.erp` hold `erp.*` ops without the prefix, and each `operations.files` entry is a `file.put`.

### 3.3 Rust API

```rust
pub enum AssetOp {
    Group(GroupOp),
    Erp(ErpOp),
    Db(DbOp),
    Lang(LangOp),
    File(FileOp),
    Warn { message: String },
}

pub struct Op {
    pub op: AssetOp,
    pub origin: Origin,
}

pub struct Origin {
    pub addon: String,
    pub item: Option<String>,
}

pub struct Source {
    pub root: Root,
    pub path: String,
}

pub enum Root {
    Addon,
    Data,
    Cache,
}

#[derive(Default)]
pub struct AssetBatch {
    pub ops: Vec<Op>,
    pub lines: Vec<String>,
    pub warnings: Vec<String>,
}
```

`Source` resolves inside the addon's folder, its `addon-data` folder or its cache folder. Any other path fails the build. `Origin.item` names the content inside an addon, such as a mod id. In JSON a `source` is a path in the addon's folder, or `{ "root": "data", "path": ... }` for the other roots.

Core plans and applies ops:

```rust
pub enum Kind { Group, Erp, Db, Lang, File }

pub enum Route { Build, Runtime, Waiting }

pub struct Features {
    pub stable: BTreeSet<Kind>,
    pub experimental: BTreeSet<Kind>,
}

pub struct Planner<'a> { /* deliveries, targets */ }

impl<'a> Planner<'a> {
    pub fn new(features: &'a Features, opt_in: &'a BTreeSet<Kind>, game: &'a dyn GameFiles) -> Self;
    pub fn route(&self, kind: Kind) -> Route;
    pub fn push(&mut self, op: Op, sources: &dyn Fn(&Source) -> Result<PathBuf>) -> Result<()>;
    pub fn finish(self, overlay: &Path) -> Result<Planned>;
}

pub struct Planned {
    pub plan: Plan,
    pub runtime: serde_json::Value,
    pub waiting: Vec<Op>,
    pub conflicts: Vec<Conflict>,
    pub lines: Vec<String>,
    pub warnings: Vec<String>,
}

pub struct Conflict {
    pub target: Target,
    pub winner: Origin,
    pub overridden: Vec<Origin>,
}

pub enum Target {
    Group(String),
    Resource { erp: String, uri: String },
    Record(u64),
    String { language: String, key: String },
    File(String),
}
```

`Planned.runtime` is written to `swofty/assets.json`, and `Built.conflicts` keeps the conflicts for the launcher. `Kind` and `Features` are in `swofty_addon::asset`, and `planner::load_features` reads `features.json`.

### 3.4 Deliveries

| Kind | Build delivery (today) | Runtime delivery (swofty_core) | Runtime state | Route now |
|---|---|---|---|---|
| `group` | none | `assets.cpp`: entries inserted into the asset manifest after it is parsed | Stable: a Grand Prix loaded a track from registered groups (T018) | Runtime |
| `erp` | `erpbuild.rs` rewrites the archive into the overlay | `resources.cpp`: an overlay archive first in every group that lists the target, bound by resource URI | Stable (T011, T013) | Runtime |
| `db` | `ops.rs` rewrites `gamedatabases.2025.bin` | `gamedb.cpp`: edits applied when the database loads, with the early stage from `stage.cpp` | Stable (T014, T015, T018) | Runtime |
| `lang` | none: the game reads `.lng` only from its packed archive | `overlay.cpp`: partial string tables added after the stock table | Stable (T016) | Runtime |
| `file` | journaled copy into the game folder | `overlay.cpp`: file system overlay from `swofty/overlay/` | Experimental: not run in game | Runtime when turned on, otherwise Build |

`RUNTIME_DELIVERIES` in `planner.rs` lists all five kinds. `packages/core/src/runtime.rs` writes `swofty/assets.json` and its files. A `db` build whose ops remove records, or write a record of tag 10, 15 or 16 or one that is not text, falls back to the build route with a warning.

Routing per kind:

- A kind with no build delivery takes the runtime route when the feature is `stable`, or `experimental` and listed in `config.json` `"runtime_assets"`, and waits otherwise. The build lists waiting ops with their origins.
- A kind with both takes the runtime route when the feature is `stable`, or `experimental` and listed in `config.json` `"runtime_assets"`. Otherwise it takes the build route. `config.json` `"build_assets"` forces the kinds it lists onto the build route.
- Every op of one kind takes the same route in a build.
- An `erp.remove-resource` puts every `erp` op on the build route, because the core cannot take a resource out of a stock archive. `Planner::route` returns `Build` for `erp` once one is pushed, and `Planner::route_note` gives the build line.

Both routes of a kind compute the same result:

- `erp`: `ErpBuild` produces each changed resource. `replace-resource` refills one fragment as `set_fragment` does for `patch-pkg`; `add-resource` builds the resource with `Resource::like`, which copies the type argument, fragment names, flags and compression of a resource of the same type and fragment count in the target archive. The build route writes the whole archive, keeping unchanged resources as byte copies (`ErpFile::write`). `add-mipmaps` files are installed as loose game files through the journal on both routes, as a track's mip files are. The runtime route writes the changed resources as overlay archives under `swofty/assets/resources/`, one per set of target archives, and lists them in `resource_erps`. It leaves out a replacement whose type, argument or fragment layout differs from the stock resource. The core registers the overlay archives in the manifest without rebuilding them.
- `db`: the planner builds the database. The build route writes the file. The runtime route writes the records that differ from the database after map activation as `db_edits` by hash, with their tag and text, and sets `early_stage`. Map activation stays in the file. On both routes, `owned.rs` finds the `unlockable_se_livery` rows the built database has and stock lacks, whichever op added them, and writes their keys to `owned_items` in `swofty/settings.json`.
- `file`: the build route installs into the game folder through the journal. The runtime route copies under `swofty/overlay/` and sets `overlay`.
- `lang`: the runtime route writes one partial table per language under `swofty/assets/lang/` and lists it as `lang.<code>.table`. `*` writes `all.lng`, which applies to every language.
- `group`: the runtime route lists the group in `asset_groups`. Core skips a group that the stock manifest already has. A database field pointed at a stock group whose ERP is missing hangs the session load.

`native/build.sh` writes `swofty/addons/core/features.json`:

```json
{ "schema": 1, "features": { "group": "stable", "erp": "stable", "db": "stable", "lang": "stable", "file": "experimental" } }
```

A feature becomes `stable` in the native code once its runtime test passes and a verifier agrees, under `re/PROTOCOL.md`. The launcher needs no change for it. A loader without `features.json` has no runtime features.

`swofty/assets.json` uses the prototype's schema (`early_stage`, `asset_groups`, `resources`, `db_edits`, `lang`, `overlay`), plus `resource_erps` for overlay archives the launcher built, `features` for the experimental features the profile turned on and an `origin` on each asset group for the log.

### 3.5 Order and conflicts

- Ops apply in this order: map activation, then addons in resolver order. Within an addon, declared ops come first, then build hook ops in the order returned. A later write wins.
- The planner records the target of each write. A write that replaces a target (`put`, `replace` and `remove` ops, `db.set`, `db.put-record`, `lang.set`, `file.put`, `erp.add-mipmaps`) after another origin wrote it is a conflict. Edits inside a resource (`patch-pkg`) and row additions compose and are not conflicts.
- The build prints each conflict with the winner and the overridden origins. The launcher shows them under the play bar.

### 3.6 Language baseline

A text diff needs the stock language files. The game keeps them in a packed archive whose index is in `F1_25.exe` and whose data is in `game.dat`. `swofty-formats` gains:

- `nefs`: reads that archive and extracts `localisation/2025_release/language_<code>.lng` into `$DATA/baseline/<build key>/lang/` once per game build;
- `lng`: reads, writes and diffs LNGT language files.

`GameFiles::original_lang` returns the extracted files. `GameFiles::exe_version` reads the exe's version resource (for example `1.0.141.7865`).

### 3.7 Native runtime SDK

API version 4 adds an `assets` table to `SwoftyApi`. `swofty/assets.hpp` wraps it:

```cpp
namespace swofty::assets {

enum class Result { ok, late, unavailable, invalid };
struct Fragment {
    std::string_view file;
    std::string_view fourcc = {};
    uint32_t flags = 0;
};

bool available(std::string_view feature);
Result register_group(std::string_view group, std::span<const std::string_view> erps,
                      std::span<const std::string_view> deps = {}, std::string_view templ = {});
Result put_resource(std::string_view uri, std::span<const Fragment> fragments,
                    std::span<const std::string_view> erps = {});
Result db_set(std::string_view key, std::string_view type, std::string_view value);
Result db_set(uint64_t hash, uint8_t tag, std::span<const std::byte> value);
Result lang_set(std::string_view language, std::string_view key, std::string_view text);
Result lang_table(std::string_view language, std::string_view file);
Result overlay_file(std::string_view game_path, std::string_view file);

}
```

| Call | Accepted until | Core part |
|---|---|---|
| `register_group` | the asset manifest is parsed (boot step 24) | `assets.cpp` |
| `put_resource` | the resource overlay is written, at the end of plugin init | `resources.cpp` |
| `db_set` | the game database loads (step 10). Only plugins with `"early": true` load before it | `gamedb.cpp`, `stage.cpp` |
| `lang_set`, `lang_table` | the language file loads (step 24) | `overlay.cpp` |
| `overlay_file` | the game first reads that path | `overlay.cpp` |

- A call after its point returns `late`. A call to a feature the core lacks, or an experimental feature the profile has not turned on, returns `unavailable`. Every call is logged with the addon id.
- `type` takes the `db_edits` type names (`int`, `float`, `string`, `enum`, `id`, `bool`, ...) or a tag number. `tag` is the raw record's tag.
- File arguments are relative to the addon's folder, `swofty/addons/<id>/`. The core identifies the addon by the calling DLL.
- The build installs each plugin's `dll` and the files and folders in `files` on its `operations.native` entry to `swofty/addons/<id>/` at their paths in the addon, through the journal, and lists the DLLs in `plugins` in `swofty/settings.json` in load order. The loader loads them from there.
- SDK calls feed the same lists as `swofty/assets.json`. Entries from `assets.json` apply first, then calls in plugin load order.
- A native plugin that calls the API declares the `assets` capability. `"early": true` on its `operations.native` entry loads it at the early stage.

## 4. Team Simplified addon

### 4.1 The addon

- The crate `addons/simplified` (`swofty-addon-simplified`) is Rust and AGPL-3.0-or-later, like the rest of the repository. Its `mod.serps` reader, package root, content root and entry path checks are ported from the Simplified Launcher (GPL-3.0, used with Team Simplified's permission) and carry the Simplified Suite notice. `THIRD_PARTY.md` credits Team Simplified for the `.serps` format and the Simplified Suite, and the Mods section shows the credit.
- Import, the library, profile state, the section and `swofty mods` are built (work item 13), and so are the diffs (work item 14), the merge and conflicts (work item 15).
- Manifest: id `teamsimplified.mods`, name `Team Simplified`, author `Team Simplified`. No options and no native plugin.
- Built in and on by default in new profiles. It can be turned off per profile, which hides its section.
- It uses `swofty-addon` and `swofty-formats` only. Core has no code for it.

### 4.2 The .serps format

- A package is a zip archive under any name, `.serps` included, or a folder.
- `mod.serps` at the package root is JSON: `mod_name` or `name` (`mod_name` wins), `author`, `version`, `game` (for example `F1® 25`), `game_version` (for example `1.0.141.7865`), and `studio_version` from Simplified Studio. Every field is optional. The reader accepts a UTF-8 byte order mark, comments and trailing commas.
- `F1 25/` holds files in the game folder's layout. RM Designs' Scuderia AlphaTauri ORLEN package has the whole `game_package/gamedatabases/gamedatabases.2025.bin`, 11 whole language files under `localisatioa/2025_release/` and four whole shader archives under `shader_package_2025/`.
- `localisatioa/` is the folder Simplified's launcher points a patched exe at. Both `localisatioa/` and `localisation/` name the stock language file with the same file name.
- Without `F1 25/`, the content root is the first folder, within three levels, whose subfolders match top-level folders of the installed game, or `localisation` or `localisatioa`.

### 4.3 Import

1. The package unpacks into `addon-data/teamsimplified.mods/.import-<n>/`. Entries with absolute paths or `..` are skipped and counted.
2. The importer reads `mod.serps` and finds the content root.
3. Each file gets a class:

| Path | Class |
|---|---|
| `game_package/gamedatabases/gamedatabases.2025.bin` | Database. It must parse as F1 25's database, or the import fails |
| `localisation/*/language_<code>.lng`, `localisatioa/*/language_<code>.lng` | Text |
| A `.erp` the game ships | Archive |
| Any other file the game ships | Replacement |
| A file the game does not ship | Addition |
| `.exe`, `.dll`, `.asi` | Skipped and named in the report |

4. The id is `<author>.<name>`, lowercased to `[a-z0-9-]`, or the file name when both are missing. Importing an id that exists replaces it: the new copy is staged, the old one moves aside, the new one moves in and the old one is deleted. Profiles keep the id's place and switch.
5. The importer diffs the package ([section 4.5](#_4-5-diff)) and writes `package.json`.

The importer reads zip archives and folders. 7z is a later item. RAR is not supported.

### 4.4 Library

```
$DATA/addon-data/teamsimplified.mods/library/<id>/
  package.json          name, author, version, game_version, studio_version, source name, sha256, import time, file list
  mod.serps             as shipped
  files/                the content root, by game path
  diff/<build key>.json the diff for one game build
  diff/<build key>.db   its database records
```

The library is shared by every profile. Each profile's state lists the mods it uses, in load order:

```json
"state": {
  "teamsimplified.mods": {
    "mods": [{ "id": "rm-designs.scuderia-alphatauri-orlen", "enabled": true }]
  }
}
```

A mod not in the list is off and shows after the list, by name. Removing a mod from the library removes it from every profile.

### 4.5 Diff

Each mod is compared with clean files from `GameFiles`, never with the game folder as modded:

| Class | Unit | Changed | Added | Removed |
|---|---|---|---|---|
| Database | record key | same key, other tag or bytes | key not in stock | stock key missing from the package |
| Archive | resource URI | index digest differs (fragment data when a digest is empty) | URI not in stock | stock URI missing from the package |
| Text | language and key | value differs | key not in stock | never |
| Replacement, Addition | file | whole file | whole file | never |

- Removals count only when `game_version` equals the installed exe's version. Otherwise they are dropped, and the row says how many and for which version the package was made.
- A language the package lacks takes the changes of the package's `language_eng.lng`, or of its first language file by name.
- Archive diffs read the index only (`ErpFile::open`). The database diff parses both files once.
- Diffs are cached per game build key in `diff/<build key>.json`, with the changed and added records in `diff/<build key>.db`. Import computes the diff when a game is found. A mod without a diff for the installed build gets one at the next build, or from `Compare` in the section.
- Without a diff for the installed build, the expanded row lists the package's files by class.

### 4.6 Merge

The build hook emits ops for each enabled mod, in profile order, with origin `teamsimplified.mods` and the mod id:

| Class | Ops |
|---|---|
| Database | `db.put-record`, `db.remove-record` |
| Archive | `erp.put-resource` from the mod's archive, `erp.remove-resource` |
| Text | `lang.set` |
| Replacement, Addition | `file.put` |

- The planner applies them after map activation and earlier addons. A package's database changes touch only the records it changed, and archives merge per resource.
- A special livery row a package adds is reported as owned through `owned_items` ([section 3.4](#_3-4-deliveries)), and the livery appears in the picker.
- The ops take the planner's routes ([section 3.4](#_3-4-deliveries)). Without the in-game loader, text ops wait and the build reports how many entries wait.
- Resource removals are `erp.remove-resource` ops on the archive the mod removed them from, and put every `erp` op on the build route. A removal of a record or resource an earlier mod already removed is left out.
- Database records are written as raw `db.put-record` values from `diff/<build key>.db`. Archive and file sources are `{ "root": "data", "path": "library/<id>/files/<game path>" }`.
- A write that repeats the value already in place, from an earlier mod, is left out.

#### Shader archive carriers

Simplified Studio packages carry a mod's textures and models in archives under `shader_package_2025/`, which the game loads at boot. No asset group the loader patches lists those archives, so an overlay targeting them is never read. This is built.

- Resources a package adds to a shader archive are put into `2025_asset_groups/vfx_package/common.erp` instead. Its overlay loads with the boot groups that list that archive, before any car or UI group, as the shader archive would. Resources the package changes stay on the shader archive.
- Team logo texture arrays carried in `ui_texture*` archives are merged across every enabled mod. Each `.texarray.image` starts from the game's own array, from the first of eight `ui_package` archives that holds it. Fragment 1 is split into the array's slices, and each mod's slices that differ from the game's go in, later mods winning. An array built for an older game, with fewer slices of the same format and size, merges over the slices it has. An array of another format is used whole, and the build warns. The result is written to `cache/addons/teamsimplified.mods/ui_textures.erp` and put into `2025_asset_groups/vfx_package/param_particles/systemstartup.erp`, so it loads as an overlay of its own with the boot group. Ported from the Simplified Launcher's `merge_resources.py` and the Team Editor's logo code.
- Simplified Studio's `eaid://tools_package/` marker is left out.

#### Text a package fills in

Team rename packages point the team name records at keys of their own, such as `lng_team_ast_title_26`, and give every team a value: the renamed team's new name, and the game's text for the rest. Those values would undo another package's rename.

- An added key whose text is the game's text of the key the stock record pointed at is a default, kept in the diff's `defaults`. English text counts too, since packages fill in untranslated names.
- A changed translation whose text is the game's English text of the same key is left out.
- Defaults go in once for each key no package renames, before every package's own changes. The diff has a `version`, and a cached diff of another version is computed again.

### 4.7 Conflicts

- The section computes conflicts between enabled mods from their cached diffs, in profile order. A mod counts once for each target. Simplified Studio's marker and the logo texture arrays merged slice by slice are not targets. A row shows `Overrides <n> in <mod>` when it wins and `<n> overridden by <mod>` when it loses. Its expanded body lists the targets under `CONFLICTS`: resource URIs with their archive, record keys in hex, text keys with their language, and files.
- The build reports conflicts with other addons and maps through the planner ([section 3.5](#_3-5-order-and-conflicts)).

### 4.8 Section

The sidebar entry `Mods` comes after `Addons`, with the library size as its count.

- **Header.** Title `Mods`. Subtitle `<n> in library · <k> on in <profile> · lower in the list wins`. Action `Import…`, a picker for `.serps` and `.zip` files and folders, several at once.
- **Notices.** The addon is off in this profile (button `Turn on`). The vanilla profile is active. Mods are not compared with the installed game build yet (button `Compare`).
- **Rows.** Reorderable, in load order. Title from `mod.serps`, subtitle `<author> · <version>`, tags `Database`, `Text`, `<n> archives`, `<n> files`, conflict tags and `Made for <game_version>` when it differs from the game. A switch turns the mod on in the profile.
- **Expanded row.** Stats: status, version, author, made for, Studio version, size, imported. `WHAT IT CHANGES`: records changed, added and removed; text entries by language; resources by archive; files. `CONFLICTS`. Buttons: Move up, Move down, Show files, and Remove from library with a confirmation.
- **Empty.** `No mods yet`, `Import a .serps package, a zip or a folder that holds an F1 25 folder.` and `Import…`.
- **Footer.** `Mod packages in the Simplified Suite's .serps format. Based on Simplified Suite by Team Simplified - https://github.com/TeamSimplified/simplified-suite`
- **Profile cards.** `summary` adds `MODS` with the enabled mods' names.
- **CLI.** `swofty mods list`, `import <path>…`, `remove <id>`, `enable <id>`, `disable <id>`, `move <id> <position>`.

## 5. Tracks are mods

Tracks are `.serps` mods in the Team Simplified library. There is no tracks addon and no Maps page. This is built.

- **Raw track zips.** A zip that holds a track package (`manifest.json` with `track_code`), or a download that holds one such zip, is claimed by Team Simplified. Import stores it as `library/<id>/` with `package.json` (`track` set to the code), a `mod.serps` written from the manifest, and the package under `track/`. The id is the track code, or `<code>-track` when a mod in the library already has that id. A track already in the library keeps its id and is replaced.
- **Track SERPs.** A package built by the Simplified Track Installer (the track archive in an invitational slot, mipmaps under `environment_package/tracks/<code>/mipmaps/`, the `track_info` shader archives and optionally a whole game database) imports, diffs and merges like any other `.serps` mod ([sections 4.3 to 4.6](#_4-3-import)), with the slot as shipped.
- **Build.** For each enabled raw track, in profile order, the build hook emits `track.add` with `source` `{ "root": "data", "path": "library/<id>/track" }`. Core processes `track.add` ops with `activation/` before other database ops: the database rows, the archive, the menus and ERS. With `group` in the loader's `features.json`, the archive goes to its natural path `2025_asset_groups/environment_package/tracks/<code>/<code>.erp` with `group.register` ops and the natural resource-row values, and there is no limit on tracks. Otherwise the tracks take the invitational slots, at most eight.
- **Migration.** `Addon::migrate` runs from `Store::migrate` (`data_version` 3). Team Simplified moves each `$DATA/maps/<code>/` into the library and each profile's `maps` list into its state, enabled, after the mods already listed. The track's id is its code, or `<code>-track` when a library mod already has that id. Each track is staged under `library/.migrate-<folder>/`, and the next run finishes a staged track. A code that matches no track in the library is dropped. `Profile` still reads `maps` and no longer writes it.
- **Removed from core.** The Maps page and sidebar entry, `Profile.maps`, `Store::maps`, `import_map`, `remove_map`, the core track importer and the `swofty maps` commands. `swofty maps` is a hidden alias of `swofty mods`.

## 6. Work items

Each item is one PR, in this order.

| # | Item | Scope | Depends on |
|---|---|---|---|
| 1 | Rename in Rust | Core, launcher and CLI identifiers; `mods/` and `templates/mod` to `addons/` and `templates/addon`; `Store::migrate`; `Profile.addons` with the `mods` alias; `swofty/settings.json` still writes `mods` | |
| 2 | Rename in the launcher and CLI | Launcher text; `swofty addons`; `swofty mods` as a hidden alias | 1 |
| 3 | Rename in game and the SDK | `addons` in `swofty/settings.json` with the native fallback; `ADDON SETTINGS`; `swofty_addon_<n>`; `swofty/addons/` loader folders; `SwoftyAddonInit` and `SWOFTY_ADDON()` with the old names accepted; Wine test | 1 |
| 4 | Rename in the docs | Titles, text, sidebar and README | 2, 3 |
| 5 | Asset op model | `packages/addon-api` with `manifest` and `ops` moved in; `AssetOp`, `Op`, `Origin`, `Source`; unified op files and `operations.assets`; build behaviour unchanged | 1 |
| 6 | Asset planner | Routing, `features.json` reading, build deliveries for `erp`, `db` and `file` (`file.put` is built for the first time), `erp.put-resource` with streamed copies, `db.put-record` and `db.remove-record`, targets and conflicts, waiting ops; `owned_items` from the built database instead of from `add-row` ops | 5 |
| 7 | Runtime asset groups | `assets.cpp` ported from the prototype into `native/`; `features.json` from `native/build.sh`; `group.register` runtime delivery into `swofty/assets.json`; stable since T018 | 3, 6 |
| 8 | Launcher API in core | `Addon`, `Importer`, `Cx`, `BuildCx`, `GameFiles`, `View` types; addon registry with built-ins; `Profile.state`; `addon-data` and cache folders; build hooks in `build.rs`; `addons/builtins` | 5 |
| 9 | Addon sections in the launcher | Generic `View` renderer, sidebar entries, events and effects on the background executor, pickers, drag and drop routing, progress, profile card summaries | 2, 8 |
| 10 | Addon commands in the CLI | `CommandSpec` dispatch; forwarding of the old `swofty mods` forms | 2, 8 |
| 11 | Language files | `swofty-formats` `lng`: read, write and diff LNGT | |
| 12 | Language baseline | `swofty-formats` `nefs` for the archive indexed in `F1_25.exe`; stock `.lng` extraction per game build; `exe_version` | 11 |
| 13 | Team Simplified: import and library | `addons/simplified` crate; zip and folder import; content root and `mod.serps`; classes; library; profile state; section with list, switches, order and removal; `swofty mods`; `THIRD_PARTY.md` credit | 9, 10 |
| 14 | Team Simplified: diff | Database, archive, file and text diffs against `GameFiles`; cache per build key; version rule for removals | 11, 12, 13 |
| 15 | Team Simplified: merge | Build hook emitting ops in profile order; conflict tags and lists in the section | 6, 14 |
| 16 | Runtime text | `overlay.cpp` string tables ported; `lang` runtime delivery writing partial tables; stable since T016 | 3, 6, 11 |
| 17 | Runtime resources | `resources.cpp` ported; `erp` runtime delivery; stable since T013 | 6, 7 |
| 18 | Runtime database | `gamedb.cpp` with raw values and `stage.cpp` ported; `db` runtime delivery; stable since T014, T015 and T018 | 6, 7 |
| 19 | Runtime file overlay | `overlay.cpp` file system overlay ported; `file` runtime delivery; experimental until it runs in game | 6, 7 |
| 20 | Native assets API | API version 4 `assets` table, `swofty/assets.hpp`, the `assets` capability and `"early": true` | 7 (groups), 16 to 19 (other calls) |
| 21 | Docs | Player docs for the Mods section; modding docs for asset ops, the native assets API and the Launcher API | 15, 20 |
| 22 | Tracks as mods | [Section 5](#_5-tracks-are-mods) | 8, 7, 15 |
