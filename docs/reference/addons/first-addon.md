---
title: Your first addon
description: Build an addon with one setting and a launcher plugin that adds a sidebar page, then install it and open the page.
---

# Your first addon

You'll build an addon with one setting and a sidebar page that reports the installed game. To add the page, you'll write a **launcher plugin**: a helper program the launcher runs. This example uses Rust, so you'll need `cargo` (the Rust toolchain). The protocol is plain JSON, though, so you can write plugins in any language.

Here's how your files fit together:

```
my-first-addon/
├── swofty.json
├── Cargo.toml
└── src/
    └── main.rs
```

## 1. Write the manifest

Start with `swofty.json` at the addon's root. This is where you give your addon its name:

```json [swofty.json]
{
  "schema_version": 1,
  "id": "yourname.my-first-addon",
  "name": "My First Addon",
  "version": "0.1.0",
  "author": "Your Name",
  "description": "A launcher page that reports the installed game.",
  "compat": { "game": "f1_25" }
}
```

Choose an `id` with at least two dot-separated parts, using lowercase letters, digits, `.`, `_` and `-`. The `swofty.` prefix is reserved. You'll find every field in [the manifest reference](./manifest).

## 2. Add a setting

To give your addon a setting, add an `options` array. This one adds a single choice, stored per profile:

```json [swofty.json]
"options": [
  {
    "id": "units",
    "type": "choice",
    "label": "Lap display",
    "description": "How the plugin labels lap times.",
    "values": ["seconds", { "value": "mmss", "label": "mm:ss.mmm" }],
    "default": "seconds",
    "binds": "native"
  }
]
```

You'll see the choice on your addon's row on the **Addons** page and on its tile in **ADDON SETTINGS** in game. With `binds: native`, an [engine plugin](./native-plugins#settings) reads the setting live. See [Settings](./settings) for how to read it.

## 3. Add a launcher page

The launcher needs to know which helper to run and which page to add. Give it a `launcher` block: `program` names the helper for each OS, and `sections` adds one sidebar page:

```json [swofty.json]
"launcher": {
  "program": { "linux": "bin/my-first-addon", "windows": "bin/my-first-addon.exe" },
  "sections": [{ "id": "game", "title": "Game", "icon": "gamepad" }]
}
```

Your complete `swofty.json` should now look like this:

```json [swofty.json]
{
  "schema_version": 1,
  "id": "yourname.my-first-addon",
  "name": "My First Addon",
  "version": "0.1.0",
  "author": "Your Name",
  "description": "A launcher page that reports the installed game.",
  "compat": { "game": "f1_25" },
  "options": [
    {
      "id": "units",
      "type": "choice",
      "label": "Lap display",
      "description": "How the plugin labels lap times.",
      "values": ["seconds", { "value": "mmss", "label": "mm:ss.mmm" }],
      "default": "seconds",
      "binds": "native"
    }
  ],
  "launcher": {
    "program": { "linux": "bin/my-first-addon", "windows": "bin/my-first-addon.exe" },
    "sections": [{ "id": "game", "title": "Game", "icon": "gamepad" }]
  }
}
```

## 4. Write the helper

Use this `Cargo.toml` to build your helper as one binary. Its only dependency is `serde_json`:

```toml [Cargo.toml]
[package]
name = "my-first-addon"
version = "0.1.0"
edition = "2024"

[dependencies]
serde_json = "1"
```

Next, write `src/main.rs` to handle `hello` and `view` requests. Your helper reads each request and sends an answer, but it can also ask the launcher questions before replying. That's how `view` gets the game's build key and exe version: it makes the `game_build_key` and `game_exe_version` [host calls](./launcher-api#host-calls-the-plugin-sends-back), then returns a page built from those values:

```rust [src/main.rs]
use std::io::{BufRead, Lines, StdinLock, StdoutLock, Write};

use serde_json::{Value, json};

const API: u64 = 1;

struct Launcher {
    lines: Lines<StdinLock<'static>>,
    out: StdoutLock<'static>,
    next: u64,
}

impl Launcher {
    fn send(&mut self, message: &Value) {
        writeln!(self.out, "{message}").and_then(|()| self.out.flush()).ok();
    }

    fn call(&mut self, method: &str) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(&json!({ "id": id, "method": method }));
        match self.lines.next() {
            Some(Ok(line)) => serde_json::from_str::<Value>(&line)
                .map(|reply| reply["result"].clone())
                .unwrap_or(Value::Null),
            _ => std::process::exit(0),
        }
    }

    fn view(&mut self) -> Value {
        let build = self.call("game_build_key");
        let exe = self.call("game_exe_version");
        let text = |v: &Value| v.as_str().unwrap_or("not found").to_owned();
        json!({
            "subtitle": "The game this profile builds against",
            "body": { "blocks": [
                { "label": "INSTALLED GAME" },
                { "stats": [
                    { "label": "Build", "value": text(&build) },
                    { "label": "F1_25.exe", "value": text(&exe) }
                ] }
            ] }
        })
    }
}

fn main() {
    let stdin = std::io::stdin();
    let mut host = Launcher {
        lines: stdin.lock().lines(),
        out: std::io::stdout().lock(),
        next: 0,
    };
    while let Some(Ok(line)) = host.lines.next() {
        let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
        let Some(id) = message["id"].as_u64() else { continue };
        let result = match message["method"].as_str().unwrap_or_default() {
            "hello" => json!({ "api": API }),
            "view" => host.view(),
            other => {
                host.send(&json!({ "id": id, "error": format!("unknown method {other}") }));
                continue;
            }
        };
        host.send(&json!({ "id": id, "result": result }));
    }
}
```

## 5. Build the helper

Compile the helper, then copy the binary to `bin/` so the launcher can find it at the path you set in `program`:

```sh
cargo build --release
mkdir -p bin
cp target/release/my-first-addon bin/
```

On Windows, build `my-first-addon.exe` and copy it to `bin/`.

## 6. Install and enable

```sh
swofty addons install .
swofty addons enable yourname.my-first-addon
```

Use a modded profile to enable your addon; the `vanilla` profile takes no addons. If you install it again, you'll replace the copy in `$DATA/addons/yourname.my-first-addon/`.

## 7. See it

Open the **Game** tile under **My First Addon** in the launcher's sidebar. This starts your helper and calls `view`, giving you a page like this:

```
INSTALLED GAME
Build       25251167-d873a1c8d204
F1_25.exe   1.0.141.7865
```

If you see `not found` for a value, no game was located. To see your setting in game, open **Career → ADDON SETTINGS** and look for the **Lap display** choice on the **My First Addon** tile.

If your helper crashes or stalls, the launcher stays running. The page shows the error and a **Restart** button. To investigate, check `$DATA/cache/addons/yourname.my-first-addon/helper.log`, where the plugin's stderr goes.

## Package it

When you're ready to share your addon, zip the folder's contents as a `.swoftyaddon` file. Keep `swofty.json` at the zip root:

```sh
zip -r ../yourname.my-first-addon.swoftyaddon .
```

Ship the binaries from `bin/`, leaving out `target/`. Include your files, never copies of the game's archives, database or executable.

::: tip Next
To add an importer and a build hook, follow the full [launcher plugin](./launcher-api) reference, or copy [the template](./template).
:::
