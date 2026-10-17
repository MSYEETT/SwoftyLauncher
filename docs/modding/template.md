# The mod template

[`templates/mod`](https://github.com/Swofty-Developments/F1ModManager/tree/main/templates/mod) is a sample mod to start from. It is under the MIT licence, so a mod built from it can use any licence.

## What it contains

| Path | What it shows |
|---|---|
| `swofty.json` | A manifest with an option, a dependency on `swofty.core` and every operation list |
| `db/example.json` | A database `set` and an `add-row` |
| `ui/example.pkgpatch.json` | A UI patch that sets a label's text |
| `native/` | A plugin skeleton in C with a build script |
| `README.md` | How to build, install and test it |

## Start a mod

1. Copy `templates/mod` to a new directory.
2. In `swofty.json`, set `id` to `<you>.<mod-name>` (lowercase, `[a-z0-9._-]`, not starting with `swofty.`), and set `name`, `author` and `version`.
3. Delete the operations you do not need, and the files they name.
4. Install it: `swofty mods install <dir>`, or Library, Mods, Install in the launcher.
5. Enable it in a profile and play.

## Packaging

A mod ships as its directory, or as a `.swoftymod` zip of the directory with `swofty.json` at the root of the zip.

Ship operations, not game files. Do not include copies of the game's archives, database or executables; describe the changes as operations so the launcher applies them to the user's own copy.
