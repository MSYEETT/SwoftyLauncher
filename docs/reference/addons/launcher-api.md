---
title: Launcher API
description: The Rust API built-in addons use for launcher sections, importers, build hooks, commands and profile summaries.
---

# Launcher API

Built-in addons are Rust crates that implement the `Addon` trait from `swofty_addon::launcher` (`packages/addon-api`). They depend on `swofty-addon` and `swofty-formats`, never on `swofty-core`. The apps register them through `addons/builtins` (`swofty_builtins::all()`), which passes them to `Store::with_builtins`.

<Badge type="warning" text="in progress" /> The types, the registry, profile state and build hooks are built. The launcher and the command line do not show sections, run importers or commands, or show summaries yet. `swofty_builtins::all()` returns no addons yet.

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
}
```

Every method except `manifest` has a default that does nothing.

| Method | Use |
|---|---|
| `sections`, `view`, `event` | Pages in the launcher's sidebar |
| `importer` | Files dropped on the launcher or picked in it |
| `build` | Asset ops for the profile being built |
| `commands`, `command` | `swofty` subcommands |
| `summary` | A line on the profile card |

## Registration

```rust
pub struct Builtin {
    pub addon: Arc<dyn Addon>,
    pub locked: bool,
    pub default_on: bool,
}
```

A `locked` addon is always on. A `default_on` addon is on in profiles that do not list it.

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

`data_dir` is `$DATA/addon-data/<id>/` and `cache_dir` is `$DATA/cache/addons/<id>/`. `state` and `set_state` read and write the addon's entry in the profile's `state` object. `ProfileInfo` has `name`, `vanilla` and `enabled`.

`GameFiles` gives clean game files: `build_key`, `exe_version`, `is_stock(rel)`, `original(rel)` and `original_lang(code)`.

## Build hooks {#build-hooks}

```rust
pub struct BuildCx<'a> {
    pub cx: Cx<'a>,
    pub options: &'a BTreeMap<String, serde_json::Value>,
    pub features: &'a Features,
}
```

`build` returns an `AssetBatch` with `ops`, `lines` and `warnings`. Each op carries its `Origin`: the addon and an optional item name. The build routes them like manifest ops ([Asset API](./asset-api#routing)).

## Sections and views

`SectionSpec` has `id`, `title` and `icon`. `view` returns a `View`: `subtitle`, `actions`, `notices` and a `Body` that is a `List` of `Row`s, an `Empty` state or `Blocks`. `event` receives an `Event` and returns an `Effect`. The types are in `swofty_addon::view`.

<Badge type="warning" text="in progress" /> The launcher's renderer for these types, drag and drop routing and helper pages are being built. This section is filled when they land.

## Importers

```rust
pub trait Importer: Send + Sync {
    fn claims(&self, path: &Path) -> Claim;
    fn import(&self, cx: &Cx, paths: &[PathBuf], progress: &dyn Progress) -> Result<Vec<String>>;
}

pub enum Claim { No, Maybe, Yes }
```

## Commands

`CommandSpec` has `name`, `about`, `args` and `subcommands`. `ArgSpec` has `name`, `help`, `required` and `many`.
