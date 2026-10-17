---
title: File operations
description: Replace or add loose game files from an addon with operations.files.
---

# File operations

`operations.files` in `swofty.json` copies files from the addon into the game folder:

```json
"operations": {
  "files": [{ "source": "files/example", "target": "game/path" }]
}
```

| Field | Meaning |
|---|---|
| `source` | A file in the addon, relative to the addon folder, with `/` separators |
| `target` | Where it goes, relative to the game folder |

A `source` outside the addon folder fails the manifest check.

Each entry is a [`file.put`](/reference/addons/asset-api#op-files) asset op. The build copies the file into the game folder through the journal, and `swofty restore` removes it or puts the stock file back. With the experimental runtime `file` feature turned on (`"runtime_assets": ["file"]` in `$DATA/config.json`), swofty_core serves the file from `swofty/overlay/` instead and the game folder keeps its stock files.

- The last write to a target wins, and the build lists the conflict.
- A target that a track, the database or an archive edit already replaces fails the build.
- The build prints `files: <n> game file(s) replaced, <m> added`.
