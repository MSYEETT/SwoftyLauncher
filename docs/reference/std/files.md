---
title: File operations
description: Copy files from an addon into the game folder with operations.files.
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

::: warning Not built yet
The build skips `operations.files` and prints a warning.
:::
