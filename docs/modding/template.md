# Making a mod

Copy [`templates/mod`](https://github.com/Swofty-Developments/F1ModManager/tree/main/templates/mod) to start a mod. It uses the MIT licence.

## Get started

1. Copy the template to a new folder.
2. Edit `swofty.json`: set your mod's `id`, `name`, `author` and `version`.
3. Remove operation entries and files you do not need.
4. Install with `swofty mods install <dir>`.
5. Enable it with `swofty mods enable <id>`.

Use a lowercase ID such as `author.my-mod`. Do not use the reserved `swofty.` prefix.

## Included examples

| Path | Example |
|---|---|
| `swofty.json` | Toggle, choice and slider settings, one of them bound to the database |
| `db/example.json` | Change values, one of them from a setting |
| `ui/example.pkgpatch.json` | Change a menu label |
| `native/` | C++ plugin that reads its settings, and its build script |
| `README.md` | Build and test instructions |

See [Current status](/design/#current-status) for unfinished operation types.

## Share your mod

Share the folder or zip it as a `.swoftymod` file, with `swofty.json` at the zip's root.

Include your changes, not copies of the game's archives, database or executable.
